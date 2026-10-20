"""PyInstaller 打包入口：vps-monitor.exe。"""

import sys

from vps_monitor.__main__ import main

if __name__ == "__main__":
    sys.exit(main())
