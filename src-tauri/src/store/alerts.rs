//! alerts 表：告警发送记录。

use rusqlite::{params, Connection, OptionalExtension};

use crate::model::{AlertView, utcnow_iso};
use crate::store::Database;

/// 一条告警落库记录
pub struct AlertRecord<'a> {
    pub event_id: i64,
    pub rule_id: i64,
    pub product_id: i64,
    pub channel: &'a str,
    pub message: &'a str,
    pub status: &'a str,
    pub error: Option<&'a str>,
}

impl Database {
    pub fn insert_alert(&self, record: &AlertRecord) -> rusqlite::Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO alerts(event_id, rule_id, product_id, channel, message,
                                   sent_at, status, error)
                 VALUES(?,?,?,?,?,?,?,?)",
                params![
                    record.event_id,
                    record.rule_id,
                    record.product_id,
                    record.channel,
                    record.message,
                    utcnow_iso(),
                    record.status,
                    record.error
                ],
            )?;
            Ok(())
        })
    }

    /// 冷却期检查：该规则对该产品在 since 之后是否已发过告警
    pub fn recent_alert_exists(&self, rule_id: i64, product_id: i64, since_iso: &str) -> bool {
        self.with_conn(|c| recent_alert_exists(c, rule_id, product_id, since_iso))
            .unwrap_or(false)
    }

    /// 删除单条告警记录；返回是否命中
    pub fn delete_alert(&self, id: i64) -> rusqlite::Result<bool> {
        self.with_conn(|c| {
            let removed = c.execute("DELETE FROM alerts WHERE id = ?", params![id])?;
            Ok(removed > 0)
        })
    }

    /// 清空全部告警记录，返回删除条数
    pub fn clear_alerts(&self) -> rusqlite::Result<usize> {
        self.with_conn(|c| c.execute("DELETE FROM alerts", []))
    }

    pub fn list_alerts(&self, limit: i64) -> rusqlite::Result<Vec<AlertView>> {
        let limit = limit.clamp(1, 500);
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT a.id, a.event_id, a.rule_id, a.product_id, a.channel, a.message,
                        a.sent_at, a.status, a.error, r.name, p.name, p.vendor_key,
                        p.price, p.buy_url, v.name, p.region_cn
                 FROM alerts a
                 JOIN products p ON p.id = a.product_id
                 JOIN vendors v ON v.key = p.vendor_key
                 LEFT JOIN rules r ON r.id = a.rule_id
                 ORDER BY a.sent_at DESC, a.id DESC
                 LIMIT ?",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(AlertView {
                    id: row.get(0)?,
                    event_id: row.get(1)?,
                    rule_id: row.get(2)?,
                    product_id: row.get(3)?,
                    channel: row.get(4)?,
                    message: row.get(5)?,
                    sent_at: row.get(6)?,
                    status: row.get(7)?,
                    error: row.get(8)?,
                    rule_name: row.get(9)?,
                    product_name: row.get(10)?,
                    vendor_key: row.get(11)?,
                    price: row.get(12)?,
                    buy_url: row.get(13)?,
                    vendor_name: row.get(14)?,
                    region_cn: row.get(15)?,
                })
            })?;
            rows.collect()
        })
    }
}

fn recent_alert_exists(
    conn: &Connection,
    rule_id: i64,
    product_id: i64,
    since_iso: &str,
) -> rusqlite::Result<bool> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM alerts
             WHERE rule_id = ? AND product_id = ? AND sent_at >= ?
             LIMIT 1",
            params![rule_id, product_id, since_iso],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EventType;
    use crate::monitor::normalize::test_util;
    use crate::store::catalog;
    use crate::store::events;
    use crate::store::test_db;

    #[test]
    fn insert_exists_and_list() {
        let db = test_db();
        let rule = db.list_rules(false).unwrap().remove(0);
        let (pid, event_id) = db
            .with_tx(|tx| {
                catalog::upsert_vendor(tx, "vultr", "Vultr", "")?;
                catalog::upsert_series(tx, "vultr", "默认", "默认", "东京")?;
                let pid = catalog::insert_product(tx, &test_util::sample_product_draft(), "2026-01-01T00:00:00+00:00")?;
                let eid = events::insert(
                    tx,
                    &crate::monitor::diff::EventDraft {
                        product_id: pid,
                        event_type: EventType::Restock,
                        old_status: Some(0),
                        new_status: Some(1),
                        old_price_usd: None,
                        new_price_usd: None,
                    },
                    "2026-01-01T00:00:00+00:00",
                )?;
                Ok((pid, eid))
            })
            .unwrap();

        db.insert_alert(&AlertRecord {
            event_id,
            rule_id: rule.id,
            product_id: pid,
            channel: "local",
            message: "消息",
            status: "local",
            error: None,
        })
        .unwrap();

        assert!(db.recent_alert_exists(rule.id, pid, "2000-01-01T00:00:00+00:00"));
        assert!(!db.recent_alert_exists(rule.id, pid, "2999-01-01T00:00:00+00:00"));

        let alerts = db.list_alerts(50).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].channel, "local");
        assert_eq!(alerts[0].status, "local");
        assert_eq!(alerts[0].rule_name.as_deref(), Some("全部补货提醒（本地）"));
    }

    #[test]
    fn delete_and_clear() {
        let db = test_db();
        let rule = db.list_rules(false).unwrap().remove(0);
        let (pid, event_id) = db
            .with_tx(|tx| {
                catalog::upsert_vendor(tx, "vultr", "Vultr", "")?;
                catalog::upsert_series(tx, "vultr", "默认", "默认", "东京")?;
                let pid = catalog::insert_product(tx, &test_util::sample_product_draft(), "2026-01-01T00:00:00+00:00")?;
                let eid = events::insert(
                    tx,
                    &crate::monitor::diff::EventDraft {
                        product_id: pid,
                        event_type: EventType::Restock,
                        old_status: Some(0),
                        new_status: Some(1),
                        old_price_usd: None,
                        new_price_usd: None,
                    },
                    "2026-01-01T00:00:00+00:00",
                )?;
                Ok((pid, eid))
            })
            .unwrap();

        for status in ["local", "sent"] {
            db.insert_alert(&AlertRecord {
                event_id,
                rule_id: rule.id,
                product_id: pid,
                channel: "local",
                message: "消息",
                status,
                error: None,
            })
            .unwrap();
        }
        let alerts = db.list_alerts(50).unwrap();
        assert_eq!(alerts.len(), 2);
        let first_id = alerts[0].id;

        // 单条删除；不存在的 id 返回 false
        assert!(db.delete_alert(first_id).unwrap());
        assert_eq!(db.list_alerts(50).unwrap().len(), 1);
        assert!(!db.delete_alert(999_999).unwrap());

        // 清空返回删除条数；空表再清空返回 0 不报错
        assert_eq!(db.clear_alerts().unwrap(), 1);
        assert!(db.list_alerts(50).unwrap().is_empty());
        assert_eq!(db.clear_alerts().unwrap(), 0);
    }
}
