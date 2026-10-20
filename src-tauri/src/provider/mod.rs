//! 外部数据源抽象层：SourceProvider trait + DTO + 错误分类 + 工厂。
//! 引擎只依赖本 trait，不感知 kyoa.de 的存在；加新源站 = 新增一个 provider 文件。

pub mod kyoa;

use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use crate::model::RequestConfig;

#[derive(Debug, Error)]
pub enum SourceError {
    #[error("网络错误: {0}")]
    Network(String),
    #[error("HTTP {status}: {snippet}")]
    Http { status: u16, snippet: String },
    #[error("解析失败: {0}")]
    Parse(String),
}

/// 一次拉取的结果
pub enum FetchOutcome {
    NotModified {
        etag: Option<String>,
        last_modified: Option<String>,
    },
    Modified {
        text: String,
        etag: Option<String>,
        last_modified: Option<String>,
    },
}

/// 数据源端口：拉取（支持 ETag/Last-Modified 条件请求）+ 文本 → payload JSON
#[async_trait]
pub trait SourceProvider: Send + Sync {
    async fn fetch(
        &self,
        url: &str,
        request: &RequestConfig,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<FetchOutcome, SourceError>;

    fn parse(&self, text: &str) -> Result<serde_json::Value, SourceError>;
}

/// 工厂：当前唯一实现是 kyoa.de
pub fn default_provider() -> Arc<dyn SourceProvider> {
    Arc::new(kyoa::KyoaProvider::new())
}
