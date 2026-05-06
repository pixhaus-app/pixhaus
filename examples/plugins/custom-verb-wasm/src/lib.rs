//! Invert Colors Verb — Pixhaus WASM plugin example.
//!
//! Demonstrates the minimum surface a WASM verb needs:
//! - `pixhaus_describe`: return a JSON descriptor the host reads at load time.
//! - `pixhaus_verb_run`: receive pixel bytes, transform them, return the result.
//!
//! Build with:
//!   cargo build --release --target wasm32-wasip1
//!
//! Then copy `target/wasm32-wasip1/release/invert_colors_verb.wasm`
//! into this plugin folder as `plugin.wasm`.  Or run `./build.sh`.

use extism_pdk::*;
use serde::{Deserialize, Serialize};

// --- Types the host uses to call this verb --------------------------------

#[derive(Deserialize)]
struct InvertInput {
    /// RGBA pixel bytes — `width * height * 4` values.
    pixels: Vec<u8>,
    width:  u32,
    height: u32,
    /// Optional display name for the resulting layer.
    #[serde(default)]
    layer_name: Option<String>,
}

#[derive(Serialize)]
struct InvertOutput {
    /// Transformed pixel bytes.
    pixels: Vec<u8>,
    width:  u32,
    height: u32,
    /// One-line description shown in the preview panel and undo history.
    summary: String,
}

// --- Plugin registration --------------------------------------------------

/// Called once when the host loads the plugin.
/// Return a JSON descriptor (object or array of objects for multi-verb plugins).
#[plugin_fn]
pub fn pixhaus_describe() -> FnResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "type":        "verb",
        "id":          "com.pixhaus.examples.invert-colors",
        "label":       "Invert Colors",
        "description": "Invert the RGB channels of every pixel on the active layer.",
        "cost":        { "credits": 0 }
    })))
}

// --- Verb implementation --------------------------------------------------

/// Called when the user runs the verb.
/// The host passes pixel data from the active layer cel; this function
/// transforms it and returns the result.  The host shows a preview before
/// committing the change to the undo stack.
#[plugin_fn]
pub fn pixhaus_verb_run(Json(input): Json<InvertInput>) -> FnResult<Json<InvertOutput>> {
    let mut pixels = input.pixels;
    let total      = (input.width * input.height * 4) as usize;

    if pixels.len() != total {
        return Err(extism_pdk::Error::msg(format!(
            "expected {} bytes for {}×{} RGBA, got {}",
            total, input.width, input.height, pixels.len()
        )));
    }

    // Invert every pixel's RGB channels; leave alpha unchanged.
    for chunk in pixels.chunks_mut(4) {
        chunk[0] = 255 - chunk[0]; // R
        chunk[1] = 255 - chunk[1]; // G
        chunk[2] = 255 - chunk[2]; // B
                                   // chunk[3] = alpha — left as-is
    }

    let name    = input.layer_name.as_deref().unwrap_or("Layer");
    let summary = format!("Invert colors on \"{}\"", name);

    Ok(Json(InvertOutput {
        pixels,
        width:  input.width,
        height: input.height,
        summary,
    }))
}
