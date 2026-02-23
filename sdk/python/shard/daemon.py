import os
import subprocess
import sys
import time
import signal
import platform
import logging
from pathlib import Path
from typing import Optional, List
import httpx
import atexit

logger = logging.getLogger(__name__)

# Constants
DEFAULT_MODEL_URL = "https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"
DEFAULT_MODEL_FILENAME = "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"

def get_shard_home() -> Path:
    """Get the Shard application data directory."""
    if platform.system() == "Windows":
        base = Path(os.environ.get("APPDATA", "~")).expanduser()
    else:
        base = Path("~/.local/share").expanduser()
    
    home = base / "shard"
    home.mkdir(parents=True, exist_ok=True)
    return home

def get_models_dir() -> Path:
    d = get_shard_home() / "models"
    d.mkdir(parents=True, exist_ok=True)
    return d

def get_lib_dir() -> Path:
    d = get_shard_home() / "lib"
    d.mkdir(parents=True, exist_ok=True)
    return d

def download_file(url: str, dest: Path):
    """Download a file with progress logging."""
    logger.info(f"Downloading {url} to {dest}...")
    try:
        with httpx.stream("GET", url, follow_redirects=True) as response:
            response.raise_for_status()
            total = int(response.headers.get("Content-Length", 0))
            
            with open(dest, "wb") as f:
                downloaded = 0
                for chunk in response.iter_bytes(chunk_size=8192):
                    f.write(chunk)
                    downloaded += len(chunk)
                    # Simple progress log every 10MB
                    if downloaded % (10 * 1024 * 1024) < 8192:
                        logger.info(f"Downloaded {downloaded / 1024 / 1024:.1f} MB")
        logger.info("Download complete.")
    except Exception as e:
        logger.error(f"Failed to download {url}: {e}")
        if dest.exists():
            dest.unlink()
        raise

def ensure_assets():
    """Ensure required model and libraries are present."""
    models_dir = get_models_dir()
    model_path = models_dir / DEFAULT_MODEL_FILENAME
    
    if not model_path.exists():
        logger.info("Default model not found. Downloading...")
        download_file(DEFAULT_MODEL_URL, model_path)
    
    # Set environment variables for the daemon
    os.environ["BITNET_MODEL"] = str(model_path)
    
    # Note: libshard_engine logic is complex due to platform differences.
    # We assume for now it is either in PATH, in lib dir, or bundled.
    # If we had a URL for the lib, we would download it here too.
    lib_dir = get_lib_dir()
    if platform.system() == "Windows":
        lib_name = "shard_engine.dll"
    elif platform.system() == "Darwin":
        lib_name = "libshard_engine.dylib"
    else:
        lib_name = "libshard_engine.so"
        
    lib_path = lib_dir / lib_name
    if lib_path.exists():
         os.environ["BITNET_LIB"] = str(lib_path)

def find_daemon_executable() -> str:
    """Find the shard-daemon executable."""
    import shutil
    if shutil.which("shard-daemon"):
        return "shard-daemon"
        
    # Check sys.prefix/bin (Unix) or sys.prefix/Scripts (Windows)
    bin_dir = Path(sys.prefix) / ("Scripts" if platform.system() == "Windows" else "bin")
    exe = bin_dir / ("shard-daemon.exe" if platform.system() == "Windows" else "shard-daemon")
    if exe.exists():
        return str(exe)

    # Check common build locations
    cwd = Path.cwd()
    release_path = cwd / "desktop" / "rust" / "target" / "release" / "shard-daemon"
    if platform.system() == "Windows":
        release_path = release_path.with_suffix(".exe")
        
    if release_path.exists():
        return str(release_path)
        
    raise FileNotFoundError("shard-daemon executable not found. Please install the package or build the daemon.")

_daemon_process: Optional[subprocess.Popen] = None

def stop_daemon():
    """Stop the background daemon process."""
    global _daemon_process
    if _daemon_process:
        logger.info("Stopping Shard daemon...")
        _daemon_process.terminate()
        try:
            _daemon_process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            _daemon_process.kill()
        _daemon_process = None

def start_daemon(port: int = 9091, background: bool = True):
    """Start the Shard daemon in the background."""
    global _daemon_process
    
    # Check if already running on the port
    url = f"http://127.0.0.1:{port}/health"
    try:
        httpx.get(url, timeout=0.5)
        logger.info("Daemon already running.")
        return
    except (httpx.ConnectError, httpx.TimeoutException):
        pass

    if _daemon_process and _daemon_process.poll() is None:
        logger.info("Daemon process is already tracked.")
        return

    ensure_assets()
    
    exe = find_daemon_executable()
    cmd = [exe, "--control-port", str(port)]
    
    logger.info(f"Starting daemon: {' '.join(cmd)}")
    
    # On Windows, use creationflags to hide the console window if desired
    creationflags = 0
    if platform.system() == "Windows" and background:
        creationflags = subprocess.CREATE_NO_WINDOW
        
    _daemon_process = subprocess.Popen(
        cmd,
        stdout=subprocess.DEVNULL if background else None,
        stderr=subprocess.DEVNULL if background else None,
        creationflags=creationflags
    )
    
    atexit.register(stop_daemon)
    
    # Wait for health check
    retries = 20
    url = f"http://127.0.0.1:{port}/health"
    for i in range(retries):
        try:
            httpx.get(url, timeout=0.5)
            logger.info("Daemon is ready.")
            return
        except (httpx.ConnectError, httpx.TimeoutException):
            time.sleep(0.5)
            
    # If we get here, daemon failed to start
    stop_daemon()
    raise RuntimeError("Failed to start Shard daemon (timeout waiting for health check)")

