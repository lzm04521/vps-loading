# -*- mode: python ; coding: utf-8 -*-
# PyInstaller spec：单文件无控制台托盘 exe
# 由 scripts/build.py 调用（先确保 assets/app.ico 已生成）

a = Analysis(
    ["entry.py"],
    pathex=[".."],
    binaries=[],
    datas=[("../web", "web"), ("../config.example.json", ".")],
    hiddenimports=["pystray._win32"],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name="vps-monitor",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=False,
    icon="../assets/app.ico",
)
