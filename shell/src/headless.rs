//! Headless command-line runner: produces a looping sprite to disk without the
//! GUI, so the full pipeline can be demonstrated and the result inspected.
//!
//! - `shell demo --out <dir>` — builds a synthetic multi-frame sprite (no
//!   backend) and writes its frames + a looping GIF. Proves the
//!   composite/integrate/loop/encode machinery end-to-end.
//! - `shell gen --prompt <p> --motion <m> --out <dir>` — runs the REAL FAL
//!   reference-sheet verb + animation pipeline (needs a FAL key in the keychain
//!   under `pixhaus.fal`) and writes the resulting looping sprite.
#![allow(clippy::print_stdout)]

use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, RgbaImage};
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::export::hard_mask_alpha;
use pixhaus_core::project::{FrameIndex, LoopDirection, Size};

/// Alpha threshold for the hard mask the headless GIF encode shares with the GUI
/// exporter (the forge default). Pixels at or above are forced opaque; below,
/// transparent — so the encoder routes them to the reserved index and the soft
/// fringe never dithers into a halo.
const GIF_ALPHA_THRESHOLD: u8 = 128;

use crate::ai;
use crate::document::DocumentStore;

const CANVAS: Size = Size { width: 64, height: 64 };

/// Dispatches headless subcommands. Returns `Ok(true)` when it handled the
/// args (so `main` skips the GUI), `Ok(false)` to fall through to the GUI.
pub fn run(args: &[String]) -> Result<bool> {
    match args.first().map(String::as_str) {
        Some("demo") => {
            run_demo(&out_dir(args))?;
            Ok(true)
        }
        Some("sheet") => {
            run_sheet(args)?;
            Ok(true)
        }
        Some("gen") => {
            run_gen(args)?;
            Ok(true)
        }
        Some("set-key") => {
            let backend = args.get(1).ok_or_else(|| anyhow!("usage: shell set-key <openai|fal> <API_KEY>"))?;
            let key = args.get(2).ok_or_else(|| anyhow!("usage: shell set-key <openai|fal> <API_KEY>"))?;
            if backend != ai::OPENAI_BACKEND_ID && backend != ai::FAL_BACKEND_ID {
                return Err(anyhow!("backend must be 'openai' or 'fal'"));
            }
            ai::store_key(backend, key).map_err(|e| anyhow!(e))?;
            println!("stored {backend} key in the OS keychain (pixhaus.{backend})");
            Ok(true)
        }
        Some("--help" | "-h" | "help") => {
            print_help();
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn print_help() {
    println!("pixhaus shell — headless modes:");
    println!("  shell set-key <openai|fal> <API_KEY>");
    println!("      store a backend key in the OS keychain (pixhaus.<backend>)");
    println!("  shell sheet [--prompt TEXT] [--out DIR]");
    println!("      real reference-sheet only (OpenAI gpt-image-2 or FAL); writes sheet PNGs");
    println!("  shell demo [--out DIR]");
    println!("      synthetic looping sprite (no backend); writes frames + loop.gif");
    println!("  shell gen [--prompt TEXT] [--motion TEXT] [--out DIR]");
    println!("      real FAL reference-sheet + animation; needs a FAL key in the keychain");
    println!("  shell            launch the GUI");
}

/// Value of `--flag` in `args`, if present.
fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn out_dir(args: &[String]) -> PathBuf {
    PathBuf::from(arg_value(args, "--out").unwrap_or("./out"))
}

/// Synthetic demo: build a multi-frame sprite and write its loop, no backend.
fn run_demo(out: &Path) -> Result<()> {
    let mut doc = DocumentStore::new();
    doc.create_sprite("demo", CANVAS);
    doc.add_demo_animation();
    let frames = composite_all_frames(&doc)?;
    write_outputs(&frames, 120, out)?;
    Ok(())
}

/// Real reference-sheet-only run: generates candidate sheets via the real verb
/// against a configured image-generation backend (`OpenAI` gpt-image-2 or FAL),
/// writing each variant PNG. Works with just an `OpenAI` key (no animation).
fn run_sheet(args: &[String]) -> Result<()> {
    let prompt = arg_value(args, "--prompt").unwrap_or(ai::DEFAULT_SHEET_PROMPT).to_owned();
    let out = out_dir(args);

    let runtime = ai::build_runtime();
    let ready = ai::register_backends_blocking(&runtime);
    if !ready {
        return Err(anyhow!("no generation backend configured — store a key: shell set-key openai <key>"));
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;

    let mut doc = DocumentStore::new();
    doc.create_sprite("sheet", CANVAS);
    let entity_id = doc.active_entity_id().ok_or_else(|| anyhow!("no active entity after create_sprite"))?;
    let meta = doc.project.metadata.clone();

    println!("generating reference sheet...");
    let job = ai::SheetJob::minimal(meta, entity_id, ai::TEMPLATES[0].1.to_owned(), prompt, 2);
    let variants = rt
        .block_on(ai::run_reference_sheet(&runtime, job, &|m| {
            println!("  [sheet] {m}");
        }))
        .map_err(|e| anyhow!(e))?;

    fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
    for (i, png) in variants.iter().enumerate() {
        let path = out.join(format!("sheet_{i:03}.png"));
        fs::write(&path, png).with_context(|| format!("writing {}", path.display()))?;
    }
    println!("wrote {} reference-sheet variant(s) to {}", variants.len(), out.display());
    Ok(())
}

/// Real pipeline: reference sheet -> approve variant 0 -> FAL animation ->
/// integrate -> write the looping sprite.
fn run_gen(args: &[String]) -> Result<()> {
    let prompt = arg_value(args, "--prompt").unwrap_or(ai::DEFAULT_SHEET_PROMPT).to_owned();
    let motion = arg_value(args, "--motion").unwrap_or("walk cycle, side view").to_owned();
    let out = out_dir(args);

    let runtime = ai::build_runtime();
    let ready = ai::register_backends_blocking(&runtime);
    if !ready {
        return Err(anyhow!(
            "no FAL backend configured — store a key first (run the GUI once and paste it, \
             or set the keychain entry `pixhaus.fal`)"
        ));
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;

    let mut doc = DocumentStore::new();
    doc.create_sprite("gen", CANVAS);
    let entity_id = doc.active_entity_id().ok_or_else(|| anyhow!("no active entity after create_sprite"))?;
    let meta = doc.project.metadata.clone();

    println!("generating reference sheet...");
    let sheet_job = ai::SheetJob::minimal(meta, entity_id, ai::TEMPLATES[0].1.to_owned(), prompt, 2);
    let variants = rt
        .block_on(ai::run_reference_sheet(&runtime, sheet_job, &|m| {
            println!("  [sheet] {m}");
        }))
        .map_err(|e| anyhow!(e))?;
    let anchor = variants.into_iter().next().ok_or_else(|| anyhow!("reference sheet returned no variant"))?;
    doc.set_active_anchor(anchor.clone());
    println!("approved variant 0 as anchor ({} bytes)", anchor.len());

    println!("generating animation...");
    let anim_job = ai::AnimJob {
        canvas: (CANVAS.width, CANVAS.height),
        anchor_png: anchor,
        first_frame_png: None,
        motion_prompt: motion,
        i2v_model: None,
        target_frames: 6,
        fps: 10,
        seed: None,
        // The headless demo is the pixel-art default, so the finisher runs.
        art_style_kind: pixhaus_core::project::library::composition::ArtStyleKind::PixelArt,
    };
    let (frames, frame_ms) = rt
        .block_on(ai::run_animation(&runtime, &anim_job, &|m| {
            println!("  [anim] {m}");
        }))
        .map_err(|e| anyhow!(e))?;
    doc.integrate_frames(frames, frame_ms, "gen", LoopDirection::Forward);

    let out_frames = composite_all_frames(&doc)?;
    write_outputs(&out_frames, frame_ms, &out)?;
    Ok(())
}

/// Composites every frame of the active sprite into a flat RGBA buffer.
fn composite_all_frames(doc: &DocumentStore) -> Result<Vec<PixelBuffer>> {
    let count = doc.frame_count();
    if count == 0 {
        return Err(anyhow!("active sprite has no frames"));
    }
    let mut frames = Vec::with_capacity(count);
    for i in 0..count {
        let idx = FrameIndex::new(u32::try_from(i).unwrap_or(0));
        let frame = doc.composite_frame(idx).ok_or_else(|| anyhow!("compositing frame {i} failed"))?;
        frames.push(frame);
    }
    Ok(frames)
}

/// Writes each frame as a PNG and the whole sequence as an infinitely-looping
/// animated GIF, then prints where everything landed.
fn write_outputs(frames: &[PixelBuffer], frame_ms: u32, out: &Path) -> Result<()> {
    fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;

    for (i, frame) in frames.iter().enumerate() {
        let img = to_rgba_image(frame)?;
        let path = out.join(format!("frame_{i:03}.png"));
        img.save(&path).with_context(|| format!("writing {}", path.display()))?;
    }

    let gif_path = out.join("loop.gif");
    let file = fs::File::create(&gif_path).with_context(|| format!("creating {}", gif_path.display()))?;
    let mut encoder = GifEncoder::new(BufWriter::new(file));
    encoder.set_repeat(Repeat::Infinite).context("setting gif repeat")?;
    for frame in frames {
        // Hard-mask alpha to 0/255 before the encode so GIF's 1-bit alpha leaves
        // no soft-fringe halo — the same anti-halo pass the GUI exporter runs.
        let masked = hard_mask_alpha(frame, GIF_ALPHA_THRESHOLD);
        let img = to_rgba_image(&masked)?;
        let delay = Delay::from_numer_denom_ms(frame_ms, 1);
        encoder.encode_frame(Frame::from_parts(img, 0, 0, delay)).context("encoding gif frame")?;
    }
    drop(encoder);

    println!(
        "wrote {} frames + {} ({}ms/frame, looping) to {}",
        frames.len(),
        gif_path.display(),
        frame_ms,
        out.display()
    );
    Ok(())
}

/// Converts a tightly-packed RGBA [`PixelBuffer`] into an `image::RgbaImage`.
fn to_rgba_image(frame: &PixelBuffer) -> Result<RgbaImage> {
    let (w, h) = (frame.width(), frame.height());
    let tight: Vec<u8> = if frame.stride() == w * 4 {
        frame.as_bytes().to_vec()
    } else {
        // Drop row padding.
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        let row = (w * 4) as usize;
        for chunk in frame.as_bytes().chunks_exact(frame.stride() as usize) {
            out.extend_from_slice(&chunk[..row]);
        }
        out
    };
    RgbaImage::from_raw(w, h, tight).ok_or_else(|| anyhow!("frame buffer size mismatch"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::decode_clip;

    mod run_demo {
        use super::*;

        #[test]
        fn writes_loop_gif_and_frames() {
            let dir = tempfile::tempdir().unwrap();
            run_demo(dir.path()).unwrap();
            assert!(dir.path().join("loop.gif").exists(), "demo must write loop.gif");
            assert!(dir.path().join("frame_000.png").exists(), "demo must write at least one frame png");
        }

        #[test]
        fn demo_gif_is_halo_free() {
            let dir = tempfile::tempdir().unwrap();
            run_demo(dir.path()).unwrap();
            let bytes = std::fs::read(dir.path().join("loop.gif")).unwrap();
            let frames = decode_clip(&bytes, "image/gif", 10).expect("re-decode demo gif");
            assert!(!frames.is_empty(), "demo gif should decode to frames");
            for frame in &frames {
                for px in frame.pixels.chunks_exact(4) {
                    let a = px[3];
                    assert!(a == 0 || a == 255, "decoded alpha {a} proves a halo survived the hard mask");
                }
            }
        }
    }
}
