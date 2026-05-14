@echo off
REM Restart flov in dev mode. Sets the cwd to the repo root so Tauri CLI
REM finds src-tauri/tauri.conf.json, and prepends cargo to PATH.
cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
".\ui\node_modules\.bin\tauri.cmd" dev
