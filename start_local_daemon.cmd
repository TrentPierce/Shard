@echo off
set BITNET_LIB=D:\Dev\Projects\Shard\Shard\desktop\rust\target\release\shard_engine.dll
set BITNET_MODEL=D:\Dev\Projects\Shard\Shard\models\Llama-3.2-1B-Instruct-Q4_K_M.gguf
set SHARD_RELEASE_PROFILE=benchmark-2026-03-05
start "" "D:\Dev\Projects\Shard\Shard\desktop\rust\target\release\shard-daemon.exe" --control-port 9191 --telemetry-ws-port 9193 --tcp-port 5001 --webrtc-port 9190 --quic-port 9192 --contribute --public-api --relay-mode --bootstrap-node /dns4/p2p.shardnetwork.live/tcp/443/wss/p2p/12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV
