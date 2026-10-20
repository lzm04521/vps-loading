//! watchlist 表：关注套餐。

use rusqlite::{params, Connection, OptionalExtension};

use crate::model::utcnow_iso;
use crate::store::Database;

impl Database {
    /// 切换关注状态；返回 Some(watched) 或 None（套餐不存在）
    pub fn toggle_watch(&self, product_id: i64) -> rusqlite::Result<Option<bool>> {
        self.with_conn(|c| toggle_watch(c, product_id))
    }
}

fn toggle_watch(conn: &Connection, product_id: i64) -> rusqlite::Result<Option<bool>> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM products WHERE id = ?",
            params![product_id],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Ok(None);
    }
    let watched: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM watchlist WHERE product_id = ?",
            params![product_id],
            |row| row.get(0),
        )
        .optional()?;
    if watched.is_some() {
        conn.execute("DELETE FROM watchlist WHERE product_id = ?", params![product_id])?;
        Ok(Some(false))
    } else {
        conn.execute(
            "INSERT INTO watchlist(product_id, created_at) VALUES(?, ?)",
            params![product_id, utcnow_iso()],
        )?;
        Ok(Some(true))
    }
}

#[cfg(test)]
mod tests {
    use crate::monitor::normalize::test_util;
    use crate::store::catalog;
    use crate::store::test_db;

    fn seed_product(db: &crate::store::Database) -> i64 {
        let draft = test_util::sample_product_draft();
        db.with_tx(|tx| {
            catalog::upsert_vendor(tx, "vultr", "Vultr", "")?;
            catalog::upsert_series(tx, "vultr", "默认", "默认", "东京")?;
            catalog::insert_product(tx, &draft, "2026-01-01T00:00:00+00:00")
        })
        .unwrap()
    }

    #[test]
    fn toggle_watch_roundtrip() {
        let db = test_db();
        let pid = seed_product(&db);
        assert_eq!(db.toggle_watch(pid).unwrap(), Some(true));
        assert_eq!(db.toggle_watch(pid).unwrap(), Some(false));
        assert_eq!(db.toggle_watch(99999).unwrap(), None);
    }
}
