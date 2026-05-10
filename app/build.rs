//! Tauri build script. Embeds the generated context the runtime macro
//! expects from `tauri.conf.json`.

use std::fs;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::Path;

fn main() {
    // tauri::generate_context!() validates frontendDist at compile time and
    // panics if ../ui/dist/index.html is missing. Drop a placeholder so raw
    // cargo invocations succeed before pnpm ui:build has run; the real bundle
    // overwrites it. The rerun directive makes cargo notice when the stub
    // appears or disappears (the default scan only watches files under app/).
    println!("cargo:rerun-if-changed=../ui/dist/index.html");
    ensure_frontend_stub();

    tauri_build::build();
}

fn ensure_frontend_stub() {
    let dist = Path::new("../ui/dist");
    let index = dist.join("index.html");
    if index.exists() {
        return;
    }
    if let Err(err) = fs::create_dir_all(dist) {
        println!("cargo:warning=create ../ui/dist failed: {err}");
        return;
    }
    // create_new is atomic: if a concurrent `pnpm ui:build` writes the real
    // bundle between our exists() check above and this open call, we get
    // AlreadyExists and bail rather than truncating real content with the stub.
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&index) {
        Ok(f) => f,
        Err(err) if err.kind() == ErrorKind::AlreadyExists => return,
        Err(err) => {
            println!("cargo:warning=create ../ui/dist/index.html failed: {err}");
            return;
        }
    };
    if let Err(err) = file.write_all(b"<!doctype html><title>stub</title>") {
        println!("cargo:warning=write ../ui/dist/index.html failed: {err}");
    }
}
