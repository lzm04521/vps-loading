//! vendors / series / products 主数据访问。
//! 事务内复用的写入函数以 `&Connection` 自由函数提供（采集引擎的事务里调用）。

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::model::{Counts, ProductPage, ProductQuery, ProductView, RegionStat, VendorStat};
use crate::monitor::normalize::ProductDraft;
use crate::store::Database;

/// products_map 的精简行（采集 diff 用）
#[derive(Debug, Clone)]
pub struct ProductKeyRow {
    pub id: i64,
    pub vendor_key: String,
    pub series_key: String,
    pub name: String,
    pub status: i64,
    pub price_usd: Option<f64>,
}

impl ProductKeyRow {
    pub fn key(&self) -> (String, String, String) {
        (self.vendor_key.clone(), self.series_key.clone(), self.name.clone())
    }
}

/// 告警匹配/消息拼装需要的完整产品信息（联表 vendors + watchlist）
#[derive(Debug, Clone)]
pub struct ProductFull {
    pub id: i64,
    pub vendor_key: String,
    pub vendor_name: String,
    pub series_name: String,
    pub name: String,
    pub price: String,
    pub price_usd: Option<f64>,
    pub region: String,
    pub region_cn: String,
    pub route: String,
    pub unit: String,
    pub promo: String,
    pub buy_url: String,
    pub status: i64,
    pub watched: bool,
}

// ---------- 事务内自由函数 ----------

pub fn upsert_vendor(conn: &Connection, key: &str, name: &str, logo: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO vendors(key, name, logo) VALUES(?, ?, ?) \
         ON CONFLICT(key) DO UPDATE SET name = excluded.name, logo = excluded.logo",
        params![key, name, logo],
    )?;
    Ok(())
}

pub fn upsert_series(
    conn: &Connection,
    vendor_key: &str,
    key: &str,
    name: &str,
    region: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO series(vendor_key, key, name, region) VALUES(?, ?, ?, ?) \
         ON CONFLICT(vendor_key, key) DO UPDATE SET name = excluded.name, region = excluded.region",
        params![vendor_key, key, name, region],
    )?;
    Ok(())
}

pub fn insert_product(conn: &Connection, p: &ProductDraft, now: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO products(
            vendor_key, series_key, series_name, name, price, price_usd,
            region, region_cn, region_flag, unit, spec_json, route, ip_attr,
            promo, promo_exp, buy_url, buy_mode, blog_url, order_text,
            status, active, first_seen, last_seen, last_change
        ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,1,?,?,?)",
        params![
            p.vendor_key,
            p.series_key,
            p.series_name,
            p.name,
            p.price,
            p.price_usd,
            p.region,
            p.region_cn,
            p.region_flag,
            p.unit,
            p.spec_json,
            p.route,
            p.ip_attr,
            p.promo,
            p.promo_exp,
            p.buy_url,
            p.buy_mode,
            p.blog_url,
            p.order_text,
            p.status,
            now,
            now,
            now,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_product(conn: &Connection, pid: i64, p: &ProductDraft, now: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE products SET
            series_name = ?, price = ?, price_usd = ?, region = ?, region_cn = ?,
            region_flag = ?, unit = ?, spec_json = ?, route = ?, ip_attr = ?,
            promo = ?, promo_exp = ?, buy_url = ?, buy_mode = ?, blog_url = ?,
            order_text = ?, status = ?, active = 1, last_seen = ?
        WHERE id = ?",
        params![
            p.series_name,
            p.price,
            p.price_usd,
            p.region,
            p.region_cn,
            p.region_flag,
            p.unit,
            p.spec_json,
            p.route,
            p.ip_attr,
            p.promo,
            p.promo_exp,
            p.buy_url,
            p.buy_mode,
            p.blog_url,
            p.order_text,
            p.status,
            now,
            pid,
        ],
    )?;
    Ok(())
}

pub fn set_last_change(conn: &Connection, pid: i64, ts: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE products SET last_change = ? WHERE id = ?", params![ts, pid])?;
    Ok(())
}

/// 把本轮未见的 active 产品置为下线，返回下线数量
pub fn mark_inactive(conn: &Connection, seen_at: &str) -> rusqlite::Result<i64> {
    let n = conn.execute(
        "UPDATE products SET active = 0 WHERE active = 1 AND last_seen <> ?",
        params![seen_at],
    )?;
    Ok(n as i64)
}

/// 事务内查询当前快照计数（total, ok, oos）
pub fn snapshot_counts(conn: &Connection) -> rusqlite::Result<(i64, i64, i64)> {
    conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(status = 1), 0),
                COALESCE(SUM(status = 0), 0)
         FROM products WHERE active = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
}

// ---------- Database 方法 ----------

/// 事务内可用的全量产品键值映射（采集 diff 用）
pub fn products_map_conn(
    conn: &Connection,
) -> rusqlite::Result<HashMap<(String, String, String), ProductKeyRow>> {
    let mut stmt =
        conn.prepare("SELECT id, vendor_key, series_key, name, status, price_usd FROM products")?;
    let rows = stmt.query_map([], |row| {
        Ok(ProductKeyRow {
            id: row.get(0)?,
            vendor_key: row.get(1)?,
            series_key: row.get(2)?,
            name: row.get(3)?,
            status: row.get(4)?,
            price_usd: row.get(5)?,
        })
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let row = row?;
        map.insert(row.key(), row);
    }
    Ok(map)
}

impl Database {
    pub fn products_map(&self) -> rusqlite::Result<HashMap<(String, String, String), ProductKeyRow>> {
        self.with_conn(products_map_conn)
    }

    pub fn product_count(&self) -> rusqlite::Result<i64> {
        self.with_conn(|c| c.query_row("SELECT COUNT(*) FROM products", [], |r| r.get(0)))
    }

    pub fn get_product_full(&self, product_id: i64) -> rusqlite::Result<Option<ProductFull>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT p.id, p.vendor_key, v.name, p.series_name, p.name, p.price, p.price_usd,
                        p.region, p.region_cn, p.route, p.unit, p.promo, p.buy_url, p.status,
                        CASE WHEN w.product_id IS NULL THEN 0 ELSE 1 END
                 FROM products p
                 JOIN vendors v ON v.key = p.vendor_key
                 LEFT JOIN watchlist w ON w.product_id = p.id
                 WHERE p.id = ?",
            )?;
            let mut rows = stmt.query_map(params![product_id], |row| {
                Ok(ProductFull {
                    id: row.get(0)?,
                    vendor_key: row.get(1)?,
                    vendor_name: row.get(2)?,
                    series_name: row.get(3)?,
                    name: row.get(4)?,
                    price: row.get(5)?,
                    price_usd: row.get(6)?,
                    region: row.get(7)?,
                    region_cn: row.get(8)?,
                    route: row.get(9)?,
                    unit: row.get(10)?,
                    promo: row.get(11)?,
                    buy_url: row.get(12)?,
                    status: row.get(13)?,
                    watched: row.get::<_, i64>(14)? != 0,
                })
            })?;
            rows.next().transpose()
        })
    }

    /// 总览计数（含 24h 事件/告警数）
    pub fn counts(&self) -> rusqlite::Result<Counts> {
        let since = (chrono::Utc::now() - chrono::Duration::hours(24))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
        self.with_conn(|c| {
            let (total, ok, oos) = c.query_row(
                "SELECT COUNT(*),
                        COALESCE(SUM(status = 1), 0),
                        COALESCE(SUM(status = 0), 0)
                 FROM products WHERE active = 1",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
            )?;
            let watched: i64 =
                c.query_row("SELECT COUNT(*) FROM watchlist", [], |r| r.get(0))?;
            let events_24h: i64 = c.query_row(
                "SELECT COUNT(*) FROM events WHERE created_at >= ?",
                params![since],
                |r| r.get(0),
            )?;
            let alerts_24h: i64 = c.query_row(
                "SELECT COUNT(*) FROM alerts WHERE sent_at >= ?",
                params![since],
                |r| r.get(0),
            )?;
            Ok(Counts {
                total,
                ok,
                oos,
                watched,
                events_24h,
                alerts_24h,
            })
        })
    }

    pub fn query_products(&self, query: &ProductQuery) -> rusqlite::Result<ProductPage> {
        let limit = query.limit.clamp(1, 2000);
        let offset = query.offset.max(0);
        let order_sql = sort_sql(&query.sort);

        self.with_conn(|c| {
            let (where_sql, params) = build_where(query);
            let base_sql = format!(
                "FROM products p
                 JOIN vendors v ON v.key = p.vendor_key
                 LEFT JOIN watchlist w ON w.product_id = p.id
                 WHERE {where_sql}"
            );
            let total: i64 = c.query_row(
                &format!("SELECT COUNT(*) {base_sql}"),
                rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                |r| r.get(0),
            )?;

            let mut stmt = c.prepare(&format!(
                "SELECT p.id, p.vendor_key, v.name, p.series_key, p.series_name, p.name,
                        p.price, p.price_usd, p.region, p.region_cn, p.region_flag, p.unit,
                        p.spec_json, p.route, p.ip_attr, p.promo, p.promo_exp, p.buy_url,
                        p.buy_mode, p.blog_url, p.order_text, p.status, p.first_seen,
                        p.last_seen, p.last_change,
                        CASE WHEN w.product_id IS NULL THEN 0 ELSE 1 END
                 {base_sql}
                 ORDER BY {order_sql}
                 LIMIT ? OFFSET ?"
            ))?;
            let mut all_params = params;
            all_params.push(Box::new(limit));
            all_params.push(Box::new(offset));
            let rows = stmt.query_map(
                rusqlite::params_from_iter(all_params.iter().map(|p| p.as_ref())),
                row_to_product_view,
            )?;
            let items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(ProductPage { total, items })
        })
    }

    pub fn list_vendors(&self) -> rusqlite::Result<Vec<VendorStat>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT v.key, v.name, v.logo,
                        COUNT(p.id) AS total,
                        COALESCE(SUM(CASE WHEN p.status = 1 THEN 1 ELSE 0 END), 0) AS ok
                 FROM vendors v
                 LEFT JOIN products p ON p.vendor_key = v.key AND p.active = 1
                 GROUP BY v.key
                 HAVING total > 0
                 ORDER BY total DESC, v.name ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(VendorStat {
                    key: row.get(0)?,
                    name: row.get(1)?,
                    logo: row.get(2)?,
                    total: row.get(3)?,
                    ok: row.get(4)?,
                })
            })?;
            rows.collect()
        })
    }

    pub fn list_regions(&self) -> rusqlite::Result<Vec<RegionStat>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT region_cn, COUNT(*) AS total, COALESCE(SUM(status = 1), 0) AS ok
                 FROM products
                 WHERE active = 1 AND region_cn <> ''
                 GROUP BY region_cn
                 ORDER BY total DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(RegionStat {
                    region_cn: row.get(0)?,
                    total: row.get(1)?,
                    ok: row.get(2)?,
                })
            })?;
            rows.collect()
        })
    }
}

/// 组装 WHERE 条件与参数（count 与列表两查共用）
fn build_where(query: &ProductQuery) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
    let mut where_sql = String::from("p.active = 1");
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    match query.status.as_str() {
        "in" => where_sql.push_str(" AND p.status = 1"),
        "out" => where_sql.push_str(" AND p.status = 0"),
        _ => {}
    }
    if let Some(vendor) = query.vendor.as_deref().filter(|s| !s.is_empty()) {
        where_sql.push_str(" AND p.vendor_key = ?");
        params.push(Box::new(vendor.to_string()));
    }
    if let Some(region) = query.region.as_deref().filter(|s| !s.is_empty()) {
        where_sql.push_str(" AND p.region_cn = ?");
        params.push(Box::new(region.to_string()));
    }
    if let Some(q) = query.q.as_deref().filter(|s| !s.trim().is_empty()) {
        let like = format!("%{}%", q.trim());
        where_sql.push_str(
            " AND (p.name LIKE ? OR p.series_name LIKE ? OR v.name LIKE ? \
               OR p.region_cn LIKE ? OR p.route LIKE ? OR p.promo LIKE ?)",
        );
        for _ in 0..6 {
            params.push(Box::new(like.clone()));
        }
    }
    if let Some(min) = query.min_price {
        where_sql.push_str(" AND p.price_usd >= ?");
        params.push(Box::new(min));
    }
    if let Some(max) = query.max_price {
        where_sql.push_str(" AND p.price_usd <= ?");
        params.push(Box::new(max));
    }
    if query.watch {
        where_sql.push_str(" AND w.product_id IS NOT NULL");
    }
    (where_sql, params)
}

fn row_to_product_view(row: &rusqlite::Row) -> rusqlite::Result<ProductView> {
    let spec_json: String = row.get(12)?;
    let spec = serde_json::from_str(&spec_json).unwrap_or_else(|_| serde_json::json!({}));
    Ok(ProductView {
        id: row.get(0)?,
        vendor_key: row.get(1)?,
        vendor_name: row.get(2)?,
        series_key: row.get(3)?,
        series_name: row.get(4)?,
        name: row.get(5)?,
        price: row.get(6)?,
        price_usd: row.get(7)?,
        region: row.get(8)?,
        region_cn: row.get(9)?,
        region_flag: row.get(10)?,
        unit: row.get(11)?,
        spec,
        route: row.get(13)?,
        ip_attr: row.get(14)?,
        promo: row.get(15)?,
        promo_exp: row.get(16)?,
        buy_url: row.get(17)?,
        buy_mode: row.get(18)?,
        blog_url: row.get(19)?,
        order_text: row.get(20)?,
        status: row.get::<_, i64>(21)? != 0,
        first_seen: row.get(22)?,
        last_seen: row.get(23)?,
        last_change: row.get(24)?,
        watched: row.get::<_, i64>(25)? != 0,
    })
}

/// 排序白名单（与旧版 SORT_SQL 一致）
fn sort_sql(sort: &str) -> &'static str {
    match sort {
        "price_asc" => "p.price_usd IS NULL, p.price_usd ASC, p.name ASC",
        "price_desc" => "p.price_usd IS NULL, p.price_usd DESC, p.name ASC",
        "name" => "p.name COLLATE NOCASE ASC",
        "change" => "COALESCE(p.last_change, p.first_seen) DESC",
        "vendor" => "v.name COLLATE NOCASE ASC, p.price_usd IS NULL, p.price_usd ASC",
        _ => "p.status DESC, p.price_usd IS NULL, p.price_usd ASC, p.name ASC",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::normalize::test_util;
    use crate::store::test_db;

    #[test]
    fn seed_query_and_counts() {
        let db = test_db();
        let d1 = test_util::sample_product_draft();
        let mut d2 = test_util::sample_product_draft();
        d2.name = "东京 2C2G".into();
        d2.status = 0;

        db.with_tx(|tx| {
            upsert_vendor(tx, "vultr", "Vultr", "")?;
            upsert_series(tx, "vultr", "默认", "默认", "东京")?;
            insert_product(tx, &d1, "2026-01-01T00:00:00+00:00")?;
            insert_product(tx, &d2, "2026-01-01T00:00:00+00:00")?;
            Ok(())
        })
        .unwrap();

        assert_eq!(db.product_count().unwrap(), 2);

        let page = db
            .query_products(&ProductQuery {
                status: "in".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].name, "东京 1C1G");

        let counts = db.counts().unwrap();
        assert_eq!((counts.total, counts.ok, counts.oos), (2, 1, 1));

        let vendors = db.list_vendors().unwrap();
        assert_eq!(vendors.len(), 1);
        assert_eq!(vendors[0].total, 2);

        let regions = db.list_regions().unwrap();
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].region_cn, "日本东京");
    }

    #[test]
    fn mark_inactive_and_map() {
        let db = test_db();
        let d = test_util::sample_product_draft();
        db.with_tx(|tx| {
            upsert_vendor(tx, "vultr", "Vultr", "")?;
            upsert_series(tx, "vultr", "默认", "默认", "东京")?;
            insert_product(tx, &d, "2026-01-01T00:00:00+00:00")
        })
        .unwrap();

        let map = db.products_map().unwrap();
        assert_eq!(map.len(), 1);

        // 下一轮未见 → 下线
        let removed = db
            .with_tx(|tx| mark_inactive(tx, "2026-01-02T00:00:00+00:00"))
            .unwrap();
        assert_eq!(removed, 1);
        assert_eq!(db.query_products(&ProductQuery::default()).unwrap().total, 0);
    }
}
