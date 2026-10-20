use std::io::{self, Write};

/// 把日志文本中疑似 telegram bot token 的子串打码：
/// 匹配 `bot<数字>:<token段>`，token 段（字母/数字/下划线/连字符）≥30 字符才打码，
/// 避免误伤普通文本（如 robot:15 之类）。
pub fn redact(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        let mut matched_end = None;
        if s[i..].starts_with("bot")
            && bytes.get(i + 3).is_some_and(u8::is_ascii_digit)
        {
            // 消费 bot 前缀后的数字段
            let mut j = i + 3;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b':' {
                let mut k = j + 1;
                while k < bytes.len()
                    && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'-')
                {
                    k += 1;
                }
                if k - (j + 1) >= 30 {
                    matched_end = Some(k);
                }
            }
        }
        if let Some(end) = matched_end {
            out.push_str("bot***");
            i = end;
        } else {
            // 按字符边界推进（内容为 UTF-8）
            let ch_len = utf8_char_len(bytes[i]);
            let end = (i + ch_len).min(bytes.len());
            out.push_str(&s[i..end]);
            i = end;
        }
    }
    out
}

fn utf8_char_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// 包装 tracing_appender 写入器，所有落盘日志过 redact（token 永不入日志）
pub struct RedactingWriter<W: Write> {
    inner: W,
}

impl<W: Write> RedactingWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let redacted = redact(&text);
        self.inner.write_all(redacted.as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// 运行时可调日志级别的句柄（设置页热更新用）
pub type LogReloadHandle = tracing_subscriber::reload::Handle<
    tracing_subscriber::filter::EnvFilter,
    tracing_subscriber::Registry,
>;

/// 初始化按天滚动的文件日志（级别来自全局设置），返回热更新句柄。
pub fn init_logging(log_dir: &std::path::Path, level: &str) -> Result<LogReloadHandle, String> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    std::fs::create_dir_all(log_dir).map_err(|e| format!("创建日志目录失败: {e}"))?;
    let file_appender = tracing_appender::rolling::daily(log_dir, "vps-monitor.log");
    let (writer, guard) = tracing_appender::non_blocking(RedactingWriter::new(file_appender));
    // guard 需要与进程同生命周期；泄漏是最简单的保活方式（进程级单次初始化）
    std::mem::forget(guard);
    let filter = tracing_subscriber::EnvFilter::try_new(level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let (filter_layer, handle) = tracing_subscriber::reload::Layer::new(filter);
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(tracing_subscriber::fmt::layer().with_ansi(false).with_writer(writer))
        .init();
    Ok(handle)
}

/// 热更新日志级别（settings_update 调用；失败时配置已保存，重启后生效）
pub fn set_log_level(handle: &LogReloadHandle, level: &str) -> Result<(), String> {
    let filter = tracing_subscriber::EnvFilter::try_new(level)
        .map_err(|e| format!("非法日志级别 {level}: {e}"))?;
    handle
        .modify(|f| *f = filter)
        .map_err(|e| format!("更新日志级别失败: {e}"))
}

/// 安装 panic hook：panic 信息同步直写 panic.log（过 redact），
/// 绕过 tracing 的 non_blocking 缓冲——panic=abort 下缓冲内容会随进程一起丢失。
pub fn install_panic_hook(log_dir: &std::path::Path) {
    let panic_log = log_dir.join("panic.log");
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!(" at {}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "non-string panic payload".to_string()
        };
        let line = format!(
            "[{}] PANIC{location}: {payload}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f")
        );
        // 尽力而为：失败也无法再上报
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&panic_log)
        {
            let _ = f.write_all(redact(&line).as_bytes());
            let _ = f.flush();
        }
        default_hook(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_masks_telegram_bot_token() {
        // 真实 telegram token 形态：bot_id:35+ 字符 hash
        assert_eq!(
            redact("POST https://api.telegram.org/bot123456789:AAE-dEMo0hWx9Kq2LmNp4RsTuVwXyZ12345/sendMessage"),
            "POST https://api.telegram.org/bot***/sendMessage"
        );
    }

    #[test]
    fn redact_keeps_normal_text() {
        let normal = "采集完成：412 款，有货 387，售罄 25 (200 OK)";
        assert_eq!(redact(normal), normal);
    }

    #[test]
    fn redact_requires_token_length() {
        // token 段过短（普通文本）不打码
        assert_eq!(redact("robot:15 speed"), "robot:15 speed");
        assert_eq!(redact("bot12:short"), "bot12:short");
    }

    #[test]
    fn redacting_writer_masks_on_write() {
        let mut buf = Vec::new();
        {
            let mut w = RedactingWriter::new(&mut buf);
            w.write_all(b"hello bot999888777:AAEq2LmNp4RsTuVwXyZ12345678901234 tail\n").unwrap();
        }
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "hello bot*** tail\n"
        );
    }
}
