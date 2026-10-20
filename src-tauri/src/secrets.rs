use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("平台密钥库错误: {0}")]
    Keyring(String),
    #[error("凭据不存在: {0}")]
    NotFound(String),
}

/// keyring 条目的 service 名
pub const KEYRING_SERVICE: &str = "vpsmonitor";

/// telegram bot token 的 keyring 条目引用（account 名）
pub const TELEGRAM_TOKEN_REF: &str = "vpsmonitor:telegram-bot-token";

pub fn save_token(token_ref: &str, token: &str) -> Result<(), SecretsError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, token_ref).map_err(keyring_err)?;
    entry.set_password(token).map_err(keyring_err)
}

pub fn load_token(token_ref: &str) -> Result<String, SecretsError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, token_ref).map_err(keyring_err)?;
    match entry.get_password() {
        Ok(token) => Ok(token),
        Err(keyring::Error::NoEntry) => Err(SecretsError::NotFound(token_ref.to_string())),
        Err(e) => Err(keyring_err(e)),
    }
}

/// token 是否已配置（不存在不算错误；keyring 本身故障才返回 Err）
pub fn token_exists(token_ref: &str) -> Result<bool, SecretsError> {
    match load_token(token_ref) {
        Ok(_) => Ok(true),
        Err(SecretsError::NotFound(_)) => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn delete_token(token_ref: &str) -> Result<(), SecretsError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, token_ref).map_err(keyring_err)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(keyring_err(e)),
    }
}

fn keyring_err(e: keyring::Error) -> SecretsError {
    SecretsError::Keyring(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每个测试用独立引用（时间戳+序号），避免串扰真实凭据 */
    fn fresh_ref() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        format!("test:{}-{}", chrono::Utc::now().timestamp_micros(), n)
    }

    #[test]
    fn token_ref_format() {
        assert_eq!(TELEGRAM_TOKEN_REF, "vpsmonitor:telegram-bot-token");
    }

    #[test]
    fn save_load_delete_roundtrip() {
        let r = fresh_ref();
        save_token(&r, "123456:AAE_unit_test_token_xxx").unwrap();
        assert_eq!(load_token(&r).unwrap(), "123456:AAE_unit_test_token_xxx");
        assert!(token_exists(&r).unwrap());
        delete_token(&r).unwrap();
        assert!(!token_exists(&r).unwrap());
        assert!(matches!(load_token(&r), Err(SecretsError::NotFound(_))));
    }

    #[test]
    fn delete_missing_is_ok() {
        let r = fresh_ref();
        delete_token(&r).unwrap();
    }
}
