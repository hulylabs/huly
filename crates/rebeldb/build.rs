// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// build.rs:

use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    // Watch for changes in runtime dependencies
    println!("cargo:rerun-if-changed=../rebeldb-runtime/src");
    println!("cargo:rerun-if-changed=../rebeldb-runtime/Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");

    // Path to runtime-wasm crate - adjusted for crates/ directory
    let wasm_runtime_crate = Path::new("../rebeldb-runtime");

    // Build runtime-wasm
    let status = Command::new("cargo")
        .current_dir(wasm_runtime_crate)
        .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
        .status()
        .expect("Failed to build wasm runtime");

    if !status.success() {
        panic!("Failed to build wasm runtime");
    }

    // Get workspace target directory
    let target_dir = if let Ok(target) = env::var("CARGO_TARGET_DIR") {
        Path::new(&target).to_path_buf()
    } else {
        // Adjusted to look for target in root, not crates/
        Path::new("../../target").to_path_buf()
    };

    // Setup paths
    let wasm_file = target_dir.join("wasm32-unknown-unknown/release/rebeldb_runtime.wasm");
    let assets_dir = Path::new("assets");

    // Create assets directory
    std::fs::create_dir_all(assets_dir).expect("Failed to create assets directory");

    // Copy WASM file
    let dest_path = assets_dir.join("rebeldb_runtime.wasm");
    std::fs::copy(&wasm_file, &dest_path).expect("Failed to copy wasm file");

    // Watch the output WASM file for changes
    println!("cargo:rerun-if-changed={}", wasm_file.display());
}
