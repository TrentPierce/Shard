@echo off
setlocal ENABLEDELAYEDEXPANSION

set "BIN=shard-daemon-x86_64-pc-windows-msvc.exe"
if not exist "%~dp0%BIN%" (
  set "BIN=shard-daemon.exe"
)
if not exist "%~dp0%BIN%" (
  echo [ERROR] Could not find shard daemon binary in:
  echo         %~dp0
  echo         Expected shard-daemon-x86_64-pc-windows-msvc.exe or shard-daemon.exe
  exit /b 1
)

set "TCP_PORT=%SHARD_TCP_PORT%"
if "%TCP_PORT%"=="" set "TCP_PORT=4001"
set "CONTROL_PORT=%SHARD_CONTROL_PORT%"
if "%CONTROL_PORT%"=="" set "CONTROL_PORT=9091"
set "LOG_DIR=%ProgramData%\Shard\logs"
if not exist "%LOG_DIR%" mkdir "%LOG_DIR%" >nul 2>&1

echo Starting Shard daemon with TCP port %TCP_PORT% and control port %CONTROL_PORT%...
echo Binary: %~dp0%BIN%
echo.

"%~dp0%BIN%" --tcp-port %TCP_PORT% --control-port %CONTROL_PORT% >> "%LOG_DIR%\daemon.log" 2>&1
