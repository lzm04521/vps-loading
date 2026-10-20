//! source_history 表：源站自带的补货动态（fingerprint 幂等）。

use rusqlite::{params, Connection};

use crate::model::HistoryView;
use crate::monitor::normalize::HistoryDraft;
use crate::store::Database;

/// 事务内批量幂等写入
pub fn upsert(conn: &Connection, items: &[HistoryDraft]) -> rusqlite::Result<()> {
    for item in items {
        conn.execute(
            "INSERT OR IGNORE INTO source_history(
                time_text, occurred_at, name, price, buy_url, fingerprint
            ) VALUES(?,?,?,?,?,?)",
            params![
                item.time_text,
                item.occurred_at,
                item.name,
                item.price,
                item.buy_url,
                item.fingerprint
            ],
        )?;
    }
    Ok(())
}

impl Database {
    pub fn list_source_history(&self, limit: i64) -> rusqlite::Result<Vec<HistoryView>> {
        let limit = limit.clamp(1, 200);
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id, time_text, occurred_at, name, price, buy_url
                 FROM source_history
                 ORDER BY COALESCE(occurred_at, '') DESC, id DESC
                 LIMIT ?",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(HistoryView {
                    id: row.get(0)?,
                    time_text: row.get(1)?,
                    occurred_at: row.get(2)?,
                    name: row.get(3)?,
                    price: row.get(4)?,
                    buy_url: row.get(5)?,
                })
            })?;
            rows.collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_db;

    fn draft(time: &str, name: &str) -> HistoryDraft {
        HistoryDraft {
            time_text: time.into(),
            occurred_at: None,
            name: name.into(),
            price: "$5".into(),
            buy_url: String::new(),
            fingerprint: format!("{time}|{name}"),
        }
    }

    #[test]
    fn upsert_idempotent_and_ordered() {
        let db = test_db();
        db.with_tx(|tx| {
            upsert(tx, &[draft("2026-01-02 10:00", "A"), draft("2026-01-01 09:00", "B")])
        })
        .unwrap();
        // 相同 fingerprint 不重复
        db.with_tx(|tx| upsert(tx, &[draft("2026-01-02 10:00", "A")]))
            .unwrap();

        let items = db.list_source_history(30).unwrap();
        assert_eq!(items.len(), 2);
        // occurred_at 为空时按 id 倒序（后插入的 B id 更大在前）
        assert_eq!(items[0].name, "B");
        assert_eq!(items[1].name, "A");
    }
}
