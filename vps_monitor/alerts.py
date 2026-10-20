from __future__ import annotations

import logging
from datetime import datetime, timedelta, timezone
from typing import Any, Optional

import requests

log = logging.getLogger(__name__)

EVENT_LABELS = {
    "restock": ("🟢", "补货"),
    "soldout": ("🔴", "售罄"),
    "price_down": ("💰", "降价"),
    "price_up": ("⬆️", "涨价"),
    "new_product": ("🆕", "新品"),
}

VALID_CHANNELS = {"local", "telegram", "webhook"}


def _contains(haystack: Any, needle: str) -> bool:
    return needle.casefold() in str(haystack or "").casefold()


def rule_matches(rule: dict[str, Any], product: dict[str, Any]) -> bool:
    if rule.get("vendor_key") and rule["vendor_key"] != product.get("vendor_key"):
        return False
    if rule.get("region_like"):
        region_text = f"{product.get('region_cn', '')} {product.get('region', '')}"
        if not _contains(region_text, rule["region_like"]):
            return False
    if rule.get("name_like"):
        name_text = " ".join(
            [
                str(product.get("vendor_name") or ""),
                str(product.get("series_name") or ""),
                str(product.get("name") or ""),
            ]
        )
        if not _contains(name_text, rule["name_like"]):
            return False
    if rule.get("route_like") and not _contains(product.get("route"), rule["route_like"]):
        return False

    price = product.get("price_usd")
    if rule.get("min_price_usd") is not None:
        if price is None or float(price) < float(rule["min_price_usd"]):
            return False
    if rule.get("max_price_usd") is not None:
        if price is None or float(price) > float(rule["max_price_usd"]):
            return False
    return True


def build_message(event: dict[str, Any], product: dict[str, Any]) -> str:
    event_type = event.get("type", "")
    icon, label = EVENT_LABELS.get(event_type, ("🔔", event_type))

    price_text = str(product.get("price") or "")
    unit = str(product.get("unit") or "")
    if unit and not price_text.endswith(unit):
        price_text = f"{price_text}/{unit}" if price_text else unit
    price_usd = product.get("price_usd")
    if price_usd is not None and "$" not in price_text:
        price_text = f"{price_text}（约 ${float(price_usd):.2f}）"

    region = str(product.get("region_cn") or product.get("region") or "")
    series = str(product.get("series_name") or "")
    meta_parts = [region] if region else []
    if series and series != region:
        meta_parts.append(series)
    meta = " · ".join(meta_parts)

    lines = [
        f"{icon} {label}提醒 · {product.get('vendor_name') or product.get('vendor_key')}",
        product.get("name") or "",
    ]
    if meta:
        lines.append(meta)
    if price_text:
        lines.append(f"价格：{price_text}")
    if product.get("route"):
        lines.append(f"线路：{product['route']}")
    if product.get("promo"):
        lines.append(f"优惠码：{product['promo']}")
    if product.get("buy_url"):
        lines.append(f"购买：{product['buy_url']}")
    return "\n".join(line for line in lines if line)


def _mask_secrets(text: str, secrets: list[str]) -> str:
    for secret in secrets:
        if secret:
            text = text.replace(secret, "***")
    return text


def _send_telegram(cfg: dict[str, Any], message: str) -> tuple[str, Optional[str]]:
    telegram = cfg.get("telegram") or {}
    token = (telegram.get("bot_token") or "").strip()
    chat_id = (telegram.get("chat_id") or "").strip()
    if not token or not chat_id:
        return "failed", "Telegram 未配置 bot_token / chat_id"

    url = f"https://api.telegram.org/bot{token}/sendMessage"
    try:
        resp = requests.post(
            url,
            json={"chat_id": chat_id, "text": message, "disable_web_page_preview": True},
            timeout=15,
        )
    except requests.RequestException as exc:
        return "failed", _mask_secrets(f"{type(exc).__name__}: {exc}", [token])

    if resp.status_code == 200:
        try:
            payload = resp.json()
        except ValueError:
            payload = {}
        if payload.get("ok"):
            return "sent", None
    return "failed", _mask_secrets(f"HTTP {resp.status_code}: {resp.text[:200]}", [token])


def _send_webhook(
    cfg: dict[str, Any], message: str, event: dict[str, Any], product: dict[str, Any]
) -> tuple[str, Optional[str]]:
    url = ((cfg.get("webhook") or {}).get("url") or "").strip()
    if not url:
        return "failed", "Webhook 未配置 url"

    payload = {
        "text": message,
        "event": {
            "id": event.get("id"),
            "type": event.get("type"),
            "created_at": event.get("created_at"),
        },
        "product": {
            "id": product.get("id"),
            "vendor": product.get("vendor_name"),
            "name": product.get("name"),
            "region": product.get("region_cn"),
            "price": product.get("price"),
            "price_usd": product.get("price_usd"),
            "status": bool(product.get("status")),
            "buy_url": product.get("buy_url"),
        },
    }
    try:
        resp = requests.post(url, json=payload, timeout=15)
    except requests.RequestException as exc:
        return "failed", f"{type(exc).__name__}: 请求失败"

    if 200 <= resp.status_code < 300:
        return "sent", None
    return "failed", _mask_secrets(f"HTTP {resp.status_code}: {resp.text[:200]}", [url])


def send_alert(
    cfg: dict[str, Any], channel: str, message: str, event: dict[str, Any], product: dict[str, Any]
) -> tuple[str, Optional[str]]:
    if channel == "local":
        return "local", None
    if channel == "telegram":
        return _send_telegram(cfg, message)
    if channel == "webhook":
        return _send_webhook(cfg, message, event, product)
    return "failed", f"未知通道：{channel}"


def process_events(cfg: dict[str, Any], db: Any, events: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """对事件套用启用的规则，命中后发送告警并落库。"""
    if not events:
        return []

    rules = db.list_rules(enabled_only=True)
    if not rules:
        return []

    results: list[dict[str, Any]] = []
    for event in events:
        product = db.get_product(int(event["product_id"]))
        if not product:
            continue

        for rule in rules:
            types = {x.strip() for x in str(rule.get("event_types") or "").split(",") if x.strip()}
            if event["type"] not in types:
                continue
            if not rule_matches(rule, product):
                continue
            if rule.get("only_watchlist") and not product.get("watched"):
                continue

            cooldown = int(rule.get("cooldown_minutes") or 0)
            if cooldown > 0:
                since = (
                    datetime.now(timezone.utc) - timedelta(minutes=cooldown)
                ).isoformat(timespec="seconds")
                if db.recent_alert_exists(int(rule["id"]), int(product["id"]), since):
                    continue

            message = build_message(event, product)
            channel = rule.get("channel") or "local"
            status, error = send_alert(cfg, channel, message, event, product)
            if status == "failed":
                log.warning("告警发送失败 rule=%s product=%s: %s", rule["id"], product["id"], error)

            db.insert_alert(
                event_id=int(event["id"]),
                rule_id=int(rule["id"]),
                product_id=int(product["id"]),
                channel=channel,
                message=message,
                status=status,
                error=error,
            )
            results.append(
                {
                    "rule_id": int(rule["id"]),
                    "product_id": int(product["id"]),
                    "channel": channel,
                    "status": status,
                    "error": error,
                }
            )
    return results
