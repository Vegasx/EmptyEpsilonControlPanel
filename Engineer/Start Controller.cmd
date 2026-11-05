@echo off
cd /d "%~dp0"
where node >nul 2>nul
if errorlevel 1 (
  echo Please install Node.js 20 or newer, then try again.
  pause
  exit /b 1
)
node "%~dp0server.mjs" --open
if errorlevel 1 pause
