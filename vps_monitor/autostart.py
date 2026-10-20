"""开机自启动管理：HKCU 注册表 Run 键，无需管理员权限。"""

from __future__ import annotations

import sys
import winreg
from pathlib import Path

RUN_KEY = r"Software\Microsoft\Windows\CurrentVersion\Run"
APP_NAME = "VpsMonitor"


def is_frozen() -> bool:
    return bool(getattr(sys, "frozen", False))


def pythonw_path() -> str:
    """优先返回 pythonw.exe（无控制台窗口）；不存在时退回当前解释器。"""
    exe = Path(sys.executable)
    windowed = exe.with_name("pythonw.exe")
    return str(windowed) if windowed.exists() else str(exe)


def build_command(config_path: str | Path) -> str:
    if is_frozen():
        # 打包后的 exe 自身即无窗口托盘程序
        return f'"{sys.executable}" tray --config "{Path(config_path)}"'
    return f'"{pythonw_path()}" -m vps_monitor tray --config "{Path(config_path)}"'


def is_enabled() -> bool:
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, RUN_KEY) as key:
            winreg.QueryValueEx(key, APP_NAME)
            return True
    except FileNotFoundError:
        return False
    except OSError:
        return False


def enable(config_path: str | Path) -> None:
    with winreg.OpenKey(
        winreg.HKEY_CURRENT_USER, RUN_KEY, 0, winreg.KEY_SET_VALUE
    ) as key:
        winreg.SetValueEx(key, APP_NAME, 0, winreg.REG_SZ, build_command(config_path))


def disable() -> None:
    try:
        with winreg.OpenKey(
            winreg.HKEY_CURRENT_USER, RUN_KEY, 0, winreg.KEY_SET_VALUE
        ) as key:
            winreg.DeleteValue(key, APP_NAME)
    except FileNotFoundError:
        pass
