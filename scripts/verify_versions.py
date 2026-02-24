#!/usr/bin/env python3
from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERSION_FILE = ROOT / "VERSION"


@dataclass(frozen=True)
class VersionCheck:
    path: Path
    pattern: str
    description: str


def read_version() -> str:
    version = VERSION_FILE.read_text(encoding="utf-8").strip()
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise SystemExit(f"Invalid VERSION value: {version!r}")
    return version


def collect_checks(version: str) -> list[VersionCheck]:
    checks = [
        VersionCheck(ROOT / "desktop" / "rust" / "daemon" / "Cargo.toml", rf'^version = "{re.escape(version)}"$', "daemon crate version"),
        VersionCheck(ROOT / "web" / "package.json", rf'"version": "{re.escape(version)}"', "web package version"),
        VersionCheck(ROOT / "web" / "src-tauri" / "tauri.conf.json", rf'"version": "{re.escape(version)}"', "tauri app version"),
        VersionCheck(ROOT / "web" / "src" / "lib" / "version.ts", rf'export const SHARD_VERSION = "{re.escape(version)}"', "web runtime version"),
        VersionCheck(ROOT / "sdk" / "python" / "pyproject.toml", rf'^version = "{re.escape(version)}"$', "python sdk version"),
        VersionCheck(ROOT / "sdk" / "python" / "shard" / "__init__.py", rf'__version__ = "{re.escape(version)}"', "python runtime sdk version"),
        VersionCheck(ROOT / "sdk" / "node" / "package.json", rf'"version": "{re.escape(version)}"', "node sdk version"),
        VersionCheck(ROOT / "sdk" / "node" / "src" / "client.ts", rf"const SDK_VERSION = '{re.escape(version)}'", "node runtime sdk version"),
        VersionCheck(ROOT / "sdk" / "widget" / "package.json", rf'"version": "{re.escape(version)}"', "widget sdk version"),
        VersionCheck(ROOT / "installers" / "homebrew" / "Formula" / "shard.rb", rf'version "{re.escape(version)}"', "homebrew formula version"),
        VersionCheck(ROOT / "installers" / "winget" / "manifest.yaml", rf"^Version:\s*{re.escape(version)}$", "winget manifest version"),
        VersionCheck(ROOT / "installers" / "winget" / "manifest.yaml", rf"releases/download/v{re.escape(version)}/shard-{re.escape(version)}-windows-x64\.exe", "winget installer URL version"),
        VersionCheck(ROOT / "installers" / "windows" / "installer.iss", rf'#define MyAppVersion "{re.escape(version)}"', "inno installer version"),
        VersionCheck(ROOT / "installers" / "windows" / "shard.nsi", rf'!define VERSION "{re.escape(version)}"', "nsis installer version"),
    ]
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
        checks.append(
            VersionCheck(
                ROOT / "desktop" / "rust" / "crates" / crate / "Cargo.toml",
                rf'^version = "{re.escape(version)}"$',
                f"{crate} crate version",
            )
        )
    return checks


def main() -> None:
    version = read_version()
    failures: list[str] = []

    for check in collect_checks(version):
        if not check.path.exists():
            failures.append(f"{check.path}: missing file for {check.description}")
            continue
        text = check.path.read_text(encoding="utf-8")
        if re.search(check.pattern, text, flags=re.MULTILINE) is None:
            failures.append(f"{check.path}: expected {check.description} to match {check.pattern!r}")

    if failures:
        print("Version verification failed:")
        for failure in failures:
            print(f" - {failure}")
        raise SystemExit(1)

    print(f"Version verification passed for {version}")


if __name__ == "__main__":
    main()
