//! 单次采集循环：条件请求 → 304/哈希短路 → 解析归一化 → 空结果/数量骤降拒绝 →
//! 事务内 diff 出事件 → 落库并更新游标。
//! 与旧版 collector.py 行为对齐；6 段重复的"计数→record_poll→meta_set→return"
//! 收敛为一个 finish_poll 收尾函数。

use std::sync::Arc;
use std::time::Instant;

use sha2::{Digest, Sha256};

use crate::config::ConfigStore;
use crate::model::{CollectResult, StatusEvent, utcnow_iso};
use crate::monitor::diff;
use crate::monitor::normalize;
use crate::provider::{FetchOutcome, SourceProvider};
use crate::store::{catalog, events, meta, polls, source_history, Database};

/// 引擎对 UI 的出站口抽象（引擎不感知 Tauri；测试注入假 sink）
pub trait StatusSink: Send + Sync {
    fn emit(&self, event: StatusEvent);
}

/// 一次构造、全应用共享的依赖上下文（手工依赖注入容器）
pub struct CollectCtx {
    pub config: Arc<ConfigStore>,
    pub db: Arc<Database>,
    pub provider: Arc<dyn SourceProvider>,
    pub sink: Arc<dyn StatusSink>,
    /// 告警通道复用的 HTTP 客户端
    pub http: reqwest::Client,
}

/// 已落库、待告警处理的事件
#[derive(Debug, Clone)]
pub struct PendingEvent {
    pub event_id: i64,
    pub product_id: i64,
    pub event_type: crate::model::EventType,
    pub created_at: String,
}

/// 未变化/失败路径的统一收尾：落 polls 行 + meta 游标，返回 CollectResult。
/// `new_cursors` 为 Some 时把响应游标写入 meta（304/哈希命中路径）。
#[allow(clippy::too_many_arguments)]
fn finish_poll(
    db: &Database,
    fetched_at: &str,
    duration_ms: u128,
    http_status: Option<i64>,
    counts: Option<(i64, i64, i64)>,
    etag: Option<&str>,
    last_modified: Option<&str>,
    error: Option<&str>,
    new_cursors: Option<(&Option<String>, &Option<String>)>,
) -> CollectResult {
    let record = polls::PollRecord {
        fetched_at,
        http_status,
        changed: false,
        total: counts.map(|c| c.0),
        ok: counts.map(|c| c.1),
        oos: counts.map(|c| c.2),
        duration_ms: duration_ms as i64,
        etag,
        last_modified,
        error,
    };
    if let Err(e) = db.record_poll(&record) {
        tracing::warn!("记录采集结果失败: {e}");
    }
    if let Err(e) = db.meta_set("last_checked", fetched_at) {
        tracing::warn!("更新 last_checked 失败: {e}");
    }
    if let Err(e) = db.meta_set("last_error", error.unwrap_or("")) {
        tracing::warn!("更新 last_error 失败: {e}");
    }
    if let Some((etag, lm)) = new_cursors {
        if let Some(e) = etag {
            let _ = db.meta_set("etag", e);
        }
        if let Some(l) = lm {
            let _ = db.meta_set("last_modified", l);
        }
    }
    CollectResult {
        ok: error.is_none(),
        not_modified: http_status == Some(304),
        changed: false,
        http_status: http_status.map(|s| s as u16),
        fetched_at: Some(fetched_at.to_string()),
        duration_ms: duration_ms as i64,
        total: counts.map(|c| c.0),
        ok_count: counts.map(|c| c.1),
        oos_count: counts.map(|c| c.2),
        error: error.map(str::to_string),
        ..Default::default()
    }
}

fn current_counts(db: &Database) -> Option<(i64, i64, i64)> {
    db.counts().ok().map(|c| (c.total, c.ok, c.oos))
}

/// 执行一次采集。返回 (结果, 待告警事件)。
pub async fn run_collect_cycle(ctx: &CollectCtx, force: bool) -> (CollectResult, Vec<PendingEvent>) {
    let started = Instant::now();
    let fetched_at = utcnow_iso();
    let cfg = ctx.config.read();

    let etag = if force {
        None
    } else {
        ctx.db.meta_get("etag").ok().flatten()
    };
    let last_modified = if force {
        None
    } else {
        ctx.db.meta_get("last_modified").ok().flatten()
    };

    // 1. 拉取
    let outcome = match ctx
        .provider
        .fetch(
            &cfg.source_url,
            &cfg.request,
            etag.as_deref(),
            last_modified.as_deref(),
        )
        .await
    {
        Ok(o) => o,
        Err(e) => {
            let msg = e.to_string();
            tracing::warn!("采集失败：{msg}");
            return (
                finish_poll(
                    &ctx.db,
                    &fetched_at,
                    started.elapsed().as_millis(),
                    None,
                    current_counts(&ctx.db),
                    etag.as_deref(),
                    last_modified.as_deref(),
                    Some(&msg),
                    None,
                ),
                vec![],
            );
        }
    };

    // 2. 304 短路
    let (text, resp_etag, resp_lm) = match outcome {
        FetchOutcome::NotModified {
            etag,
            last_modified,
        } => {
            tracing::debug!("数据未变化（304）");
            return (
                finish_poll(
                    &ctx.db,
                    &fetched_at,
                    started.elapsed().as_millis(),
                    Some(304),
                    current_counts(&ctx.db),
                    etag.as_deref(),
                    last_modified.as_deref(),
                    None,
                    Some((&etag, &last_modified)),
                ),
                vec![],
            );
        }
        FetchOutcome::Modified {
            text,
            etag,
            last_modified,
        } => (text, etag, last_modified),
    };

    // 3. 内容哈希短路
    let content_hash = {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        format!("{:x}", hasher.finalize())
    };
    if !force && ctx.db.meta_get("content_hash").ok().flatten().as_deref() == Some(content_hash.as_str())
    {
        tracing::debug!("内容哈希未变化，跳过 diff");
        let merged_etag = resp_etag.clone().or(etag.clone());
        let merged_lm = resp_lm.clone().or(last_modified.clone());
        return (
            finish_poll(
                &ctx.db,
                &fetched_at,
                started.elapsed().as_millis(),
                Some(200),
                current_counts(&ctx.db),
                merged_etag.as_deref(),
                merged_lm.as_deref(),
                None,
                Some((&merged_etag, &merged_lm)),
            ),
            vec![],
        );
    }

    // 4. 解析 + 归一化
    let snapshot = match ctx.provider.parse(&text).map(|payload| normalize::normalize(&payload)) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            tracing::error!("解析失败：{msg}");
            return (
                finish_poll(
                    &ctx.db,
                    &fetched_at,
                    started.elapsed().as_millis(),
                    Some(200),
                    current_counts(&ctx.db),
                    etag.as_deref(),
                    last_modified.as_deref(),
                    Some(&msg),
                    None,
                ),
                vec![],
            );
        }
    };

    // 5. 空结果 / 数量骤降拒绝（避免误判全部下架）
    if snapshot.products.is_empty() {
        return refuse(
            &ctx.db,
            &fetched_at,
            started,
            &etag,
            &last_modified,
            "解析结果为空，已拒绝本次更新（避免误判全部下架）",
        );
    }
    let previous_count = ctx.db.product_count().unwrap_or(0);
    if previous_count > 0 && (snapshot.products.len() as i64) < previous_count / 2 {
        return refuse(
            &ctx.db,
            &fetched_at,
            started,
            &etag,
            &last_modified,
            &format!(
                "套餐数量异常下降（{previous_count} -> {}），已拒绝本次更新（避免误判全部下架）",
                snapshot.products.len()
            ),
        );
    }

    // 6. 事务内 diff + 落库
    let is_first = ctx.db.meta_get("initialized").ok().flatten().as_deref() != Some("1")
        && previous_count == 0;
    let mut pending: Vec<PendingEvent> = Vec::new();
    let mut added: i64 = 0;

    let tx_outcome = ctx.db.with_tx(|tx| {
        apply_snapshot(
            tx,
            &snapshot,
            &fetched_at,
            is_first,
            &mut pending,
            &mut added,
        )?;
        let removed = catalog::mark_inactive(tx, &fetched_at)?;
        source_history::upsert(tx, &snapshot.history)?;
        let (total, ok_count, oos_count) = catalog::snapshot_counts(tx)?;
        let duration = started.elapsed().as_millis();
        let changed_any = !pending.is_empty() || added > 0 || removed > 0;

        polls::record(
            tx,
            &polls::PollRecord {
                fetched_at: &fetched_at,
                http_status: Some(200),
                changed: changed_any,
                total: Some(total),
                ok: Some(ok_count),
                oos: Some(oos_count),
                duration_ms: duration as i64,
                etag: resp_etag.as_deref(),
                last_modified: resp_lm.as_deref(),
                error: None,
            },
        )?;
        meta::set(tx, "initialized", "1")?;
        meta::set(tx, "last_checked", &fetched_at)?;
        meta::set(tx, "last_success", &fetched_at)?;
        meta::set(tx, "last_error", "")?;
        meta::set(tx, "content_hash", &content_hash)?;
        if let Some(e) = &resp_etag {
            meta::set(tx, "etag", e)?;
        }
        if let Some(l) = &resp_lm {
            meta::set(tx, "last_modified", l)?;
        }
        if changed_any {
            meta::set(tx, "last_data_change", &fetched_at)?;
        }
        Ok((total, ok_count, oos_count, removed, changed_any))
    });

    match tx_outcome {
        Ok((total, ok_count, oos_count, removed, changed_any)) => {
            let duration_ms = started.elapsed().as_millis();
            if changed_any {
                tracing::info!(
                    "采集完成：{total} 款，有货 {ok_count}，售罄 {oos_count}，新增 {added}，下线 {removed}，事件 {}",
                    pending.len()
                );
            }
            (
                CollectResult {
                    ok: true,
                    changed: changed_any,
                    http_status: Some(200),
                    fetched_at: Some(fetched_at),
                    duration_ms: duration_ms as i64,
                    total: Some(total),
                    ok_count: Some(ok_count),
                    oos_count: Some(oos_count),
                    added: Some(added),
                    removed: Some(removed),
                    events: Some(pending.len() as i64),
                    ..Default::default()
                },
                pending,
            )
        }
        Err(e) => {
            let msg = format!("数据库写入失败: {e}");
            tracing::error!("{msg}");
            (
                finish_poll(
                    &ctx.db,
                    &fetched_at,
                    started.elapsed().as_millis(),
                    Some(200),
                    current_counts(&ctx.db),
                    etag.as_deref(),
                    last_modified.as_deref(),
                    Some(&msg),
                    None,
                ),
                vec![],
            )
        }
    }
}

/// 拒绝更新路径（空结果 / 数量骤降）：错误级日志 + 常规失败收尾
fn refuse(
    db: &Database,
    fetched_at: &str,
    started: Instant,
    etag: &Option<String>,
    last_modified: &Option<String>,
    message: &str,
) -> (CollectResult, Vec<PendingEvent>) {
    tracing::error!("{message}");
    (
        finish_poll(
            db,
            fetched_at,
            started.elapsed().as_millis(),
            Some(200),
            current_counts(db),
            etag.as_deref(),
            last_modified.as_deref(),
            Some(message),
            None,
        ),
        vec![],
    )
}

/// 事务体：主数据 upsert + 产品 diff 出事件（写入 pending）。
#[allow(clippy::too_many_arguments)]
fn apply_snapshot(
    tx: &rusqlite::Transaction,
    snapshot: &normalize::Snapshot,
    fetched_at: &str,
    is_first: bool,
    pending: &mut Vec<PendingEvent>,
    added: &mut i64,
) -> rusqlite::Result<()> {
    for vendor in &snapshot.vendors {
        catalog::upsert_vendor(tx, &vendor.key, &vendor.name, &vendor.logo)?;
    }
    for series in &snapshot.series {
        catalog::upsert_series(tx, &series.vendor_key, &series.key, &series.name, &series.region)?;
    }

    let existing = catalog::products_map_conn(tx)?;
    for product in &snapshot.products {
        let key = (
            product.vendor_key.clone(),
            product.series_key.clone(),
            product.name.clone(),
        );
            let Some(old) = existing.get(&key) else {
                let pid = catalog::insert_product(tx, product, fetched_at)?;
                *added += 1;
                if !is_first {
                    let draft = diff::new_product_event(pid, product.status, product.price_usd);
                    let eid = events::insert(tx, &draft, fetched_at)?;
                    pending.push(PendingEvent {
                        event_id: eid,
                        product_id: pid,
                        event_type: draft.event_type,
                        created_at: fetched_at.to_string(),
                    });
                }
                continue;
            };

        let pid = old.id;
        let mut changed = false;
        for draft in diff::product_events(
            pid,
            old.status,
            product.status,
            old.price_usd,
            product.price_usd,
        ) {
            let eid = events::insert(tx, &draft, fetched_at)?;
            pending.push(PendingEvent {
                event_id: eid,
                product_id: pid,
                event_type: draft.event_type,
                created_at: fetched_at.to_string(),
            });
            changed = true;
        }
        catalog::update_product(tx, pid, product, fetched_at)?;
        if changed {
            catalog::set_last_change(tx, pid, fetched_at)?;
        }
    }
    Ok(())
}
