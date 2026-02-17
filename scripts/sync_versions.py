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
    text = path.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, text, flags=re.MULTILINE)
    if count == 0:
        raise RuntimeError(f"No version pattern matched in {path}")
    path.write_text(updated, encoding="utf-8")


def sync_version(version: str) -> None:
    replace_in_file(
        ROOT / "desktop" / "rust" / "Cargo.toml",
        r'^version = "\d+\.\d+\.\d+"$',
        f'version = "{version}"',
    )
    replace_in_file(
        ROOT / "web" / "package.json",
        r'"version": "\d+\.\d+\.\d+"',
        f'"version": "{version}"',
    )
    replace_in_file(
        ROOT / "pyproject.toml",
        r'^version = "\d+\.\d+\.\d+"$',
        f'version = "{version}"',
    )
    replace_in_file(
        ROOT / "desktop" / "python" / "shard_api.py",
        r'version="\d+\.\d+\.\d+"',
        f'version="{version}"',
    )
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
            raise SystemExit("Version must follow semver format, e.g. 0.4.5")
        VERSION_FILE.write_text(args.set_version + "\n", encoding="utf-8")

    version = read_version()
    sync_version(version)
    print(f"Synchronized project version: {version}")


if __name__ == "__main__":
    main()
