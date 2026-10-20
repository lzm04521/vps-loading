"""打包脚本：生成图标 -> PyInstaller 出单文件 exe。

用法（在项目根目录）：python scripts/build.py
产物：dist/vps-monitor.exe
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT))


def main() -> int:
    # CI 英文 Windows 控制台是 cp1252，中文输出会 UnicodeEncodeError
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, OSError):
            pass

    from vps_monitor.tray import icon_image

    assets = PROJECT_ROOT / "assets"
    assets.mkdir(exist_ok=True)
    icon_image(256).save(
        assets / "app.ico",
        sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    print("图标已生成：assets/app.ico")

    result = subprocess.run(
        [sys.executable, "-m", "PyInstaller", "scripts/vps-monitor.spec", "--noconfirm"],
        cwd=PROJECT_ROOT,
    )
    if result.returncode != 0:
        print("PyInstaller 打包失败", file=sys.stderr)
        return result.returncode
    print("打包完成：dist/vps-monitor.exe")
    return 0


if __name__ == "__main__":
    sys.exit(main())
