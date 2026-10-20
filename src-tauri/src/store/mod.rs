//! SQLite 持久化层（rusqlite bundled，沿用旧版 10 表 schema 与 WAL）。
//! 组织方式：`Database` 仅持有连接与事务助手；各表的数据访问按文件拆分
//! （meta / catalog / events / polls / rules / watchlist / alerts / source_history），
//! 事务内复用的函数以 `&Connection` 自由函数形式提供（Transaction 自动解引用传入）。

pub mod alerts;
pub mod catalog;
pub mod events;
pub mod meta;
pub mod polls;
pub mod rules;
pub mod source_history;
pub mod watchlist;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};

/// 与旧版 db.py 完全一致的 schema（新库直接建，旧库 CREATE IF NOT EXISTS 幂等）。
pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT
);

CREATE TABLE IF NOT EXISTS vendors (
    key TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    logo TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS series (
    vendor_key TEXT NOT NULL,
    key TEXT NOT NULL,
    name TEXT NOT NULL,
    region TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (vendor_key, key)
);

CREATE TABLE IF NOT EXISTS products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vendor_key TEXT NOT NULL,
    series_key TEXT NOT NULL,
    series_name TEXT NOT NULL DEFAULT '',
    name TEXT NOT NULL,
    price TEXT NOT NULL DEFAULT '',
    price_usd REAL,
    region TEXT NOT NULL DEFAULT '',
    region_cn TEXT NOT NULL DEFAULT '',
    region_flag TEXT NOT NULL DEFAULT '',
    unit TEXT NOT NULL DEFAULT '',
    spec_json TEXT NOT NULL DEFAULT '{}',
    route TEXT NOT NULL DEFAULT '',
    ip_attr TEXT NOT NULL DEFAULT '',
    promo TEXT NOT NULL DEFAULT '',
    promo_exp TEXT NOT NULL DEFAULT '',
    buy_url TEXT NOT NULL DEFAULT '',
    buy_mode TEXT NOT NULL DEFAULT '',
    blog_url TEXT NOT NULL DEFAULT '',
    order_text TEXT NOT NULL DEFAULT '',
    status INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1,
    first_seen TEXT NOT NULL,
    last_seen TEXT NOT NULL,
    last_change TEXT,
    UNIQUE (vendor_key, series_key, name)
);
CREATE INDEX IF NOT EXISTS idx_products_status ON products(status);
CREATE INDEX IF NOT EXISTS idx_products_vendor ON products(vendor_key);
CREATE INDEX IF NOT EXISTS idx_products_region ON products(region_cn);
CREATE INDEX IF NOT EXISTS idx_products_price ON products(price_usd);
CREATE INDEX IF NOT EXISTS idx_products_active ON products(active);

CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL,
    type TEXT NOT NULL,
    old_status INTEGER,
    new_status INTEGER,
    old_price_usd REAL,
    new_price_usd REAL,
    created_at TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'poll'
);
CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at);
CREATE INDEX IF NOT EXISTS idx_events_product ON events(product_id);

CREATE TABLE IF NOT EXISTS polls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fetched_at TEXT NOT NULL,
    http_status INTEGER,
    changed INTEGER NOT NULL DEFAULT 0,
    total INTEGER,
    ok INTEGER,
    oos INTEGER,
    duration_ms INTEGER,
    etag TEXT,
    last_modified TEXT,
    error TEXT
);

CREATE TABLE IF NOT EXISTS source_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    time_text TEXT NOT NULL,
    occurred_at TEXT,
    name TEXT NOT NULL,
    price TEXT NOT NULL DEFAULT '',
    buy_url TEXT NOT NULL DEFAULT '',
    fingerprint TEXT NOT NULL UNIQUE
);
CREATE INDEX IF NOT EXISTS idx_source_history_time ON source_history(occurred_at);

CREATE TABLE IF NOT EXISTS rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    event_types TEXT NOT NULL DEFAULT 'restock',
    vendor_key TEXT,
    region_like TEXT,
    name_like TEXT,
    route_like TEXT,
    min_price_usd REAL,
    max_price_usd REAL,
    only_watchlist INTEGER NOT NULL DEFAULT 0,
    channel TEXT NOT NULL DEFAULT 'local',
    cooldown_minutes INTEGER NOT NULL DEFAULT 30,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS watchlist (
    product_id INTEGER PRIMARY KEY,
    note TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id INTEGER NOT NULL,
    rule_id INTEGER NOT NULL,
    product_id INTEGER NOT NULL,
    channel TEXT NOT NULL,
    message TEXT NOT NULL DEFAULT '',
    sent_at TEXT NOT NULL,
    status TEXT NOT NULL,
    error TEXT
);
CREATE INDEX IF NOT EXISTS idx_alerts_sent ON alerts(sent_at);
CREATE INDEX IF NOT EXISTS idx_alerts_product ON alerts(product_id);
CREATE INDEX IF NOT EXISTS idx_alerts_rule ON alerts(rule_id);
"#;

pub struct Database {
    path: PathBuf,
    conn: Mutex<Connection>,
}

impl Database {
    /// 打开（不存在则创建）数据库并初始化 schema、播种默认规则。
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建数据目录失败: {e}"))?;
        }
        let conn = Self::open_conn(path).map_err(|e| format!("打开数据库失败: {e}"))?;
        let db = Self {
            path: path.to_path_buf(),
            conn: Mutex::new(conn),
        };
        db.init_schema().map_err(|e| format!("初始化数据库失败: {e}"))?;
        Ok(db)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn open_conn(path: &Path) -> rusqlite::Result<Connection> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(15))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Ok(conn)
    }

    fn lock_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("db lock poisoned")
    }

    fn init_schema(&self) -> rusqlite::Result<()> {
        self.lock_conn().execute_batch(SCHEMA)?;
        // 默认规则播种（与旧版一致：仅首次）
        if meta::get(&self.lock_conn(), "rules_seeded")?.as_deref() != Some("1") {
            if self.list_rules(false)?.is_empty() {
                self.create_rule(&rules::default_seed_input())?;
            }
            meta::set(&self.lock_conn(), "rules_seeded", "1")?;
        }
        Ok(())
    }

    /// 单连接只读/单语句访问
    pub fn with_conn<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        let conn = self.lock_conn();
        f(&conn)
    }

    /// IMMEDIATE 事务；闭包内通过 store::* 自由函数共用同一连接
    pub fn with_tx<T>(
        &self,
        f: impl FnOnce(&rusqlite::Transaction) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    /// 用旧版数据目录里的 monitor.db 替换当前库（legacy_import 用）。
    /// 先把连接切到内存库以释放文件句柄，再复制（含 -wal/-shm），最后重开并重建 schema。
    pub fn replace_with(&self, src_db: &Path) -> Result<(), String> {
        {
            let mut conn = self.lock_conn();
            *conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        }
        for suffix in ["", "-wal", "-shm"] {
            let from = src_db.with_file_name(format!(
                "{}{suffix}",
                src_db.file_name().and_then(|n| n.to_str()).unwrap_or_default()
            ));
            if !from.exists() {
                continue;
            }
            let to = self.path.with_file_name(format!(
                "{}{suffix}",
                self.path.file_name().and_then(|n| n.to_str()).unwrap_or_default()
            ));
            if suffix.is_empty() {
                // 主库必须存在
                fs::copy(&from, &to).map_err(|e| format!("复制数据库失败: {e}"))?;
            } else {
                let _ = fs::copy(&from, &to);
            }
        }
        {
            let mut conn = self.lock_conn();
            *conn = Self::open_conn(&self.path).map_err(|e| format!("重开数据库失败: {e}"))?;
        }
        self.init_schema().map_err(|e| format!("重建 schema 失败: {e}"))
    }
}

#[cfg(test)]
pub(crate) fn test_db() -> Database {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep().join("monitor.db");
    Database::open(&path).unwrap()
}
