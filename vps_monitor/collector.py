from __future__ import annotations

import hashlib
import logging
import threading
import time
from typing import Any, Optional

from . import alerts
from .db import Database, utcnow_iso
from .source import FetchError, ParseError, fetch_source, normalize, parse_payload

log = logging.getLogger(__name__)


def _price_change_type(old_price: Optional[float], new_price: Optional[float]) -> Optional[str]:
    if old_price is None or new_price is None:
        return None
    if abs(float(old_price) - float(new_price)) < 0.005:
        return None
    return "price_down" if float(new_price) < float(old_price) else "price_up"


class Collector:
    def __init__(self, cfg: dict[str, Any], db: Database):
        self.cfg = cfg
        self.db = db
        self.lock = threading.Lock()
        self.last_result: Optional[dict[str, Any]] = None

    def collect_once(self, force: bool = False) -> dict[str, Any]:
        if not self.lock.acquire(blocking=False):
            return {"ok": False, "skipped": True, "error": "已有采集任务在进行中"}
        try:
            result = self._collect(force=force)
        except Exception as exc:  # noqa: BLE001 - 采集失败需要落库而不是中断循环
            log.exception("采集出现未预期异常")
            counts = self.db.counts()
            self.db.record_poll(
                fetched_at=utcnow_iso(),
                http_status=None,
                changed=False,
                total=counts["total"],
                ok=counts["ok"],
                oos=counts["oos"],
                duration_ms=0,
                etag=None,
                last_modified=None,
                error=f"{type(exc).__name__}: {exc}",
            )
            self.db.meta_set("last_error", f"{type(exc).__name__}: {exc}")
            result = {"ok": False, "error": f"{type(exc).__name__}: {exc}"}
        finally:
            self.lock.release()

        self.last_result = result
        return result

    def _collect(self, force: bool = False) -> dict[str, Any]:
        started = time.monotonic()
        fetched_at = utcnow_iso()
        etag = None if force else self.db.meta_get("etag")
        last_modified = None if force else self.db.meta_get("last_modified")

        try:
            response = fetch_source(self.cfg, etag, last_modified)
        except FetchError as exc:
            duration = int((time.monotonic() - started) * 1000)
            counts = self.db.counts()
            self.db.record_poll(
                fetched_at=fetched_at,
                http_status=None,
                changed=False,
                total=counts["total"],
                ok=counts["ok"],
                oos=counts["oos"],
                duration_ms=duration,
                etag=etag,
                last_modified=last_modified,
                error=str(exc),
            )
            self.db.meta_set("last_checked", fetched_at)
            self.db.meta_set("last_error", str(exc))
            log.warning("采集失败：%s", exc)
            return {"ok": False, "error": str(exc), "fetched_at": fetched_at, "duration_ms": duration}

        if response["not_modified"]:
            duration = int((time.monotonic() - started) * 1000)
            counts = self.db.counts()
            self.db.record_poll(
                fetched_at=fetched_at,
                http_status=304,
                changed=False,
                total=counts["total"],
                ok=counts["ok"],
                oos=counts["oos"],
                duration_ms=duration,
                etag=response.get("etag") or etag,
                last_modified=response.get("last_modified") or last_modified,
                error=None,
            )
            self.db.meta_set("last_checked", fetched_at)
            self.db.meta_set("last_error", "")
            if response.get("etag"):
                self.db.meta_set("etag", response["etag"])
            if response.get("last_modified"):
                self.db.meta_set("last_modified", response["last_modified"])
            log.debug("数据未变化（304）")
            return {
                "ok": True,
                "not_modified": True,
                "changed": False,
                "http_status": 304,
                "fetched_at": fetched_at,
                "duration_ms": duration,
                "total": counts["total"],
                "ok_count": counts["ok"],
                "oos_count": counts["oos"],
                "events": [],
                "alerts": [],
            }

        text = response["text"] or ""
        content_hash = hashlib.sha256(text.encode("utf-8")).hexdigest()
        if not force and content_hash == self.db.meta_get("content_hash"):
            duration = int((time.monotonic() - started) * 1000)
            counts = self.db.counts()
            self.db.record_poll(
                fetched_at=fetched_at,
                http_status=200,
                changed=False,
                total=counts["total"],
                ok=counts["ok"],
                oos=counts["oos"],
                duration_ms=duration,
                etag=response.get("etag") or etag,
                last_modified=response.get("last_modified") or last_modified,
                error=None,
            )
            self.db.meta_set("last_checked", fetched_at)
            self.db.meta_set("last_error", "")
            if response.get("etag"):
                self.db.meta_set("etag", response["etag"])
            if response.get("last_modified"):
                self.db.meta_set("last_modified", response["last_modified"])
            log.debug("内容哈希未变化，跳过 diff")
            return {
                "ok": True,
                "not_modified": False,
                "changed": False,
                "http_status": 200,
                "fetched_at": fetched_at,
                "duration_ms": duration,
                "total": counts["total"],
                "ok_count": counts["ok"],
                "oos_count": counts["oos"],
                "events": [],
                "alerts": [],
            }

        try:
            payload = parse_payload(text)
            snapshot = normalize(payload)
        except ParseError as exc:
            duration = int((time.monotonic() - started) * 1000)
            counts = self.db.counts()
            self.db.record_poll(
                fetched_at=fetched_at,
                http_status=200,
                changed=False,
                total=counts["total"],
                ok=counts["ok"],
                oos=counts["oos"],
                duration_ms=duration,
                etag=etag,
                last_modified=last_modified,
                error=str(exc),
            )
            self.db.meta_set("last_checked", fetched_at)
            self.db.meta_set("last_error", str(exc))
            log.error("解析失败：%s", exc)
            return {"ok": False, "error": str(exc), "fetched_at": fetched_at, "duration_ms": duration}

        products = snapshot["products"]
        if not products:
            message = "解析结果为空，已拒绝本次更新（避免误判全部下架）"
            duration = int((time.monotonic() - started) * 1000)
            counts = self.db.counts()
            self.db.record_poll(
                fetched_at=fetched_at,
                http_status=200,
                changed=False,
                total=counts["total"],
                ok=counts["ok"],
                oos=counts["oos"],
                duration_ms=duration,
                etag=etag,
                last_modified=last_modified,
                error=message,
            )
            self.db.meta_set("last_checked", fetched_at)
            self.db.meta_set("last_error", message)
            return {"ok": False, "error": message, "fetched_at": fetched_at, "duration_ms": duration}

        previous_count = self.db.product_count()
        if previous_count > 0 and len(products) < previous_count * 0.5:
            message = (
                f"套餐数量异常下降（{previous_count} -> {len(products)}），"
                "已拒绝本次更新（避免误判全部下架）"
            )
            duration = int((time.monotonic() - started) * 1000)
            counts = self.db.counts()
            self.db.record_poll(
                fetched_at=fetched_at,
                http_status=200,
                changed=False,
                total=counts["total"],
                ok=counts["ok"],
                oos=counts["oos"],
                duration_ms=duration,
                etag=etag,
                last_modified=last_modified,
                error=message,
            )
            self.db.meta_set("last_checked", fetched_at)
            self.db.meta_set("last_error", message)
            log.error("%s", message)
            return {"ok": False, "error": message, "fetched_at": fetched_at, "duration_ms": duration}

        is_first = self.db.meta_get("initialized") != "1" and previous_count == 0
        events: list[dict[str, Any]] = []
        added = 0

        with self.db.transaction() as conn:
            for vendor in snapshot["vendors"]:
                self.db.upsert_vendor(conn, vendor["key"], vendor["name"], vendor["logo"])
            for series in snapshot["series"]:
                self.db.upsert_series(
                    conn,
                    series["vendor_key"],
                    series["key"],
                    series["name"],
                    series["region"],
                )

            existing = self.db.products_map(conn)
            for product in products:
                key = (product["vendor_key"], product["series_key"], product["name"])
                old = existing.get(key)
                if old is None:
                    product_id = self.db.insert_product(conn, product, fetched_at)
                    added += 1
                    if not is_first:
                        event_id = self.db.insert_event(
                            conn,
                            product_id,
                            "new_product",
                            None,
                            product["status"],
                            None,
                            product["price_usd"],
                            fetched_at,
                        )
                        events.append(
                            {
                                "id": event_id,
                                "product_id": product_id,
                                "type": "new_product",
                                "created_at": fetched_at,
                            }
                        )
                    continue

                product_id = int(old["id"])
                old_status = int(old["status"])
                new_status = int(product["status"])
                old_price = old["price_usd"]
                new_price = product["price_usd"]
                changed = False

                if old_status != new_status:
                    event_type = "restock" if new_status == 1 else "soldout"
                    event_id = self.db.insert_event(
                        conn,
                        product_id,
                        event_type,
                        old_status,
                        new_status,
                        old_price,
                        new_price,
                        fetched_at,
                    )
                    events.append(
                        {
                            "id": event_id,
                            "product_id": product_id,
                            "type": event_type,
                            "created_at": fetched_at,
                        }
                    )
                    changed = True

                price_type = _price_change_type(old_price, new_price)
                if price_type:
                    event_id = self.db.insert_event(
                        conn,
                        product_id,
                        price_type,
                        old_status,
                        new_status,
                        old_price,
                        new_price,
                        fetched_at,
                    )
                    events.append(
                        {
                            "id": event_id,
                            "product_id": product_id,
                            "type": price_type,
                            "created_at": fetched_at,
                        }
                    )
                    changed = True

                self.db.update_product(conn, product_id, product, fetched_at)
                if changed:
                    self.db.set_last_change(conn, product_id, fetched_at)

            removed = self.db.mark_inactive(conn, fetched_at)
            self.db.upsert_source_history(conn, snapshot["history"])

            counts_row = conn.execute(
                """
                SELECT COUNT(*) AS total,
                       COALESCE(SUM(status = 1), 0) AS ok,
                       COALESCE(SUM(status = 0), 0) AS oos
                FROM products WHERE active = 1
                """
            ).fetchone()
            total = int(counts_row["total"])
            ok_count = int(counts_row["ok"])
            oos_count = int(counts_row["oos"])
            duration = int((time.monotonic() - started) * 1000)
            changed_any = bool(events) or added > 0 or removed > 0

            self.db.record_poll(
                fetched_at=fetched_at,
                http_status=200,
                changed=changed_any,
                total=total,
                ok=ok_count,
                oos=oos_count,
                duration_ms=duration,
                etag=response.get("etag"),
                last_modified=response.get("last_modified"),
                error=None,
                conn=conn,
            )
            self.db.meta_set("initialized", "1", conn)
            self.db.meta_set("last_checked", fetched_at, conn)
            self.db.meta_set("last_success", fetched_at, conn)
            self.db.meta_set("last_error", "", conn)
            self.db.meta_set("content_hash", content_hash, conn)
            if response.get("etag"):
                self.db.meta_set("etag", response["etag"], conn)
            if response.get("last_modified"):
                self.db.meta_set("last_modified", response["last_modified"], conn)
            if changed_any:
                self.db.meta_set("last_data_change", fetched_at, conn)

        alert_results = alerts.process_events(self.cfg, self.db, events)
        if changed_any:
            log.info(
                "采集完成：%s 款，有货 %s，售罄 %s，新增 %s，下线 %s，事件 %s，告警 %s",
                total,
                ok_count,
                oos_count,
                added,
                removed,
                len(events),
                len(alert_results),
            )

        return {
            "ok": True,
            "not_modified": False,
            "changed": changed_any,
            "http_status": 200,
            "fetched_at": fetched_at,
            "duration_ms": duration,
            "total": total,
            "ok_count": ok_count,
            "oos_count": oos_count,
            "added": added,
            "removed": removed,
            "events": events,
            "alerts": alert_results,
            "source_last_modified": response.get("last_modified"),
        }


def run_loop(collector: Collector, interval_seconds: int, stop_event: threading.Event) -> None:
    interval = max(60, int(interval_seconds))
    while not stop_event.is_set():
        try:
            collector.collect_once()
        except Exception:  # noqa: BLE001 - 循环不能因单次异常退出
            log.exception("采集循环异常")
        stop_event.wait(interval)
