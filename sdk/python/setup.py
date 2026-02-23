from setuptools import setup, Command
from setuptools.command.build_py import build_py
from setuptools_rust import RustBin
import os
import shutil
import subprocess
import platform

class BuildShardEngine(Command):
    description = "Build shard-bridge C++ engine"
    user_options = []
    def initialize_options(self): pass
    def finalize_options(self): pass
    
    def run(self):
        cwd = os.getcwd()
        # In isolated builds (like python -m build), the root might not be 2 levels up
        # We check common locations
        root_dir = os.path.abspath(os.path.join(cwd, "../.."))
        cpp_dir = os.path.join(root_dir, "cpp/shard-bridge")
        
        if not os.path.exists(cpp_dir):
            print(f"Warning: C++ source not found at {cpp_dir}. Skipping engine build.")
            print("If this is a binary wheel build, ensure the engine is already in shard/.")
            return

        build_dir = os.path.join(cwd, "build/cpp")
        
        if not os.path.exists(build_dir):
            os.makedirs(build_dir)
            
        print(f"Building shard-engine in {build_dir}...")
        subprocess.check_call(["cmake", cpp_dir, "-B", build_dir, "-DCMAKE_BUILD_TYPE=Release"])
        subprocess.check_call(["cmake", "--build", build_dir, "--config", "Release"])
        
        # Copy the resulting library to shard/
        lib_name = "shard_engine.dll" if platform.system() == "Windows" else \
                   "libshard_engine.dylib" if platform.system() == "Darwin" else \
                   "libshard_engine.so"
        
        # Find the lib (might be in Release/ on Windows)
        src_lib = os.path.join(build_dir, lib_name)
        if not os.path.exists(src_lib):
            src_lib = os.path.join(build_dir, "Release", lib_name)
            
        dest_lib = os.path.join(cwd, "shard", lib_name)
        print(f"Copying {src_lib} to {dest_lib}")
        shutil.copy(src_lib, dest_lib)

class CustomBuildPy(build_py):
    def run(self):
        self.run_command("build_shard_engine")
        super().run()

setup(
    cmdclass={
        "build_shard_engine": BuildShardEngine,
        "build_py": CustomBuildPy,
    },
    rust_extensions=[
        RustBin(
            "shard.bin.shard-daemon",
            path="../../desktop/rust/daemon/Cargo.toml",
            args=["--bin", "shard-daemon"]
        )
    ],
    # package_data is handled by MANIFEST.in or here
    package_data={"shard": ["*.so", "*.dll", "*.dylib", "bin/*"]},
)
