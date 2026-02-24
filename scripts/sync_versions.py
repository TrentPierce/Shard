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
    for crate in [
        "shard-common",
        "shard-crypto",
        "shard-gateway",
        "shard-ledger",
        "shard-metrics",
        "shard-network",
        "shard-scheduler",
        "shard-verifier",
    ]:
        replace_in_file(
            ROOT / "desktop" / "rust" / "crates" / crate / "Cargo.toml",
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
    replace_in_file(
        ROOT / "web" / "src" / "lib" / "version.ts",
        r'export const SHARD_VERSION = "\d+\.\d+\.\d+"',
        f'export const SHARD_VERSION = "{version}"',
    )
    # Python SDK
    replace_in_file(
        ROOT / "sdk" / "python" / "pyproject.toml",
        r'^version = "\d+\.\d+\.\d+"$',
        f'version = "{version}"',
    )
    replace_in_file(
        ROOT / "sdk" / "python" / "shard" / "__init__.py",
        r'__version__ = "\d+\.\d+\.\d+"',
        f'__version__ = "{version}"',
    )
    # Node SDK
    replace_in_file(
        ROOT / "sdk" / "node" / "package.json",
        r'"version": "\d+\.\d+\.\d+"',
        f'"version": "{version}"',
    )
    replace_in_file(
        ROOT / "sdk" / "node" / "src" / "client.ts",
        r"const SDK_VERSION = '\d+\.\d+\.\d+'",
        f"const SDK_VERSION = '{version}'",
    )
    # Widget SDK
    replace_in_file(
        ROOT / "sdk" / "widget" / "package.json",
        r'"version": "\d+\.\d+\.\d+"',
        f'"version": "{version}"',
    )
    # Homebrew formula
    replace_in_file(
        ROOT / "installers" / "homebrew" / "Formula" / "shard.rb",
        r'version "\d+\.\d+\.\d+"',
        f'version "{version}"',
    )
    # Winget manifest
    replace_in_file(
        ROOT / "installers" / "winget" / "manifest.yaml",
        r"^Version:\s*\d+\.\d+\.\d+$",
        f"Version: {version}",
    )
    replace_in_file(
        ROOT / "installers" / "winget" / "manifest.yaml",
        r"releases/download/v\d+\.\d+\.\d+/shard-\d+\.\d+\.\d+-windows-x64\.exe",
        f"releases/download/v{version}/shard-{version}-windows-x64.exe",
    )
    replace_in_file(
        ROOT / "installers" / "windows" / "installer.iss",
        r'#define MyAppVersion "\d+\.\d+\.\d+"',
        f'#define MyAppVersion "{version}"',
    )
    replace_in_file(
        ROOT / "installers" / "windows" / "shard.nsi",
        r'!define VERSION "\d+\.\d+\.\d+"',
        f'!define VERSION "{version}"',
    )
    replace_in_file(
        ROOT / "installers" / "windows" / "install.bat",
        r'set "VERSION=\d+\.\d+\.\d+"',
        f'set "VERSION={version}"',
    )
    # README badge
    replace_in_file(
        ROOT / "README.md",
        r"version-\d+\.\d+\.\d+",
        f"version-{version}",
    )
    replace_in_file(
        ROOT / "README.md",
        r"releases/tag/v\d+\.\d+\.\d+",
        f"releases/tag/v{version}",
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
