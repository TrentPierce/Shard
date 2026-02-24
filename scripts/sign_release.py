#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path


def run(cmd: list[str], dry_run: bool) -> None:
    print("$", " ".join(cmd))
    if not dry_run:
        subprocess.run(cmd, check=True)


def sign_windows(path: Path, timestamp_url: str, dry_run: bool) -> None:
    signtool = shutil.which("signtool")
    if not signtool:
        raise SystemExit("signtool not found in PATH")
    run([signtool, "sign", "/fd", "SHA256", "/tr", timestamp_url, "/td", "SHA256", "/a", str(path)], dry_run)
    run([signtool, "verify", "/pa", "/v", str(path)], dry_run)


def sign_macos(path: Path, identity: str, dry_run: bool, notarize: bool) -> None:
    if not shutil.which("codesign"):
        raise SystemExit("codesign not found")
    run(["codesign", "--force", "--options", "runtime", "--sign", identity, str(path)], dry_run)
    if notarize:
        if not shutil.which("xcrun"):
            raise SystemExit("xcrun not found for notarization")
        run(["xcrun", "notarytool", "submit", str(path), "--wait"], dry_run)


def sign_linux(path: Path, dry_run: bool) -> None:
    if not shutil.which("gpg"):
        raise SystemExit("gpg not found")
    run(["gpg", "--batch", "--yes", "--armor", "--detach-sign", str(path)], dry_run)


def main() -> None:
    parser = argparse.ArgumentParser(description="Sign Shard release artifacts")
    parser.add_argument("artifact", help="Path to artifact")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--timestamp-url", default="http://timestamp.digicert.com")
    parser.add_argument("--macos-identity", default="Developer ID Application")
    parser.add_argument("--notarize", action="store_true")
    args = parser.parse_args()

    path = Path(args.artifact)
    if not path.exists() and not args.dry_run:
        raise SystemExit(f"Artifact not found: {path}")

    if os.name == "nt":
        sign_windows(path, args.timestamp_url, args.dry_run)
    elif sys.platform == "darwin":
        sign_macos(path, args.macos_identity, args.dry_run, args.notarize)
    else:
        sign_linux(path, args.dry_run)


if __name__ == "__main__":
    main()
