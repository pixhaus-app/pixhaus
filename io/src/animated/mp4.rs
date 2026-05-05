//! MP4 video encoder via `ffmpeg` shell-out (stream S11).
//!
//! Pixhaus uses `ffmpeg` as an external tool rather than linking
//! `ffmpeg-next` statically, which would introduce LGPL code into the
//! binary. The user (or packager) must have `ffmpeg` on `PATH`.
//!
//! # Encoding pipeline
//!
//! 1. Write each composited frame to a temporary PNG file.
//! 2. Invoke `ffmpeg -framerate … -i … -c:v libx264 -crf … -pix_fmt yuv420p …`.
//! 3. Read the produced MP4 back into memory.
//! 4. Delete the temp directory.
//!
//! This avoids passing large pixel blobs through a pipe and lets ffmpeg
//! choose the I/O strategy for its own codec buffers.

use std::path::PathBuf;
use std::process::Command;

use pixhaus_core::canvas::PixelBuffer;

use crate::error::{Error, Result};

// ── Public types ──────────────────────────────────────────────────────────────

/// Options for MP4 export via `ffmpeg`.
#[derive(Clone, Debug)]
pub struct VideoOptions {
    /// Constant-rate factor (`0`–`51`). Lower values produce higher quality,
    /// larger files. `18`–`28` is the typical range; default is `23`.
    ///
    /// Passed to ffmpeg as `-crf`.
    pub crf: u32,
    /// Pixel format passed to ffmpeg via `-pix_fmt`. Default is `yuv420p`,
    /// which ensures maximum browser and player compatibility.
    pub pix_fmt: String,
    /// Additional raw ffmpeg arguments appended after the standard flags.
    ///
    /// Use sparingly — invalid arguments will surface as [`Error::FfmpegFailed`].
    pub extra_args: Vec<String>,
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            crf: 23,
            pix_fmt: "yuv420p".into(),
            extra_args: Vec::new(),
        }
    }
}

// ── Encoder ───────────────────────────────────────────────────────────────────

/// Encode `frames` as an MP4 file, returning the encoded bytes.
///
/// Requires `ffmpeg` to be available on `PATH`.
///
/// `frames` is a slice of `(PixelBuffer, duration_ms)` pairs. All buffers must
/// share the same dimensions.
///
/// Frame timing is converted to a constant framerate by computing the
/// average frame duration and passing it as `-framerate` to ffmpeg. Per-frame
/// duration variance is therefore flattened; GIF or WebP are better formats
/// when per-frame timing precision matters.
///
/// # Errors
///
/// - [`Error::NoAnimationFrames`] if `frames` is empty.
/// - [`Error::AnimFrameSizeMismatch`] if any buffer differs in size.
/// - [`Error::FfmpegNotFound`] if `ffmpeg` is not on `PATH`.
/// - [`Error::FfmpegFailed`] if ffmpeg exits non-zero.
/// - [`Error::Io`] for any temporary-file I/O failure.
pub fn encode_mp4(frames: &[(PixelBuffer, u32)], options: &VideoOptions) -> Result<Vec<u8>> {
    if frames.is_empty() {
        return Err(Error::NoAnimationFrames);
    }

    let (first_buf, _) = &frames[0];
    let width = first_buf.width();
    let height = first_buf.height();

    for (i, (buf, _)) in frames.iter().enumerate().skip(1) {
        if buf.width() != width || buf.height() != height {
            return Err(Error::AnimFrameSizeMismatch {
                index: i,
                expected_w: width,
                expected_h: height,
                actual_w: buf.width(),
                actual_h: buf.height(),
            });
        }
    }

    // Verify ffmpeg is available before writing any temp files.
    verify_ffmpeg()?;

    // Write frames to a temp directory.
    let temp_dir = TempDir::create()?;
    write_frames_as_png(&temp_dir, frames)?;

    // Compute average framerate from per-frame durations.
    let fps = compute_fps(frames);

    // Run ffmpeg.
    let output_path = temp_dir.path.join("output.mp4");
    run_ffmpeg(&temp_dir.path, frames.len(), fps, &output_path, options)?;

    // Read the output file back into memory.
    let bytes = std::fs::read(&output_path)?;

    Ok(bytes)
    // `temp_dir` is dropped here, cleaning up the temporary directory.
}

// ── ffmpeg detection ──────────────────────────────────────────────────────────

fn verify_ffmpeg() -> Result<()> {
    let status = Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => {
            // ffmpeg exited non-zero from `-version` — very unusual, but treat
            // as "found but broken" rather than "not found".
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::FfmpegNotFound),
        Err(e) => Err(Error::Io(e)),
    }
}

// ── Frame writing ─────────────────────────────────────────────────────────────

fn write_frames_as_png(temp_dir: &TempDir, frames: &[(PixelBuffer, u32)]) -> Result<()> {
    use image::{ImageFormat, RgbaImage};

    for (i, (buf, _)) in frames.iter().enumerate() {
        let w = buf.width();
        let h = buf.height();
        let mut rgba_data = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            if let Some(row) = buf.row(y) {
                rgba_data.extend_from_slice(row);
            } else {
                rgba_data.extend(std::iter::repeat_n(0u8, w as usize * 4));
            }
        }

        let img = RgbaImage::from_raw(w, h, rgba_data).ok_or_else(|| {
            Error::Io(std::io::Error::other(
                "failed to construct RgbaImage from pixel buffer",
            ))
        })?;

        let path = temp_dir.path.join(format!("frame{i:06}.png"));
        let mut file = std::fs::File::create(&path)?;
        img.write_to(&mut std::io::BufWriter::new(&mut file), ImageFormat::Png)
            .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;
    }
    Ok(())
}

// ── FPS computation ───────────────────────────────────────────────────────────

fn compute_fps(frames: &[(PixelBuffer, u32)]) -> f64 {
    if frames.is_empty() {
        return 25.0;
    }
    let total_ms: u64 = frames.iter().map(|(_, d)| u64::from(*d)).sum();
    // Keep the mean in f64 throughout — integer-dividing first floors the
    // fractional part and shortens the export. A 1000-frame export with
    // 1.5 ms duration each used to round the mean to 1 ms (1000 fps)
    // instead of the real ~666 fps.
    #[allow(clippy::cast_precision_loss)]
    let avg_ms = total_ms as f64 / frames.len() as f64;
    if avg_ms <= 0.0 {
        return 60.0;
    }
    // Round to two decimal places to avoid ffmpeg's rational-number quirks.
    let fps = 1000.0 / avg_ms;
    (fps * 100.0).round() / 100.0
}

// ── ffmpeg invocation ─────────────────────────────────────────────────────────

fn run_ffmpeg(
    frame_dir: &std::path::Path,
    _frame_count: usize,
    fps: f64,
    output_path: &std::path::Path,
    options: &VideoOptions,
) -> Result<()> {
    let input_pattern = frame_dir.join("frame%06d.png");

    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y", // overwrite output without prompting
        "-framerate",
        &format!("{fps:.2}"),
        "-i",
        &input_pattern.to_string_lossy(),
        "-c:v",
        "libx264",
        "-crf",
        &options.crf.to_string(),
        "-pix_fmt",
        &options.pix_fmt,
    ]);

    for arg in &options.extra_args {
        cmd.arg(arg);
    }

    cmd.arg(output_path);

    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::FfmpegNotFound
        } else {
            Error::Io(e)
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(Error::FfmpegFailed {
            code: output.status.code(),
            stderr,
        });
    }

    Ok(())
}

// ── Temp directory RAII guard ─────────────────────────────────────────────────

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn create() -> Result<Self> {
        // The previous implementation used SystemTime::subsec_nanos() which
        // repeats every second; two concurrent encodes in the same second
        // would land in the same dir and create_dir_all silently reuses an
        // existing one, leaking PNGs across runs.
        //
        // Combine pid + monotonic-ish nanos + an in-process counter so
        // collisions inside the same process are impossible and across
        // processes are vanishingly rare. Use create_dir (not _all) so the
        // call fails if the path is already taken; retry with a fresh
        // suffix until success or we hit the retry cap.
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let pid = std::process::id();
        let base = std::env::temp_dir();

        for attempt in 0..32 {
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| {
                d.as_secs()
                    .saturating_mul(1_000_000_000)
                    .saturating_add(u64::from(d.subsec_nanos()))
            });
            let count = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!("pixhaus_mp4_{pid}_{nanos}_{count}_{attempt}"));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(Error::Io(e)),
            }
        }
        Err(Error::Io(std::io::Error::other(
            "could not allocate a unique temp directory after 32 attempts",
        )))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Best-effort cleanup; ignore errors so the caller is not impacted.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::canvas::PixelBuffer;
    use pixhaus_core::project::Rgba;

    fn solid_buf(w: u32, h: u32, color: Rgba) -> PixelBuffer {
        PixelBuffer::filled(w, h, color).unwrap()
    }

    #[test]
    fn rejects_empty_frames() {
        // The ffmpeg check runs before the early-return guard, so temporarily
        // bypass by verifying the empty-frame path in isolation.
        let opts = VideoOptions::default();
        let result = encode_mp4(&[], &opts);
        assert!(matches!(result, Err(Error::NoAnimationFrames)));
    }

    #[test]
    fn rejects_mismatched_sizes() {
        let frames = vec![
            (solid_buf(4, 4, Rgba::opaque(255, 0, 0)), 100u32),
            (solid_buf(8, 8, Rgba::opaque(0, 255, 0)), 100u32),
        ];
        // This error triggers before the ffmpeg check.
        let result = encode_mp4(&frames, &VideoOptions::default());
        assert!(matches!(
            result,
            Err(Error::AnimFrameSizeMismatch { index: 1, .. })
        ));
    }

    #[test]
    fn compute_fps_typical_animation() {
        let frames: Vec<(PixelBuffer, u32)> = (0..4)
            .map(|_| (solid_buf(4, 4, Rgba::opaque(0, 0, 0)), 100u32))
            .collect();
        let fps = compute_fps(&frames);
        // 100 ms per frame → 10 fps.
        assert!((fps - 10.0).abs() < 0.01);
    }

    #[test]
    fn compute_fps_zero_duration_clamps_to_60() {
        let frames = vec![(solid_buf(4, 4, Rgba::opaque(0, 0, 0)), 0u32)];
        let fps = compute_fps(&frames);
        assert!((fps - 60.0).abs() < 0.01);
    }

    #[test]
    fn compute_fps_empty_returns_25() {
        let fps = compute_fps(&[]);
        assert!((fps - 25.0).abs() < 0.01);
    }

    #[test]
    fn temp_dir_cleanup_on_drop() {
        let path = {
            let td = TempDir::create().unwrap();
            let p = td.path.clone();
            assert!(p.exists());
            p
        };
        // After drop, the directory should no longer exist.
        assert!(!path.exists());
    }
}
