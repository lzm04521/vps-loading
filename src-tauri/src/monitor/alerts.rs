//! 规则匹配与告警通道：命中规则 → 构建消息 → 发送（telegram/webhook/local）→ 落库。
//! 纯函数（rule_matches / build_message）与 IO（发送）分离，前者直接单测。

use std::time::Duration;

use crate::config::ConfigStore;
use crate::model::{AppConfig, Channel, EventType, Rule};
use crate::monitor::engine::{CollectCtx, PendingEvent};
use crate::secrets::{self, TELEGRAM_TOKEN_REF};
use crate::store::catalog::ProductFull;

fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|v| !v.is_empty())
}

fn contains(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// 规则条件是否命中该产品
pub fn rule_matches(rule: &Rule, product: &ProductFull) -> bool {
    if let Some(vk) = non_empty(&rule.vendor_key)
        && vk != product.vendor_key
    {
        return false;
    }
    if let Some(rl) = non_empty(&rule.region_like)
        && !contains(&format!("{} {}", product.region_cn, product.region), rl)
    {
        return false;
    }
    if let Some(nl) = non_empty(&rule.name_like)
        && !contains(
            &format!("{} {} {}", product.vendor_name, product.series_name, product.name),
            nl,
        )
    {
        return false;
    }
    if let Some(rl) = non_empty(&rule.route_like)
        && !contains(&product.route, rl)
    {
        return false;
    }
    if let Some(min) = rule.min_price_usd
        && product.price_usd.is_none_or(|p| p < min)
    {
        return false;
    }
    if let Some(max) = rule.max_price_usd
        && product.price_usd.is_none_or(|p| p > max)
    {
        return false;
    }
    true
}

fn event_label(event_type: EventType) -> (&'static str, &'static str) {
    match event_type {
        EventType::Restock => ("🟢", "补货"),
        EventType::Soldout => ("🔴", "售罄"),
        EventType::PriceDown => ("💰", "降价"),
        EventType::PriceUp => ("⬆️", "涨价"),
        EventType::NewProduct => ("🆕", "新品"),
    }
}

/// 组装告警消息（发送到 telegram/webhook 的正文）
pub fn build_message(event_type: EventType, product: &ProductFull) -> String {
    let (icon, label) = event_label(event_type);

    let mut price_text = product.price.clone();
    if !product.unit.is_empty() && !price_text.ends_with(&product.unit) {
        price_text = if price_text.is_empty() {
            product.unit.clone()
        } else {
            format!("{price_text}/{}", product.unit)
        };
    }
    if let Some(usd) = product.price_usd
        && !price_text.contains('$')
    {
        price_text = format!("{price_text}（约 ${usd:.2}）");
    }

    let region = if product.region_cn.is_empty() {
        &product.region
    } else {
        &product.region_cn
    };
    let mut meta_parts: Vec<String> = Vec::new();
    if !region.is_empty() {
        meta_parts.push(region.clone());
    }
    if !product.series_name.is_empty() && product.series_name != *region {
        meta_parts.push(product.series_name.clone());
    }
    let meta = meta_parts.join(" · ");

    let vendor = if product.vendor_name.is_empty() {
        &product.vendor_key
    } else {
        &product.vendor_name
    };
    let mut lines = vec![format!("{icon} {label}提醒 · {vendor}"), product.name.clone()];
    if !meta.is_empty() {
        lines.push(meta);
    }
    if !price_text.is_empty() {
        lines.push(format!("价格：{price_text}"));
    }
    if !product.route.is_empty() {
        lines.push(format!("线路：{}", product.route));
    }
    if !product.promo.is_empty() {
        lines.push(format!("优惠码：{}", product.promo));
    }
    if !product.buy_url.is_empty() {
        lines.push(format!("购买：{}", product.buy_url));
    }
    lines.join("\n")
}

fn mask_secrets(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for secret in secrets {
        if !secret.is_empty() {
            out = out.replace(secret, "***");
        }
    }
    out
}

async fn send_telegram(
    ctx: &CollectCtx,
    cfg: &AppConfig,
    message: &str,
) -> (String, Option<String>) {
    let token = secrets::load_token(TELEGRAM_TOKEN_REF)
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    let chat_id = cfg.telegram.chat_id.trim();
    let Some(token) = token else {
        return ("failed".into(), Some("Telegram 未配置 bot token".into()));
    };
    if chat_id.is_empty() {
        return ("failed".into(), Some("Telegram 未配置 chat_id".into()));
    }

    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let payload = serde_json::json!({
        "chat_id": chat_id,
        "text": message,
        "disable_web_page_preview": true,
    });
    let resp = match ctx
        .http
        .post(&url)
        .json(&payload)
        .timeout(Duration::from_secs(15))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return ("failed".into(), Some(mask_secrets(&format!("{e}"), &[&token]))),
    };
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    if status == 200
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&body)
        && value.get("ok").and_then(|v| v.as_bool()) == Some(true)
    {
        return ("sent".into(), None);
    }
    let snippet: String = body.chars().take(200).collect();
    (
        "failed".into(),
        Some(mask_secrets(&format!("HTTP {status}: {snippet}"), &[&token])),
    )
}

async fn send_webhook(
    ctx: &CollectCtx,
    cfg: &AppConfig,
    message: &str,
    event: &PendingEvent,
    product: &ProductFull,
) -> (String, Option<String>) {
    let url = cfg.webhook.url.trim();
    if url.is_empty() {
        return ("failed".into(), Some("Webhook 未配置 url".into()));
    }

    let payload = serde_json::json!({
        "text": message,
        "event": {
            "id": event.event_id,
            "type": event.event_type.as_str(),
            "created_at": event.created_at,
        },
        "product": {
            "id": product.id,
            "vendor": product.vendor_name,
            "name": product.name,
            "region": product.region_cn,
            "price": product.price,
            "priceUsd": product.price_usd,
            "status": product.status != 0,
            "buyUrl": product.buy_url,
        },
    });
    let resp = match ctx
        .http
        .post(url)
        .json(&payload)
        .timeout(Duration::from_secs(15))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return ("failed".into(), Some(format!("{e}: 请求失败"))),
    };
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    if (200..300).contains(&status) {
        return ("sent".into(), None);
    }
    let snippet: String = body.chars().take(200).collect();
    (
        "failed".into(),
        Some(mask_secrets(&format!("HTTP {status}: {snippet}"), &[url])),
    )
}

async fn send_windows(ctx: &CollectCtx, message: &str) -> (String, Option<String>) {
    match ctx.notifier.notify("VPS Monitor", message) {
        Ok(()) => ("sent".into(), None),
        Err(e) => ("failed".into(), Some(e)),
    }
}

async fn dispatch(
    ctx: &CollectCtx,
    cfg: &AppConfig,
    channel: Channel,
    message: &str,
    event: &PendingEvent,
    product: &ProductFull,
) -> (String, Option<String>) {
    match channel {
        Channel::Local => ("local".into(), None),
        Channel::Telegram => send_telegram(ctx, cfg, message).await,
        Channel::Webhook => send_webhook(ctx, cfg, message, event, product).await,
        Channel::Windows => send_windows(ctx, message).await,
    }
}

/// 对事件套用启用的规则，命中后发送告警并落库；返回处理条数。
/// 由 service 在采集事务提交后异步 spawn 调用，不阻塞下一次采集。
pub async fn process_events(ctx: &CollectCtx, events: &[PendingEvent]) -> usize {
    if events.is_empty() {
        return 0;
    }
    let rules = match ctx.db.list_rules(true) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("加载告警规则失败: {e}");
            return 0;
        }
    };
    if rules.is_empty() {
        return 0;
    }
    let cfg = ctx.config.read();
    let mut handled = 0usize;

    for event in events {
        let Some(product) = ctx.db.get_product_full(event.product_id).ok().flatten() else {
            continue;
        };
        for rule in &rules {
            let types: Vec<&str> = rule
                .event_types
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            if !types.contains(&event.event_type.as_str()) {
                continue;
            }
            if !rule_matches(rule, &product) {
                continue;
            }
            if rule.only_watchlist && !product.watched {
                continue;
            }
            if rule.cooldown_minutes > 0 {
                let since = (chrono::Utc::now()
                    - chrono::Duration::minutes(rule.cooldown_minutes))
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
                if ctx.db.recent_alert_exists(rule.id, product.id, &since) {
                    continue;
                }
            }

            let message = build_message(event.event_type, &product);
            let channel = Channel::parse(&rule.channel).unwrap_or(Channel::Local);
            let (status, error) = dispatch(ctx, &cfg, channel, &message, event, &product).await;
            if status == "failed" {
                tracing::warn!(
                    "告警发送失败 rule={} product={}: {}",
                    rule.id,
                    product.id,
                    error.as_deref().unwrap_or("未知错误")
                );
            }
            if let Err(e) = ctx.db.insert_alert(&crate::store::alerts::AlertRecord {
                event_id: event.event_id,
                rule_id: rule.id,
                product_id: product.id,
                channel: channel.as_str(),
                message: &message,
                status: &status,
                error: error.as_deref(),
            }) {
                tracing::warn!("记录告警失败: {e}");
            }
            handled += 1;
        }
    }
    handled
}

/// 供 commands/config 模块复用：配置层面 telegram 是否就绪
pub fn telegram_ready(config: &ConfigStore) -> bool {
    let cfg = config.read();
    if cfg.telegram.chat_id.trim().is_empty() {
        return false;
    }
    secrets::token_exists(TELEGRAM_TOKEN_REF).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::catalog::ProductFull;
    use std::sync::Arc;

    fn product() -> ProductFull {
        ProductFull {
            id: 1,
            vendor_key: "vultr".into(),
            vendor_name: "Vultr".into(),
            series_name: "东京".into(),
            name: "东京 1C1G".into(),
            price: "$5.00".into(),
            price_usd: Some(5.0),
            region: "JP".into(),
            region_cn: "日本东京".into(),
            route: "软银 CN2".into(),
            unit: "月".into(),
            promo: "NEW10".into(),
            buy_url: "https://example.com/buy".into(),
            status: 1,
            watched: false,
        }
    }

    fn rule() -> Rule {
        Rule {
            id: 1,
            name: "测试规则".into(),
            enabled: true,
            event_types: "restock".into(),
            vendor_key: None,
            region_like: None,
            name_like: None,
            route_like: None,
            min_price_usd: None,
            max_price_usd: None,
            only_watchlist: false,
            channel: "local".into(),
            cooldown_minutes: 30,
            created_at: String::new(),
        }
    }

    #[test]
    fn rule_match_conditions() {
        let p = product();
        let base = rule();

        // 无条件规则全命中
        assert!(rule_matches(&base, &p));

        // vendor 过滤
        let mut r = base.clone();
        r.vendor_key = Some("vultr".into());
        assert!(rule_matches(&r, &p));
        r.vendor_key = Some("other".into());
        assert!(!rule_matches(&r, &p));
        // 空串视为无条件
        r.vendor_key = Some(String::new());
        assert!(rule_matches(&r, &p));

        // 地区包含（忽略大小写）
        let mut r = base.clone();
        r.region_like = Some("日本".into());
        assert!(rule_matches(&r, &p));
        r.region_like = Some("大阪".into());
        assert!(!rule_matches(&r, &p));

        // 名称包含覆盖 vendor+series+name
        let mut r = base.clone();
        r.name_like = Some("东京".into());
        assert!(rule_matches(&r, &p));
        r.name_like = Some("VULTR".into());
        assert!(rule_matches(&r, &p)); // 大小写不敏感
        r.name_like = Some("首尔".into());
        assert!(!rule_matches(&r, &p));

        // 线路包含
        let mut r = base.clone();
        r.route_like = Some("cn2".into());
        assert!(rule_matches(&r, &p));
        r.route_like = Some("gia".into());
        assert!(!rule_matches(&r, &p));

        // 价格区间；无价产品不命中
        let mut r = base.clone();
        r.min_price_usd = Some(4.0);
        r.max_price_usd = Some(6.0);
        assert!(rule_matches(&r, &p));
        r.min_price_usd = Some(6.0);
        assert!(!rule_matches(&r, &p));
        r.min_price_usd = None;
        r.max_price_usd = None;
        let mut no_price = p.clone();
        no_price.price_usd = None;
        r.min_price_usd = Some(1.0);
        assert!(!rule_matches(&r, &no_price));
    }

    #[test]
    fn message_contains_key_fields() {
        let msg = build_message(EventType::Restock, &product());
        assert!(msg.contains("🟢 补货提醒 · Vultr"));
        assert!(msg.contains("东京 1C1G"));
        assert!(msg.contains("日本东京 · 东京"));
        assert!(msg.contains("价格：$5.00/月"));
        assert!(msg.contains("线路：软银 CN2"));
        assert!(msg.contains("优惠码：NEW10"));
        assert!(msg.contains("购买：https://example.com/buy"));
    }

    #[test]
    fn mask_secrets_replaces_token() {
        assert_eq!(
            mask_secrets("url with bot123:SECRET_SECRET_SECRET inside", &["bot123:SECRET_SECRET_SECRET"]),
            "url with *** inside"
        );
    }

    #[tokio::test]
    async fn windows_dispatch_routes_to_notifier() {
        struct RecordingNotifier {
            calls: std::sync::Mutex<Vec<(String, String)>>,
            fail: bool,
        }
        impl crate::monitor::engine::Notifier for RecordingNotifier {
            fn notify(&self, title: &str, body: &str) -> Result<(), String> {
                if self.fail {
                    return Err("系统通知被拒绝".into());
                }
                self.calls
                    .lock()
                    .unwrap()
                    .push((title.into(), body.into()));
                Ok(())
            }
        }
        struct NoopSink;
        impl crate::monitor::engine::StatusSink for NoopSink {
            fn emit(&self, _event: crate::model::StatusEvent) {}
        }

        fn ctx_with(
            dir: &std::path::Path,
            notifier: Arc<RecordingNotifier>,
        ) -> Arc<CollectCtx> {
            let config = Arc::new(
                ConfigStore::load_from(dir.join("config.json")).unwrap(),
            );
            let db = Arc::new(crate::store::Database::open(&dir.join("monitor.db")).unwrap());
            Arc::new(CollectCtx {
                config,
                db,
                provider: crate::provider::default_provider(),
                sink: Arc::new(NoopSink),
                http: reqwest::Client::new(),
                notifier,
            })
        }

        let event = PendingEvent {
            event_id: 1,
            product_id: 1,
            event_type: EventType::Restock,
            created_at: String::new(),
        };
        let product = product();

        // 成功路径：调用记为 sent，标题固定为应用名
        let ok_notifier = Arc::new(RecordingNotifier {
            calls: std::sync::Mutex::new(Vec::new()),
            fail: false,
        });
        {
            let dir = tempfile::tempdir().unwrap();
            let ctx = ctx_with(dir.path(), ok_notifier.clone());
            let cfg = ctx.config.read();
            let (status, error) =
                dispatch(&ctx, &cfg, Channel::Windows, "测试消息", &event, &product).await;
            assert_eq!(status, "sent");
            assert!(error.is_none());
            let calls = ok_notifier.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(
                calls[0],
                ("VPS Monitor".to_string(), "测试消息".to_string())
            );
        }

        // 失败路径：记为 failed 并携带错误信息
        let fail_notifier = Arc::new(RecordingNotifier {
            calls: std::sync::Mutex::new(Vec::new()),
            fail: true,
        });
        {
            let dir = tempfile::tempdir().unwrap();
            let ctx = ctx_with(dir.path(), fail_notifier.clone());
            let cfg = ctx.config.read();
            let (status, error) =
                dispatch(&ctx, &cfg, Channel::Windows, "测试消息", &event, &product).await;
            assert_eq!(status, "failed");
            assert!(error.as_deref().unwrap().contains("系统通知被拒绝"));
            assert!(fail_notifier.calls.lock().unwrap().is_empty());
        }
    }
}
