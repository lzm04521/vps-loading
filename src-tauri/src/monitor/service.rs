//! 常驻调度：轮询循环 + 手动触发 + 采集互斥（busy 原子标志）+ 状态推送 +
//! 告警异步派发（独立任务 + 串行锁，修复旧版"告警发送阻塞采集锁"的问题）。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::model::{CollectResult, StatusEvent, utcnow_iso};
use crate::monitor::alerts;
use crate::monitor::engine::{self, CollectCtx};

/// 轮询间隔下限（秒），与旧版 run_loop 的 max(60, interval) 一致
const MIN_INTERVAL_SECONDS: u64 = 60;

pub struct MonitorService {
    ctx: Arc<CollectCtx>,
    busy: AtomicBool,
    interval_secs: AtomicU64,
    last_result: Mutex<Option<CollectResult>>,
    /// 告警派发串行锁：防止上一轮告警未发完时新一轮又并发发送
    alert_lock: Arc<tokio::sync::Mutex<()>>,
}

impl MonitorService {
    pub fn new(ctx: Arc<CollectCtx>) -> Arc<Self> {
        let interval = ctx.config.read().poll_interval_seconds;
        Arc::new(Self {
            ctx,
            busy: AtomicBool::new(false),
            interval_secs: AtomicU64::new(interval.max(MIN_INTERVAL_SECONDS)),
            last_result: Mutex::new(None),
            alert_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    pub fn interval_seconds(&self) -> u64 {
        self.interval_secs.load(Ordering::Relaxed).max(MIN_INTERVAL_SECONDS)
    }

    /// 设置页热更新轮询间隔（下一轮睡眠生效）
    pub fn set_interval_seconds(&self, seconds: u64) {
        self.interval_secs
            .store(seconds.max(MIN_INTERVAL_SECONDS), Ordering::Relaxed);
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    pub fn last_result(&self) -> Option<CollectResult> {
        self.last_result.lock().expect("last_result lock poisoned").clone()
    }

    /// 通知设置页"发送测试通知"：用已保存配置向指定通道发一条测试消息
    pub async fn send_test(&self, channel: crate::model::Channel) -> (String, Option<String>) {
        let cfg = self.ctx.config.read();
        alerts::send_test(&self.ctx, &cfg, channel).await
    }

    fn emit(&self, state: &str, message: &str) {
        self.ctx.sink.emit(StatusEvent {
            state: state.into(),
            message: message.into(),
            at: utcnow_iso(),
        });
    }

    /// 常驻轮询：先立即采一次，再按间隔循环（由 lib.rs spawn）
    pub async fn run(self: Arc<Self>) {
        loop {
            self.collect(false).await;
            let interval = self.interval_seconds();
            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    }

    /// 触发一次采集（自动/手动共用）。busy 时立即返回 skipped。
    pub async fn collect(&self, force: bool) -> CollectResult {
        if self.busy.swap(true, Ordering::SeqCst) {
            let result = CollectResult {
                ok: false,
                skipped: true,
                error: Some("已有采集任务在进行中".into()),
                ..Default::default()
            };
            self.emit("skipped", "已有采集任务在进行中");
            return result;
        }

        self.emit("running", if force { "手动强制采集开始" } else { "开始采集" });
        let (result, pending) = engine::run_collect_cycle(&self.ctx, force).await;
        self.busy.store(false, Ordering::SeqCst);
        *self.last_result.lock().expect("last_result lock poisoned") = Some(result.clone());

        let (state, message) = if !result.ok {
            (
                "error",
                result.error.clone().unwrap_or_else(|| "采集失败".into()),
            )
        } else if result.changed {
            (
                "done",
                format!(
                    "采集完成：{} 款（有货 {} / 售罄 {}），新增 {}，下线 {}，事件 {}",
                    result.total.unwrap_or(0),
                    result.ok_count.unwrap_or(0),
                    result.oos_count.unwrap_or(0),
                    result.added.unwrap_or(0),
                    result.removed.unwrap_or(0),
                    result.events.unwrap_or(0),
                ),
            )
        } else {
            (
                "done",
                if result.not_modified {
                    "数据未变化（304）".to_string()
                } else {
                    "数据无变化".to_string()
                },
            )
        };
        self.emit(state, &message);

        // 告警派发独立任务：不阻塞本次返回与下一次采集
        if !pending.is_empty() {
            let ctx = self.ctx.clone();
            let alert_lock = self.alert_lock.clone();
            tokio::spawn(async move {
                // 串行化：上一轮告警未发完时排队等待，避免并发轰炸通道
                let _guard = alert_lock.lock().await;
                let n = alerts::process_events(&ctx, &pending).await;
                if n > 0 {
                    tracing::info!("告警处理完成：{n} 条");
                }
            });
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigStore;
    use crate::model::{Counts, StatusEvent};
    use crate::monitor::engine::StatusSink;
    use crate::provider::SourceError;
    use crate::store::Database;
    use async_trait::async_trait;

    /// 收集状态事件的假 sink
    struct CapturingSink(std::sync::Mutex<Vec<StatusEvent>>);

    impl StatusSink for CapturingSink {
        fn emit(&self, event: StatusEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    // provider trait 需要 async_trait；直接复用 kyoa 的实现太重，测试里用 mock
    struct FailingProvider;

    #[async_trait]
    impl crate::provider::SourceProvider for FailingProvider {
        async fn fetch(
            &self,
            _url: &str,
            _request: &crate::model::RequestConfig,
            _etag: Option<&str>,
            _last_modified: Option<&str>,
        ) -> Result<crate::provider::FetchOutcome, SourceError> {
            Err(SourceError::Network("模拟网络失败".into()))
        }

        fn parse(&self, _text: &str) -> Result<serde_json::Value, SourceError> {
            Err(SourceError::Parse("不应被调用".into()))
        }
    }

    fn test_ctx(dir: &std::path::Path) -> Arc<CollectCtx> {
        struct NoopNotifier;
        impl crate::monitor::engine::Notifier for NoopNotifier {
            fn notify(&self, _title: &str, _body: &str) -> Result<(), String> {
                Ok(())
            }
        }
        let config = Arc::new(ConfigStore::load_from(dir.join("config.json")).unwrap());
        let db = Arc::new(Database::open(&dir.join("monitor.db")).unwrap());
        let sink: Arc<dyn StatusSink> = Arc::new(CapturingSink(std::sync::Mutex::new(Vec::new())));
        Arc::new(CollectCtx {
            config,
            db,
            provider: Arc::new(FailingProvider),
            sink,
            http: reqwest::Client::new(),
            notifier: Arc::new(NoopNotifier),
        })
    }

    #[tokio::test]
    async fn collect_failure_is_recorded_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_ctx(dir.path());
        let service = MonitorService::new(ctx.clone());

        let result = service.collect(false).await;
        assert!(!result.ok);
        assert!(!result.skipped);
        assert!(result.error.as_deref().unwrap().contains("模拟网络失败"));

        // 失败也落 polls 一行 + last_error
        let polls = ctx.db.list_polls(10).unwrap();
        assert_eq!(polls.len(), 1);
        assert!(polls[0].error.is_some());
        assert_eq!(
            ctx.db.meta_get("last_error").unwrap(),
            Some(result.error.unwrap())
        );
        assert!(service.last_result().is_some());
        assert!(!service.is_busy());
    }

    #[tokio::test]
    async fn busy_returns_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let service = MonitorService::new(test_ctx(dir.path()));
        service.busy.store(true, Ordering::SeqCst);
        let result = service.collect(false).await;
        assert!(result.skipped);
        assert!(!result.ok);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn interval_updates_apply() {
        let dir = tempfile::tempdir().unwrap();
        let service = MonitorService::new(test_ctx(dir.path()));
        assert_eq!(service.interval_seconds(), 180); // 默认
        service.set_interval_seconds(10);
        assert_eq!(service.interval_seconds(), 60); // 下限 60
        service.set_interval_seconds(300);
        assert_eq!(service.interval_seconds(), 300);
    }

    // Counts 字段被 summary 使用，避免未使用告警的编译期引用
    #[test]
    fn counts_type_usable() {
        let c = Counts::default();
        assert_eq!(c.total, 0);
    }
}
