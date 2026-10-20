from __future__ import annotations

import hashlib
import json
import re
import time
from datetime import datetime, timedelta, timezone
from typing import Any, Optional

import requests

DATA_PREFIX_RE = re.compile(r"^\s*window\.DATA\s*=\s*(\{.*\})\s*;?\s*$", re.S)


class FetchError(Exception):
    """网络或 HTTP 层失败。"""


class ParseError(Exception):
    """data.js 内容无法解析或结构异常。"""


def parse_payload(text: str) -> dict[str, Any]:
    text = text.lstrip("\ufeff").strip()
    if not text:
        raise ParseError("响应内容为空")

    candidate: Optional[str] = None
    match = DATA_PREFIX_RE.match(text)
    if match:
        candidate = match.group(1)

    if candidate is None:
        try:
            data = json.loads(text)
            if isinstance(data, dict):
                return data
        except json.JSONDecodeError:
            pass
        start = text.find("{")
        if start < 0:
            raise ParseError("未找到 window.DATA JSON 对象")
        try:
            data, _ = json.JSONDecoder().raw_decode(text[start:])
        except json.JSONDecodeError as exc:
            raise ParseError(f"JSON 解析失败：{exc}") from exc
        if not isinstance(data, dict) or "vendors" not in data:
            raise ParseError("数据结构异常：缺少 vendors 字段")
        return data

    try:
        data = json.loads(candidate)
    except json.JSONDecodeError as exc:
        raise ParseError(f"JSON 解析失败：{exc}") from exc
    if not isinstance(data, dict) or "vendors" not in data:
        raise ParseError("数据结构异常：缺少 vendors 字段")
    return data


def _text(value: Any) -> str:
    if value is None:
        return ""
    return str(value)


def _price_usd(value: Any) -> Optional[float]:
    if value is None or value == "":
        return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def _status(value: Any) -> int:
    if value is True or value == 1:
        return 1
    if isinstance(value, str) and value.strip().lower() in {"1", "true", "yes", "in", "ok"}:
        return 1
    return 0


def _beijing_to_utc(time_text: str) -> Optional[str]:
    try:
        dt = datetime.strptime(time_text.strip(), "%Y-%m-%d %H:%M")
    except (ValueError, AttributeError):
        return None
    return dt.replace(tzinfo=timezone(timedelta(hours=8))).astimezone(timezone.utc).isoformat(
        timespec="seconds"
    )


def normalize(data: dict[str, Any]) -> dict[str, Any]:
    """把源数据整理成 vendors / series / products / history 四组。"""
    vendors: list[dict[str, Any]] = []
    series_list: list[dict[str, Any]] = []
    products: dict[tuple[str, str, str], dict[str, Any]] = {}

    for vendor in data.get("vendors") or []:
        if not isinstance(vendor, dict):
            continue
        vendor_key = _text(vendor.get("key")).strip()
        if not vendor_key or vendor_key == "all":
            continue
        vendor_name = _text(vendor.get("name")).strip() or vendor_key
        vendors.append(
            {
                "key": vendor_key,
                "name": vendor_name,
                "logo": _text(vendor.get("logo")),
            }
        )

        for series in vendor.get("series") or []:
            if not isinstance(series, dict):
                continue
            series_key = (_text(series.get("key")).strip() or _text(series.get("name")).strip())
            series_name = (_text(series.get("name")).strip() or series_key)
            if not series_key:
                series_key = "默认"
            series_region = _text(series.get("region")).strip()
            series_list.append(
                {
                    "vendor_key": vendor_key,
                    "key": series_key,
                    "name": series_name,
                    "region": series_region,
                }
            )

            for product in series.get("products") or []:
                if not isinstance(product, dict):
                    continue
                name = _text(product.get("name")).strip()
                if not name:
                    continue
                region = _text(product.get("region")).strip() or series_region
                region_cn = _text(product.get("region_cn")).strip() or region
                spec = product.get("spec")
                if not isinstance(spec, dict):
                    spec = {}
                item = {
                    "vendor_key": vendor_key,
                    "series_key": series_key,
                    "series_name": series_name,
                    "name": name,
                    "price": _text(product.get("price")),
                    "price_usd": _price_usd(product.get("price_usd")),
                    "region": region,
                    "region_cn": region_cn,
                    "region_flag": _text(product.get("region_flag")),
                    "unit": _text(product.get("unit")),
                    "spec_json": json.dumps(spec, ensure_ascii=False),
                    "route": _text(product.get("route")),
                    "ip_attr": _text(product.get("ip_attr")),
                    "promo": _text(product.get("promo")),
                    "promo_exp": _text(product.get("promo_exp")),
                    "buy_url": _text(product.get("buy_url")),
                    "buy_mode": _text(product.get("buy_mode")),
                    "blog_url": _text(product.get("blog_url")),
                    "order_text": _text(product.get("order")),
                    "status": _status(product.get("status")),
                }
                products[(vendor_key, series_key, name)] = item

    history: list[dict[str, Any]] = []
    for item in data.get("history") or []:
        if not isinstance(item, dict):
            continue
        name = _text(item.get("name")).strip()
        time_text = _text(item.get("time")).strip()
        if not name or not time_text:
            continue
        price = _text(item.get("price"))
        buy_url = _text(item.get("buy_url"))
        fingerprint = hashlib.sha1(
            "|".join([time_text, name, price, buy_url]).encode("utf-8")
        ).hexdigest()
        history.append(
            {
                "time_text": time_text,
                "occurred_at": _beijing_to_utc(time_text),
                "name": name,
                "price": price,
                "buy_url": buy_url,
                "fingerprint": fingerprint,
            }
        )

    return {
        "vendors": vendors,
        "series": series_list,
        "products": list(products.values()),
        "history": history,
    }


def fetch_source(
    cfg: dict[str, Any], etag: Optional[str], last_modified: Optional[str]
) -> dict[str, Any]:
    """拉取 data.js，支持 ETag / Last-Modified 条件请求。"""
    request_cfg = cfg.get("request") or {}
    timeout = float(request_cfg.get("timeout_seconds", 30))
    headers = {
        "User-Agent": request_cfg.get("user_agent") or "vps-loading-local/0.1",
        "Accept": "application/javascript, text/javascript, */*;q=0.8",
    }
    if etag:
        headers["If-None-Match"] = etag
    if last_modified:
        headers["If-Modified-Since"] = last_modified

    last_error: Optional[Exception] = None
    response: Optional[requests.Response] = None
    for attempt in range(3):
        try:
            response = requests.get(cfg["source_url"], headers=headers, timeout=timeout)
        except requests.RequestException as exc:
            last_error = exc
            time.sleep(2 * (attempt + 1))
            continue

        if response.status_code in (429, 500, 502, 503, 504) and attempt < 2:
            time.sleep(3 * (attempt + 1))
            continue
        break

    if response is None:
        raise FetchError(f"请求失败：{last_error}")
    if response.status_code == 304:
        return {
            "not_modified": True,
            "http_status": 304,
            "text": None,
            "etag": response.headers.get("ETag") or etag,
            "last_modified": response.headers.get("Last-Modified") or last_modified,
        }
    if response.status_code != 200:
        snippet = response.text[:200].replace("\n", " ")
        raise FetchError(f"HTTP {response.status_code}：{snippet}")

    text = response.content.decode("utf-8-sig", errors="replace")
    return {
        "not_modified": False,
        "http_status": 200,
        "text": text,
        "etag": response.headers.get("ETag"),
        "last_modified": response.headers.get("Last-Modified"),
    }
