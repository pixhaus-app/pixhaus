//! Animated export: GIF, animated WebP, and MP4.
//!
//! All three encoders accept a slice of `(PixelBuffer, duration_ms)` pairs
//! where all buffers must share the same dimensions.
//!
//! # Format guide
//!
//! | Format | Best for | Notes |
//! |--------|----------|-------|
//! | GIF    | pixel art, indexed colour | 256-colour limit; lossless when palette fits |
//! | WebP   | pixel art, full RGBA      | lossless VP8L recommended; smaller than GIF |
//! | MP4    | sharing / video           | requires `ffmpeg` on PATH; h.264 + yuv420p |
//!
//! See `docs/export-formats.md` for per-format quality knob documentation.

pub mod decode;
pub mod dither;
pub mod gif;
pub mod mp4;
pub mod webp;

// ── Re-exports ────────────────────────────────────────────────────────────────

pub use decode::{DecodeOptions, DecodedFrame, decode_video};
pub use dither::{DitherMode, apply as apply_dither, palette_to_rgba4};
pub use gif::{GifOptions, LoopCount, PaletteMode, encode_gif};
pub use mp4::{VideoOptions, encode_mp4};
pub use webp::{WebPOptions, encode_webp};
