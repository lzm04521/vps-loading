//! events 表：本地长期事件（restock/soldout/price_up/price_down/new_product）。

use rusqlite::{params, Connection};

use crate::model::EventView;
use crate::monitor::diff::EventDraft;
use crate::store::Database;

/// 事务内写入一条事件，返回事件 id
pub fn insert(conn: &Connection, draft: &EventDraft, created_at: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO events(product_id, type, old_status, new_status,
                            old_price_usd, new_price_usd, created_at)
         VALUES(?,?,?,?,?,?,?)",
        params![
            draft.product_id,
            draft.event_type.as_str(),
            draft.old_status,
            draft.new_status,
            draft.old_price_usd,
            draft.new_price_usd,
            created_at
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

impl Database {
    pub fn list_events(&self, limit: i64, event_type: Option<&str>) -> rusqlite::Result<Vec<EventView>> {
        let limit = limit.clamp(1, 1000);
        self.with_conn(|c| {
            let to_view = |row: &rusqlite::Row| {
                Ok(EventView {
                    id: row.get(0)?,
                    product_id: row.get(1)?,
                    event_type: row.get(2)?,
                    old_status: row.get(3)?,
                    new_status: row.get(4)?,
                    old_price_usd: row.get(5)?,
                    new_price_usd: row.get(6)?,
                    created_at: row.get(7)?,
                    product_name: row.get(8)?,
                    vendor_key: row.get(9)?,
                    series_name: row.get(10)?,
                    current_price: row.get(11)?,
                    current_price_usd: row.get(12)?,
                    buy_url: row.get(13)?,
                    blog_url: row.get(17)?,
                    region_cn: row.get(14)?,
                    vendor_name: row.get(15)?,
                    unit: row.get(16)?,
                })
            };
            let mut stmt = c.prepare(&format!(
                "SELECT e.id, e.product_id, e.type, e.old_status, e.new_status,
                        e.old_price_usd, e.new_price_usd, e.created_at,
                        p.name, p.vendor_key, p.series_name, p.price, p.price_usd,
                        p.buy_url, p.region_cn, v.name, p.unit, p.blog_url
                 FROM events e
                 JOIN products p ON p.id = e.product_id
                 JOIN vendors v ON v.key = p.vendor_key
                 {}
                 ORDER BY e.created_at DESC, e.id DESC
                 LIMIT ?",
                match event_type.filter(|s| !s.is_empty()) {
                    Some(_) => "WHERE e.type = ?",
                    None => "",
                }
            ))?;
            match event_type.filter(|s| !s.is_empty()) {
                Some(t) => stmt.query_map(params![t, limit], to_view)?.collect(),
                None => stmt.query_map(params![limit], to_view)?.collect(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EventType, utcnow_iso};
    use crate::monitor::normalize::test_util;
    use crate::store::catalog;
    use crate::store::test_db;

    #[test]
    fn insert_and_list() {
        let db = test_db();
        let at = utcnow_iso();
        let mut draft = test_util::sample_product_draft();
        draft.blog_url = "https://example.com/review".into();
        db.with_tx(|tx| {
            catalog::upsert_vendor(tx, "vultr", "Vultr", "")?;
            catalog::upsert_series(tx, "vultr", "默认", "默认", "东京")?;
            let pid = catalog::insert_product(tx, &draft, &at)?;
            insert(
                tx,
                &EventDraft {
                    product_id: pid,
                    event_type: EventType::Restock,
                    old_status: Some(0),
                    new_status: Some(1),
                    old_price_usd: None,
                    new_price_usd: Some(5.5),
                },
                &at,
            )?;
            insert(
                tx,
                &EventDraft {
                    product_id: pid,
                    event_type: EventType::PriceDown,
                    old_status: Some(1),
                    new_status: Some(1),
                    old_price_usd: Some(6.0),
                    new_price_usd: Some(5.5),
                },
                &at,
            )
        })
        .unwrap();

        let all = db.list_events(100, None).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].event_type, "price_down"); // 新事件在前
        let filtered = db.list_events(100, Some("restock")).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].event_type, "restock");
        assert_eq!(filtered[0].new_price_usd, Some(5.5));
        assert_eq!(filtered[0].unit, "月");
        assert_eq!(filtered[0].blog_url, "https://example.com/review");
    }
}
