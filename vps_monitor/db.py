from __future__ import annotations

import json
import sqlite3
from contextlib import contextmanager
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Iterator, Optional

SCHEMA = """
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
"""

SORT_SQL = {
    "default": "p.status DESC, p.price_usd IS NULL, p.price_usd ASC, p.name ASC",
    "price_asc": "p.price_usd IS NULL, p.price_usd ASC, p.name ASC",
    "price_desc": "p.price_usd IS NULL, p.price_usd DESC, p.name ASC",
    "name": "p.name COLLATE NOCASE ASC",
    "change": "COALESCE(p.last_change, p.first_seen) DESC",
    "vendor": "v.name COLLATE NOCASE ASC, p.price_usd IS NULL, p.price_usd ASC",
}


def utcnow_iso() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


class Database:
    def __init__(self, path: str | Path):
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.init_schema()

    def connect(self) -> sqlite3.Connection:
        conn = sqlite3.connect(self.path, timeout=15, isolation_level=None)
        conn.row_factory = sqlite3.Row
        conn.execute("PRAGMA busy_timeout = 15000")
        conn.execute("PRAGMA foreign_keys = ON")
        return conn

    @contextmanager
    def _using(self, conn: Optional[sqlite3.Connection] = None) -> Iterator[sqlite3.Connection]:
        if conn is not None:
            yield conn
            return
        own = self.connect()
        try:
            yield own
        finally:
            own.close()

    @contextmanager
    def transaction(self) -> Iterator[sqlite3.Connection]:
        conn = self.connect()
        conn.execute("BEGIN IMMEDIATE")
        try:
            yield conn
            conn.execute("COMMIT")
        except Exception:
            conn.execute("ROLLBACK")
            raise
        finally:
            conn.close()

    def init_schema(self) -> None:
        conn = self.connect()
        try:
            conn.execute("PRAGMA journal_mode = WAL")
            conn.executescript(SCHEMA)
        finally:
            conn.close()
        if self.meta_get("rules_seeded") != "1":
            if not self.list_rules():
                self.create_rule(
                    {
                        "name": "全部补货提醒（本地）",
                        "enabled": True,
                        "event_types": "restock",
                        "channel": "local",
                        "cooldown_minutes": 30,
                    }
                )
            self.meta_set("rules_seeded", "1")

    # ---------- meta ----------

    def meta_get(self, key: str, default: Any = None, conn: Optional[sqlite3.Connection] = None) -> Any:
        with self._using(conn) as c:
            row = c.execute("SELECT value FROM meta WHERE key = ?", (key,)).fetchone()
        return row["value"] if row else default

    def meta_set(self, key: str, value: Any, conn: Optional[sqlite3.Connection] = None) -> None:
        with self._using(conn) as c:
            c.execute(
                "INSERT INTO meta(key, value) VALUES(?, ?) "
                "ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (key, "" if value is None else str(value)),
            )

    # ---------- vendors / series / products ----------

    def upsert_vendor(self, conn: sqlite3.Connection, key: str, name: str, logo: str) -> None:
        conn.execute(
            "INSERT INTO vendors(key, name, logo) VALUES(?, ?, ?) "
            "ON CONFLICT(key) DO UPDATE SET name = excluded.name, logo = excluded.logo",
            (key, name, logo),
        )

    def upsert_series(
        self, conn: sqlite3.Connection, vendor_key: str, key: str, name: str, region: str
    ) -> None:
        conn.execute(
            "INSERT INTO series(vendor_key, key, name, region) VALUES(?, ?, ?, ?) "
            "ON CONFLICT(vendor_key, key) DO UPDATE SET name = excluded.name, region = excluded.region",
            (vendor_key, key, name, region),
        )

    def products_map(self, conn: Optional[sqlite3.Connection] = None) -> dict[tuple[str, str, str], sqlite3.Row]:
        with self._using(conn) as c:
            rows = c.execute(
                "SELECT id, vendor_key, series_key, name, status, price, price_usd FROM products"
            ).fetchall()
        return {(r["vendor_key"], r["series_key"], r["name"]): r for r in rows}

    def product_count(self) -> int:
        with self._using() as c:
            row = c.execute("SELECT COUNT(*) AS n FROM products").fetchone()
        return int(row["n"])

    def insert_product(self, conn: sqlite3.Connection, p: dict[str, Any], now: str) -> int:
        cur = conn.execute(
            """
            INSERT INTO products(
                vendor_key, series_key, series_name, name, price, price_usd,
                region, region_cn, region_flag, unit, spec_json, route, ip_attr,
                promo, promo_exp, buy_url, buy_mode, blog_url, order_text,
                status, active, first_seen, last_seen, last_change
            ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,1,?,?,?)
            """,
            (
                p["vendor_key"],
                p["series_key"],
                p["series_name"],
                p["name"],
                p["price"],
                p["price_usd"],
                p["region"],
                p["region_cn"],
                p["region_flag"],
                p["unit"],
                p["spec_json"],
                p["route"],
                p["ip_attr"],
                p["promo"],
                p["promo_exp"],
                p["buy_url"],
                p["buy_mode"],
                p["blog_url"],
                p["order_text"],
                p["status"],
                now,
                now,
                now,
            ),
        )
        return int(cur.lastrowid)

    def update_product(self, conn: sqlite3.Connection, pid: int, p: dict[str, Any], now: str) -> None:
        conn.execute(
            """
            UPDATE products SET
                series_name = ?, price = ?, price_usd = ?, region = ?, region_cn = ?,
                region_flag = ?, unit = ?, spec_json = ?, route = ?, ip_attr = ?,
                promo = ?, promo_exp = ?, buy_url = ?, buy_mode = ?, blog_url = ?,
                order_text = ?, status = ?, active = 1, last_seen = ?
            WHERE id = ?
            """,
            (
                p["series_name"],
                p["price"],
                p["price_usd"],
                p["region"],
                p["region_cn"],
                p["region_flag"],
                p["unit"],
                p["spec_json"],
                p["route"],
                p["ip_attr"],
                p["promo"],
                p["promo_exp"],
                p["buy_url"],
                p["buy_mode"],
                p["blog_url"],
                p["order_text"],
                p["status"],
                now,
                pid,
            ),
        )

    def set_last_change(self, conn: sqlite3.Connection, pid: int, ts: str) -> None:
        conn.execute("UPDATE products SET last_change = ? WHERE id = ?", (ts, pid))

    def mark_inactive(self, conn: sqlite3.Connection, seen_at: str) -> int:
        cur = conn.execute(
            "UPDATE products SET active = 0 WHERE active = 1 AND last_seen <> ?", (seen_at,)
        )
        return int(cur.rowcount or 0)

    def get_product(self, product_id: int) -> Optional[dict[str, Any]]:
        with self._using() as c:
            row = c.execute(
                """
                SELECT p.*, v.name AS vendor_name, v.logo AS vendor_logo,
                       CASE WHEN w.product_id IS NULL THEN 0 ELSE 1 END AS watched
                FROM products p
                JOIN vendors v ON v.key = p.vendor_key
                LEFT JOIN watchlist w ON w.product_id = p.id
                WHERE p.id = ?
                """,
                (product_id,),
            ).fetchone()
        return dict(row) if row else None

    def query_products(
        self,
        status: str = "all",
        vendor: Optional[str] = None,
        region: Optional[str] = None,
        q: Optional[str] = None,
        min_price: Optional[float] = None,
        max_price: Optional[float] = None,
        watch_only: bool = False,
        sort: str = "default",
        limit: int = 500,
        offset: int = 0,
    ) -> tuple[list[dict[str, Any]], int]:
        where = ["p.active = 1"]
        params: list[Any] = []

        if status == "in":
            where.append("p.status = 1")
        elif status == "out":
            where.append("p.status = 0")
        if vendor:
            where.append("p.vendor_key = ?")
            params.append(vendor)
        if region:
            where.append("p.region_cn = ?")
            params.append(region)
        if q:
            like = f"%{q}%"
            where.append(
                "(p.name LIKE ? OR p.series_name LIKE ? OR v.name LIKE ? "
                "OR p.region_cn LIKE ? OR p.route LIKE ? OR p.promo LIKE ?)"
            )
            params.extend([like] * 6)
        if min_price is not None:
            where.append("p.price_usd >= ?")
            params.append(min_price)
        if max_price is not None:
            where.append("p.price_usd <= ?")
            params.append(max_price)
        if watch_only:
            where.append("w.product_id IS NOT NULL")

        where_sql = " AND ".join(where)
        order_sql = SORT_SQL.get(sort, SORT_SQL["default"])
        limit = max(1, min(int(limit), 2000))
        offset = max(0, int(offset))

        with self._using() as c:
            total = c.execute(
                f"""
                SELECT COUNT(*) AS n
                FROM products p
                JOIN vendors v ON v.key = p.vendor_key
                LEFT JOIN watchlist w ON w.product_id = p.id
                WHERE {where_sql}
                """,
                params,
            ).fetchone()["n"]
            rows = c.execute(
                f"""
                SELECT p.*, v.name AS vendor_name, v.logo AS vendor_logo,
                       CASE WHEN w.product_id IS NULL THEN 0 ELSE 1 END AS watched
                FROM products p
                JOIN vendors v ON v.key = p.vendor_key
                LEFT JOIN watchlist w ON w.product_id = p.id
                WHERE {where_sql}
                ORDER BY {order_sql}
                LIMIT ? OFFSET ?
                """,
                [*params, limit, offset],
            ).fetchall()
        return [dict(r) for r in rows], int(total)

    def counts(self) -> dict[str, int]:
        with self._using() as c:
            row = c.execute(
                """
                SELECT COUNT(*) AS total,
                       COALESCE(SUM(status = 1), 0) AS ok,
                       COALESCE(SUM(status = 0), 0) AS oos
                FROM products WHERE active = 1
                """
            ).fetchone()
            watched = c.execute("SELECT COUNT(*) AS n FROM watchlist").fetchone()["n"]
            since = (datetime.now(timezone.utc) - timedelta(hours=24)).isoformat(timespec="seconds")
            events_24h = c.execute(
                "SELECT COUNT(*) AS n FROM events WHERE created_at >= ?", (since,)
            ).fetchone()["n"]
            alerts_24h = c.execute(
                "SELECT COUNT(*) AS n FROM alerts WHERE sent_at >= ?", (since,)
            ).fetchone()["n"]
        return {
            "total": int(row["total"]),
            "ok": int(row["ok"]),
            "oos": int(row["oos"]),
            "watched": int(watched),
            "events_24h": int(events_24h),
            "alerts_24h": int(alerts_24h),
        }

    def list_vendors(self) -> list[dict[str, Any]]:
        with self._using() as c:
            rows = c.execute(
                """
                SELECT v.key, v.name, v.logo,
                       COUNT(p.id) AS total,
                       COALESCE(SUM(CASE WHEN p.status = 1 THEN 1 ELSE 0 END), 0) AS ok
                FROM vendors v
                LEFT JOIN products p ON p.vendor_key = v.key AND p.active = 1
                GROUP BY v.key
                HAVING total > 0
                ORDER BY total DESC, v.name ASC
                """
            ).fetchall()
        return [dict(r) for r in rows]

    def list_regions(self) -> list[dict[str, Any]]:
        with self._using() as c:
            rows = c.execute(
                """
                SELECT region_cn,
                       COUNT(*) AS total,
                       COALESCE(SUM(status = 1), 0) AS ok
                FROM products
                WHERE active = 1 AND region_cn <> ''
                GROUP BY region_cn
                ORDER BY total DESC
                """
            ).fetchall()
        return [dict(r) for r in rows]

    # ---------- events / polls / source history ----------

    def insert_event(
        self,
        conn: sqlite3.Connection,
        product_id: int,
        event_type: str,
        old_status: Optional[int],
        new_status: Optional[int],
        old_price_usd: Optional[float],
        new_price_usd: Optional[float],
        created_at: str,
    ) -> int:
        cur = conn.execute(
            """
            INSERT INTO events(product_id, type, old_status, new_status,
                               old_price_usd, new_price_usd, created_at)
            VALUES(?,?,?,?,?,?,?)
            """,
            (product_id, event_type, old_status, new_status, old_price_usd, new_price_usd, created_at),
        )
        return int(cur.lastrowid)

    def list_events(self, limit: int = 100, event_type: Optional[str] = None) -> list[dict[str, Any]]:
        where = ""
        params: list[Any] = []
        if event_type:
            where = "WHERE e.type = ?"
            params.append(event_type)
        params.append(max(1, min(int(limit), 1000)))
        with self._using() as c:
            rows = c.execute(
                f"""
                SELECT e.*, p.name AS product_name, p.vendor_key, p.series_name,
                       p.price AS current_price, p.price_usd AS current_price_usd,
                       p.buy_url, p.region_cn, v.name AS vendor_name
                FROM events e
                JOIN products p ON p.id = e.product_id
                JOIN vendors v ON v.key = p.vendor_key
                {where}
                ORDER BY e.created_at DESC, e.id DESC
                LIMIT ?
                """,
                params,
            ).fetchall()
        return [dict(r) for r in rows]

    def record_poll(
        self,
        fetched_at: str,
        http_status: Optional[int],
        changed: bool,
        total: Optional[int],
        ok: Optional[int],
        oos: Optional[int],
        duration_ms: int,
        etag: Optional[str],
        last_modified: Optional[str],
        error: Optional[str],
        conn: Optional[sqlite3.Connection] = None,
    ) -> None:
        with self._using(conn) as c:
            c.execute(
                """
                INSERT INTO polls(fetched_at, http_status, changed, total, ok, oos,
                                  duration_ms, etag, last_modified, error)
                VALUES(?,?,?,?,?,?,?,?,?,?)
                """,
                (
                    fetched_at,
                    http_status,
                    1 if changed else 0,
                    total,
                    ok,
                    oos,
                    duration_ms,
                    etag,
                    last_modified,
                    error,
                ),
            )

    def list_polls(self, limit: int = 50) -> list[dict[str, Any]]:
        with self._using() as c:
            rows = c.execute(
                "SELECT * FROM polls ORDER BY id DESC LIMIT ?",
                (max(1, min(int(limit), 500)),),
            ).fetchall()
        return [dict(r) for r in rows]

    def latest_poll(self) -> Optional[dict[str, Any]]:
        with self._using() as c:
            row = c.execute("SELECT * FROM polls ORDER BY id DESC LIMIT 1").fetchone()
        return dict(row) if row else None

    def upsert_source_history(self, conn: sqlite3.Connection, items: list[dict[str, Any]]) -> None:
        for item in items:
            conn.execute(
                """
                INSERT OR IGNORE INTO source_history(
                    time_text, occurred_at, name, price, buy_url, fingerprint
                ) VALUES(?,?,?,?,?,?)
                """,
                (
                    item["time_text"],
                    item.get("occurred_at"),
                    item["name"],
                    item.get("price", ""),
                    item.get("buy_url", ""),
                    item["fingerprint"],
                ),
            )

    def list_source_history(self, limit: int = 30) -> list[dict[str, Any]]:
        with self._using() as c:
            rows = c.execute(
                """
                SELECT * FROM source_history
                ORDER BY COALESCE(occurred_at, '') DESC, id DESC
                LIMIT ?
                """,
                (max(1, min(int(limit), 200)),),
            ).fetchall()
        return [dict(r) for r in rows]

    # ---------- rules ----------

    def _rule_dict(self, row: sqlite3.Row) -> dict[str, Any]:
        d = dict(row)
        d["enabled"] = bool(d["enabled"])
        d["only_watchlist"] = bool(d["only_watchlist"])
        return d

    def list_rules(self, enabled_only: bool = False) -> list[dict[str, Any]]:
        sql = "SELECT * FROM rules"
        if enabled_only:
            sql += " WHERE enabled = 1"
        sql += " ORDER BY id ASC"
        with self._using() as c:
            rows = c.execute(sql).fetchall()
        return [self._rule_dict(r) for r in rows]

    def get_rule(self, rule_id: int) -> Optional[dict[str, Any]]:
        with self._using() as c:
            row = c.execute("SELECT * FROM rules WHERE id = ?", (rule_id,)).fetchone()
        return self._rule_dict(row) if row else None

    def create_rule(self, data: dict[str, Any]) -> dict[str, Any]:
        now = utcnow_iso()
        with self._using() as c:
            cur = c.execute(
                """
                INSERT INTO rules(name, enabled, event_types, vendor_key, region_like,
                                  name_like, route_like, min_price_usd, max_price_usd,
                                  only_watchlist, channel, cooldown_minutes, created_at)
                VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)
                """,
                (
                    data["name"],
                    1 if data.get("enabled", True) else 0,
                    data.get("event_types") or "restock",
                    data.get("vendor_key") or None,
                    data.get("region_like") or None,
                    data.get("name_like") or None,
                    data.get("route_like") or None,
                    data.get("min_price_usd"),
                    data.get("max_price_usd"),
                    1 if data.get("only_watchlist") else 0,
                    data.get("channel") or "local",
                    int(data.get("cooldown_minutes", 30)),
                    now,
                ),
            )
            rule_id = int(cur.lastrowid)
        return self.get_rule(rule_id)  # type: ignore[return-value]

    def update_rule(self, rule_id: int, data: dict[str, Any]) -> Optional[dict[str, Any]]:
        if not self.get_rule(rule_id):
            return None
        with self._using() as c:
            c.execute(
                """
                UPDATE rules SET name = ?, enabled = ?, event_types = ?, vendor_key = ?,
                    region_like = ?, name_like = ?, route_like = ?, min_price_usd = ?,
                    max_price_usd = ?, only_watchlist = ?, channel = ?, cooldown_minutes = ?
                WHERE id = ?
                """,
                (
                    data["name"],
                    1 if data.get("enabled", True) else 0,
                    data.get("event_types") or "restock",
                    data.get("vendor_key") or None,
                    data.get("region_like") or None,
                    data.get("name_like") or None,
                    data.get("route_like") or None,
                    data.get("min_price_usd"),
                    data.get("max_price_usd"),
                    1 if data.get("only_watchlist") else 0,
                    data.get("channel") or "local",
                    int(data.get("cooldown_minutes", 30)),
                    rule_id,
                ),
            )
        return self.get_rule(rule_id)

    def delete_rule(self, rule_id: int) -> bool:
        with self._using() as c:
            cur = c.execute("DELETE FROM rules WHERE id = ?", (rule_id,))
        return bool(cur.rowcount)

    # ---------- watchlist ----------

    def is_watched(self, product_id: int) -> bool:
        with self._using() as c:
            row = c.execute(
                "SELECT 1 FROM watchlist WHERE product_id = ?", (product_id,)
            ).fetchone()
        return bool(row)

    def toggle_watch(self, product_id: int) -> Optional[bool]:
        with self._using() as c:
            if not c.execute("SELECT 1 FROM products WHERE id = ?", (product_id,)).fetchone():
                return None
            row = c.execute(
                "SELECT 1 FROM watchlist WHERE product_id = ?", (product_id,)
            ).fetchone()
            if row:
                c.execute("DELETE FROM watchlist WHERE product_id = ?", (product_id,))
                return False
            c.execute(
                "INSERT INTO watchlist(product_id, created_at) VALUES(?, ?)",
                (product_id, utcnow_iso()),
            )
            return True

    # ---------- alerts ----------

    def insert_alert(
        self,
        event_id: int,
        rule_id: int,
        product_id: int,
        channel: str,
        message: str,
        status: str,
        error: Optional[str] = None,
    ) -> None:
        with self._using() as c:
            c.execute(
                """
                INSERT INTO alerts(event_id, rule_id, product_id, channel, message,
                                   sent_at, status, error)
                VALUES(?,?,?,?,?,?,?,?)
                """,
                (event_id, rule_id, product_id, channel, message, utcnow_iso(), status, error),
            )

    def recent_alert_exists(self, rule_id: int, product_id: int, since_iso: str) -> bool:
        with self._using() as c:
            row = c.execute(
                """
                SELECT 1 FROM alerts
                WHERE rule_id = ? AND product_id = ? AND sent_at >= ?
                LIMIT 1
                """,
                (rule_id, product_id, since_iso),
            ).fetchone()
        return bool(row)

    def list_alerts(self, limit: int = 100) -> list[dict[str, Any]]:
        with self._using() as c:
            rows = c.execute(
                """
                SELECT a.*, r.name AS rule_name, p.name AS product_name,
                       p.vendor_key, p.price, p.buy_url, v.name AS vendor_name,
                       p.region_cn
                FROM alerts a
                JOIN products p ON p.id = a.product_id
                JOIN vendors v ON v.key = p.vendor_key
                LEFT JOIN rules r ON r.id = a.rule_id
                ORDER BY a.sent_at DESC, a.id DESC
                LIMIT ?
                """,
                (max(1, min(int(limit), 500)),),
            ).fetchall()
        return [dict(r) for r in rows]


def json_dumps(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False)
