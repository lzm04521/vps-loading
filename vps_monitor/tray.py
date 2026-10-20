"""Windows 托盘模式：托盘图标 + 菜单 + 设置窗口 + Web 服务托管。

主线程运行托盘消息循环（icon.run），Web 服务与采集在后台线程，
tkinter 界面固定在独立 UI 线程，跨线程操作一律经队列提交。
"""

from __future__ import annotations

import ctypes
import logging
import socket
import threading
import time
import webbrowser
from queue import Empty, Queue
from typing import Any, Callable, Optional

import pystray
import uvicorn
from PIL import Image, ImageDraw

from .autostart import disable as autostart_disable
from .autostart import enable as autostart_enable
from .autostart import is_enabled as autostart_is_enabled
from .collector import Collector
from .config import save_web_port
from .db import Database
from .web import UVICORN_LOG_CONFIG, create_app

log = logging.getLogger(__name__)

MUTEX_NAME = "VpsMonitorSingleInstance"
ERROR_ALREADY_EXISTS = 183
_mutex_handle: Optional[int] = None


def acquire_single_instance() -> bool:
    """同名互斥体已存在说明已有实例在跑，弹框提示后返回 False。"""
    global _mutex_handle
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    handle = kernel32.CreateMutexW(None, False, MUTEX_NAME)
    if ctypes.get_last_error() == ERROR_ALREADY_EXISTS:
        ctypes.windll.user32.MessageBoxW(
            0,
            "VPS 库存监控已在运行，请在任务栏右下角托盘中找到图标。\n"
            "左键双击图标打开看板，右键图标可打开看板或退出。",
            "VPS 库存监控",
            0x40,  # MB_ICONINFORMATION
        )
        return False
    _mutex_handle = handle
    return True


def port_bindable(host: str, port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        try:
            sock.bind((host, port))
        except OSError:
            return False
    return True


def dashboard_url(port: int) -> str:
    return f"http://127.0.0.1:{port}"


def icon_image(size: int = 64) -> Image.Image:
    """按 web/favicon.svg 复刻：蓝色渐变圆角方块 + 两道白条 + 绿点。"""
    scale = size / 64

    def box(x0: float, y0: float, x1: float, y1: float) -> list[int]:
        return [round(v * scale) for v in (x0, y0, x1, y1)]

    # 渐变底
    base = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    top, bottom = (59, 130, 246), (29, 78, 216)
    draw = ImageDraw.Draw(base)
    for y in range(size):
        t = y / max(size - 1, 1)
        color = tuple(round(top[i] + (bottom[i] - top[i]) * t) for i in range(3)) + (255,)
        draw.line([(0, y), (size, y)], fill=color)

    # 圆角遮罩
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        box(1, 1, 63, 63), radius=round(15 * scale), fill=255
    )
    icon = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    icon.paste(base, (0, 0), mask)

    overlay = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(overlay)
    d.rounded_rectangle(box(11, 19, 53, 31), radius=round(6 * scale), fill=(255, 255, 255, 245))
    d.rounded_rectangle(box(11, 34, 53, 46), radius=round(6 * scale), fill=(255, 255, 255, 140))
    cx, cy = round(46 * scale), round(25 * scale)
    halo = round(9 * scale)
    d.ellipse([cx - halo, cy - halo, cx + halo, cy + halo], fill=(74, 222, 128, 97))
    r = max(2, round(4.6 * scale))
    d.ellipse(
        [cx - r, cy - r, cx + r, cy + r],
        fill=(34, 197, 94, 255),
        outline=(255, 255, 255, 255),
        width=max(1, round(2.2 * scale)),
    )
    return Image.alpha_composite(icon, overlay)


class ServerManager:
    """在后台线程托管 uvicorn 服务，支持换端口重启。"""

    def __init__(self, app_factory: Callable[[], Any], host: str) -> None:
        self._app_factory = app_factory
        self._host = host
        self._lock = threading.Lock()
        self._server: Optional[uvicorn.Server] = None
        self._thread: Optional[threading.Thread] = None
        self.port: Optional[int] = None

    def start(self, port: int, timeout: float = 10.0) -> bool:
        server = uvicorn.Server(
            uvicorn.Config(
                self._app_factory(),
                host=self._host,
                port=port,
                log_level="warning",
                access_log=False,
                log_config=UVICORN_LOG_CONFIG,
            )
        )
        thread = threading.Thread(target=self._serve, args=(server,), name="vps-web", daemon=True)
        with self._lock:
            self._server = server
            self._thread = thread
            self.port = port
        thread.start()
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if server.started:
                return True
            if not thread.is_alive():
                log.error("Web 服务线程异常退出（端口 %s 可能被占用）", port)
                return False
            time.sleep(0.2)
        return server.started

    @staticmethod
    def _serve(server: uvicorn.Server) -> None:
        try:
            server.run()
        except Exception:
            logging.getLogger(__name__).exception("Web 服务异常退出")

    def stop(self) -> None:
        with self._lock:
            server, thread = self._server, self._thread
            self._server = None
            self._thread = None
        if server is not None:
            server.should_exit = True
        if thread is not None:
            thread.join(timeout=8)
        self.port = None

    def restart(self, port: int) -> bool:
        self.stop()
        return self.start(port)


class UiThread:
    """独立线程持有隐藏 Tk root，窗口操作经队列在该线程执行。"""

    def __init__(self) -> None:
        self._queue: Queue = Queue()
        self._root: Any = None
        self._thread: Optional[threading.Thread] = None
        self._ready = threading.Event()
        self.tk_error: Optional[str] = None

    def start(self) -> bool:
        self._thread = threading.Thread(target=self._run, name="vps-ui", daemon=True)
        self._thread.start()
        self._ready.wait(timeout=10)
        return self._root is not None

    def _run(self) -> None:
        try:
            import tkinter as tk
        except Exception as exc:  # pragma: no cover - 环境缺 tk
            self.tk_error = str(exc)
            self._ready.set()
            return
        self._root = tk.Tk()
        self._root.withdraw()
        self._ready.set()
        self._poll()
        self._root.mainloop()

    def _poll(self) -> None:
        try:
            while True:
                task = self._queue.get_nowait()
                try:
                    task()
                except Exception:
                    log.exception("UI 任务执行失败")
        except Empty:
            pass
        if self._root is not None:
            self._root.after(80, self._poll)

    def submit(self, task: Callable[[], None]) -> None:
        self._queue.put(task)

    def stop(self) -> None:
        if self._root is not None:
            self.submit(self._root.destroy)
        if self._thread is not None:
            self._thread.join(timeout=3)


class TrayApp:
    def __init__(
        self,
        cfg: dict[str, Any],
        db: Database,
        collector: Collector,
        host: str,
        port: int,
    ) -> None:
        self.cfg = cfg
        self.collector = collector
        self.host = host
        self.port = port
        self.manager = ServerManager(
            lambda: create_app(cfg, db, collector, start_collector=True), host
        )
        self.ui = UiThread()
        self.icon: Optional[pystray.Icon] = None
        self._settings_win: Any = None

    # ---- 托盘菜单回调（pystray 线程）----

    def _open_dashboard(self, icon: pystray.Icon, item: Any) -> None:
        webbrowser.open(dashboard_url(self.manager.port or self.port))

    def _collect_now(self, icon: pystray.Icon, item: Any) -> None:
        def worker() -> None:
            result = self.collector.collect_once(force=False)
            self._notify(self._collect_summary(result))

        threading.Thread(target=worker, name="vps-collect-manual", daemon=True).start()

    @staticmethod
    def _collect_summary(result: dict[str, Any]) -> str:
        if not result.get("ok"):
            return f"采集失败：{result.get('error') or '未知错误'}"
        if result.get("changed"):
            events = len(result.get("events") or [])
            alerts = len(result.get("alerts") or [])
            return f"采集完成：{events} 条新事件，发出告警 {alerts} 条"
        return "采集完成：数据无变化"

    def _toggle_autostart(self, icon: pystray.Icon, item: Any) -> None:
        try:
            if autostart_is_enabled():
                autostart_disable()
                self._notify("已关闭开机自启动")
            else:
                autostart_enable(self.cfg["_config_path"])
                self._notify("已开启开机自启动（当前用户注册表 Run 键）")
        except OSError as exc:
            self._notify(f"自启动设置失败：{exc}")

    def _show_settings(self, icon: pystray.Icon, item: Any) -> None:
        self.ui.submit(self._build_settings_window)

    def _quit(self, icon: pystray.Icon, item: Any) -> None:
        icon.stop()

    # ---- 设置窗口（UI 线程）----

    def _build_settings_window(self) -> None:
        if self.ui._root is None:
            self._notify(f"tkinter 不可用：{self.ui.tk_error}")
            return
        import tkinter as tk
        from tkinter import ttk

        # 已打开时置顶既有窗口，避免叠出多个设置窗口
        existing = self._settings_win
        if existing is not None and existing.winfo_exists():
            existing.deiconify()
            existing.lift()
            existing.focus_force()
            return

        current_port = self.manager.port or self.port
        win = tk.Toplevel(self.ui._root)
        self._settings_win = win
        win.title("VPS 库存监控 · 设置")
        win.resizable(False, False)
        # 保证标题栏完整显示
        win.minsize(340, 0)

        frame = ttk.Frame(win, padding=16)
        frame.grid(sticky="nsew")

        ttk.Label(frame, text="看板地址").grid(row=0, column=0, sticky="w", pady=4)
        url_var = tk.StringVar(value=dashboard_url(current_port))
        ttk.Label(frame, textvariable=url_var, foreground="#2563eb").grid(
            row=0, column=1, sticky="w", pady=4
        )

        ttk.Label(frame, text="Web 端口").grid(row=1, column=0, sticky="w", pady=4)
        port_var = tk.StringVar(value=str(current_port))
        ttk.Entry(frame, textvariable=port_var, width=12).grid(row=1, column=1, sticky="w", pady=4)

        auto_var = tk.BooleanVar(value=autostart_is_enabled())
        ttk.Checkbutton(frame, text="开机自启动（当前用户，无需管理员）", variable=auto_var).grid(
            row=2, column=0, columnspan=2, sticky="w", pady=4
        )

        def save() -> None:
            self._save_settings(win, port_var, auto_var, url_var)

        btns = ttk.Frame(frame)
        btns.grid(row=3, column=0, columnspan=2, pady=(12, 0))
        ttk.Button(btns, text="保存", command=save).pack(side="left", padx=4)
        ttk.Button(btns, text="取消", command=win.destroy).pack(side="left", padx=4)
        win.bind("<Return>", lambda _e: save())
        win.bind("<Escape>", lambda _e: win.destroy())

        # 托盘进程通常没有前台焦点，置顶保证设置窗口不被遮挡；
        # root 是隐藏状态，不能用 transient（会导致窗口不显示）
        win.attributes("-topmost", True)
        try:
            win.grab_set()
        except tk.TclError:
            pass
        win.deiconify()
        win.focus_force()

    def _save_settings(
        self, win: Any, port_var: Any, auto_var: Any, url_var: Any
    ) -> None:
        from tkinter import messagebox

        raw = str(port_var.get()).strip()
        if not raw.isdigit() or not 1 <= int(raw) <= 65535:
            messagebox.showerror("设置", "端口必须是 1-65535 之间的整数", parent=win)
            return
        new_port = int(raw)

        try:
            if auto_var.get():
                autostart_enable(self.cfg["_config_path"])
            else:
                autostart_disable()
        except OSError as exc:
            messagebox.showerror("设置", f"开机自启动写入失败：{exc}", parent=win)
            return

        current = self.manager.port or self.port
        if new_port == current:
            win.destroy()
            self._notify("设置已保存")
            return

        if not port_bindable(self.host, new_port):
            messagebox.showerror(
                "设置", f"端口 {new_port} 无法使用（被占用或被系统保留），请换一个", parent=win
            )
            return
        try:
            self.cfg.setdefault("web", {})["port"] = new_port
            save_web_port(self.cfg["_config_path"], new_port)
        except OSError as exc:
            messagebox.showerror("设置", f"配置写入失败：{exc}", parent=win)
            return

        win.destroy()
        threading.Thread(target=self._apply_port, args=(new_port,), daemon=True).start()

    def _apply_port(self, new_port: int) -> None:
        log.info("Web 服务切换端口：%s -> %s", self.manager.port or self.port, new_port)
        if self.manager.restart(new_port):
            self._notify(f"端口已切换为 {new_port}，看板 {dashboard_url(new_port)}")
        else:
            self._notify(f"端口 {new_port} 启动失败，请右键图标重新设置")

    # ---- 通知与主流程 --------

    def _notify(self, message: str) -> None:
        icon = self.icon
        if icon is None:
            log.info("通知（托盘未就绪）：%s", message)
            return
        try:
            icon.notify(message, "VPS 库存监控")
        except Exception:
            log.info("通知：%s", message)

    def _icon_ready(self, started: bool) -> None:
        if not started:
            self._notify(
                f"端口 {self.port} 启动失败（可能被占用）。右键图标 → 设置… 更改端口"
            )

    def run(self) -> int:
        if not acquire_single_instance():
            return 1
        ui_ok = self.ui.start()
        started = self.manager.start(self.port)
        if started:
            log.info("托盘模式启动，看板 %s", dashboard_url(self.port))
        if not ui_ok:
            log.warning("tkinter 不可用，设置窗口将无法打开：%s", self.ui.tk_error)

        settings_action = self._show_settings
        menu = pystray.Menu(
            pystray.MenuItem("打开看板", self._open_dashboard, default=True),
            pystray.MenuItem("立即采集", self._collect_now),
            pystray.MenuItem("设置…", settings_action),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem(
                "开机自启动",
                self._toggle_autostart,
                checked=lambda item: autostart_is_enabled(),
            ),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem("退出", self._quit),
        )
        self.icon = pystray.Icon("vps_monitor", icon_image(), "VPS 库存监控", menu)
        try:
            # pystray 的 setup 回调签名为 setup(icon)
            self.icon.run(setup=lambda icon: self._icon_ready(started))
        except KeyboardInterrupt:
            self.icon.stop()
        log.info("正在退出，停止 Web 服务…")
        self.manager.stop()
        self.ui.stop()
        return 0


def run_tray(
    cfg: dict[str, Any], db: Database, collector: Collector, host: str, port: int
) -> int:
    return TrayApp(cfg, db, collector, host, port).run()
