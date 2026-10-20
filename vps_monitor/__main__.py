from __future__ import annotations

import argparse
import json
import logging
import sys
import threading
from logging.handlers import RotatingFileHandler
from typing import Optional, Sequence

from . import __version__
from .collector import Collector, run_loop
from .config import ROOT, load_config, resolve_db_path
from .db import Database
from .web import UVICORN_LOG_CONFIG, create_app

log = logging.getLogger(__name__)


def _setup_logging(verbose: bool = False, file_log: bool = False) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    formatter = logging.Formatter("%(asctime)s %(levelname)s %(name)s: %(message)s")
    handlers: list[logging.Handler] = []
    # pythonw 下 sys.stderr 为 None，控制台 handler 不可用
    if sys.stderr is not None:
        console = logging.StreamHandler()
        console.setFormatter(formatter)
        handlers.append(console)
    if file_log or sys.stderr is None:
        log_dir = ROOT / "data"
        log_dir.mkdir(parents=True, exist_ok=True)
        file_handler = RotatingFileHandler(
            log_dir / "vps_monitor.log", maxBytes=1_000_000, backupCount=3, encoding="utf-8"
        )
        file_handler.setFormatter(formatter)
        handlers.append(file_handler)
    logging.basicConfig(level=level, handlers=handlers)


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="vps_monitor",
        description="本地 VPS 库存监控（数据源 kyoa.de）",
    )
    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    parser.add_argument("-v", "--verbose", action="store_true", help="输出调试日志")

    sub = parser.add_subparsers(dest="command")

    def add_common(p: argparse.ArgumentParser) -> None:
        p.add_argument("--config", help="配置文件路径（默认 config.json）")
        p.add_argument("--db", help="数据库路径（默认取配置 db_path）")

    run_p = sub.add_parser("run", help="启动采集循环 + Web 看板")
    add_common(run_p)
    run_p.add_argument("--host")
    run_p.add_argument("--port", type=int)

    tray_p = sub.add_parser("tray", help="托盘模式：托盘图标 + 采集 + Web 看板")
    add_common(tray_p)
    tray_p.add_argument("--host")
    tray_p.add_argument("--port", type=int)

    serve_p = sub.add_parser("serve", help="只启动 Web 看板（不采集）")
    add_common(serve_p)
    serve_p.add_argument("--host")
    serve_p.add_argument("--port", type=int)

    collect_p = sub.add_parser("collect", help="执行采集")
    add_common(collect_p)
    collect_p.add_argument("--force", action="store_true", help="忽略 ETag/内容哈希，强制重新比对")
    mode = collect_p.add_mutually_exclusive_group()
    mode.add_argument("--once", action="store_true", help="只采集一次（默认）")
    mode.add_argument("--loop", action="store_true", help="按配置间隔循环采集")

    return parser


def main(argv: Optional[Sequence[str]] = None) -> int:
    args = _build_parser().parse_args(argv)

    if args.command is None:
        # 双击打包 exe（无参数）默认进托盘模式；源码命令行默认 run
        args.command = "tray" if getattr(sys, "frozen", False) else "run"
        args.config = None
        args.db = None
        args.host = None
        args.port = None

    command = args.command
    _setup_logging(args.verbose, file_log=(command == "tray"))

    cfg = load_config(getattr(args, "config", None))
    db = Database(resolve_db_path(cfg, getattr(args, "db", None)))
    collector = Collector(cfg, db)

    web_cfg = cfg.get("web") or {}
    host = getattr(args, "host", None) or web_cfg.get("host") or "127.0.0.1"
    port = getattr(args, "port", None) or int(web_cfg.get("port") or 8787)

    if command == "tray":
        try:
            from .tray import run_tray
        except ImportError:
            message = "缺少托盘依赖 pystray / Pillow，请先执行：pip install -r requirements.txt"
            print(message)
            try:
                import ctypes

                ctypes.windll.user32.MessageBoxW(0, message, "VPS 库存监控", 0x30)
            except Exception:
                pass
            return 1
        return run_tray(cfg, db, collector, host, port)

    if command == "collect":
        if args.loop:
            stop_event = threading.Event()
            log.info("开始循环采集，间隔 %s 秒，Ctrl+C 退出", cfg["poll_interval_seconds"])
            try:
                run_loop(collector, int(cfg["poll_interval_seconds"]), stop_event)
            except KeyboardInterrupt:
                stop_event.set()
                log.info("已停止")
            return 0

        result = collector.collect_once(force=args.force)
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0 if result.get("ok") else 1

    import uvicorn

    app = create_app(cfg, db, collector, start_collector=(command == "run"))

    log.info("Web 看板：http://%s:%s", "127.0.0.1" if host == "0.0.0.0" else host, port)
    uvicorn.run(
        app,
        host=host,
        port=port,
        log_level="info",
        access_log=False,
        log_config=UVICORN_LOG_CONFIG,
    )
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except SystemExit:
        raise
    except Exception:
        # pythonw 无控制台，异常写 crash.log 留痕，否则会无声退出
        if sys.stderr is None:
            import traceback

            crash_path = ROOT / "data" / "crash.log"
            try:
                crash_path.parent.mkdir(parents=True, exist_ok=True)
                crash_path.write_text(traceback.format_exc(), encoding="utf-8")
            except OSError:
                pass
        raise
