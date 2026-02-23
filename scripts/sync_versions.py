#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERSION_FILE = ROOT / "VERSION"


def read_version() -> str:
    raw = VERSION_FILE.read_text(encoding="utf-8").strip()
    if not re.fullmatch(r"\d+\.\d+\.\d+", raw):
        raise ValueError(f"Invalid semantic version in VERSION: {raw!r}")
    return raw


def replace_in_file(path: Path, pattern: str, replacement: str) -> None:
    if not path.exists():
        print(f"Warning: {path} not found, skipping version sync.")
        return
    text = path.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, text, flags=re.MULTILINE)
    if count == 0:
        print(f"Warning: No version pattern matched in {path}")
    path.write_text(updated, encoding="utf-8")


def sync_version(version: str) -> None:
    # Rust Workspace
    replace_in_file(
        ROOT / "desktop" / "rust" / "Cargo.toml",
        r'^version = "\d+\.\d+\.\d+"$',
        f'version = "{version}"',
    )
    # Web Package
    replace_in_file(
        ROOT / "web" / "package.json",
        r'"version": "\d+\.\d+\.\d+"',
        f'"version": "{version}"',
    )
    # Tauri Config
    replace_in_file(
        ROOT / "web" / "src-tauri" / "tauri.conf.json",
        r'"version": "\d+\.\d+\.\d+"',
        f'"version": "{version}"',
    )
    # Python SDK
    replace_in_file(
        ROOT / "sdk" / "python" / "pyproject.toml",
        r'^version = "\d+\.\d+\.\d+"$',
        f'version = "{version}"',
    )
    # README badge
    replace_in_file(
        ROOT / "README.md",
        r"version-\d+\.\d+\.\d+",
        f"version-{version}",
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Sync project versions from VERSION file.")
    parser.add_argument("--set", dest="set_version", help="Set VERSION file then sync.")
    args = parser.parse_args()

    if args.set_version:
        if not re.fullmatch(r"\d+\.\d+\.\d+", args.set_version):
            raise SystemExit("Version must follow semver format, e.g. 0.6.0")
        VERSION_FILE.write_text(args.set_version + "\n", encoding="utf-8")

    version = read_version()
    sync_version(version)
    print(f"Synchronized project version: {version}")


if __name__ == "__main__":
    main()
