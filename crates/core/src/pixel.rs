//! RGBA8 pixel data and the colour/blend value types.
//!
//! A [`PixelBuffer`] is a flat `Vec<u8>` with an explicit row `stride`, never a
//! `Vec<Vec<u8>>`: contiguous, indexable, and SIMD-friendly. The constructors guard
//! the invariants (`stride >= width * 4`, `len == stride * height`) so a buffer that
//! exists is always well-formed.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One straight-alpha RGBA8 pixel.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct Rgba {
    /// Red channel, 0..=255.
    pub r: u8,
    /// Green channel, 0..=255.
    pub g: u8,
    /// Blue channel, 0..=255.
    pub b: u8,
    /// Alpha channel, 0 transparent .. 255 opaque.
    pub a: u8,
}

impl Rgba {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };

    /// A pixel from its four channels.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// How a layer composites over what is beneath it.
///
/// Only `Normal` (source-over) for the foundation; the enum is the seam for ramps,
/// multiply, and the rest as the editor grows.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum BlendMode {
    /// Straight-alpha source-over compositing.
    #[default]
    Normal,
}

/// RGBA8 pixel data: a flat byte buffer with an explicit row stride.
///
/// `stride` is the number of bytes per row and may exceed `width * 4` to allow
/// padding. The bytes are straight-alpha RGBA, row-major, top-left origin.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PixelBuffer {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    stride: u32,
}

impl PixelBuffer {
    /// A tightly-packed (`stride == width * 4`) fully-transparent buffer.
    ///
    /// # Errors
    /// Returns [`PixelError::Overflow`] if `width * 4` or the total byte length
    /// overflows.
    pub fn new(width: u32, height: u32) -> Result<Self, PixelError> {
        let stride = width.checked_mul(4).ok_or(PixelError::Overflow)?;
        let len = (stride as usize).checked_mul(height as usize).ok_or(PixelError::Overflow)?;
        Ok(Self {
            pixels: vec![0u8; len],
            width,
            height,
            stride,
        })
    }

    /// Builds a buffer from owned RGBA8 bytes, validating the stride and length.
    ///
    /// # Errors
    /// Returns [`PixelError::StrideTooSmall`] if `stride < width * 4`,
    /// [`PixelError::LengthMismatch`] if `pixels.len() != stride * height`, and
    /// [`PixelError::Overflow`] if the dimensions overflow.
    pub fn from_rgba8(width: u32, height: u32, stride: u32, pixels: Vec<u8>) -> Result<Self, PixelError> {
        let min_stride = width.checked_mul(4).ok_or(PixelError::Overflow)?;
        if stride < min_stride {
            return Err(PixelError::StrideTooSmall { stride, min: min_stride });
        }
        let expected = (stride as usize).checked_mul(height as usize).ok_or(PixelError::Overflow)?;
        if pixels.len() != expected {
            return Err(PixelError::LengthMismatch { len: pixels.len(), expected });
        }
        Ok(Self { pixels, width, height, stride })
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Bytes per row (`>= width * 4`).
    pub fn stride(&self) -> u32 {
        self.stride
    }

    /// The raw RGBA bytes, row-major at `stride` bytes per row.
    pub fn as_bytes(&self) -> &[u8] {
        &self.pixels
    }

    /// Consumes the buffer and returns its bytes (used by undo to reclaim pixels).
    pub fn into_bytes(self) -> Vec<u8> {
        self.pixels
    }

    /// The pixel at `(x, y)`, or `None` if out of bounds.
    pub fn pixel(&self, x: u32, y: u32) -> Option<Rgba> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = (y as usize) * (self.stride as usize) + (x as usize) * 4;
        let px = self.pixels.get(offset..offset + 4)?;
        Some(Rgba {
            r: px[0],
            g: px[1],
            b: px[2],
            a: px[3],
        })
    }
}

/// Why a [`PixelBuffer`] could not be constructed.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PixelError {
    /// `stride` is smaller than the minimum `width * 4`.
    #[error("stride {stride} is smaller than width*4 ({min})")]
    StrideTooSmall {
        /// The stride that was given.
        stride: u32,
        /// The minimum stride `width * 4`.
        min: u32,
    },
    /// `pixels.len()` does not equal `stride * height`.
    #[error("pixel buffer length {len} does not match stride*height ({expected})")]
    LengthMismatch {
        /// The length that was given.
        len: usize,
        /// The expected length `stride * height`.
        expected: usize,
    },
    /// The dimensions overflow a `usize` byte count.
    #[error("image dimensions overflow")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn new_is_transparent_and_tightly_packed() {
        let buf = PixelBuffer::new(3, 2).unwrap();
        assert_eq!(buf.width(), 3);
        assert_eq!(buf.height(), 2);
        assert_eq!(buf.stride(), 12);
        assert!(buf.as_bytes().iter().all(|&b| b == 0));
        assert_eq!(buf.pixel(0, 0), Some(Rgba::TRANSPARENT));
    }

    #[rstest]
    #[case(2, 2, 7, vec![0u8; 28])] // stride 7 < width*4 = 8
    fn from_rgba8_rejects_stride_too_small(#[case] w: u32, #[case] h: u32, #[case] stride: u32, #[case] bytes: Vec<u8>) {
        let err = PixelBuffer::from_rgba8(w, h, stride, bytes).unwrap_err();
        assert!(matches!(err, PixelError::StrideTooSmall { .. }));
    }

    #[test]
    fn from_rgba8_rejects_length_mismatch() {
        let err = PixelBuffer::from_rgba8(2, 2, 8, vec![0u8; 15]).unwrap_err();
        assert_eq!(err, PixelError::LengthMismatch { len: 15, expected: 16 });
    }

    #[test]
    fn pixel_reads_back_and_is_none_out_of_bounds() {
        let bytes = vec![10, 20, 30, 40, 50, 60, 70, 80];
        let buf = PixelBuffer::from_rgba8(2, 1, 8, bytes).unwrap();
        assert_eq!(buf.pixel(0, 0), Some(Rgba::new(10, 20, 30, 40)));
        assert_eq!(buf.pixel(1, 0), Some(Rgba::new(50, 60, 70, 80)));
        assert_eq!(buf.pixel(2, 0), None);
        assert_eq!(buf.pixel(0, 1), None);
    }
}
