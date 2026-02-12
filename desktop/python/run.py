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
import logging
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
    parser.add_argument(
        "--log-level",
        default=os.getenv("SHARD_LOG_LEVEL", "INFO"),
        choices=["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"],
        help="Application log level",
    )
    parser.add_argument("--public-api", action="store_true", help="Expose API publicly to internet")
    parser.add_argument("--public-host", type=str, default=None, help="Public hostname/IP for API (auto-detect if not set)")
    parser.add_argument("--https", action="store_true", help="Enable HTTPS with Let's Encrypt")
    args = parser.parse_args()

    # Expose sidecar URL as env var so oracle_api can read it
    os.environ.setdefault("SHARD_RUST_URL", args.rust_url)
    # Pass public API flags to API
    if args.public_api:
        os.environ["SHARD_PUBLIC_API"] = "true"
    if args.public_host:
        os.environ["SHARD_PUBLIC_HOST"] = args.public_host
    if args.https:
        os.environ["SHARD_HTTPS"] = "true"

    try:
        import uvicorn
    except ImportError:
        print("ERROR: uvicorn not installed.  Run:  pip install -r requirements.txt", file=sys.stderr)
        sys.exit(1)
    
    print(f"  ╔══════════════════════════════════════╗")
    print(f"  ║       Shard Oracle API  v0.3.0           ║")
    print(f"  ╠══════════════════════════════════════╣")
    print(f"  ║  API        → http{'s' if args.https else ''}://{args.host}:{args.port}       ║")
    print(f"  ║  Public API  : {'enabled' if args.public_api else 'disabled'}          ║")
    print(f"  ║  Public Host : {args.public_host or 'auto-detect'}      ║")
    print(f"  ║  Rust Sidecar → {args.rust_url}  ║")
    print(f"  ╚══════════════════════════════════════╝")
    print()

    logging.basicConfig(
        level=getattr(logging, args.log_level.upper(), logging.INFO),
        format="%(asctime)s | %(levelname)s | %(name)s | %(message)s",
    )

    uvicorn.run(
        "oracle_api:app",
        host=args.host,
        port=args.port,
        reload=args.reload,
        log_level=args.log_level.lower(),
    )


if __name__ == "__main__":
    main()
