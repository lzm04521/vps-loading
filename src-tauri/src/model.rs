//! 领域模型与 IPC DTO。
//! 跨边界（IPC / 配置文件）一律 camelCase；枚举序列化为小写字符串。
//! 事件类型 / 告警通道的合法清单在本文件单源定义，前端 types.ts 镜像。

use serde::{Deserialize, Serialize};

// ---------- 枚举 ----------

/// 事件类型。DB 中以小写字符串存储（沿用旧版 schema）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Restock,
    Soldout,
    PriceDown,
    PriceUp,
    NewProduct,
}

impl EventType {
    pub const ALL: [EventType; 5] = [
        EventType::Restock,
        EventType::Soldout,
        EventType::PriceDown,
        EventType::PriceUp,
        EventType::NewProduct,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::Restock => "restock",
            EventType::Soldout => "soldout",
            EventType::PriceDown => "price_down",
            EventType::PriceUp => "price_up",
            EventType::NewProduct => "new_product",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        EventType::ALL.iter().copied().find(|t| t.as_str() == s)
    }
}

/// 告警通道。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Local,
    Telegram,
    Webhook,
    Windows,
    Ntfy,
}

impl Channel {
    pub const ALL: [Channel; 5] = [
        Channel::Local,
        Channel::Telegram,
        Channel::Webhook,
        Channel::Windows,
        Channel::Ntfy,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Channel::Local => "local",
            Channel::Telegram => "telegram",
            Channel::Webhook => "webhook",
            Channel::Windows => "windows",
            Channel::Ntfy => "ntfy",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Channel::ALL.iter().copied().find(|c| c.as_str() == s)
    }
}

/// 当前 UTC 时间，ISO-8601 带毫秒（与旧版 Python `utcnow_iso` 同格式含 +00:00 后缀，
/// DB 内时间统一用它，保证与旧库数据字符串比较语义一致）。
pub fn utcnow_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
}

// ---------- 配置 ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub source_url: String,
    pub poll_interval_seconds: u64,
    pub request: RequestConfig,
    /// telegram bot token 不落配置文件，存 Windows 凭据管理器（secrets.rs）
    pub telegram: TelegramConfig,
    pub webhook: WebhookConfig,
    /// ntfy token 同样不落配置文件；此处只存服务地址与 topic
    pub ntfy: NtfyConfig,
    pub settings: AppSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            source_url: "https://kyoa.de/data.js".into(),
            poll_interval_seconds: 180,
            request: RequestConfig::default(),
            telegram: TelegramConfig::default(),
            webhook: WebhookConfig::default(),
            ntfy: NtfyConfig::default(),
            settings: AppSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RequestConfig {
    pub timeout_seconds: u64,
    pub user_agent: String,
}

impl Default for RequestConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 30,
            user_agent: "vps-monitor/0.3 (personal inventory monitor)".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct TelegramConfig {
    pub chat_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct WebhookConfig {
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct NtfyConfig {
    pub server_url: String,
    pub topic: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub auto_start: bool,
    pub language: String,
    pub log_level: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_start: true,
            language: "zh-CN".into(),
            log_level: "info".into(),
        }
    }
}

// ---------- 规则 ----------

/// 规则完整视图（IPC 出参 / DB 行）。event_types 为逗号分隔串（沿用旧库存储格式）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub event_types: String,
    pub vendor_key: Option<String>,
    pub region_like: Option<String>,
    pub name_like: Option<String>,
    pub route_like: Option<String>,
    pub min_price_usd: Option<f64>,
    pub max_price_usd: Option<f64>,
    pub only_watchlist: bool,
    pub channel: String,
    pub cooldown_minutes: i64,
    pub created_at: String,
}

/// 规则创建/更新入参。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInput {
    pub name: String,
    pub enabled: bool,
    pub event_types: String,
    pub vendor_key: Option<String>,
    pub region_like: Option<String>,
    pub name_like: Option<String>,
    pub route_like: Option<String>,
    pub min_price_usd: Option<f64>,
    pub max_price_usd: Option<f64>,
    pub only_watchlist: bool,
    pub channel: String,
    pub cooldown_minutes: i64,
}

// ---------- 查询与视图 DTO ----------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProductQuery {
    pub status: String,
    pub vendor: Option<String>,
    pub region: Option<String>,
    pub q: Option<String>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub watch: bool,
    pub sort: String,
    pub limit: i64,
    pub offset: i64,
}

impl Default for ProductQuery {
    fn default() -> Self {
        Self {
            status: "all".into(),
            vendor: None,
            region: None,
            q: None,
            min_price: None,
            max_price: None,
            watch: false,
            sort: "default".into(),
            limit: 60,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductView {
    pub id: i64,
    pub vendor_key: String,
    pub vendor_name: String,
    pub series_key: String,
    pub series_name: String,
    pub name: String,
    pub price: String,
    pub price_usd: Option<f64>,
    pub region: String,
    pub region_cn: String,
    pub region_flag: String,
    pub unit: String,
    pub spec: serde_json::Value,
    pub route: String,
    pub ip_attr: String,
    pub promo: String,
    pub promo_exp: String,
    pub buy_url: String,
    pub buy_mode: String,
    pub blog_url: String,
    pub order_text: String,
    pub status: bool,
    pub first_seen: String,
    pub last_seen: String,
    pub last_change: Option<String>,
    pub watched: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductPage {
    pub total: i64,
    pub items: Vec<ProductView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorStat {
    pub key: String,
    pub name: String,
    pub logo: String,
    pub total: i64,
    pub ok: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionStat {
    pub region_cn: String,
    pub total: i64,
    pub ok: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventView {
    pub id: i64,
    pub product_id: i64,
    #[serde(rename = "type")]
    pub event_type: String,
    pub old_status: Option<i64>,
    pub new_status: Option<i64>,
    pub old_price_usd: Option<f64>,
    pub new_price_usd: Option<f64>,
    pub created_at: String,
    pub product_name: String,
    pub vendor_key: String,
    pub series_name: String,
    pub current_price: String,
    pub current_price_usd: Option<f64>,
    pub buy_url: String,
    /// 产品当前评测地址（事件表不存历史快照，JOIN 取当前值，语义与 buy_url 一致）
    pub blog_url: String,
    pub region_cn: String,
    pub vendor_name: String,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PollView {
    pub id: i64,
    pub fetched_at: String,
    pub http_status: Option<i64>,
    pub changed: bool,
    pub total: Option<i64>,
    pub ok: Option<i64>,
    pub oos: Option<i64>,
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertView {
    pub id: i64,
    pub event_id: i64,
    pub rule_id: i64,
    pub product_id: i64,
    pub channel: String,
    pub message: String,
    pub sent_at: String,
    pub status: String,
    pub error: Option<String>,
    pub rule_name: Option<String>,
    pub product_name: String,
    pub vendor_key: String,
    pub price: String,
    pub buy_url: String,
    pub vendor_name: String,
    pub region_cn: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryView {
    pub id: i64,
    pub time_text: String,
    pub occurred_at: Option<String>,
    pub name: String,
    pub price: String,
    pub buy_url: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub total: i64,
    pub ok: i64,
    pub oos: i64,
    pub watched: i64,
    pub events_24h: i64,
    pub alerts_24h: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryView {
    pub counts: Counts,
    pub last_poll: Option<PollView>,
    pub last_success: Option<String>,
    pub last_checked: Option<String>,
    pub last_data_change: Option<String>,
    pub last_error: String,
    pub source_url: String,
    pub initialized: bool,
    pub collector_busy: bool,
    pub poll_interval_seconds: u64,
    pub telegram_configured: bool,
    pub webhook_configured: bool,
    pub ntfy_configured: bool,
    pub db_path: String,
}

/// 单次采集结果（IPC 出参与引擎内部共用）。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CollectResult {
    pub ok: bool,
    pub skipped: bool,
    pub not_modified: bool,
    pub changed: bool,
    pub http_status: Option<u16>,
    pub fetched_at: Option<String>,
    pub duration_ms: i64,
    pub total: Option<i64>,
    pub ok_count: Option<i64>,
    pub oos_count: Option<i64>,
    pub added: Option<i64>,
    pub removed: Option<i64>,
    pub events: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigView {
    pub source_url: String,
    pub poll_interval_seconds: u64,
    pub telegram_chat_id: String,
    pub telegram_token_set: bool,
    pub webhook_url: String,
    pub ntfy_server_url: String,
    pub ntfy_topic: String,
    pub ntfy_token_set: bool,
    pub settings: AppSettings,
    pub config_path: String,
    pub db_path: String,
}

/// 配置更新入参：字段为 None 表示不改。telegramToken / ntfyToken 为 Some("") 表示清除。
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigInput {
    pub source_url: Option<String>,
    pub poll_interval_seconds: Option<u64>,
    pub telegram_chat_id: Option<String>,
    pub telegram_token: Option<String>,
    pub webhook_url: Option<String>,
    pub ntfy_server_url: Option<String>,
    pub ntfy_topic: Option<String>,
    pub ntfy_token: Option<String>,
    pub settings: Option<AppSettings>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportReport {
    pub config_imported: bool,
    pub telegram_token_imported: bool,
    pub db_imported: bool,
    pub source_url: Option<String>,
    pub detail: Vec<String>,
}

/// 引擎 → UI 的状态事件（前端 listen("monitor://status")）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEvent {
    /// running | done | error | skipped
    pub state: String,
    pub message: String,
    pub at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_type_roundtrip() {
        for t in EventType::ALL {
            assert_eq!(EventType::parse(t.as_str()), Some(t));
        }
        assert_eq!(EventType::parse("bogus"), None);
        let json = serde_json::to_string(&EventType::PriceDown).unwrap();
        assert_eq!(json, "\"price_down\"");
    }

    #[test]
    fn channel_roundtrip() {
        for c in Channel::ALL {
            assert_eq!(Channel::parse(c.as_str()), Some(c));
        }
        assert_eq!(Channel::parse("ntfy"), Some(Channel::Ntfy));
    }

    #[test]
    fn app_config_defaults_and_forward_compat() {
        let cfg: AppConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg.source_url, "https://kyoa.de/data.js");
        assert_eq!(cfg.poll_interval_seconds, 180);
        assert!(cfg.settings.auto_start);
        assert!(cfg.ntfy.server_url.is_empty());
        assert!(cfg.ntfy.topic.is_empty());
        // 旧配置缺 settings.log_level 键也能加载
        let legacy: AppConfig =
            serde_json::from_str(r#"{"sourceUrl":"https://x/data.js","settings":{"language":"en"}}"#)
                .unwrap();
        assert_eq!(legacy.source_url, "https://x/data.js");
        assert_eq!(legacy.settings.language, "en");
        assert_eq!(legacy.settings.log_level, "info");
        assert!(legacy.ntfy.server_url.is_empty());
        assert!(legacy.ntfy.topic.is_empty());
    }
}
