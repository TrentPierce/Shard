#!/usr/bin/env python3
"""Shard Oracle — unified entry point.

Starts the FastAPI Oracle API server. The Rust sidecar (shard-daemon.exe)
should be started separately or via a wrapper script.

Usage:
    python run.py                       # default: 0.0.0.0:8000
    python run.py --host 127.0.0.1      # localhost only
    python run.py --port 8080           # custom port
    python run.py --rust-url http://192.168.1.10:9091  # remote sidecar
"""

from __future__ import annotations

import argparse
import os
import sys


def main() -> None:
    parser = argparse.ArgumentParser(description="Shard Oracle API Server")
    parser.add_argument("--host", default="0.0.0.0", help="Bind address (default: 0.0.0.0)")
    parser.add_argument("--port", type=int, default=8000, help="Bind port (default: 8000)")
    parser.add_argument(
        "--rust-url",
        default="http://127.0.0.1:9091",
        help="Rust sidecar control-plane URL (default: http://127.0.0.1:9091)",
    )
    parser.add_argument("--reload", action="store_true", help="Enable auto-reload for development")
    args = parser.parse_args()

    # Expose sidecar URL as env var so oracle_api can read it
    os.environ.setdefault("SHARD_RUST_URL", args.rust_url)

    try:
        import uvicorn
    except ImportError:
        print("ERROR: uvicorn not installed.  Run:  pip install -r requirements.txt", file=sys.stderr)
        sys.exit(1)

    print(f"  ╔══════════════════════════════════════════╗")
    print(f"  ║       Shard Oracle API  v0.3.0           ║")
    print(f"  ╠══════════════════════════════════════════╣")
    print(f"  ║  API        → http://{args.host}:{args.port}       ║")
    print(f"  ║  Rust Sidecar → {args.rust_url}  ║")
    print(f"  ╚══════════════════════════════════════════╝")
    print()

    uvicorn.run(
        "oracle_api:app",
        host=args.host,
        port=args.port,
        reload=args.reload,
        log_level="info",
    )


if __name__ == "__main__":
    main()
