from __future__ import annotations

import copy
import json
import os
import sys
from pathlib import Path
from typing import Any


def _dir_writable(path: Path) -> bool:
    try:
        probe = path / ".write_probe"
        probe.touch()
        probe.unlink()
        return True
    except OSError:
        return False


def _app_root() -> Path:
    """数据/配置根：源码模式=项目根；打包模式=exe 旁（只读则退到 %LOCALAPPDATA%）。"""
    if getattr(sys, "frozen", False):
        exe_dir = Path(sys.executable).resolve().parent
        if _dir_writable(exe_dir):
            return exe_dir
        base = Path(os.environ.get("LOCALAPPDATA") or Path.home()) / "vps-monitor"
        base.mkdir(parents=True, exist_ok=True)
        return base
    return Path(__file__).resolve().parents[1]


ROOT = _app_root()


def resource_root() -> Path:
    """只读资源根（web 静态页等）：打包模式为 PyInstaller 解包目录。"""
    if getattr(sys, "frozen", False):
        return Path(getattr(sys, "_MEIPASS", Path(sys.executable).parent))
    return ROOT

DEFAULT_CONFIG: dict[str, Any] = {
    "source_url": "https://kyoa.de/data.js",
    "poll_interval_seconds": 180,
    "db_path": "data/monitor.db",
    "web": {"host": "127.0.0.1", "port": 8787},
    "request": {
        "timeout_seconds": 30,
        "user_agent": "vps-loading-local/0.1 (personal inventory monitor)",
    },
    "telegram": {"bot_token": "", "chat_id": ""},
    "webhook": {"url": ""},
}


def _deep_merge(base: dict[str, Any], override: dict[str, Any]) -> dict[str, Any]:
    for key, value in override.items():
        if isinstance(value, dict) and isinstance(base.get(key), dict):
            _deep_merge(base[key], value)
        else:
            base[key] = value
    return base


def load_config(config_path: str | Path | None = None) -> dict[str, Any]:
    """加载配置；首次运行时用示例配置生成 config.json。"""
    path = Path(config_path) if config_path else ROOT / "config.json"
    if not path.exists():
        # 打包模式下示例配置在 PyInstaller 解包目录里
        example = next(
            (
                candidate
                for candidate in (
                    ROOT / "config.example.json",
                    resource_root() / "config.example.json",
                )
                if candidate.exists()
            ),
            None,
        )
        seed = (
            json.loads(example.read_text(encoding="utf-8"))
            if example is not None
            else copy.deepcopy(DEFAULT_CONFIG)
        )
        path.write_text(
            json.dumps(seed, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )

    user_config = json.loads(path.read_text(encoding="utf-8-sig"))
    cfg = _deep_merge(copy.deepcopy(DEFAULT_CONFIG), user_config)
    cfg["_root"] = str(ROOT)
    cfg["_config_path"] = str(path)
    return cfg


def save_web_port(config_path: str | Path, port: int) -> None:
    """把 web.port 写回配置文件，保留其他字段原样。"""
    path = Path(config_path)
    if path.exists():
        raw = json.loads(path.read_text(encoding="utf-8-sig"))
    else:
        raw = copy.deepcopy(DEFAULT_CONFIG)
    raw.setdefault("web", {})
    raw["web"]["port"] = int(port)
    path.write_text(
        json.dumps(raw, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


def resolve_path(value: str | Path, base: Path = ROOT) -> Path:
    path = Path(value)
    return path if path.is_absolute() else base / path


def resolve_db_path(cfg: dict[str, Any], override: str | Path | None = None) -> Path:
    return resolve_path(override or cfg["db_path"])
