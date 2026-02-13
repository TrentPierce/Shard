"""Global Crash Handler and Auto-Updater for Shard.

This module provides:
- Global exception handling with crash logging
- Auto-restart after crashes
- Auto-update checking from GitHub releases
"""

from __future__ import annotations

import datetime
import json
import logging
import os
import platform
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Callable, Optional

# Try to import httpx for auto-updater
try:
    import httpx
    HTTPX_AVAILABLE = True
except ImportError:
    HTTPX_AVAILABLE = False

LOGGER = logging.getLogger("shard.crash_handler")

# Default paths
def get_data_dir() -> Path:
    """Get the platform-specific data directory for Shard."""
    if sys.platform == "win32":
        base = Path(os.environ.get("APPDATA", Path.home() / "AppData" / "Roaming"))
    elif sys.platform == "darwin":
        base = Path.home() / "Library" / "Application Support"
    else:
        base = Path.home() / ".local" / "share"
    return base / "shard"


class GlobalCrashHandler:
    """Global exception handler with crash logging and auto-restart."""
    
    def __init__(
        self,
        data_dir: Optional[Path] = None,
        restart_delay: float = 3.0,
        on_crash: Optional[Callable[[Exception], None]] = None,
    ):
        self.data_dir = data_dir or get_data_dir()
        self.crash_log_path = self.data_dir / "crash.log"
        self.restart_delay = restart_delay
        self.on_crash = on_crash
        self._original_excepthook = sys.excepthook
        self._original_thread_excepthook = None
        
        # Ensure data directory exists
        self.data_dir.mkdir(parents=True, exist_ok=True)
        
    def _get_crash_report(self, exc_type: type, exc_value: Exception, tb) -> str:
        """Generate a crash report."""
        import traceback
        
        crash_id = f"crash-{int(time.time() * 1000)}"
        
        report = [
            "=" * 60,
            f"Shard Crash Report - {crash_id}",
            "=" * 60,
            f"Timestamp: {datetime.datetime.now().isoformat()}",
            f"Platform: {platform.platform()}",
            f"Python: {platform.python_version()}",
            f"Shard Version: {self._get_version()}",
            "",
            "Exception Type:",
            f"  {exc_type.__name__}: {exc_value}",
            "",
            "Traceback:",
            "".join(traceback.format_exception(exc_type, exc_value, tb)),
            "",
            "Environment:",
        ]
        
        # Add relevant environment variables (sanitized)
        for key in ["PATH", "PYTHONPATH", "SHARD_"]:
            if key.startswith("SHARD_"):
                val = os.getenv(key, "")
                if key == "SHARD_API_KEYS":
                    val = "[REDACTED]" if val else ""
                report.append(f"  {key}={val}")
            else:
                report.append(f"  {key}=(present)" if os.getenv(key) else f"  {key}=(not set)")
        
        report.extend([
            "",
            "=" * 60,
        ])
        
        return "\n".join(report)
    
    def _get_version(self) -> str:
        """Get Shard version."""
        version_file = self.data_dir.parent / "version.txt"
        if version_file.exists():
            return version_file.read_text().strip()
        return "unknown"
    
    def _write_crash_log(self, report: str) -> None:
        """Write crash report to log file."""
        try:
            # Append to crash log
            with open(self.crash_log_path, "a", encoding="utf-8") as f:
                f.write(report + "\n")
        except Exception as e:
            LOGGER.error(f"Failed to write crash log: {e}")
    
    def _handle_exception(self, exc_type: type, exc_value: Exception, tb) -> None:
        """Handle uncaught exception."""
        # Generate crash report
        report = self._get_crash_report(exc_type, exc_value, tb)
        
        # Log to file
        self._write_crash_log(report)
        
        # Log to stderr
        print(f"\n[CRITICAL] Shard encountered a fatal error!", file=sys.stderr)
        print(f"Crash report written to: {self.crash_log_path}", file=sys.stderr)
        print(f"Auto-restarting in {self.restart_delay} seconds...\n", file=sys.stderr)
        
        # Call custom handler if provided
        if self.on_crash:
            try:
                self.on_crash(exc_value)
            except Exception:
                pass
        
        # Schedule restart
        self._schedule_restart()
        
        # Call original excepthook
        if self._original_excepthook:
            try:
                self._original_excepthook(exc_type, exc_value, tb)
            except Exception:
                pass
    
    def _schedule_restart(self) -> None:
        """Schedule application restart."""
        def restart():
            time.sleep(self.restart_delay)
            LOGGER.info("Attempting auto-restart...")
            
            # Get the current executable/script path
            if getattr(sys, 'frozen', False):
                # Running as PyInstaller bundle
                executable = sys.executable
            else:
                # Running as script
                executable = sys.executable
                script = Path(sys.argv[0]).resolve()
            
            try:
                # Restart the application
                if getattr(sys, 'frozen', False):
                    subprocess.Popen([executable])
                else:
                    subprocess.Popen([executable, str(script)])
            except Exception as e:
                LOGGER.error(f"Failed to restart: {e}")
        
        thread = threading.Thread(target=restart, daemon=True)
        thread.start()
    
    def install(self) -> None:
        """Install the global exception handler."""
        sys.excepthook = self._handle_exception
        
        # Also handle thread exceptions if available
        if hasattr(sys, "version_info") and sys.version_info >= (3, 8):
            try:
                import threading
                self._original_thread_excepthook = threading.excepthook
                threading.excepthook = self._handle_exception
            except AttributeError:
                pass
        
        LOGGER.info(f"Global crash handler installed. Crash logs: {self.crash_log_path}")
    
    def uninstall(self) -> None:
        """Uninstall the global exception handler."""
        sys.excepthook = self._original_excepthook
        if self._original_thread_excepthook:
            import threading
            threading.excepthook = self._original_thread_excepthook


class AutoUpdater:
    """Auto-updater that checks GitHub releases for new versions."""
    
    def __init__(
        self,
        repo_owner: str = "TrentPierce",
        repo_name: str = "Shard",
        current_version: Optional[str] = None,
        data_dir: Optional[Path] = None,
    ):
        self.repo_owner = repo_owner
        self.repo_name = repo_name
        self.current_version = current_version or "0.3.0"
        self.data_dir = data_dir or get_data_dir()
        self.version_file = self.data_dir / "version.txt"
        
        self.api_url = f"https://api.github.com/repos/{repo_owner}/{repo_name}/releases/latest"
        
    def _get_local_version(self) -> str:
        """Get the locally stored version."""
        if self.version_file.exists():
            return self.version_file.read_text().strip()
        return self.current_version
    
    def _save_local_version(self, version: str) -> None:
        """Save version to local file."""
        self.version_file.write_text(version)
    
    def _parse_version(self, version_str: str) -> tuple[int, ...]:
        """Parse version string into tuple for comparison."""
        # Remove 'v' prefix if present
        version_str = version_str.lstrip('v')
        try:
            return tuple(int(x) for x in version_str.split('.'))
        except ValueError:
            return (0, 0, 0)
    
    def _compare_versions(self, local: str, remote: str) -> int:
        """Compare versions.
        
        Returns:
            -1 if local < remote
             0 if local == remote
             1 if local > remote
        """
        local_tuple = self._parse_version(local)
        remote_tuple = self._parse_version(remote)
        
        if local_tuple < remote_tuple:
            return -1
        elif local_tuple > remote_tuple:
            return 1
        else:
            return 0
    
    async def check_for_updates(self) -> dict:
        """Check for available updates.
        
        Returns:
            Dict with update info:
            {
                "update_available": bool,
                "current_version": str,
                "latest_version": str,
                "release_url": str,
                "download_url": str,
                "release_notes": str,
            }
        """
        result = {
            "update_available": False,
            "current_version": self._get_local_version(),
            "latest_version": "",
            "release_url": f"https://github.com/{self.repo_owner}/{self.repo_name}/releases",
            "download_url": "",
            "release_notes": "",
        }
        
        if not HTTPX_AVAILABLE:
            LOGGER.warning("httpx not available - cannot check for updates")
            return result
        
        try:
            async with httpx.AsyncClient(timeout=10.0) as client:
                response = await client.get(
                    self.api_url,
                    headers={
                        "Accept": "application/vnd.github+json",
                        "X-GitHub-Api-Version": "2022-11-28",
                    }
                )
                
                if response.status_code != 200:
                    LOGGER.warning(f"GitHub API returned {response.status_code}")
                    return result
                
                data = response.json()
                latest_version = data.get("tag_name", "")
                result["latest_version"] = latest_version
                result["release_url"] = data.get("html_url", result["release_url"])
                result["release_notes"] = data.get("body", "")[:500]  # First 500 chars
                
                # Find Windows executable asset
                assets = data.get("assets", [])
                for asset in assets:
                    name = asset.get("name", "")
                    if name.endswith(".exe") or name.endswith(".msi"):
                        result["download_url"] = asset.get("browser_download_url", "")
                        break
                
                # Check if update is available
                if self._compare_versions(self._get_local_version(), latest_version) < 0:
                    result["update_available"] = True
                    
        except Exception as e:
            LOGGER.warning(f"Failed to check for updates: {e}")
        
        return result
    
    async def download_update(self, download_url: str, progress_callback: Optional[Callable[[float], None]] = None) -> bool:
        """Download the update installer.
        
        Args:
            download_url: URL to download from
            progress_callback: Optional callback for progress updates (0-100)
            
        Returns:
            True if download succeeded, False otherwise
        """
        if not HTTPX_AVAILABLE:
            return False
        
        try:
            update_dir = self.data_dir / "updates"
            update_dir.mkdir(parents=True, exist_ok=True)
            
            # Extract filename from URL
            filename = download_url.split("/")[-1].split("?")[0]
            output_path = update_dir / filename
            
            async with httpx.AsyncClient(timeout=60.0, follow_redirects=True) as client:
                async with client.stream("GET", download_url) as response:
                    if response.status_code != 200:
                        return False
                    
                    total_size = int(response.headers.get("content-length", 0))
                    downloaded = 0
                    
                    with open(output_path, "wb") as f:
                        async for chunk in response.aiter_bytes(chunk_size=8192):
                            f.write(chunk)
                            downloaded += len(chunk)
                            
                            if progress_callback and total_size > 0:
                                progress = (downloaded / total_size) * 100
                                progress_callback(progress)
            
            LOGGER.info(f"Update downloaded to: {output_path}")
            return True
            
        except Exception as e:
            LOGGER.error(f"Failed to download update: {e}")
            return False
    
    async def prompt_update(self) -> bool:
        """Check for updates and prompt user if available.
        
        Returns:
            True if user wants to update, False otherwise
        """
        update_info = await self.check_for_updates()
        
        if not update_info["update_available"]:
            return False
        
        print(f"\n{'='*50}")
        print(f"  ⚡ New Swarm Protocol Available!")
        print(f"{'='*50}")
        print(f"  Current: v{update_info['current_version']}")
        print(f"  Latest:  v{update_info['latest_version']}")
        print(f"{'='*50}")
        print(f"\nRelease Notes:")
        print(f"  {update_info['release_notes'][:200]}...")
        print(f"\nDownload: {update_info['release_url']}")
        
        # In non-interactive mode, just log the update
        if not sys.stdin.isatty():
            LOGGER.info(f"Update available: v{update_info['latest_version']}")
            return False
        
        try:
            response = input("\nDownload and install update? [y/N]: ").strip().lower()
            return response in ("y", "yes")
        except (EOFError, KeyboardInterrupt):
            return False


def install_crash_handler(
    data_dir: Optional[Path] = None,
    restart_delay: float = 3.0,
) -> GlobalCrashHandler:
    """Convenience function to install the global crash handler."""
    handler = GlobalCrashHandler(
        data_dir=data_dir,
        restart_delay=restart_delay,
    )
    handler.install()
    return handler


if __name__ == "__main__":
    # Test the crash handler
    handler = install_crash_handler()
    print(f"Crash handler installed. Crash log: {handler.crash_log_path}")
    
    # Test the auto-updater
    async def test_updater():
        updater = AutoUpdater()
        result = await updater.check_for_updates()
        print(f"\nUpdate check result:")
        for k, v in result.items():
            print(f"  {k}: {v}")
    
    import asyncio
    asyncio.run(test_updater())
