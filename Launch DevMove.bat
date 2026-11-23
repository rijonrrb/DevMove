@echo off
setlocal
cd /d "%~dp0"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0DevMove.ps1"
if errorlevel 1 (
  echo.
  echo DevMove failed to start. Check the PowerShell error above.
  pause
)
