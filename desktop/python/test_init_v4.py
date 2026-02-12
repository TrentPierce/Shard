"""Capture ALL output from shard_init including C-level stderr."""
import ctypes, os, sys, tempfile, msvcrt

MODEL = r"E:\ggml-model-i2_s.gguf"
DLL   = os.path.join(os.path.dirname(__file__), "shard_engine.dll")

# Redirect C-level stderr to a temp file so we capture llama.cpp logs
log_path = os.path.join(os.path.dirname(__file__), "init_full_log.txt")
old_stderr_fd = os.dup(2)
log_fd = os.open(log_path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC)
os.dup2(log_fd, 2)

lib = ctypes.CDLL(DLL)
lib.shard_init.argtypes = [ctypes.c_char_p]
lib.shard_init.restype  = ctypes.c_void_p

print(f"Calling shard_init({MODEL})...")
ctx = lib.shard_init(MODEL.encode())

# Flush and restore stderr
os.dup2(old_stderr_fd, 2)
os.close(log_fd)
os.close(old_stderr_fd)

if ctx:
    print(f"SUCCESS: ctx = {ctx:#x}")
else:
    print("FAILED: shard_init returned NULL")

# Print last 100 lines of the log
print(f"\n--- Full log ({log_path}) last 100 lines ---")
with open(log_path, "r", errors="replace") as f:
    lines = f.readlines()
    for line in lines[-100:]:
        print(line, end="")
