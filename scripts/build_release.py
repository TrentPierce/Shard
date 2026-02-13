#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BUILD = ROOT / "build"
DIST = ROOT / "dist"


def run(cmd: list[str], cwd: Path | None = None) -> None:
    print("+", " ".join(cmd))
    subprocess.run(cmd, cwd=str(cwd) if cwd else None, check=True)


def build_rust() -> Path:
    rust_dir = ROOT / "desktop" / "rust"
    run(["cargo", "build", "--release"], cwd=rust_dir)
    src = rust_dir / "target" / "release" / ("shard-daemon.exe" if os.name == "nt" else "shard-daemon")
    out = BUILD / "bin"
    out.mkdir(parents=True, exist_ok=True)
    dst = out / ("shard-daemon.exe" if os.name == "nt" else "shard-daemon")
    shutil.copy2(src, dst)
    return dst


def build_cpp() -> Path:
    cpp_dir = ROOT / "cpp" / "shard-bridge"
    cmake_build = BUILD / "cpp"
    cmake_build.mkdir(parents=True, exist_ok=True)
    run(["cmake", "-S", str(cpp_dir), "-B", str(cmake_build), "-DCMAKE_BUILD_TYPE=Release"])
    run(["cmake", "--build", str(cmake_build), "--config", "Release"])

    lib_name = "shard_engine.dll" if os.name == "nt" else ("libshard_engine.dylib" if sys.platform == "darwin" else "libshard_engine.so")
    candidate = list(cmake_build.rglob(lib_name))
    if not candidate:
        raise FileNotFoundError(f"built library {lib_name} not found under {cmake_build}")

    out = BUILD / "lib"
    out.mkdir(parents=True, exist_ok=True)
    dst = out / lib_name
    shutil.copy2(candidate[0], dst)
    return dst


def freeze_python(daemon_bin: Path, engine_lib: Path) -> Path:
    api_entry = ROOT / "desktop" / "python" / "run.py"
    pyinstaller = shutil.which("pyinstaller")
    if not pyinstaller:
        raise RuntimeError("pyinstaller is required for release build")

    one_dir = BUILD / "pyinstaller"
    one_dir.mkdir(parents=True, exist_ok=True)

    data_sep = ";" if os.name == "nt" else ":"
    python_src = ROOT / "desktop" / "python"
    add_data = [
        f"{daemon_bin}{data_sep}_internal/bin",
        f"{engine_lib}{data_sep}_internal/lib",
        f"{ROOT / 'web' / 'public'}{data_sep}_internal/web/public",
        f"{ROOT / 'web' / 'src'}{data_sep}_internal/web/src",
        f"{python_src / 'bitnet'}{data_sep}bitnet",
    ]

    cmd = [
        pyinstaller,
        "--name",
        "ShardAI",
        "--onedir",
        "--clean",
        "--distpath",
        str(DIST),
        "--workpath",
        str(one_dir),
        "--specpath",
        str(BUILD),
        "--hidden-import", "oracle_api",
        "--hidden-import", "inference",
        "--hidden-import", "bitnet",
        "--hidden-import", "bitnet.ctypes_bridge",
        "--paths", str(python_src),
    ]

    for item in add_data:
        cmd.extend(["--add-data", item])

    cmd.append(str(api_entry))
    run(cmd)
    
    # Copy installer script to distribution
    frozen = DIST / "ShardAI"
    installer_script = ROOT / "scripts" / "install.bat"
    if installer_script.exists():
        shutil.copy2(installer_script, frozen / "install.bat")
    
    return frozen


def write_manifest(target: Path, daemon_bin: Path, engine_lib: Path) -> None:
    manifest = target / "RELEASE_MANIFEST.txt"
    manifest.write_text(
        "\n".join(
            [
                "Shard Release Bundle",
                f"Platform: {platform.platform()}",
                f"Daemon: {daemon_bin.name}",
                f"Engine: {engine_lib.name}",
                "Includes web assets under _internal/web/",
            ]
        ),
        encoding="utf-8",
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Build Shard production bundle")
    parser.add_argument("--skip-freeze", action="store_true", help="skip pyinstaller step")
    args = parser.parse_args()

    BUILD.mkdir(parents=True, exist_ok=True)
    DIST.mkdir(parents=True, exist_ok=True)

    daemon_bin = build_rust()
    engine_lib = build_cpp()

    if args.skip_freeze:
        raw = DIST / "raw"
        (raw / "_internal" / "bin").mkdir(parents=True, exist_ok=True)
        (raw / "_internal" / "lib").mkdir(parents=True, exist_ok=True)
        shutil.copy2(daemon_bin, raw / "_internal" / "bin" / daemon_bin.name)
        shutil.copy2(engine_lib, raw / "_internal" / "lib" / engine_lib.name)
        write_manifest(raw, daemon_bin, engine_lib)
        print(f"raw bundle created at {raw}")
        return

    frozen = freeze_python(daemon_bin, engine_lib)
    write_manifest(frozen, daemon_bin, engine_lib)
    print(f"release bundle created at {frozen}")


if __name__ == "__main__":
    main()
