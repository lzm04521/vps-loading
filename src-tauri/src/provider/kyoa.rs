//! kyoa.de data.js 拉取与解析。
//! 行为与旧版 source.py 对齐：条件请求、429/5xx 与网络错误退避重试（最多 3 次）、
//! `window.DATA = {...};` 前缀剥除 + 首个 JSON 值流式解析（等价 Python raw_decode）。

use async_trait::async_trait;
use std::time::Duration;

use super::{FetchOutcome, SourceError, SourceProvider};
use crate::model::RequestConfig;

pub struct KyoaProvider {
    client: reqwest::Client,
}

impl KyoaProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for KyoaProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// 响应是否可重试（与旧版一致：429/500/502/503/504）
fn retryable_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

#[async_trait]
impl SourceProvider for KyoaProvider {
    async fn fetch(
        &self,
        url: &str,
        request: &RequestConfig,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<FetchOutcome, SourceError> {
        let timeout = Duration::from_secs(request.timeout_seconds.clamp(1, 120));
        let mut last_error: Option<String> = None;

        for attempt in 0..3 {
            let mut builder = self
                .client
                .get(url)
                .timeout(timeout)
                .header(
                    "User-Agent",
                    if request.user_agent.is_empty() {
                        "vps-monitor/0.3"
                    } else {
                        &request.user_agent
                    },
                )
                .header("Accept", "application/javascript, text/javascript, */*;q=0.8");
            if let Some(etag) = etag {
                builder = builder.header("If-None-Match", etag);
            }
            if let Some(lm) = last_modified {
                builder = builder.header("If-Modified-Since", lm);
            }

            let response = match builder.send().await {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some(format!("{e}"));
                    tokio::time::sleep(Duration::from_secs(2 * (attempt as u64 + 1))).await;
                    continue;
                }
            };

            let status = response.status().as_u16();
            if retryable_status(status) && attempt < 2 {
                // body 丢弃即可；记录状态便于失败后诊断
                last_error = Some(format!("HTTP {status}"));
                tokio::time::sleep(Duration::from_secs(3 * (attempt as u64 + 1))).await;
                continue;
            }

            if status == 304 {
                return Ok(FetchOutcome::NotModified {
                    etag: response
                        .headers()
                        .get(reqwest::header::ETAG)
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_string)
                        .or_else(|| etag.map(str::to_string)),
                    last_modified: response
                        .headers()
                        .get(reqwest::header::LAST_MODIFIED)
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_string)
                        .or_else(|| last_modified.map(str::to_string)),
                });
            }
            if status != 200 {
                let body = response.text().await.unwrap_or_default();
                let snippet: String = body.chars().take(200).collect::<String>().replace('\n', " ");
                return Err(SourceError::Http { status, snippet });
            }

            let etag_out = response
                .headers()
                .get(reqwest::header::ETAG)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            let lm_out = response
                .headers()
                .get(reqwest::header::LAST_MODIFIED)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            // 与旧版 utf-8-sig + errors=replace 对齐：BOM 剥除、非法字节替换
            let bytes = response.bytes().await.map_err(|e| SourceError::Network(format!("{e}")))?;
            let text = String::from_utf8_lossy(&bytes).to_string();
            return Ok(FetchOutcome::Modified {
                text,
                etag: etag_out,
                last_modified: lm_out,
            });
        }

        Err(SourceError::Network(format!(
            "请求失败：{}",
            last_error.unwrap_or_else(|| "未知错误".into())
        )))
    }

    fn parse(&self, text: &str) -> Result<serde_json::Value, SourceError> {
        parse_payload(text)
    }
}

fn parse_err<T>(msg: impl Into<String>) -> Result<T, SourceError> {
    Err(SourceError::Parse(msg.into()))
}

/// data.js 文本 → payload JSON。
/// 支持三种形态：`window.DATA = {...};` 前缀、纯 JSON、以及 `{` 之前的杂前缀。
fn parse_payload(text: &str) -> Result<serde_json::Value, SourceError> {
    let text = text.trim_start_matches('\u{feff}').trim();
    if text.is_empty() {
        return parse_err("响应内容为空");
    }

    let candidate: &str = if let Some(rest) = text.strip_prefix("window.DATA") {
        let rest = rest.trim_start();
        match rest.strip_prefix('=') {
            Some(after) => after.trim(),
            None => return parse_err("window.DATA 后缺少 ="),
        }
    } else {
        // 纯 JSON 直接试
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(text)
            && v.is_object()
        {
            return Ok(v);
        }
        match text.find('{') {
            Some(start) => &text[start..],
            None => return parse_err("未找到 window.DATA JSON 对象"),
        }
    };

    // 流式取首个 JSON 值，忽略尾部多余内容（等价 Python raw_decode，兼容尾部分号）
    let mut stream = serde_json::Deserializer::from_str(candidate).into_iter::<serde_json::Value>();
    let value = match stream.next() {
        Some(Ok(v)) => v,
        Some(Err(e)) => return parse_err(format!("JSON 解析失败：{e}")),
        None => return parse_err("JSON 解析失败：空内容"),
    };
    if !value.as_object().is_some_and(|o| o.contains_key("vendors")) {
        return parse_err("数据结构异常：缺少 vendors 字段");
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_with_window_data_prefix_and_trailing_semicolon() {
        let text = r#"window.DATA = {"vendors":[{"key":"v"}],"history":[]};"#;
        let value = parse_payload(text).unwrap();
        assert!(value.get("vendors").is_some());
    }

    #[test]
    fn parse_plain_json_and_prefix_noise() {
        let plain = r#" {"vendors":[]} "#;
        assert!(parse_payload(plain).is_ok());

        // { 前有杂前缀：退到首个 '{' 解析
        let noisy = "var x = 1;\n{\"vendors\":[]}";
        assert!(parse_payload(noisy).is_ok());
    }

    #[test]
    fn parse_bom_stripped() {
        let text = "\u{feff}window.DATA={\"vendors\":[]};";
        assert!(parse_payload(text).is_ok());
    }

    #[test]
    fn parse_rejects_bad_input() {
        assert!(matches!(parse_payload(""), Err(SourceError::Parse(_))));
        assert!(matches!(parse_payload("no braces here"), Err(SourceError::Parse(_))));
        // 缺 vendors 字段
        assert!(matches!(
            parse_payload(r#"window.DATA = {"foo":1};"#),
            Err(SourceError::Parse(_))
        ));
        // JSON 中断
        assert!(matches!(
            parse_payload(r#"window.DATA = {"vendors":[};"#),
            Err(SourceError::Parse(_))
        ));
    }
}
