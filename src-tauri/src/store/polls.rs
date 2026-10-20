//! polls 表：每次采集的 HTTP 状态/耗时/快照计数/错误。

use rusqlite::{params, Connection};

use crate::model::PollView;
use crate::store::Database;

pub struct PollRecord<'a> {
    pub fetched_at: &'a str,
    pub http_status: Option<i64>,
    pub changed: bool,
    pub total: Option<i64>,
    pub ok: Option<i64>,
    pub oos: Option<i64>,
    pub duration_ms: i64,
    pub etag: Option<&'a str>,
    pub last_modified: Option<&'a str>,
    pub error: Option<&'a str>,
}

/// 事务内外均可：记录一次采集
pub fn record(conn: &Connection, r: &PollRecord) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO polls(fetched_at, http_status, changed, total, ok, oos,
                          duration_ms, etag, last_modified, error)
         VALUES(?,?,?,?,?,?,?,?,?,?)",
        params![
            r.fetched_at,
            r.http_status,
            r.changed as i64,
            r.total,
            r.ok,
            r.oos,
            r.duration_ms,
            r.etag,
            r.last_modified,
            r.error
        ],
    )?;
    Ok(())
}

fn row_to_view(row: &rusqlite::Row) -> rusqlite::Result<PollView> {
    Ok(PollView {
        id: row.get(0)?,
        fetched_at: row.get(1)?,
        http_status: row.get(2)?,
        changed: row.get::<_, i64>(3)? != 0,
        total: row.get(4)?,
        ok: row.get(5)?,
        oos: row.get(6)?,
        duration_ms: row.get(7)?,
        error: row.get(10)?,
    })
}

impl Database {
    pub fn record_poll(&self, r: &PollRecord) -> rusqlite::Result<()> {
        self.with_conn(|c| record(c, r))
    }

    pub fn list_polls(&self, limit: i64) -> rusqlite::Result<Vec<PollView>> {
        let limit = limit.clamp(1, 500);
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id, fetched_at, http_status, changed, total, ok, oos,
                        duration_ms, etag, last_modified, error
                 FROM polls ORDER BY id DESC LIMIT ?",
            )?;
            let rows = stmt.query_map(params![limit], row_to_view)?;
            rows.collect()
        })
    }

    pub fn latest_poll(&self) -> rusqlite::Result<Option<PollView>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id, fetched_at, http_status, changed, total, ok, oos,
                        duration_ms, etag, last_modified, error
                 FROM polls ORDER BY id DESC LIMIT 1",
            )?;
            let mut rows = stmt.query([])?;
            match rows.next()? {
                Some(row) => Ok(Some(row_to_view(row)?)),
                None => Ok(None),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::utcnow_iso;
    use crate::store::test_db;

    #[test]
    fn record_and_list_desc() {
        let db = test_db();
        db.record_poll(&PollRecord {
            fetched_at: &utcnow_iso(),
            http_status: Some(200),
            changed: false,
            total: Some(10),
            ok: Some(8),
            oos: Some(2),
            duration_ms: 120,
            etag: None,
            last_modified: None,
            error: None,
        })
        .unwrap();
        db.record_poll(&PollRecord {
            fetched_at: &utcnow_iso(),
            http_status: None,
            changed: false,
            total: None,
            ok: None,
            oos: None,
            duration_ms: 5,
            etag: None,
            last_modified: None,
            error: Some("网络失败"),
        })
        .unwrap();

        let polls = db.list_polls(50).unwrap();
        assert_eq!(polls.len(), 2);
        assert_eq!(polls[0].http_status, None);
        assert_eq!(polls[0].error.as_deref(), Some("网络失败"));
        assert_eq!(polls[1].http_status, Some(200));
        let latest = db.latest_poll().unwrap().unwrap();
        assert_eq!(latest.error.as_deref(), Some("网络失败"));
    }
}
