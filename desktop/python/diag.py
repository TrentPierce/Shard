
import os
print(f"CWD: {os.getcwd()}")
print(f"Files: {os.listdir('.')}")
print(f"BITNET_MODEL: {os.getenv('BITNET_MODEL')}")
dll = "shard_engine.dll"
print(f"Exists {dll}: {os.path.exists(dll)}")
