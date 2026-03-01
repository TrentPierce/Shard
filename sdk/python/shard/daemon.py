import os
import subprocess
import sys
import time
import platform
import logging
from pathlib import Path
from typing import Optional
import httpx
import atexit
import socket

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

def download_file(url: str, dest: Path):
    """Download a file with progress logging."""
    logger.info(f"Downloading {url} to {dest}...")
    try:
        with httpx.stream("GET", url, follow_redirects=True) as response:
            response.raise_for_status()
            with open(dest, "wb") as f:
                for chunk in response.iter_bytes(chunk_size=8192):
                    f.write(chunk)
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
    
    # Check for bundled library first
    package_root = Path(__file__).parent
    lib_name = "shard_engine.dll" if platform.system() == "Windows" else \
               "libshard_engine.dylib" if platform.system() == "Darwin" else \
               "libshard_engine.so"
               
    bundled_lib = package_root / lib_name
    if bundled_lib.exists():
        os.environ["BITNET_LIB"] = str(bundled_lib)
        logger.debug(f"Using bundled engine: {bundled_lib}")

def find_daemon_executable() -> str:
    """Find the bundled shard-daemon executable."""
    package_root = Path(__file__).parent
    exe_name = "shard-daemon.exe" if platform.system() == "Windows" else "shard-daemon"
    
    # Check bundled location (site-packages/shard/bin/)
    bundled_exe = package_root / "bin" / exe_name
    if bundled_exe.exists():
        return str(bundled_exe)
        
    # Fallback to PATH
    import shutil
    if shutil.which("shard-daemon"):
        return "shard-daemon"
        
    # Development fallback
    cwd = Path.cwd()
    dev_path = cwd / "desktop" / "rust" / "target" / "release" / exe_name
    if dev_path.exists():
        return str(dev_path)
        
    raise FileNotFoundError("shard-daemon executable not found. Please ensure the package is installed correctly.")

_daemon_process: Optional[subprocess.Popen] = None

def _is_port_available(host: str, port: int) -> bool:
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        sock.bind((host, port))
    except OSError:
        return False
    finally:
        sock.close()
    return True


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
    
    # Check if already running
    url = f"http://127.0.0.1:{port}/health"
    try:
        httpx.get(url, timeout=0.2)
        logger.debug("Shard daemon already running.")
        return
    except (httpx.ConnectError, httpx.TimeoutException):
        pass

    if _daemon_process and _daemon_process.poll() is None:
        return

    ensure_assets()

    if not _is_port_available("127.0.0.1", port):
        raise RuntimeError(f"Port {port} is already in use. Set a different control port.")

    exe = find_daemon_executable()
    # Add --public-api to help stats visibility if requested, 
    # but here we focus on the local onboarding.
    cmd = [exe, "--control-port", str(port)]
    
    logger.info(f"Launching Shard P2P node...")
    
    creationflags = 0
    if platform.system() == "Windows" and background:
        creationflags = subprocess.CREATE_NO_WINDOW
        
    popen_kwargs = {
        "stdout": subprocess.DEVNULL if background else None,
        "stderr": subprocess.DEVNULL if background else None,
        "creationflags": creationflags,
        "env": os.environ.copy(),
    }
    if platform.system() != "Windows":
        popen_kwargs["start_new_session"] = True

    _daemon_process = subprocess.Popen(cmd, **popen_kwargs)
    
    atexit.register(stop_daemon)
    
    # Wait for health check
    retries = 30
    for i in range(retries):
        try:
            httpx.get(url, timeout=0.2)
            logger.info("Shard node is online and ready.")
            return
        except (httpx.ConnectError, httpx.TimeoutException):
            time.sleep(0.5)
            
    stop_daemon()
    raise RuntimeError("Shard node failed to start or respond to health checks.")
