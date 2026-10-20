@echo off
chcp 65001 >nul
cd /d "%~dp0"
set PYTHONUTF8=1
where pythonw >nul 2>nul
if %errorlevel%==0 (
    start "" pythonw -m vps_monitor tray
) else (
    start "" /min python -m vps_monitor tray
)
