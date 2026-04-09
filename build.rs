use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=helpers/island.swift");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let source = manifest_dir.join("helpers").join("island.swift");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("missing OUT_DIR"));
    let output = out_dir.join("island-overlay");
    let module_cache = out_dir.join("swift-module-cache");
    let _ = fs::create_dir_all(&module_cache);

    let status = Command::new("swiftc")
        .arg(&source)
        .arg("-O")
        .arg("-module-cache-path")
        .arg(&module_cache)
        .arg("-o")
        .arg(&output)
        .status()
        .unwrap_or_else(|err| panic!("failed to invoke swiftc for island helper: {err}"));

    if !status.success() {
        panic!("swiftc failed to build island helper from {}", source.display());
    }

    println!(
        "cargo:rustc-env=AGENTDESK_ISLAND_OVERLAY_PATH={}",
        output.display()
    );
}
