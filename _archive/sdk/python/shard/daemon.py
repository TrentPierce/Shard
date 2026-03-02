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
import signal
import queue
import threading
import re
import blake3

logger = logging.getLogger(__name__)

# Constants
DEFAULT_MODEL_URL = os.environ.get("SHARD_MODEL_URL")
DEFAULT_MODEL_FILENAME = os.environ.get("SHARD_MODEL_FILENAME")
DEFAULT_MODEL_BLAKE3 = os.environ.get("SHARD_MODEL_BLAKE3")
DEFAULT_MODEL_BLAKE3_URL = os.environ.get("SHARD_MODEL_BLAKE3_URL")

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

def _compute_blake3(path: Path) -> str:
    hasher = blake3.blake3()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def _fetch_expected_blake3(url: str) -> Optional[str]:
    if DEFAULT_MODEL_BLAKE3:
        return DEFAULT_MODEL_BLAKE3.strip()
    if DEFAULT_MODEL_BLAKE3_URL:
        try:
            resp = httpx.get(DEFAULT_MODEL_BLAKE3_URL, timeout=10.0)
            resp.raise_for_status()
            return resp.text.strip().split()[0]
        except Exception as e:
            logger.error(f"Failed to fetch BLAKE3 checksum: {e}")
            return None
    if url:
        guessed_url = f"{url}.blake3"
        try:
            resp = httpx.get(guessed_url, timeout=10.0)
            resp.raise_for_status()
            return resp.text.strip().split()[0]
        except Exception:
            return None
    return None


def _verify_blake3(path: Path, expected: str):
    actual = _compute_blake3(path)
    if actual.lower() != expected.lower():
        raise RuntimeError(f"BLAKE3 checksum mismatch for {path.name}")


def download_file(url: str, dest: Path, expected_blake3: Optional[str]):
    """Download a file with progress logging."""
    logger.info(f"Downloading {url} to {dest}...")
    try:
        with httpx.stream("GET", url, follow_redirects=True) as response:
            response.raise_for_status()
            with open(dest, "wb") as f:
                for chunk in response.iter_bytes(chunk_size=8192):
                    f.write(chunk)
        if expected_blake3:
            _verify_blake3(dest, expected_blake3)
        logger.info("Download complete.")
    except Exception as e:
        logger.error(f"Failed to download {url}: {e}")
        if dest.exists():
            dest.unlink()
        raise

def ensure_assets():
    """Ensure required model and libraries are present."""
    models_dir = get_models_dir()
    env_model = os.environ.get("BITNET_MODEL")
    model_path = Path(env_model).expanduser() if env_model else None

    if model_path is None:
        if not DEFAULT_MODEL_URL or not DEFAULT_MODEL_FILENAME:
            raise RuntimeError(
                "No BITNET_MODEL set and no SHARD_MODEL_URL/SHARD_MODEL_FILENAME provided."
            )
        model_path = models_dir / DEFAULT_MODEL_FILENAME

    expected_blake3 = _fetch_expected_blake3(DEFAULT_MODEL_URL or "")
    if expected_blake3 is None:
        raise RuntimeError(
            "Missing model BLAKE3 checksum. Set SHARD_MODEL_BLAKE3 or SHARD_MODEL_BLAKE3_URL."
        )

    if not model_path.exists():
        if DEFAULT_MODEL_URL:
            logger.info("Default model not found. Downloading...")
            download_file(DEFAULT_MODEL_URL, model_path, expected_blake3)
        else:
            raise RuntimeError(f"BITNET_MODEL not found at {model_path}")
    else:
        try:
            _verify_blake3(model_path, expected_blake3)
        except RuntimeError:
            if DEFAULT_MODEL_URL:
                logger.warning("Model checksum mismatch; re-downloading.")
                model_path.unlink(missing_ok=True)
                download_file(DEFAULT_MODEL_URL, model_path, expected_blake3)
            else:
                raise

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


def _start_output_reader(proc: subprocess.Popen) -> "queue.Queue[str]":
    q: "queue.Queue[str]" = queue.Queue()

    def _reader():
        if proc.stdout is None:
            return
        for line in proc.stdout:
            q.put(line)

    t = threading.Thread(target=_reader, daemon=True)
    t.start()
    return q


def _wait_for_control_port(output_queue: "queue.Queue[str]", timeout: float = 10.0) -> Optional[int]:
    pattern = re.compile(r"control_port=(\d+)")
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            line = output_queue.get(timeout=0.2)
        except queue.Empty:
            continue
        match = pattern.search(line)
        if match:
            return int(match.group(1))
    return None


def stop_daemon():
    """Stop the background daemon process."""
    global _daemon_process
    if _daemon_process:
        logger.info("Stopping Shard daemon...")
        if platform.system() == "Windows":
            try:
                _daemon_process.send_signal(signal.CTRL_BREAK_EVENT)
            except Exception:
                _daemon_process.terminate()
        else:
            _daemon_process.terminate()
        try:
            _daemon_process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            _daemon_process.kill()
        _daemon_process = None

def start_daemon(port: int = 9091, background: bool = True) -> int:
    """Start the Shard daemon in the background."""
    global _daemon_process
    
    if port != 0:
        # Check if already running
        url = f"http://127.0.0.1:{port}/health"
        try:
            httpx.get(url, timeout=0.2)
            logger.debug("Shard daemon already running.")
            return port
        except (httpx.ConnectError, httpx.TimeoutException):
            pass

    if _daemon_process and _daemon_process.poll() is None:
        return port

    ensure_assets()

    if port != 0 and not _is_port_available("127.0.0.1", port):
        raise RuntimeError(f"Port {port} is already in use. Set a different control port.")

    exe = find_daemon_executable()
    # Add --public-api to help stats visibility if requested, 
    # but here we focus on the local onboarding.
    cmd = [exe, "--control-port", str(port)]
    
    logger.info(f"Launching Shard P2P node...")
    
    creationflags = 0
    if platform.system() == "Windows":
        creationflags = subprocess.CREATE_NEW_PROCESS_GROUP
        if background:
            creationflags |= subprocess.CREATE_NO_WINDOW
        
    popen_kwargs = {
        "stdout": subprocess.DEVNULL if background else None,
        "stderr": subprocess.DEVNULL if background else None,
        "creationflags": creationflags,
        "env": os.environ.copy(),
    }
    if port == 0:
        popen_kwargs["stdout"] = subprocess.PIPE
        popen_kwargs["stderr"] = subprocess.STDOUT
        popen_kwargs["text"] = True
        popen_kwargs["bufsize"] = 1
    if platform.system() != "Windows":
        popen_kwargs["start_new_session"] = True

    _daemon_process = subprocess.Popen(cmd, **popen_kwargs)
    output_queue = _start_output_reader(_daemon_process) if port == 0 else None
    
    atexit.register(stop_daemon)

    if port == 0:
        detected = _wait_for_control_port(output_queue)
        if detected is None:
            stop_daemon()
            raise RuntimeError("Failed to detect daemon control port from stdout.")
        port = detected
        os.environ["SHARD_CONTROL_PORT"] = str(port)
    
    # Wait for health check
    retries = 30
    for i in range(retries):
        try:
            httpx.get(f"http://127.0.0.1:{port}/health", timeout=0.2)
            logger.info("Shard node is online and ready.")
            return port
        except (httpx.ConnectError, httpx.TimeoutException):
            time.sleep(0.5)
            
    stop_daemon()
    raise RuntimeError("Shard node failed to start or respond to health checks.")
