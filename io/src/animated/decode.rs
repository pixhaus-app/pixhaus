//! Video clip decode via `ffmpeg` shell-out (i2v walk-cycle path).
//!
//! The image-to-video backend returns an MP4/WebM clip; the walk-cycle
//! pipeline needs that clip as a sequence of RGBA frames to frame-pick a
//! clean loop. We decode the same way [`super::mp4`] encodes — by shelling
//! out to `ffmpeg` on `PATH` rather than linking an LGPL decoder into the
//! binary.
//!
//! # Decode pipeline
//!
//! 1. Write the clip bytes to a temporary input file.
//! 2. Invoke `ffmpeg -i clip -vf fps=N -vsync 0 frame%06d.png`.
//! 3. Read the produced PNGs back into [`PixelBuffer`]s in order.
//! 4. Delete the temp directory.
//!
//! Frames are extracted at a fixed `fps` so each frame's presentation
//! timestamp is exact (`index * 1000 / fps` ms), which is what the frame
//! picker needs to space evenly across one cycle.

use std::process::Command;

use pixhaus_core::canvas::PixelBuffer;
use tempfile::TempDir;

use crate::error::{Error, Result};

/// Options for [`decode_video`].
#[derive(Clone, Debug)]
pub struct DecodeOptions {
    /// Frames per second to extract. The clip is resampled to this rate so
    /// frame timestamps are deterministic.
    pub fps: u32,
    /// Cap on the number of frames returned. `None` decodes the whole clip.
    /// The picker only needs a few seconds of a walk, so capping keeps the
    /// decode bounded for long clips.
    pub max_frames: Option<u32>,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            fps: 12,
            max_frames: Some(120),
        }
    }
}

/// One decoded frame: its pixels plus presentation timestamp.
#[derive(Clone, Debug)]
pub struct DecodedFrame {
    /// RGBA pixel data, tightly packed.
    pub buffer: PixelBuffer,
    /// Presentation timestamp in milliseconds from the clip start.
    pub timestamp_ms: u32,
}

/// Maps a clip MIME type to a file extension ffmpeg recognises.
fn ext_for_mime(mime: &str) -> &'static str {
    match mime {
        "video/webm" => "webm",
        "image/gif" => "gif",
        _ => "mp4",
    }
}

/// Decodes `clip` into a sequence of RGBA frames at the requested fps.
///
/// Requires `ffmpeg` on `PATH`.
///
/// # Errors
///
/// - [`Error::FfmpegNotFound`] if `ffmpeg` is not on `PATH`.
/// - [`Error::FfmpegFailed`] if ffmpeg exits non-zero.
/// - [`Error::VideoDecodeFailed`] if no frames were produced.
/// - [`Error::Io`] for any temporary-file I/O failure.
pub fn decode_video(clip: &[u8], mime: &str, options: &DecodeOptions) -> Result<Vec<DecodedFrame>> {
    if clip.is_empty() {
        return Err(Error::VideoDecodeFailed("clip is empty".into()));
    }
    let fps = options.fps.max(1);

    let temp_dir = TempDir::new().map_err(Error::Io)?;
    let input_path = temp_dir.path().join(format!("clip.{}", ext_for_mime(mime)));
    std::fs::write(&input_path, clip).map_err(Error::Io)?;

    let output_pattern = temp_dir.path().join("frame%06d.png");
    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-i",
        &input_path.to_string_lossy(),
        "-vf",
        &format!("fps={fps}"),
        "-vsync",
        "0",
    ]);
    if let Some(max) = options.max_frames {
        cmd.args(["-frames:v", &max.to_string()]);
    }
    cmd.arg(&output_pattern);

    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::FfmpegNotFound
        } else {
            Error::Io(e)
        }
    })?;
    if !output.status.success() {
        return Err(Error::FfmpegFailed {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    // Collect produced PNGs in filename order (ffmpeg zero-pads to 6 digits).
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(temp_dir.path())
        .map_err(Error::Io)?
        .map(|entry| entry.map(|e| e.path()).map_err(Error::Io))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|p| p.extension().is_some_and(|x| x == "png"))
        .collect();
    entries.sort();

    let mut frames = Vec::with_capacity(entries.len());
    for (i, path) in entries.iter().enumerate() {
        let bytes = std::fs::read(path).map_err(Error::Io)?;
        let img = image::load_from_memory(&bytes)
            .map_err(|e| Error::VideoDecodeFailed(format!("frame decode failed: {e}")))?
            .to_rgba8();
        let buffer = PixelBuffer::from_image(img);
        #[allow(clippy::cast_possible_truncation)]
        let timestamp_ms = (i as u64 * 1000 / u64::from(fps)) as u32;
        frames.push(DecodedFrame {
            buffer,
            timestamp_ms,
        });
    }

    if frames.is_empty() {
        return Err(Error::VideoDecodeFailed(
            "ffmpeg produced no frames from the clip".into(),
        ));
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_clip() {
        let res = decode_video(&[], "video/mp4", &DecodeOptions::default());
        assert!(matches!(res, Err(Error::VideoDecodeFailed(_))));
    }

    #[test]
    fn ext_mapping() {
        assert_eq!(ext_for_mime("video/webm"), "webm");
        assert_eq!(ext_for_mime("video/mp4"), "mp4");
        assert_eq!(ext_for_mime("application/octet-stream"), "mp4");
    }

    #[test]
    fn default_options_reasonable() {
        let o = DecodeOptions::default();
        assert_eq!(o.fps, 12);
        assert_eq!(o.max_frames, Some(120));
    }
}
