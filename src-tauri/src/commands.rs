//! IPC 命令层：DTO 校验、错误码化（ipc_err）、不含业务算法；AppState 注入各 Store。
//! 命令失败返回 `{"code":"...","detail":"..."}` JSON 串，前端按 errors.<code> 走 i18n。

use std::sync::Arc;

use tauri::State;

use crate::config::ConfigStore;
use crate::logging::LogReloadHandle;
use crate::model::{
    AppConfig, AppSettings, Channel, CollectResult, ConfigInput, ConfigView, EventType,
    LegacyImportReport, PollView, ProductPage, ProductQuery, RegionStat, Rule, RuleInput,
    StatusEvent, SummaryView, VendorStat, EventView, AlertView, HistoryView,
};
use crate::monitor::alerts;
use crate::monitor::engine::StatusSink;
use crate::monitor::service::MonitorService;
use crate::secrets::{self, NTFY_TOKEN_REF, TELEGRAM_TOKEN_REF};
use crate::store::Database;

pub struct AppState {
    pub config: Arc<ConfigStore>,
    pub db: Arc<Database>,
    pub service: Arc<MonitorService>,
    pub log_dir: std::path::PathBuf,
    pub log_reload: Option<LogReloadHandle>,
}

/// 引擎状态事件 → 前端 listen("monitor://status")
pub struct IpcSink {
    pub app: tauri::AppHandle,
}

impl StatusSink for IpcSink {
    fn emit(&self, event: StatusEvent) {
        use tauri::Emitter;
        let _ = self.app.emit("monitor://status", event);
    }
}

/// Windows 原生通知出口（tauri-plugin-notification），供 windows 告警通道使用
pub struct WindowsNotifier {
    pub app: tauri::AppHandle,
}

impl crate::monitor::engine::Notifier for WindowsNotifier {
    fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        use tauri_plugin_notification::NotificationExt;
        self.app
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|e| format!("Windows 通知发送失败: {e}"))
    }
}

fn ipc_err(code: &str, detail: impl std::fmt::Display) -> String {
    serde_json::json!({ "code": code, "detail": detail.to_string() }).to_string()
}

fn db_err(e: rusqlite::Error) -> String {
    ipc_err("INTERNAL", format!("数据库错误: {e}"))
}

// ---------- 总览 ----------

#[tauri::command]
pub fn summary_get(state: State<'_, AppState>) -> Result<SummaryView, String> {
    let counts = state.db.counts().map_err(db_err)?;
    let last_poll: Option<PollView> = state.db.latest_poll().map_err(db_err)?;
    let meta = |key: &str| state.db.meta_get(key).unwrap_or(None);
    let cfg = state.config.read();
    Ok(SummaryView {
        counts,
        last_poll,
        last_success: meta("last_success"),
        last_checked: meta("last_checked"),
        last_data_change: meta("last_data_change"),
        last_error: meta("last_error").unwrap_or_default(),
        source_url: cfg.source_url.clone(),
        initialized: meta("initialized").as_deref() == Some("1"),
        collector_busy: state.service.is_busy(),
        poll_interval_seconds: state.service.interval_seconds(),
        telegram_configured: alerts::telegram_ready(&state.config),
        webhook_configured: !cfg.webhook.url.trim().is_empty(),
        ntfy_configured: alerts::ntfy_ready(&state.config),
        db_path: state.db.path().display().to_string(),
    })
}

// ---------- 库存 ----------

#[tauri::command]
pub fn products_list(state: State<'_, AppState>, query: ProductQuery) -> Result<ProductPage, String> {
    state.db.query_products(&query).map_err(db_err)
}

#[tauri::command]
pub fn vendors_list(state: State<'_, AppState>) -> Result<Vec<VendorStat>, String> {
    state.db.list_vendors().map_err(db_err)
}

#[tauri::command]
pub fn regions_list(state: State<'_, AppState>) -> Result<Vec<RegionStat>, String> {
    state.db.list_regions().map_err(db_err)
}

#[tauri::command]
pub fn watch_toggle(state: State<'_, AppState>, product_id: i64) -> Result<bool, String> {
    state
        .db
        .toggle_watch(product_id)
        .map_err(db_err)?
        .ok_or_else(|| ipc_err("PRODUCT_NOT_FOUND", "套餐不存在"))
}

// ---------- 事件 / 动态 / 告警 / 采集日志 ----------

#[tauri::command]
pub fn events_list(
    state: State<'_, AppState>,
    limit: Option<i64>,
    event_type: Option<String>,
) -> Result<Vec<EventView>, String> {
    state
        .db
        .list_events(limit.unwrap_or(100), event_type.as_deref())
        .map_err(db_err)
}

#[tauri::command]
pub fn source_history_list(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<HistoryView>, String> {
    state
        .db
        .list_source_history(limit.unwrap_or(30))
        .map_err(db_err)
}

#[tauri::command]
pub fn alerts_list(state: State<'_, AppState>, limit: Option<i64>) -> Result<Vec<AlertView>, String> {
    state.db.list_alerts(limit.unwrap_or(100)).map_err(db_err)
}

#[tauri::command]
pub fn alert_delete(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    if !state.db.delete_alert(id).map_err(db_err)? {
        return Err(ipc_err("ALERT_NOT_FOUND", "告警记录不存在"));
    }
    Ok(())
}

#[tauri::command]
pub fn alerts_clear(state: State<'_, AppState>) -> Result<usize, String> {
    state.db.clear_alerts().map_err(db_err)
}

#[tauri::command]
pub fn polls_list(state: State<'_, AppState>, limit: Option<i64>) -> Result<Vec<PollView>, String> {
    state.db.list_polls(limit.unwrap_or(50)).map_err(db_err)
}

// ---------- 规则 ----------

fn validate_rule(input: &RuleInput) -> Result<(), String> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(ipc_err("RULE_INVALID", "规则名称须为 1~100 字符"));
    }
    let types: Vec<&str> = input
        .event_types
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if types.is_empty() {
        return Err(ipc_err("RULE_INVALID", "至少选择一个事件类型"));
    }
    for t in &types {
        if EventType::parse(t).is_none() {
            return Err(ipc_err("RULE_INVALID", format!("未知事件类型：{t}")));
        }
    }
    if Channel::parse(&input.channel).is_none() {
        return Err(ipc_err("RULE_INVALID", format!("未知通道：{}", input.channel)));
    }
    if !(0..=1440).contains(&input.cooldown_minutes) {
        return Err(ipc_err("RULE_INVALID", "冷却分钟须在 0~1440 之间"));
    }
    if let (Some(min), Some(max)) = (input.min_price_usd, input.max_price_usd)
        && min > max
    {
        return Err(ipc_err("RULE_INVALID", "最低价不能大于最高价"));
    }
    Ok(())
}

#[tauri::command]
pub fn rules_list(state: State<'_, AppState>) -> Result<Vec<Rule>, String> {
    state.db.list_rules(false).map_err(db_err)
}

#[tauri::command]
pub fn rule_create(state: State<'_, AppState>, input: RuleInput) -> Result<Rule, String> {
    validate_rule(&input)?;
    state.db.create_rule(&input).map_err(db_err)
}

#[tauri::command]
pub fn rule_update(
    state: State<'_, AppState>,
    id: i64,
    input: RuleInput,
) -> Result<Rule, String> {
    validate_rule(&input)?;
    state
        .db
        .update_rule(id, &input)
        .map_err(db_err)?
        .ok_or_else(|| ipc_err("RULE_NOT_FOUND", "规则不存在"))
}

#[tauri::command]
pub fn rule_delete(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    if !state.db.delete_rule(id).map_err(db_err)? {
        return Err(ipc_err("RULE_NOT_FOUND", "规则不存在"));
    }
    Ok(())
}

// ---------- 采集 ----------

#[tauri::command]
pub async fn collect_manual(
    state: State<'_, AppState>,
    force: Option<bool>,
) -> Result<CollectResult, String> {
    let service = state.service.clone();
    Ok(service.collect(force.unwrap_or(false)).await)
}

// ---------- 配置 ----------

fn config_view(state: &AppState) -> ConfigView {
    let cfg = state.config.read();
    ConfigView {
        source_url: cfg.source_url.clone(),
        poll_interval_seconds: state.service.interval_seconds(),
        telegram_chat_id: cfg.telegram.chat_id.clone(),
        telegram_token_set: secrets::token_exists(TELEGRAM_TOKEN_REF).unwrap_or(false),
        webhook_url: cfg.webhook.url.clone(),
        ntfy_server_url: cfg.ntfy.server_url.clone(),
        ntfy_topic: cfg.ntfy.topic.clone(),
        ntfy_token_set: secrets::token_exists(NTFY_TOKEN_REF).unwrap_or(false),
        settings: cfg.settings.clone(),
        config_path: state.config.config_path().display().to_string(),
        db_path: state.db.path().display().to_string(),
    }
}

#[tauri::command]
pub fn config_get(state: State<'_, AppState>) -> Result<ConfigView, String> {
    Ok(config_view(&state))
}

#[tauri::command]
pub fn config_update(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    input: ConfigInput,
) -> Result<ConfigView, String> {
    // 1. 机密先行：token 写 keyring 失败则整体失败（配置不动）
    if let Some(token) = &input.telegram_token {
        if token.trim().is_empty() {
            secrets::delete_token(TELEGRAM_TOKEN_REF)
                .map_err(|e| ipc_err("SECRET_SAVE_FAILED", e))?;
        } else {
            secrets::save_token(TELEGRAM_TOKEN_REF, token.trim())
                .map_err(|e| ipc_err("SECRET_SAVE_FAILED", e))?;
        }
    }
    if let Some(token) = &input.ntfy_token {
        if token.trim().is_empty() {
            secrets::delete_token(NTFY_TOKEN_REF)
                .map_err(|e| ipc_err("SECRET_SAVE_FAILED", e))?;
        } else {
            secrets::save_token(NTFY_TOKEN_REF, token.trim())
                .map_err(|e| ipc_err("SECRET_SAVE_FAILED", e))?;
        }
    }

    // 2. 校验 + 原子落盘
    let interval_from_input = input.poll_interval_seconds;
    state
        .config
        .try_update::<_, String>(|c: &mut AppConfig| {
            if let Some(url) = &input.source_url {
                let url = url.trim();
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err("数据源地址须为 http(s) URL".into());
                }
                c.source_url = url.to_string();
            }
            if let Some(seconds) = interval_from_input {
                c.poll_interval_seconds = seconds.clamp(60, 86_400);
            }
            if let Some(chat) = &input.telegram_chat_id {
                c.telegram.chat_id = chat.trim().to_string();
            }
            if let Some(url) = &input.webhook_url {
                c.webhook.url = url.trim().to_string();
            }
            if let Some(url) = &input.ntfy_server_url {
                c.ntfy.server_url = url.trim().to_string();
            }
            if let Some(topic) = &input.ntfy_topic {
                c.ntfy.topic = topic.trim().to_string();
            }
            if let Some(settings) = &input.settings {
                c.settings = AppSettings {
                    auto_start: settings.auto_start,
                    language: settings.language.clone(),
                    log_level: settings.log_level.clone(),
                };
            }
            Ok(())
        })
        .map_err(|e| ipc_err("CONFIG_SAVE_FAILED", e))?
        .map_err(|e| ipc_err("CONFIG_INVALID", e))?;

    // 3. 运行时热更新
    let cfg = state.config.read();
    state.service.set_interval_seconds(cfg.poll_interval_seconds);

    if let Some(settings) = &input.settings {
        // 日志级别热更新（失败仅警告：配置已保存，重启后生效）
        if let (Some(handle), level) = (&state.log_reload, settings.log_level.trim())
            && !level.is_empty()
            && let Err(e) = crate::logging::set_log_level(handle, level)
        {
            tracing::warn!("{e}");
        }
        // 开机自启按设置对齐
        use tauri_plugin_autostart::ManagerExt;
        let autolaunch = app.autolaunch();
        let result = if settings.auto_start {
            autolaunch.enable()
        } else {
            autolaunch.disable()
        };
        if let Err(e) = result {
            tracing::warn!("对齐开机自启设置失败: {e}");
        }
    }

    Ok(config_view(&state))
}

// ---------- 旧版数据导入 ----------

#[tauri::command]
pub fn legacy_import(
    state: State<'_, AppState>,
    dir: String,
    overwrite_db: Option<bool>,
) -> Result<LegacyImportReport, String> {
    let dir = std::path::PathBuf::from(dir);
    if !dir.is_dir() {
        return Err(ipc_err("LEGACY_INVALID_DIR", "目录不存在"));
    }
    let mut report = LegacyImportReport::default();

    // 1. 旧 config.json（源地址 / 间隔 / telegram chat_id / webhook；token 转 keyring）
    let config_path = dir.join("config.json");
    if config_path.exists() {
        let text = std::fs::read_to_string(&config_path)
            .map_err(|e| ipc_err("LEGACY_READ_FAILED", e))?;
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| ipc_err("LEGACY_CONFIG_CORRUPT", e))?;

        if let Some(url) = value.get("source_url").and_then(|v| v.as_str()) {
            report.source_url = Some(url.to_string());
        }
        let interval = value.get("poll_interval_seconds").and_then(|v| v.as_u64());
        let chat_id = value
            .pointer("/telegram/chat_id")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let webhook = value
            .pointer("/webhook/url")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        state
            .config
            .update(|c: &mut AppConfig| {
                if let Some(url) = &report.source_url {
                    c.source_url = url.clone();
                }
                if let Some(seconds) = interval {
                    c.poll_interval_seconds = seconds.clamp(60, 86_400);
                }
                if let Some(chat) = &chat_id {
                    c.telegram.chat_id = chat.clone();
                }
                if let Some(url) = &webhook {
                    c.webhook.url = url.clone();
                }
            })
            .map_err(|e| ipc_err("CONFIG_SAVE_FAILED", e))?;
        report.config_imported = true;
        report.detail.push("已导入配置（数据源 / 采集间隔 / 告警通道）".into());

        if let Some(token) = value
            .pointer("/telegram/bot_token")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            secrets::save_token(TELEGRAM_TOKEN_REF, token)
                .map_err(|e| ipc_err("SECRET_SAVE_FAILED", e))?;
            report.telegram_token_imported = true;
            report.detail.push("Telegram bot token 已转入凭据管理器".into());
        }
    }

    // 2. 旧数据库（data/monitor.db 或目录下 monitor.db）
    let candidates = [dir.join("data").join("monitor.db"), dir.join("monitor.db")];
    if let Some(src_db) = candidates.iter().find(|p| p.exists()) {
        let current = state.db.product_count().map_err(db_err)?;
        if current > 0 && !overwrite_db.unwrap_or(false) {
            return Err(ipc_err(
                "LEGACY_DB_CONFLICT",
                format!("当前库已有 {current} 条产品数据；覆盖导入旧库会替换它们，请勾选确认"),
            ));
        }
        state
            .db
            .replace_with(src_db)
            .map_err(|e| ipc_err("LEGACY_DB_IMPORT_FAILED", e))?;
        report.db_imported = true;
        report.detail.push("已导入旧数据库（产品 / 事件 / 规则 / 关注 / 告警历史）".into());
    }

    // 3. 间隔同步到调度器
    state
        .service
        .set_interval_seconds(state.config.read().poll_interval_seconds);
    if report.detail.is_empty() {
        report
            .detail
            .push("目录中未找到可导入的 config.json 或数据库".into());
    }
    Ok(report)
}

#[tauri::command]
pub fn app_open_log_dir(state: State<'_, AppState>) -> Result<(), String> {
    crate::open_in_explorer(&state.log_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> RuleInput {
        RuleInput {
            name: "规则".into(),
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
        }
    }

    #[test]
    fn rule_validation() {
        assert!(validate_rule(&input()).is_ok());
        // 多事件类型、telegram 通道合法
        let mut r = input();
        r.event_types = "restock, price_down,price_up".into();
        r.channel = "telegram".into();
        assert!(validate_rule(&r).is_ok());

        // ntfy 通道合法
        let mut r = input();
        r.channel = "ntfy".into();
        assert!(validate_rule(&r).is_ok());

        // 各类非法输入
        let mut r = input();
        r.name = "  ".into();
        assert!(validate_rule(&r).unwrap_err().contains("RULE_INVALID"));
        let mut r = input();
        r.event_types = "".into();
        assert!(validate_rule(&r).is_err());
        let mut r = input();
        r.event_types = "restock,bogus".into();
        assert!(validate_rule(&r).is_err());
        let mut r = input();
        r.channel = "sms".into();
        assert!(validate_rule(&r).is_err());
        let mut r = input();
        r.cooldown_minutes = 1441;
        assert!(validate_rule(&r).is_err());
        let mut r = input();
        r.min_price_usd = Some(10.0);
        r.max_price_usd = Some(5.0);
        assert!(validate_rule(&r).is_err());
    }

    #[test]
    fn ipc_err_shape() {
        let raw = ipc_err("RULE_NOT_FOUND", "规则不存在");
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["code"], "RULE_NOT_FOUND");
        assert_eq!(value["detail"], "规则不存在");
    }
}
