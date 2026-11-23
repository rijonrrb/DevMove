@echo off
setlocal
cd /d "%~dp0desktop"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>nul
if errorlevel 1 (
  echo Rust is missing. Install Rust via rustup and retry.
  pause
  exit /b 1
)
if not exist "node_modules" (
  echo Installing frontend dependencies...
  call npm ci
  if errorlevel 1 (pause & exit /b 1)
)
echo Launching DevMove Rust preview. The first build may take several minutes...
call npx tauri dev
if errorlevel 1 (
  echo DevMove could not start. Check the errors above.
  pause
)
