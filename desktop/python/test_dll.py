
import ctypes
import os

dll_path = os.path.join(os.getcwd(), "shard_engine.dll")
print(f"Loading {dll_path}...")
try:
    lib = ctypes.CDLL(dll_path)
    print("Successfully loaded DLL")
    print(f"shard_init exists: {hasattr(lib, 'shard_init')}")
    print(f"shard_eval exists: {hasattr(lib, 'shard_eval')}")
    print(f"shard_tokenize exists: {hasattr(lib, 'shard_tokenize')}")
except Exception as e:
    print(f"Failed to load DLL: {e}")
    # On Windows, missing dependencies are a common cause of failure
    import subprocess
    try:
        # Check if we can find llama.dll and ggml.dll
        print(f"llama.dll exists: {os.path.exists('llama.dll')}")
        print(f"ggml.dll exists: {os.path.exists('ggml.dll')}")
    except:
        pass
