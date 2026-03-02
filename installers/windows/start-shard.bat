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
set "WEBRTC_PORT=%SHARD_WEBRTC_PORT%"
if "%WEBRTC_PORT%"=="" set "WEBRTC_PORT=9090"
set "QUIC_PORT=%SHARD_QUIC_PORT%"
if "%QUIC_PORT%"=="" set "QUIC_PORT=9092"
set "LOG_DIR=%ProgramData%\Shard\logs"
if not exist "%LOG_DIR%" mkdir "%LOG_DIR%" >nul 2>&1

echo Starting Shard daemon with TCP port %TCP_PORT% and control port %CONTROL_PORT%...
echo Binary: %~dp0%BIN%
echo.

set "BOOTSTRAP_ARGS="
if not "%SHARD_BOOTSTRAP_NODES%"=="" (
  for %%N in (%SHARD_BOOTSTRAP_NODES:,= %) do (
    set "BOOTSTRAP_ARGS=!BOOTSTRAP_ARGS! --bootstrap-node %%~N"
  )
)

"%~dp0%BIN%" --tcp-port %TCP_PORT% --control-port %CONTROL_PORT% --webrtc-port %WEBRTC_PORT% --quic-port %QUIC_PORT% !BOOTSTRAP_ARGS! >> "%LOG_DIR%\daemon.log" 2>&1
