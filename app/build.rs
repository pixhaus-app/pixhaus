//! Tauri build script. Embeds the generated context the runtime macro
//! expects from `tauri.conf.json`.

fn main() {
    tauri_build::build();
}
