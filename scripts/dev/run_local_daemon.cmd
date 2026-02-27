@echo off
setlocal
set "ROOT=%~dp0..\.."
for %%I in ("%ROOT%") do set "ROOT=%%~fI"

set "BITNET_LIB=%ROOT%\web\src-tauri\resources\shard_engine.dll"
if not exist "%BITNET_LIB%" set "BITNET_LIB=%ROOT%\desktop\rust\target\release\shard_engine.dll"
set "BITNET_MODEL=%ROOT%\models\tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"
set "PATH=%ROOT%\web\src-tauri\resources;%ROOT%\desktop\rust\target\release;%PATH%"

"%ROOT%\desktop\rust\target\release\shard-daemon.exe" ^
  --bootstrap-node /dns4/35.175.242.222.nip.io/tcp/4001/p2p/12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV ^
  --bootstrap-node /dns4/35.175.242.222.nip.io/udp/9092/quic-v1/p2p/12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV ^
  %*
