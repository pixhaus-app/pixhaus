//! Tauri build script. Embeds the generated context the runtime macro
//! expects from `tauri.conf.json`.

use std::fs;
use std::io::Write;
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
    let mut file = match fs::File::create(&index) {
        Ok(f) => f,
        Err(err) => {
            println!("cargo:warning=create ../ui/dist/index.html failed: {err}");
            return;
        }
    };
    if let Err(err) = file.write_all(b"<!doctype html><title>stub</title>") {
        println!("cargo:warning=write ../ui/dist/index.html failed: {err}");
    }
}
