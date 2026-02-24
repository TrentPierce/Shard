#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description="Verify release manifest artifact checksums")
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--require-signatures", action="store_true")
    args = parser.parse_args()

    manifest_path = ROOT / args.manifest
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    failures: list[str] = []

    for artifact in manifest.get("artifacts", []):
        path = ROOT / artifact["path"]
        if not path.exists():
            failures.append(f"missing artifact: {artifact['path']}")
            continue
        digest = sha256_file(path)
        if digest != artifact["sha256"]:
            failures.append(f"sha256 mismatch: {artifact['path']}")
        if args.require_signatures:
            sig = artifact.get("signature_path")
            if not sig:
                failures.append(f"missing signature path: {artifact['path']}")
            elif not (ROOT / sig).exists():
                failures.append(f"missing signature file: {sig}")

    if failures:
        print("Release manifest verification failed:")
        for failure in failures:
            print(f" - {failure}")
        raise SystemExit(1)

    print("Release manifest verification passed.")


if __name__ == "__main__":
    main()

