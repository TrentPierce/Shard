#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import platform
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUILD = ROOT / "build"
DIST = ROOT / "dist"


def run(cmd: list[str], cwd: Path | None = None, allow_fail: bool = False) -> int:
    print("+", " ".join(cmd))
    proc = subprocess.run(cmd, cwd=cwd or ROOT)
    if proc.returncode != 0 and not allow_fail:
        raise SystemExit(proc.returncode)
    return proc.returncode


def ensure_layout() -> None:
    (BUILD / "bin").mkdir(parents=True, exist_ok=True)
    (BUILD / "lib").mkdir(parents=True, exist_ok=True)
    (DIST / "ShardAI" / "_internal").mkdir(parents=True, exist_ok=True)


def build_rust(mock: bool = False) -> Path:
    out = BUILD / "bin" / ("shard-daemon.exe" if platform.system() == "Windows" else "shard-daemon")
    if mock:
        out.write_text("#!/bin/sh\necho shard-daemon mock\n", encoding="utf-8")
        out.chmod(0o755)
        return out

    run(["cargo", "build", "--release"], cwd=ROOT / "desktop" / "rust")
    src = ROOT / "desktop" / "rust" / "target" / "release" / ("shard-oracle-daemon.exe" if platform.system() == "Windows" else "shard-oracle-daemon")
    shutil.copy2(src, out)
    return out


def build_cpp(mock: bool = False) -> Path:
    libname = "shard_engine.dll" if platform.system() == "Windows" else "libshard_engine.so"
    out = BUILD / "lib" / libname
    if mock:
        out.write_bytes(b"mock-shard-engine")
        return out

    bdir = BUILD / "cpp"
    bdir.mkdir(parents=True, exist_ok=True)
    run(["cmake", str(ROOT / "cpp" / "shard-bridge"), "-DCMAKE_BUILD_TYPE=Release"], cwd=bdir)
    run(["cmake", "--build", ".", "--config", "Release"], cwd=bdir)

    candidates = [
        bdir / libname,
        bdir / "Release" / libname,
        bdir / "lib" / libname,
    ]
    src = next((c for c in candidates if c.exists()), None)
    if src is None:
        raise FileNotFoundError(f"Could not locate built library {libname}")
    shutil.copy2(src, out)
    return out


def freeze_python(mock: bool = False) -> Path:
    bundle = DIST / "ShardAI"
    app_name = "shard-api.exe" if platform.system() == "Windows" else "shard-api"
    app = bundle / app_name

    if mock:
        app.write_text("#!/bin/sh\necho shard-api mock\n", encoding="utf-8")
        app.chmod(0o755)
    else:
        run([
            "pyinstaller",
            "--noconfirm",
            "--clean",
            "--name",
            "shard-api",
            "desktop/python/oracle_api.py",
            "--add-data",
            "web/public:web/public",
            "--add-data",
            "web/src:web/src",
        ], cwd=ROOT)
        built = ROOT / "dist" / "shard-api" / app_name
        if not built.exists():
            raise FileNotFoundError(f"PyInstaller output missing: {built}")
        shutil.copy2(built, app)

    return app


def assemble(mock: bool) -> None:
    ensure_layout()
    daemon = build_rust(mock=mock)
    engine = build_cpp(mock=mock)
    app = freeze_python(mock=mock)

    internal = DIST / "ShardAI" / "_internal"
    shutil.copy2(daemon, internal / daemon.name)
    shutil.copy2(engine, internal / engine.name)

    # Keep web assets near packaged app.
    web_dst = internal / "web"
    if web_dst.exists():
        shutil.rmtree(web_dst)
    shutil.copytree(ROOT / "web", web_dst)

    manifest = DIST / "ShardAI" / "release_manifest.txt"
    manifest.write_text(
        "\n".join([
            f"app={app.name}",
            f"daemon={daemon.name}",
            f"engine={engine.name}",
            f"mock_build={mock}",
        ])
        + "\n",
        encoding="utf-8",
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Build Shard release bundle")
    parser.add_argument("--mock", action="store_true", help="Create distributable placeholders without external toolchains")
    args = parser.parse_args()

    assemble(mock=args.mock)
    print(f"Release bundle ready: {DIST / 'ShardAI'}")


if __name__ == "__main__":
    main()
