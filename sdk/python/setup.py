from setuptools import setup
from setuptools_rust import Binding, RustBin
import os

# Ensure we can find the cargo manifest relative to this file
# This assumes we are building from within the repo structure
cargo_path = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../desktop/rust/daemon/Cargo.toml"))

setup(
    rust_extensions=[
        RustBin(
            "shard-daemon",
            path=cargo_path,
        )
    ],
)
