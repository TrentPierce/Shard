#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERSION_FILE = ROOT / "VERSION"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def git_rev_parse(ref: str) -> str:
    return (
        subprocess.check_output(["git", "rev-parse", ref], cwd=ROOT)
        .decode("utf-8")
        .strip()
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate Shard release manifest")
    parser.add_argument("--out", default="dist/release-manifest.json")
    parser.add_argument("--tag", help="Git tag, defaults to v<VERSION>")
    parser.add_argument(
        "--artifact",
        dest="artifacts",
        action="append",
        default=[],
        help="Artifact path (repeatable)",
    )
    args = parser.parse_args()

    version = VERSION_FILE.read_text(encoding="utf-8").strip()
    tag = args.tag or f"v{version}"
    commit = git_rev_parse("HEAD")

    entries = []
    for raw_path in args.artifacts:
        path = (ROOT / raw_path).resolve() if not Path(raw_path).is_absolute() else Path(raw_path)
        if not path.exists():
            raise SystemExit(f"Artifact not found: {path}")
        rel = path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else str(path)
        sig_path = Path(str(path) + ".sig")
        entries.append(
            {
                "path": rel,
                "size_bytes": path.stat().st_size,
                "sha256": sha256_file(path),
                "signature_path": (
                    sig_path.relative_to(ROOT).as_posix()
                    if sig_path.exists() and sig_path.is_relative_to(ROOT)
                    else (str(sig_path) if sig_path.exists() else None)
                ),
                "signed": sig_path.exists(),
            }
        )

    manifest = {
        "version": version,
        "git_tag": tag,
        "git_commit": commit,
        "artifacts": entries,
    }

    # Auto-update Winget manifest if we have a Windows executable
    winget_manifest = ROOT / "installers" / "winget" / "manifest.yaml"
    if winget_manifest.exists():
        for entry in entries:
            if entry["path"].endswith(".exe"):
                text = winget_manifest.read_text(encoding="utf-8")
                text = text.replace("REPLACE_WITH_ACTUAL_SHA256", entry["sha256"])
                winget_manifest.write_text(text, encoding="utf-8")
                break

    # Auto-update Homebrew formula if we have a tar.gz
    brew_formula = ROOT / "installers" / "homebrew" / "Formula" / "shard.rb"
    if brew_formula.exists():
        for entry in entries:
            if entry["path"].endswith(".tar.gz"):
                text = brew_formula.read_text(encoding="utf-8")
                text = text.replace("REPLACE_WITH_ACTUAL_SHA256", entry["sha256"])
                brew_formula.write_text(text, encoding="utf-8")
                break

    out_path = ROOT / args.out
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote release manifest: {out_path}")


if __name__ == "__main__":
    main()

