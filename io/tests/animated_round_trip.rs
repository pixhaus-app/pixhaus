//! Round-trip decode tests for animated export: GIF, WebP, and MP4 (S11 followup).
//!
//! GIF decoding uses `image-rs` (gif feature). WebP integrity is checked by
//! parsing the RIFF chunk structure directly (the `image` workspace dependency
//! does not enable the webp feature) and pixel round-trips use the `webp`
//! crate's `AnimDecoder`. MP4 tests are gated on `ffmpeg`/`ffprobe` being
//! available on `PATH`; they skip gracefully when neither is found.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::disallowed_methods,
    clippy::print_stderr,
    clippy::many_single_char_names,
    clippy::uninlined_format_args
)]

use image::AnimationDecoder;
use image::codecs::gif::GifDecoder;
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::Rgba;
use pixhaus_io::animated::{
    GifOptions, LoopCount, PaletteMode, VideoOptions, WebPOptions, encode_gif, encode_mp4,
    encode_webp,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn solid_buf(w: u32, h: u32, r: u8, g: u8, b: u8) -> PixelBuffer {
    PixelBuffer::filled(w, h, Rgba::opaque(r, g, b)).unwrap()
}

fn gif_bytes(frames: &[(PixelBuffer, u32)], opts: &GifOptions) -> Vec<u8> {
    let mut out = Vec::new();
    encode_gif(frames, opts, None, &mut out).unwrap();
    out
}

/// Decode GIF bytes into collected frames using `image-rs`.
fn decode_gif(bytes: &[u8]) -> Vec<image::Frame> {
    GifDecoder::new(std::io::Cursor::new(bytes))
        .expect("GIF must be parseable")
        .into_frames()
        .collect::<Result<Vec<_>, _>>()
        .expect("GIF frames must decode without error")
}

/// Count `ANMF` chunks in an animated WebP byte stream.
///
/// Each frame in an animated WebP is stored as one ANMF chunk. The count
/// must equal the number of frames passed to the encoder.
fn webp_anmf_count(bytes: &[u8]) -> usize {
    // Animated WebP layout: RIFF(4) + size(4) + WEBP(4) = 12 bytes header,
    // then a sequence of chunks. Each chunk: tag(4) + size(4) + data(size),
    // with data padded to an even byte count.
    if bytes.len() < 12 {
        return 0;
    }
    let mut count = 0usize;
    let mut pos = 12usize;
    while pos + 8 <= bytes.len() {
        let tag = &bytes[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        if tag == b"ANMF" {
            count += 1;
        }
        pos += 8 + chunk_size;
        if chunk_size % 2 != 0 {
            pos += 1; // RIFF chunks are padded to an even byte boundary
        }
    }
    count
}

/// Extract `(width, height)` from the VP8X chunk of an animated WebP.
///
/// VP8X stores canvas dimensions as `(dim - 1)` packed into 24-bit LE fields.
fn webp_vp8x_dims(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 12 {
        return None;
    }
    let mut pos = 12usize;
    while pos + 8 <= bytes.len() {
        let tag = &bytes[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        if tag == b"VP8X" && chunk_size >= 10 {
            let d = &bytes[pos + 8..pos + 8 + chunk_size];
            // Canvas width  - 1 at bytes 4–6 (24-bit LE)
            let w = u32::from_le_bytes([d[4], d[5], d[6], 0]) + 1;
            // Canvas height - 1 at bytes 7–9 (24-bit LE)
            let h = u32::from_le_bytes([d[7], d[8], d[9], 0]) + 1;
            return Some((w, h));
        }
        pos += 8 + chunk_size;
        if chunk_size % 2 != 0 {
            pos += 1;
        }
    }
    None
}

/// Return `true` when `tool -version` exits successfully, i.e. the tool is on PATH.
fn external_tool_available(tool: &str) -> bool {
    std::process::Command::new(tool)
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

// ── GIF round-trip ────────────────────────────────────────────────────────────

#[test]
fn gif_decoded_frame_count_matches_input() {
    let frames = vec![
        (solid_buf(8, 8, 255, 0, 0), 100u32),
        (solid_buf(8, 8, 0, 255, 0), 100u32),
        (solid_buf(8, 8, 0, 0, 255), 100u32),
    ];
    let bytes = gif_bytes(&frames, &GifOptions::default());
    let decoded = decode_gif(&bytes);
    assert_eq!(
        decoded.len(),
        3,
        "frame count must survive the GIF round-trip"
    );
}

#[test]
fn gif_decoded_dimensions_match_input() {
    let frames = vec![(solid_buf(12, 16, 100, 100, 100), 50u32)];
    let bytes = gif_bytes(&frames, &GifOptions::default());
    let decoded = decode_gif(&bytes);
    let buf = decoded[0].buffer();
    assert_eq!(buf.width(), 12, "width must survive GIF round-trip");
    assert_eq!(buf.height(), 16, "height must survive GIF round-trip");
}

#[test]
fn gif_decoded_frame_delay_approximately_preserved() {
    // 200 ms → encoder stores 20 centiseconds → decoder reads ≈ 200 ms.
    // GIF timing resolution is 10 ms, so we tolerate a ±10 ms delta.
    let frames = vec![(solid_buf(4, 4, 128, 64, 32), 200u32)];
    let bytes = gif_bytes(
        &frames,
        &GifOptions {
            palette_mode: PaletteMode::GlobalQuantize,
            ..Default::default()
        },
    );
    let decoded = decode_gif(&bytes);
    // image::Delay returns a rational (numerator/denominator) in ms; the
    // upstream method name is numer_denom_ms() so the local binding name
    // mirrors that.
    let (n, d) = decoded[0].delay().numer_denom_ms();
    let delay_ms = n.checked_div(d).unwrap_or(0);
    assert!(
        (delay_ms as i64 - 200).abs() <= 10,
        "frame delay {delay_ms}ms should be ≈200ms (±10ms GIF resolution)"
    );
}

#[test]
fn gif_decoded_solid_red_frame_is_red_dominant() {
    // After GIF palette quantization the red channel must dominate. We do not
    // assert an exact value because NeuQuant may shift hues slightly, but for
    // a pure-red input the decoded colour must be unambiguously red.
    let frames = vec![(solid_buf(8, 8, 255, 0, 0), 100u32)];
    let bytes = gif_bytes(
        &frames,
        &GifOptions {
            palette_mode: PaletteMode::GlobalQuantize,
            ..Default::default()
        },
    );
    let decoded = decode_gif(&bytes);
    let px = decoded[0].buffer().get_pixel(4, 4).0;
    assert!(
        px[0] >= 200,
        "red channel must be ≥200 after round-trip, got {}",
        px[0]
    );
    assert!(
        px[0] > px[1] + 50,
        "red must dominate green by ≥50 (got r={} g={})",
        px[0],
        px[1]
    );
    assert!(
        px[0] > px[2] + 50,
        "red must dominate blue by ≥50 (got r={} b={})",
        px[0],
        px[2]
    );
}

#[test]
fn gif_decoded_multi_frame_colours_stay_distinct() {
    // Three frames with pure primary colours. In PerFrameQuantize mode each
    // frame carries its own palette, so each decoded frame must still be
    // dominated by its original channel.
    let frames = vec![
        (solid_buf(8, 8, 255, 0, 0), 100u32),
        (solid_buf(8, 8, 0, 255, 0), 100u32),
        (solid_buf(8, 8, 0, 0, 255), 100u32),
    ];
    let bytes = gif_bytes(
        &frames,
        &GifOptions {
            palette_mode: PaletteMode::PerFrameQuantize,
            ..Default::default()
        },
    );
    let decoded = decode_gif(&bytes);
    let r_px = decoded[0].buffer().get_pixel(4, 4).0;
    let g_px = decoded[1].buffer().get_pixel(4, 4).0;
    let b_px = decoded[2].buffer().get_pixel(4, 4).0;

    assert!(
        r_px[0] > r_px[1] && r_px[0] > r_px[2],
        "frame 0 must be red-dominant (got {:?})",
        r_px
    );
    assert!(
        g_px[1] > g_px[0] && g_px[1] > g_px[2],
        "frame 1 must be green-dominant (got {:?})",
        g_px
    );
    assert!(
        b_px[2] > b_px[0] && b_px[2] > b_px[1],
        "frame 2 must be blue-dominant (got {:?})",
        b_px
    );
}

#[test]
fn gif_infinite_loop_contains_netscape_extension() {
    // The NETSCAPE 2.0 application extension is how GIF encodes infinite
    // looping. Its 11-byte identifier must appear literally in the output.
    let frames = vec![(solid_buf(4, 4, 0, 0, 0), 100u32)];
    let bytes = gif_bytes(
        &frames,
        &GifOptions {
            repeat: LoopCount::Infinite,
            ..Default::default()
        },
    );
    let has_netscape = bytes.windows(11).any(|w| w == b"NETSCAPE2.0");
    assert!(
        has_netscape,
        "infinite-loop GIF must contain the NETSCAPE2.0 application extension"
    );
}

#[test]
fn gif_finite_loop_contains_netscape_extension() {
    // Finite loop count also uses the NETSCAPE extension (with a non-zero count).
    let frames = vec![(solid_buf(4, 4, 128, 128, 128), 100u32)];
    let bytes = gif_bytes(
        &frames,
        &GifOptions {
            repeat: LoopCount::Count(3),
            ..Default::default()
        },
    );
    let has_netscape = bytes.windows(11).any(|w| w == b"NETSCAPE2.0");
    assert!(
        has_netscape,
        "finite-loop GIF must also contain the NETSCAPE2.0 extension"
    );
}

// ── WebP round-trip ───────────────────────────────────────────────────────────

#[test]
fn webp_riff_header_and_webp_id_present() {
    let frames = vec![(solid_buf(8, 8, 100, 150, 200), 100u32)];
    let bytes = encode_webp(&frames, &WebPOptions::default()).unwrap();
    assert_eq!(&bytes[0..4], b"RIFF", "must start with RIFF");
    assert_eq!(&bytes[8..12], b"WEBP", "RIFF type field must be WEBP");
}

#[test]
fn webp_vp8x_canvas_dimensions_match_input() {
    // libwebp's AnimEncoder may produce a simple (non-animated) WebP for a
    // single-frame input, omitting the VP8X chunk. Use two frames to force
    // the animated container format that always includes VP8X.
    let frames = vec![
        (solid_buf(12, 20, 0, 128, 255), 100u32),
        (solid_buf(12, 20, 0, 64, 128), 100u32),
    ];
    let bytes = encode_webp(&frames, &WebPOptions::default()).unwrap();
    let (w, h) = webp_vp8x_dims(&bytes).expect("VP8X chunk must be present in animated WebP");
    assert_eq!(w, 12, "canvas width in VP8X chunk");
    assert_eq!(h, 20, "canvas height in VP8X chunk");
}

#[test]
fn webp_anmf_chunk_count_matches_frame_count() {
    let frames: Vec<(PixelBuffer, u32)> = (0u8..4)
        .map(|i| (solid_buf(4, 4, i.saturating_mul(60), 0, 0), 100u32))
        .collect();
    let bytes = encode_webp(&frames, &WebPOptions::default()).unwrap();
    assert_eq!(
        webp_anmf_count(&bytes),
        4,
        "one ANMF chunk per encoded frame"
    );
}

#[test]
fn webp_two_frames_produce_two_anmf_chunks() {
    // libwebp's AnimEncoder may omit ANMF chunks when only one frame is
    // provided (it can fall back to simple WebP). Two frames guarantee the
    // animated container format.
    let frames = vec![
        (solid_buf(4, 4, 200, 100, 50), 150u32),
        (solid_buf(4, 4, 50, 200, 100), 150u32),
    ];
    let bytes = encode_webp(&frames, &WebPOptions::default()).unwrap();
    assert_eq!(
        webp_anmf_count(&bytes),
        2,
        "two-frame animated WebP must have exactly two ANMF chunks"
    );
}

#[test]
fn webp_lossless_round_trip_pixel_exact() {
    // VP8L (lossless) must reproduce every channel exactly. We encode a
    // solid-colour frame, decode with the webp crate's AnimDecoder, and
    // compare the first pixel against the original values.
    //
    // webp 0.3.1 API:
    //   AnimDecoder::new(&[u8]) -> AnimDecoder<'_>
    //   AnimDecoder::decode(&self) -> Result<DecodeAnimImage, String>
    //   DecodeAnimImage::get_frame(usize) -> Option<AnimFrame<'_>>
    //   AnimFrame::get_image() -> &[u8]   (RGBA pixel data)
    let (r, g, b) = (210u8, 80u8, 40u8);
    let frames = vec![(solid_buf(4, 4, r, g, b), 100u32)];
    let bytes = encode_webp(
        &frames,
        &WebPOptions {
            lossless: true,
            ..Default::default()
        },
    )
    .unwrap();

    let decoder = webp::AnimDecoder::new(&bytes);
    let anim = decoder
        .decode()
        .expect("lossless animated WebP must decode");
    assert!(
        anim.len() > 0,
        "decoded animation must contain at least one frame"
    );

    let frame = anim.get_frame(0).expect("frame 0 must exist");
    let pixel_data = frame.get_image();
    assert!(
        pixel_data.len() >= 4,
        "frame pixel data must contain at least one RGBA pixel"
    );
    assert_eq!(
        pixel_data[0], r,
        "red channel exact after lossless round-trip"
    );
    assert_eq!(
        pixel_data[1], g,
        "green channel exact after lossless round-trip"
    );
    assert_eq!(
        pixel_data[2], b,
        "blue channel exact after lossless round-trip"
    );
    assert_eq!(
        pixel_data[3], 255,
        "alpha must be 255 for fully opaque frame"
    );
}

#[test]
fn webp_lossy_round_trip_pixel_approximate() {
    // VP8 (lossy) introduces block artefacts; at quality 90 a solid colour
    // must decode to within ±20 of the original on every channel.
    let (r, g, b) = (200u8, 100u8, 50u8);
    let frames = vec![(solid_buf(8, 8, r, g, b), 100u32)];
    let bytes = encode_webp(
        &frames,
        &WebPOptions {
            lossless: false,
            quality: 90.0,
            ..Default::default()
        },
    )
    .unwrap();

    let decoder = webp::AnimDecoder::new(&bytes);
    let anim = decoder.decode().expect("lossy animated WebP must decode");
    let frame = anim.get_frame(0).expect("frame 0 must exist");
    let pixel_data = frame.get_image();

    let tolerance = 20i16;
    let dr = (pixel_data[0] as i16 - r as i16).abs();
    let dg = (pixel_data[1] as i16 - g as i16).abs();
    let db = (pixel_data[2] as i16 - b as i16).abs();
    assert!(
        dr <= tolerance,
        "lossy red delta {dr} exceeds ±{tolerance} (got {}, expected {r})",
        pixel_data[0]
    );
    assert!(
        dg <= tolerance,
        "lossy green delta {dg} exceeds ±{tolerance} (got {}, expected {g})",
        pixel_data[1]
    );
    assert!(
        db <= tolerance,
        "lossy blue delta {db} exceeds ±{tolerance} (got {}, expected {b})",
        pixel_data[2]
    );
}

#[test]
fn webp_multi_frame_anmf_count_large() {
    // Verify the ANMF counter scales correctly for more frames than fit in a
    // typical on-screen animation.
    let frames: Vec<(PixelBuffer, u32)> = (0u8..10)
        .map(|i| (solid_buf(4, 4, i.saturating_mul(25), 0, 0), 80u32))
        .collect();
    let bytes = encode_webp(&frames, &WebPOptions::default()).unwrap();
    assert_eq!(
        webp_anmf_count(&bytes),
        10,
        "ten-frame WebP must have ten ANMF chunks"
    );
}

// ── MP4 external decoder gate ─────────────────────────────────────────────────

#[test]
fn mp4_external_gate_encodes_non_empty_bytes() {
    // Requires ffmpeg on PATH. Skips gracefully when absent.
    if !external_tool_available("ffmpeg") {
        eprintln!("mp4_external_gate: ffmpeg not on PATH — skipping encode test");
        return;
    }

    let frames = vec![
        (solid_buf(8, 8, 255, 0, 0), 100u32),
        (solid_buf(8, 8, 0, 255, 0), 100u32),
        (solid_buf(8, 8, 0, 0, 255), 100u32),
    ];
    let bytes = encode_mp4(&frames, &VideoOptions::default()).unwrap();
    assert!(
        !bytes.is_empty(),
        "MP4 encode must produce a non-empty byte stream"
    );
}

#[test]
fn mp4_external_gate_ffprobe_validates_container() {
    // Encodes with ffmpeg, then probes the output with ffprobe to confirm the
    // container is valid and its duration is plausible.
    if !external_tool_available("ffmpeg") {
        eprintln!("mp4_ffprobe_gate: ffmpeg not on PATH — skipping");
        return;
    }

    let frames: Vec<(PixelBuffer, u32)> = vec![
        (solid_buf(8, 8, 200, 0, 0), 100u32),
        (solid_buf(8, 8, 0, 200, 0), 100u32),
        (solid_buf(8, 8, 0, 0, 200), 100u32),
    ];
    let bytes = encode_mp4(&frames, &VideoOptions::default()).unwrap();

    let temp_path = std::env::temp_dir().join("pixhaus_mp4_gate_test.mp4");
    std::fs::write(&temp_path, &bytes).unwrap();

    let probe = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(&temp_path)
        .output();

    let _ = std::fs::remove_file(&temp_path);

    match probe {
        Ok(out) if out.status.success() => {
            // 3 frames × 100 ms = 300 ms total → duration ≈ 0.3 s.
            let duration_str = String::from_utf8_lossy(&out.stdout);
            let duration: f64 = duration_str.trim().parse().unwrap_or(0.0);
            assert!(
                duration > 0.1,
                "probed duration {duration:.3}s must be > 0.1s for a 300ms animation"
            );
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            panic!("ffprobe failed (exit {:?}): {stderr}", out.status.code());
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("mp4_ffprobe_gate: ffprobe not on PATH — skipping probe step");
        }
        Err(e) => panic!("ffprobe spawn error: {e}"),
    }
}

#[test]
fn mp4_external_gate_odd_dimensions_succeed() {
    // 17 × 17 is odd on both axes. The encoder must pad to 18 × 18 for
    // yuv420p chroma subsampling without returning an error.
    if !external_tool_available("ffmpeg") {
        eprintln!("mp4_odd_dims: ffmpeg not on PATH — skipping");
        return;
    }

    let frames = vec![(solid_buf(17, 17, 128, 128, 128), 100u32)];
    let result = encode_mp4(&frames, &VideoOptions::default());
    assert!(
        result.is_ok(),
        "odd-dimension MP4 encode must succeed: {result:?}"
    );
    assert!(!result.unwrap().is_empty());
}

#[test]
fn mp4_external_gate_single_frame_encodes() {
    // Single-frame MP4 is valid; ffmpeg converts it to a static video clip.
    if !external_tool_available("ffmpeg") {
        eprintln!("mp4_single_frame: ffmpeg not on PATH — skipping");
        return;
    }

    let frames = vec![(solid_buf(16, 16, 255, 128, 0), 100u32)];
    let result = encode_mp4(&frames, &VideoOptions::default());
    assert!(result.is_ok(), "single-frame MP4 must encode: {result:?}");
}
