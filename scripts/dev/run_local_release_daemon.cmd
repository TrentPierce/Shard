@echo off
setlocal enabledelayedexpansion

set "ROOT=%~dp0..\.."
for %%I in ("%ROOT%") do set "ROOT=%%~fI"

set "BITNET_LIB=%ROOT%\desktop\rust\target\release\shard_engine.dll"
if not exist "%BITNET_LIB%" set "BITNET_LIB=%ROOT%\web\src-tauri\resources\shard_engine.dll"
set "BITNET_MODEL=%ROOT%\models\Llama-3.2-1B-Instruct-Q4_K_M.gguf"
set "DAEMON_EXE=%ROOT%\desktop\rust\target\release\shard-daemon.exe"
if not exist "%DAEMON_EXE%" set "DAEMON_EXE=%ROOT%\desktop\rust\target\release\shard-daemon.locked.exe"
set "PATH=%ROOT%\desktop\rust\target\release;%ROOT%\web\src-tauri\resources;%PATH%"
set "RUNTIME_DIR=%ROOT%\runtime"
set "LOG_FILE=%RUNTIME_DIR%\local-release-daemon.log"

if not exist "%RUNTIME_DIR%" mkdir "%RUNTIME_DIR%"

for %%F in ("%ROOT%\deploy\release\rc1.env" "%ROOT%\deploy\release\benchmark.env") do (
  for /f "usebackq eol=# tokens=1* delims==" %%A in ("%%~fF") do (
    if not "%%~A"=="" set "%%~A=%%~B"
  )
)

echo [%DATE% %TIME%] launching local release daemon > "%LOG_FILE%"
echo exe=%DAEMON_EXE% >> "%LOG_FILE%"
"%DAEMON_EXE%" --control-port 9191 --telemetry-ws-port 9193 --bootstrap-node /dns4/35.175.242.222.nip.io/tcp/4001/p2p/12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV --bootstrap-node /dns4/35.175.242.222.nip.io/udp/9092/quic-v1/p2p/12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV --contribute >> "%LOG_FILE%" 2>&1
