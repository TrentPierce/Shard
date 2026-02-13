"""Resource Fetcher - First-run model downloader with tkinter splash screen.

This module handles:
- Checking for required model files on startup
- Displaying a progress dialog during download
- Verifying SHA256 checksum after download
- Cleaning up and retrying on checksum failure
"""

from __future__ import annotations

import hashlib
import os
import sys
import threading
import time
from pathlib import Path
from typing import Callable, Optional

# Try to import tkinter - may not be available in all environments
try:
    import tkinter as tk
    from tkinter import ttk
    TKINTER_AVAILABLE = True
except ImportError:
    TKINTER_AVAILABLE = False
    # Create dummy classes for non-tkinter environments
    class tk:
        class Tk:
            pass
        class StringVar:
            def __init__(self, *args, **kwargs): pass
            def set(self, *args, **kwargs): pass
    class ttk:
        class Progressbar:
            def __init__(self, *args, **kwargs): pass
            def configure(self, *args, **kwargs): pass

# Default model configuration
DEFAULT_MODEL_URL = "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-Q4_K_M-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf"
FALLBACK_MODEL_URLS = [
    "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-Q4_K_M-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf",
    "https://huggingface.co/TheBloke/Llama-3.2-1B-Instruct-Q4_K_M-GGUF/resolve/main/llama-3.2-1b-instruct-q4_k_m.gguf",
]

# Expected SHA256 checksums (to be updated with actual model checksums)
MODEL_CHECKSUMS: dict[str, str] = {
    "Llama-3.2-1B-Instruct-Q4_K_M.gguf": "e2c8e4e3f5a7c9d1b4f6e8a2c3d5e7f9a1b3c5d7e9f1a3b5c7d9e1f3a5b7c9d",  # Placeholder
}


class ResourceFetcher:
    """Handles model resource checking and download with UI progress."""
    
    def __init__(
        self,
        models_dir: Optional[Path] = None,
        model_filename: str = "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
        expected_checksum: Optional[str] = None,
    ):
        self.models_dir = models_dir or self._get_default_models_dir()
        self.model_filename = model_filename
        self.model_path = self.models_dir / model_filename
        self.expected_checksum = expected_checksum or MODEL_CHECKSUMS.get(model_filename, "")
        
        # UI state
        self._root: Optional[tk.Tk] = None
        self._progress_var: Optional[tk.StringVar] = None
        self._progress_bar: Optional[ttk.Progressbar] = None
        self._status_var: Optional[tk.StringVar] = None
        self._cancel_flag = threading.Event()
        self._download_complete = threading.Event()
        self._download_error: Optional[str] = None
        
    def _get_default_models_dir(self) -> Path:
        """Get default models directory."""
        # Check environment variable first
        env_model_path = os.getenv("SHARD_MODEL_DIR")
        if env_model_path:
            return Path(env_model_path)
        
        # Default locations
        if sys.platform == "win32":
            base = Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData" / "Local"))
        elif sys.platform == "darwin":
            base = Path.home() / "Library" / "Application Support"
        else:
            base = Path.home() / ".local" / "share"
        
        return base / "shard" / "models"
    
    def check_model_exists(self) -> bool:
        """Check if the model file already exists."""
        return self.model_path.exists()
    
    def verify_checksum(self, file_path: Path) -> bool:
        """Verify SHA256 checksum of a file."""
        if not self.expected_checksum:
            return True  # Skip if no checksum configured
            
        sha256_hash = hashlib.sha256()
        try:
            with open(file_path, "rb") as f:
                for byte_block in iter(lambda: f.read(4096), b""):
                    sha256_hash.update(byte_block)
            computed = sha256_hash.hexdigest()
            return computed.lower() == self.expected_checksum.lower()
        except Exception:
            return False
    
    def _create_splash_window(self) -> None:
        """Create the tkinter splash window."""
        if not TKINTER_AVAILABLE:
            return
            
        self._root = tk.Tk()
        self._root.title("Shard - Downloading Model")
        self._root.geometry("450x180")
        self._root.resizable(False, False)
        self._root.attributes("-topmost", True)
        
        # Center the window
        self._root.update_idletasks()
        x = (self._root.winfo_screenwidth() // 2) - (450 // 2)
        y = (self._root.winfo_screenheight() // 2) - (180 // 2)
        self._root.geometry(f"450x180+{x}+{y}")
        
        # Style
        style = ttk.Style()
        style.theme_use("clam")
        
        # Main frame
        main_frame = ttk.Frame(self._root, padding="20")
        main_frame.pack(fill=tk.BOTH, expand=True)
        
        # Title
        title_label = ttk.Label(
            main_frame,
            text="Shard Network",
            font=("Segoe UI", 16, "bold"),
        )
        title_label.pack(pady=(0, 5))
        
        subtitle_label = ttk.Label(
            main_frame,
            text="Initializing Swarm Protocol...",
            font=("Segoe UI", 10),
        )
        subtitle_label.pack(pady=(0, 15))
        
        # Progress variable
        self._progress_var = tk.StringVar(value="0%")
        self._status_var = tk.StringVar(value="Preparing download...")
        
        # Progress bar
        self._progress_bar = ttk.Progressbar(
            main_frame,
            mode="determinate",
            length=350,
        )
        self._progress_bar.pack(pady=(10, 5))
        
        # Progress label
        progress_label = ttk.Label(
            main_frame,
            textvariable=self._progress_var,
            font=("Segoe UI", 9),
        )
        progress_label.pack()
        
        # Status label
        status_label = ttk.Label(
            main_frame,
            textvariable=self._status_var,
            font=("Segoe UI", 8),
            foreground="gray",
        )
        status_label.pack(pady=(5, 0))
        
        # Cancel button
        cancel_btn = ttk.Button(
            main_frame,
            text="Cancel",
            command=self._cancel_download,
        )
        cancel_btn.pack(pady=(10, 0))
        
    def _update_progress(self, percent: float, status: str) -> None:
        """Update progress bar and status."""
        if not TKINTER_AVAILABLE or not self._root:
            return
            
        def _update():
            try:
                self._progress_bar.configure(value=percent)
                self._progress_var.set(f"{percent:.1f}%")
                self._status_var.set(status)
                self._root.update()
            except Exception:
                pass
                
        if threading.current_thread() == threading.main_thread():
            _update()
        else:
            self._root.after(0, _update)
    
    def _cancel_download(self) -> None:
        """Handle cancel button press."""
        self._cancel_flag.set()
        self._status_var.set("Download cancelled")
    
    def _download_with_progress(self, url: str) -> bool:
        """Download file with progress tracking."""
        try:
            import urllib.request
            import urllib.error
        except ImportError:
            import urllib.request
            import urllib.error
        
        self._models_dir.mkdir(parents=True, exist_ok=True)
        
        # Get file size
        try:
            with urllib.request.urlopen(url) as response:
                total_size = int(response.headers.get("content-length", 0))
        except Exception:
            total_size = 0
        
        downloaded = 0
        chunk_size = 8192
        
        try:
            # Use a temporary file for download
            temp_path = self.model_path.with_suffix(".tmp")
            
            def report_progress(block_num: int, block_size: int, total: int):
                if self._cancel_flag.is_set():
                    raise Exception("Download cancelled")
                    
                nonlocal downloaded
                downloaded += block_size
                
                if total > 0:
                    percent = (downloaded / total) * 100
                    self._update_progress(percent, f"Downloading... {downloaded // (1024*1024)}MB / {total // (1024*1024)}MB")
            
            # Download the file
            urllib.request.urlretrieve(
                url,
                temp_path,
                reporthook=report_progress if total_size > 0 else None
            )
            
            # Verify checksum
            self._update_progress(100, "Verifying checksum...")
            if not self.verify_checksum(temp_path):
                temp_path.unlink()
                self._download_error = "Checksum verification failed. The downloaded file may be corrupted."
                return False
            
            # Move to final location
            temp_path.rename(self.model_path)
            self._download_complete.set()
            self._update_progress(100, "Download complete!")
            return True
            
        except Exception as e:
            self._download_error = str(e)
            return False
    
    def download_model(
        self,
        url: Optional[str] = None,
        fallback_urls: Optional[list[str]] = None,
    ) -> bool:
        """Download the model file with progress UI.
        
        Args:
            url: Primary download URL
            fallback_urls: List of fallback URLs if primary fails
            
        Returns:
            True if download succeeded, False otherwise
        """
        if self.check_model_exists():
            if self.verify_checksum(self.model_path):
                return True
            else:
                # Delete corrupted file
                self.model_path.unlink()
        
        urls = [url] if url else [DEFAULT_MODEL_URL]
        if fallback_urls:
            urls.extend(fallback_urls)
        elif not url:
            urls.extend(FALLBACK_MODEL_URLS)
        
        # Remove duplicates while preserving order
        seen = set()
        unique_urls = []
        for u in urls:
            if u not in seen:
                seen.add(u)
                unique_urls.append(u)
        
        # Create splash window in main thread
        if TKINTER_AVAILABLE:
            self._create_splash_window()
        
        # Download in background thread
        def download_thread():
            for attempt_url in unique_urls:
                if self._cancel_flag.is_set():
                    break
                    
                self._update_progress(0, f"Attempting download from {attempt_url[:50]}...")
                
                if self._download_with_progress(attempt_url):
                    break
                    
                time.sleep(1)  # Brief delay before retry
            
            # Close window after short delay
            if TKINTER_AVAILABLE and self._root:
                self._root.after(1500, self._root.destroy)
        
        if TKINTER_AVAILABLE and self._root:
            thread = threading.Thread(target=download_thread, daemon=True)
            thread.start()
            self._root.mainloop()
        else:
            # No GUI available - run synchronously
            for attempt_url in unique_urls:
                if self._cancel_flag.is_set():
                    break
                print(f"Downloading model from {attempt_url}...")
                if self._download_with_progress(attempt_url):
                    return True
            return False
        
        return self._download_complete.is_set()
    
    def get_model_path(self) -> Optional[Path]:
        """Get the path to the model file."""
        if self.check_model_exists():
            return self.model_path
        return None


def check_and_fetch_model(
    model_filename: str = "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
    models_dir: Optional[Path] = None,
) -> Optional[Path]:
    """Convenience function to check and fetch model if needed.
    
    This should be called on first run to ensure model is available.
    
    Args:
        model_filename: Name of the model file
        models_dir: Optional custom models directory
        
    Returns:
        Path to model if available, None otherwise
    """
    fetcher = ResourceFetcher(
        models_dir=models_dir,
        model_filename=model_filename,
    )
    
    if fetcher.check_model_exists() and fetcher.verify_checksum(fetcher.model_path):
        return fetcher.model_path
    
    # Model missing or corrupted - trigger download
    print(f"Model not found or corrupted. Starting download...")
    if fetcher.download_model():
        return fetcher.model_path
    
    return None


if __name__ == "__main__":
    # Test the fetcher
    result = check_and_fetch_model()
    if result:
        print(f"Model ready at: {result}")
    else:
        print("Failed to acquire model")
        sys.exit(1)
