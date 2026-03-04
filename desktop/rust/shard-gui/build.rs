/// build.rs — copies the pre-built shard_engine DLLs (and their llama.cpp
/// dependencies) next to shard-gui.exe so the daemon can load them at runtime
/// without requiring the user to configure a path.
use std::path::PathBuf;

fn main() {
    // Workspace root is three levels up from desktop/rust/shard-gui/
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent() // rust/
        .and_then(|p| p.parent()) // desktop/
        .and_then(|p| p.parent()) // Shard/ (workspace root)
        .expect("workspace root");

    // OUT_DIR = .../target/<profile>/build/shard-gui-<hash>/out
    // Go up 3 levels to reach .../target/<profile>/
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let profile_dir = out_dir
        .parent() // build/shard-gui-<hash>
        .and_then(|p| p.parent()) // build/
        .and_then(|p| p.parent()) // <profile>/
        .expect("profile dir")
        .to_path_buf();

    let dll_sources: &[(&str, &str)] = &[
        ("cpp/shard-bridge/build/Release", "shard_engine.dll"),
        ("cpp/shard-bridge/build/bin/Release", "llama.dll"),
        ("cpp/shard-bridge/build/bin/Release", "ggml.dll"),
        ("cpp/shard-bridge/build/bin/Release", "ggml-base.dll"),
        ("cpp/shard-bridge/build/bin/Release", "ggml-cpu.dll"),
    ];

    for (src_subdir, dll_name) in dll_sources {
        let src = workspace_root.join(src_subdir).join(dll_name);
        let dst = profile_dir.join(dll_name);
        if src.exists() {
            if let Err(e) = std::fs::copy(&src, &dst) {
                eprintln!(
                    "cargo:warning=Failed to copy {} → {}: {}",
                    src.display(),
                    dst.display(),
                    e
                );
            }
            println!("cargo:rerun-if-changed={}", src.display());
        } else {
            eprintln!("cargo:warning=DLL not found (skipping): {}", src.display());
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
}
