#!/usr/bin/env python3
from __future__ import annotations

import json
import re
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERSION_FILE = ROOT / "VERSION"
MODEL_MANIFEST = ROOT / "deploy" / "models" / "manifest.json"


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


def validate_model_manifest() -> list[str]:
    if not MODEL_MANIFEST.exists():
        return [f"{MODEL_MANIFEST}: file not found"]

    try:
        manifest = json.loads(MODEL_MANIFEST.read_text(encoding="utf-8"))
    except Exception as exc:  # pragma: no cover - defensive
        return [f"{MODEL_MANIFEST}: invalid JSON ({exc})"]

    models = manifest.get("models")
    if not isinstance(models, list) or not models:
        return [f"{MODEL_MANIFEST}: expected a non-empty 'models' array"]

    failures: list[str] = []
    for idx, model in enumerate(models):
        if not isinstance(model, dict):
            failures.append(f"{MODEL_MANIFEST}: models[{idx}] must be an object")
            continue

        download_url = str(model.get("download_url", "")).strip()
        sha256 = str(model.get("sha256", "")).strip()
        size_bytes = model.get("size_bytes")

        if (
            not download_url
            or download_url.startswith("REPLACE_")
            or re.fullmatch(r"https?://.+", download_url) is None
        ):
            failures.append(
                f"{MODEL_MANIFEST}: models[{idx}].download_url must be a real http(s) URL"
            )

        if re.fullmatch(r"[0-9a-fA-F]{64}", sha256) is None:
            failures.append(
                f"{MODEL_MANIFEST}: models[{idx}].sha256 must be a real 64-char hex digest"
            )

        if not isinstance(size_bytes, int) or size_bytes <= 0:
            failures.append(
                f"{MODEL_MANIFEST}: models[{idx}].size_bytes must be a positive integer"
            )

    return failures


def main() -> None:
    version = read_version()
    failures: list[str] = []

    for check in collect_checks(version):
        if not check.path.exists():
            print(f"Warning: {check.path} not found, skipping version check.")
            continue
        text = check.path.read_text(encoding="utf-8")
        if re.search(check.pattern, text, flags=re.MULTILINE) is None:
            failures.append(f"{check.path}: expected {check.description} to match {check.pattern!r}")

    failures.extend(validate_model_manifest())

    if failures:
        print("Version verification failed:")
        for failure in failures:
            print(f" - {failure}")
        raise SystemExit(1)

    print(f"Version verification passed for {version}")


if __name__ == "__main__":
    main()
