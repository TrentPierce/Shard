@echo off
setlocal enabledelayedexpansion

set "ROOT=%~dp0..\.."
for %%I in ("%ROOT%") do set "ROOT=%%~fI"

set "BENCHMARK_PROFILE=%1"
if "%BENCHMARK_PROFILE%"=="" set "BENCHMARK_PROFILE=short"

set "BITNET_LIB=%ROOT%\desktop\rust\target\release\shard_engine.dll"
if not exist "%BITNET_LIB%" set "BITNET_LIB=%ROOT%\web\src-tauri\resources\shard_engine.dll"
set "BITNET_MODEL=%ROOT%\models\Llama-3.2-1B-Instruct-Q4_K_M.gguf"
set "MODEL_ID=meta-llama/Llama-3.2-1B"
set "DAEMON_EXE=%ROOT%\desktop\rust\target\release\shard-daemon.exe"
if not exist "%DAEMON_EXE%" set "DAEMON_EXE=%ROOT%\desktop\rust\target\release\shard-daemon.locked.exe"
set "PATH=%ROOT%\desktop\rust\target\release;%ROOT%\web\src-tauri\resources;%PATH%"
set "RUNTIME_DIR=%ROOT%\runtime"
set "LOG_FILE=%RUNTIME_DIR%\local-release-daemon.log"

if not exist "%RUNTIME_DIR%" mkdir "%RUNTIME_DIR%"

set "PROFILE_ENV=%ROOT%\deploy\release\benchmark.env"
if /I "%BENCHMARK_PROFILE%"=="long" set "PROFILE_ENV=%ROOT%\deploy\release\long_benchmark.env"

for %%F in ("%ROOT%\deploy\release\rc1.env" "%PROFILE_ENV%") do (
  for /f "usebackq eol=# tokens=1* delims==" %%A in ("%%~fF") do (
    if not "%%~A"=="" set "%%~A=%%~B"
  )
)
if /I "%BENCHMARK_PROFILE%"=="long" set "SHARD_SCOUT_BOOTSTRAP_ALLOW_HARD_CIRCUIT=true"

echo [%DATE% %TIME%] launching local release daemon > "%LOG_FILE%"
echo benchmark_profile=%BENCHMARK_PROFILE% >> "%LOG_FILE%"
echo exe=%DAEMON_EXE% >> "%LOG_FILE%"
"%DAEMON_EXE%" --control-port 9191 --telemetry-ws-port 9193 --model-id "%MODEL_ID%" --bootstrap-node /ip4/35.175.242.222/tcp/4001/p2p/12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV --bootstrap-node /ip4/35.175.242.222/udp/9092/quic-v1/p2p/12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV --contribute >> "%LOG_FILE%" 2>&1
