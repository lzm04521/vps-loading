from __future__ import annotations

import json
import logging
import threading
from contextlib import asynccontextmanager
from typing import Any, AsyncIterator, Optional

from fastapi import FastAPI, HTTPException
from fastapi.responses import FileResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel, Field

from . import __version__
from .collector import Collector, run_loop
from .config import resource_root
from .db import Database

log = logging.getLogger(__name__)

WEB_DIR = resource_root() / "web"

# pythonw/打包 exe 下 sys.stdout 为 None，uvicorn 默认日志配置的
# ColourizedFormatter 会调用 sys.stdout.isatty() 直接崩溃；
# 改为无 handler、上抛 root（控制台/文件日志），run/serve/tray 共用
UVICORN_LOG_CONFIG: dict[str, Any] = {
    "version": 1,
    "disable_existing_loggers": False,
    "handlers": {},
    "loggers": {
        "uvicorn": {"handlers": [], "level": "WARNING", "propagate": True},
        "uvicorn.error": {"handlers": [], "level": "WARNING", "propagate": True},
        "uvicorn.access": {"handlers": [], "level": "CRITICAL", "propagate": False},
    },
}
VALID_EVENTS = {"restock", "soldout", "price_down", "price_up", "new_product"}
VALID_CHANNELS = {"local", "telegram", "webhook"}


class RuleIn(BaseModel):
    name: str = Field(min_length=1, max_length=100)
    enabled: bool = True
    event_types: str = "restock"
    vendor_key: Optional[str] = None
    region_like: Optional[str] = None
    name_like: Optional[str] = None
    route_like: Optional[str] = None
    min_price_usd: Optional[float] = None
    max_price_usd: Optional[float] = None
    only_watchlist: bool = False
    channel: str = "local"
    cooldown_minutes: int = Field(default=30, ge=0, le=1440)


def _product_dict(row: dict[str, Any]) -> dict[str, Any]:
    data = dict(row)
    raw_spec = data.pop("spec_json", "{}")
    try:
        data["spec"] = json.loads(raw_spec or "{}")
    except (TypeError, ValueError):
        data["spec"] = {}
    data["status"] = bool(data.get("status"))
    data["watched"] = bool(data.get("watched"))
    return data


def _validate_rule(model: RuleIn) -> dict[str, Any]:
    event_types = [item.strip() for item in model.event_types.split(",") if item.strip()]
    if not event_types:
        raise HTTPException(status_code=400, detail="至少选择一个事件类型")
    invalid = sorted(set(event_types) - VALID_EVENTS)
    if invalid:
        raise HTTPException(status_code=400, detail=f"未知事件类型：{', '.join(invalid)}")
    if model.channel not in VALID_CHANNELS:
        raise HTTPException(status_code=400, detail=f"未知通道：{model.channel}")
    if (
        model.min_price_usd is not None
        and model.max_price_usd is not None
        and model.min_price_usd > model.max_price_usd
    ):
        raise HTTPException(status_code=400, detail="最低价不能大于最高价")

    data = model.model_dump()
    data["event_types"] = ",".join(event_types)
    return data


def create_app(
    cfg: dict[str, Any], db: Database, collector: Collector, start_collector: bool = True
) -> FastAPI:
    poll_interval = int(cfg.get("poll_interval_seconds", 180))

    @asynccontextmanager
    async def lifespan(app: FastAPI) -> AsyncIterator[None]:
        stop_event = threading.Event()
        thread: Optional[threading.Thread] = None
        if start_collector:
            thread = threading.Thread(
                target=run_loop,
                args=(collector, poll_interval, stop_event),
                name="vps-collector",
                daemon=True,
            )
            thread.start()
            log.info("后台采集线程已启动，间隔 %s 秒", max(60, poll_interval))
        app.state.stop_event = stop_event
        app.state.collector_thread = thread
        try:
            yield
        finally:
            stop_event.set()
            if thread:
                thread.join(timeout=5)

    app = FastAPI(title="VPS 库存监控", version=__version__, lifespan=lifespan)
    app.mount("/static", StaticFiles(directory=WEB_DIR), name="static")

    @app.middleware("http")
    async def no_store(request, call_next):
        response = await call_next(request)
        if request.url.path.startswith("/api"):
            response.headers["Cache-Control"] = "no-store"
        return response

    @app.get("/")
    def index() -> FileResponse:
        return FileResponse(WEB_DIR / "index.html", headers={"Cache-Control": "no-cache"})

    @app.get("/favicon.ico")
    def favicon() -> FileResponse:
        return FileResponse(WEB_DIR / "favicon.svg", media_type="image/svg+xml")

    @app.get("/api/summary")
    def summary() -> dict[str, Any]:
        telegram = cfg.get("telegram") or {}
        webhook = cfg.get("webhook") or {}
        return {
            "counts": db.counts(),
            "last_poll": db.latest_poll(),
            "last_success": db.meta_get("last_success"),
            "last_checked": db.meta_get("last_checked"),
            "last_data_change": db.meta_get("last_data_change"),
            "last_error": db.meta_get("last_error") or "",
            "source_last_modified": db.meta_get("last_modified"),
            "source_url": cfg.get("source_url"),
            "initialized": db.meta_get("initialized") == "1",
            "collector_busy": collector.lock.locked(),
            "collector_last_result": collector.last_result,
            "poll_interval_seconds": poll_interval,
            "telegram_configured": bool(
                (telegram.get("bot_token") or "").strip()
                and (telegram.get("chat_id") or "").strip()
            ),
            "webhook_configured": bool((webhook.get("url") or "").strip()),
            "config_path": cfg.get("_config_path"),
            "db_path": str(db.path),
        }

    @app.get("/api/products")
    def products(
        status: str = "all",
        vendor: Optional[str] = None,
        region: Optional[str] = None,
        q: Optional[str] = None,
        min_price: Optional[float] = None,
        max_price: Optional[float] = None,
        watch: bool = False,
        sort: str = "default",
        limit: int = 500,
        offset: int = 0,
    ) -> dict[str, Any]:
        items, total = db.query_products(
            status=status,
            vendor=vendor,
            region=region,
            q=q,
            min_price=min_price,
            max_price=max_price,
            watch_only=watch,
            sort=sort,
            limit=limit,
            offset=offset,
        )
        return {"total": total, "items": [_product_dict(item) for item in items]}

    @app.get("/api/vendors")
    def vendors() -> dict[str, Any]:
        return {"items": db.list_vendors()}

    @app.get("/api/regions")
    def regions() -> dict[str, Any]:
        return {"items": db.list_regions()}

    @app.get("/api/events")
    def events(limit: int = 100, type: Optional[str] = None) -> dict[str, Any]:
        return {"items": db.list_events(limit=limit, event_type=type)}

    @app.get("/api/source-history")
    def source_history(limit: int = 30) -> dict[str, Any]:
        return {"items": db.list_source_history(limit=limit)}

    @app.get("/api/alerts")
    def alerts(limit: int = 100) -> dict[str, Any]:
        return {"items": db.list_alerts(limit=limit)}

    @app.get("/api/polls")
    def polls(limit: int = 50) -> dict[str, Any]:
        return {"items": db.list_polls(limit=limit)}

    @app.get("/api/rules")
    def list_rules() -> dict[str, Any]:
        return {"items": db.list_rules()}

    @app.post("/api/rules")
    def create_rule(rule: RuleIn) -> dict[str, Any]:
        return db.create_rule(_validate_rule(rule))

    @app.put("/api/rules/{rule_id}")
    def update_rule(rule_id: int, rule: RuleIn) -> dict[str, Any]:
        updated = db.update_rule(rule_id, _validate_rule(rule))
        if not updated:
            raise HTTPException(status_code=404, detail="规则不存在")
        return updated

    @app.delete("/api/rules/{rule_id}")
    def delete_rule(rule_id: int) -> dict[str, Any]:
        if not db.delete_rule(rule_id):
            raise HTTPException(status_code=404, detail="规则不存在")
        return {"ok": True}

    @app.post("/api/watch/{product_id}")
    def toggle_watch(product_id: int) -> dict[str, Any]:
        watched = db.toggle_watch(product_id)
        if watched is None:
            raise HTTPException(status_code=404, detail="套餐不存在")
        return {"ok": True, "watched": watched}

    @app.post("/api/collect")
    def collect_now(force: bool = False) -> dict[str, Any]:
        return collector.collect_once(force=force)

    @app.get("/api/config")
    def config() -> dict[str, Any]:
        telegram = cfg.get("telegram") or {}
        webhook = cfg.get("webhook") or {}
        return {
            "source_url": cfg.get("source_url"),
            "poll_interval_seconds": poll_interval,
            "telegram_configured": bool(
                (telegram.get("bot_token") or "").strip()
                and (telegram.get("chat_id") or "").strip()
            ),
            "webhook_configured": bool((webhook.get("url") or "").strip()),
            "config_path": cfg.get("_config_path"),
            "db_path": str(db.path),
        }

    return app
