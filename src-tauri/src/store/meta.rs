//! meta KV 表：采集游标（etag / content_hash / 时间戳）。

use rusqlite::{params, Connection, OptionalExtension};

use crate::store::Database;

/// 事务内外均可：读取 meta 值
pub fn get(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    let value = conn
        .query_row("SELECT value FROM meta WHERE key = ?", params![key], |row| {
            row.get::<_, Option<String>>(0)
        })
        .optional()?;
    Ok(value.flatten())
}

/// 事务内外均可：写入 meta 值
pub fn set(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

impl Database {
    pub fn meta_get(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.with_conn(|c| get(c, key))
    }

    pub fn meta_set(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.with_conn(|c| set(c, key, value))
    }
}

#[cfg(test)]
mod tests {
    use crate::store::test_db;

    #[test]
    fn get_set_roundtrip_and_missing() {
        let db = test_db();
        assert_eq!(db.meta_get("nope").unwrap(), None);
        db.meta_set("etag", "\"abc\"").unwrap();
        assert_eq!(db.meta_get("etag").unwrap(), Some("\"abc\"".into()));
        // upsert 覆盖
        db.meta_set("etag", "\"def\"").unwrap();
        assert_eq!(db.meta_get("etag").unwrap(), Some("\"def\"".into()));
    }
}
