//! rules 表：告警规则。

use rusqlite::{params, Connection, OptionalExtension};

use crate::model::{Rule, RuleInput, utcnow_iso};
use crate::store::Database;

/// 首次初始化播种的默认规则（与旧版一致）
pub fn default_seed_input() -> RuleInput {
    RuleInput {
        name: "全部补货提醒（本地）".into(),
        enabled: true,
        event_types: "restock".into(),
        vendor_key: None,
        region_like: None,
        name_like: None,
        route_like: None,
        min_price_usd: None,
        max_price_usd: None,
        only_watchlist: false,
        channel: "local".into(),
        cooldown_minutes: 30,
    }
}

fn row_to_rule(row: &rusqlite::Row) -> rusqlite::Result<Rule> {
    Ok(Rule {
        id: row.get(0)?,
        name: row.get(1)?,
        enabled: row.get::<_, i64>(2)? != 0,
        event_types: row.get(3)?,
        vendor_key: row.get(4)?,
        region_like: row.get(5)?,
        name_like: row.get(6)?,
        route_like: row.get(7)?,
        min_price_usd: row.get(8)?,
        max_price_usd: row.get(9)?,
        only_watchlist: row.get::<_, i64>(10)? != 0,
        channel: row.get(11)?,
        cooldown_minutes: row.get(12)?,
        created_at: row.get(13)?,
    })
}

fn insert_rule(conn: &Connection, input: &RuleInput) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO rules(name, enabled, event_types, vendor_key, region_like,
                          name_like, route_like, min_price_usd, max_price_usd,
                          only_watchlist, channel, cooldown_minutes, created_at)
         VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)",
        params![
            input.name,
            input.enabled as i64,
            non_empty(input.event_types.as_str()).unwrap_or("restock"),
            input.vendor_key.as_deref().filter(|s| !s.is_empty()),
            input.region_like.as_deref().filter(|s| !s.is_empty()),
            input.name_like.as_deref().filter(|s| !s.is_empty()),
            input.route_like.as_deref().filter(|s| !s.is_empty()),
            input.min_price_usd,
            input.max_price_usd,
            input.only_watchlist as i64,
            non_empty(input.channel.as_str()).unwrap_or("local"),
            input.cooldown_minutes,
            utcnow_iso(),
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn non_empty(s: &str) -> Option<&str> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn get_rule(conn: &Connection, rule_id: i64) -> rusqlite::Result<Option<Rule>> {
    conn.query_row(
        "SELECT id, name, enabled, event_types, vendor_key, region_like, name_like,
                route_like, min_price_usd, max_price_usd, only_watchlist, channel,
                cooldown_minutes, created_at
         FROM rules WHERE id = ?",
        params![rule_id],
        row_to_rule,
    )
    .optional()
}

impl Database {
    pub fn list_rules(&self, enabled_only: bool) -> rusqlite::Result<Vec<Rule>> {
        self.with_conn(|c| {
            let sql = if enabled_only {
                "SELECT id, name, enabled, event_types, vendor_key, region_like, name_like,
                        route_like, min_price_usd, max_price_usd, only_watchlist, channel,
                        cooldown_minutes, created_at
                 FROM rules WHERE enabled = 1 ORDER BY id ASC"
            } else {
                "SELECT id, name, enabled, event_types, vendor_key, region_like, name_like,
                        route_like, min_price_usd, max_price_usd, only_watchlist, channel,
                        cooldown_minutes, created_at
                 FROM rules ORDER BY id ASC"
            };
            let mut stmt = c.prepare(sql)?;
            let rows = stmt.query_map([], row_to_rule)?;
            rows.collect()
        })
    }

    pub fn get_rule(&self, rule_id: i64) -> rusqlite::Result<Option<Rule>> {
        self.with_conn(|c| get_rule(c, rule_id))
    }

    pub fn create_rule(&self, input: &RuleInput) -> rusqlite::Result<Rule> {
        let id = self.with_conn(|c| insert_rule(c, input))?;
        self.get_rule(id).map(|r| r.expect("刚插入的规则必存在"))
    }

    pub fn update_rule(&self, rule_id: i64, input: &RuleInput) -> rusqlite::Result<Option<Rule>> {
        self.with_conn(|c| {
            if get_rule(c, rule_id)?.is_none() {
                return Ok(false);
            }
            c.execute(
                "UPDATE rules SET name = ?, enabled = ?, event_types = ?, vendor_key = ?,
                    region_like = ?, name_like = ?, route_like = ?, min_price_usd = ?,
                    max_price_usd = ?, only_watchlist = ?, channel = ?, cooldown_minutes = ?
                 WHERE id = ?",
                params![
                    input.name,
                    input.enabled as i64,
                    non_empty(input.event_types.as_str()).unwrap_or("restock"),
                    input.vendor_key.as_deref().filter(|s| !s.is_empty()),
                    input.region_like.as_deref().filter(|s| !s.is_empty()),
                    input.name_like.as_deref().filter(|s| !s.is_empty()),
                    input.route_like.as_deref().filter(|s| !s.is_empty()),
                    input.min_price_usd,
                    input.max_price_usd,
                    input.only_watchlist as i64,
                    non_empty(input.channel.as_str()).unwrap_or("local"),
                    input.cooldown_minutes,
                    rule_id,
                ],
            )?;
            Ok(true)
        })?;
        self.get_rule(rule_id)
    }

    pub fn delete_rule(&self, rule_id: i64) -> rusqlite::Result<bool> {
        self.with_conn(|c| {
            let n = c.execute("DELETE FROM rules WHERE id = ?", params![rule_id])?;
            Ok(n > 0)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::store::test_db;

    #[test]
    fn seeded_default_rule_present() {
        let db = test_db();
        let rules = db.list_rules(false).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "全部补货提醒（本地）");
        assert_eq!(rules[0].event_types, "restock");
        assert_eq!(rules[0].channel, "local");
        assert!(rules[0].enabled);
    }

    #[test]
    fn crud_roundtrip() {
        let db = test_db();
        let mut input = crate::store::rules::default_seed_input();
        input.name = "日本低价".into();
        input.event_types = "restock,price_down".into();
        input.region_like = Some("日本".into());
        input.max_price_usd = Some(10.0);
        input.channel = "telegram".into();

        let rule = db.create_rule(&input).unwrap();
        assert!(rule.id > 1);
        assert_eq!(rule.name, "日本低价");

        input.name = "日本低价 v2".into();
        input.enabled = false;
        let updated = db.update_rule(rule.id, &input).unwrap().unwrap();
        assert_eq!(updated.name, "日本低价 v2");
        assert!(!updated.enabled);
        assert_eq!(updated.max_price_usd, Some(10.0));

        assert!(db.delete_rule(rule.id).unwrap());
        assert!(!db.delete_rule(rule.id).unwrap());
        assert!(db.get_rule(rule.id).unwrap().is_none());
        // update 不存在的规则返回 None
        assert!(db.update_rule(99999, &input).unwrap().is_none());
    }
}
