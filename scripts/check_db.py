import os
import sqlite3

path = os.path.join(os.environ["APPDATA"], "com.vpsmonitor.app", "monitor.db")
conn = sqlite3.connect(path)
cur = conn.cursor()
checks = [
    ("products(active)", "SELECT COUNT(*), SUM(status=1), SUM(status=0) FROM products WHERE active=1"),
    ("vendors", "SELECT COUNT(*) FROM vendors"),
    ("events", "SELECT COUNT(*) FROM events"),
    ("polls", "SELECT COUNT(*) FROM polls"),
    ("source_history", "SELECT COUNT(*) FROM source_history"),
    ("rules", "SELECT id, name, event_types, channel, enabled FROM rules"),
    ("meta", "SELECT key, substr(value,1,40) FROM meta ORDER BY key"),
    ("recent_polls", "SELECT id, http_status, changed, total, ok, oos, duration_ms, substr(COALESCE(error,''),1,50) FROM polls ORDER BY id DESC LIMIT 3"),
]
for name, sql in checks:
    cur.execute(sql)
    print(f"{name}: {cur.fetchall()}")
conn.close()
