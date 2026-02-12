
import ctypes
import os

dll_path = os.path.join(os.getcwd(), "shard_engine.dll")
model_path = os.getenv("BITNET_MODEL", "E:\\ggml-model-i2_s.gguf")

print(f"Loading {dll_path}...")
lib = ctypes.CDLL(dll_path)
lib.shard_init.argtypes = [ctypes.c_char_p]
lib.shard_init.restype = ctypes.c_void_p

print(f"Calling shard_init with {model_path}...")
handle = lib.shard_init(model_path.encode("utf-8"))
if handle:
    print(f"Successfully initialized handle: {handle}")
    lib.shard_free.argtypes = [ctypes.c_void_p]
    lib.shard_free(handle)
    print("Successfully freed handle")
else:
    print("shard_init returned NULL")
