//! Pixel buffer storage for the editor.
//!
//! The canvas subsystem owns two concrete buffer kinds:
//!
//! - [`PixelBuffer`] — an 8-bit-per-channel RGBA buffer, the universal
//!   intermediate that every compositor pass operates on.
//! - [`IndexedBuffer`] — an 8-bit-per-pixel palette-index buffer, the
//!   storage form for indexed-mode sprites; resolves to [`PixelBuffer`]
//!   at the rendering boundary.
//!
//! Both types carry an explicit `stride` (bytes per row). Stride may
//! *exceed* `width * bytes_per_pixel` to accommodate SIMD alignment;
//! it must never be smaller. Constructors enforce this invariant.
//!
//! Pixel data lives in a flat `Vec<u8>`. The conventions doc is
//! deliberate about this — `Vec<Vec<u8>>` fragments memory, defeats
//! cache locality, and breaks `rayon`'s row-parallel patterns.

use crate::project::Rgba;

use super::error::{Error, Result};

/// Bytes per pixel for an RGBA buffer.
pub const RGBA_BYTES_PER_PIXEL: usize = 4;

/// `RGBA_BYTES_PER_PIXEL` typed as `u32` for stride math without
/// truncation casts. The value is fixed at 4 — see the const above.
pub const RGBA_BYTES_PER_PIXEL_U32: u32 = 4;

/// 8-bit-per-channel RGBA pixel buffer with explicit row stride.
///
/// `stride >= width * 4`. Out-of-band rows can hold scratch padding
/// for SIMD; the buffer never reads or writes them, so callers may
/// leave them uninitialised. Pixel access methods clamp to `width`
/// and ignore the padding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PixelBuffer {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    stride: u32,
}

impl PixelBuffer {
    /// Constructs an empty (`0 × 0`) buffer.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            pixels: Vec::new(),
            width: 0,
            height: 0,
            stride: 0,
        }
    }

    /// Constructs an opaque-black buffer of the given dimensions.
    /// Stride is set to `width * 4`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::StrideTooSmall`] if `width` is zero alongside a
    /// non-zero `height` (the resulting stride would be ambiguous);
    /// callers wanting an empty buffer should use [`PixelBuffer::empty`].
    pub fn new(width: u32, height: u32) -> Result<Self> {
        Self::filled(width, height, Rgba::transparent())
    }

    /// Constructs a buffer of the given dimensions filled with `color`.
    /// Stride is set to `width * 4`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::StrideTooSmall`] if `width` is zero with a
    /// non-zero `height`.
    pub fn filled(width: u32, height: u32, color: Rgba) -> Result<Self> {
        if width == 0 && height == 0 {
            return Ok(Self::empty());
        }
        if width == 0 {
            return Err(Error::StrideTooSmall {
                stride: 0,
                min_stride: 4,
            });
        }
        let stride = width
            .checked_mul(RGBA_BYTES_PER_PIXEL_U32)
            .ok_or(Error::StrideTooSmall {
                stride: 0,
                min_stride: u32::MAX,
            })?;
        let len =
            (stride as usize)
                .checked_mul(height as usize)
                .ok_or(Error::ByteLengthMismatch {
                    expected: usize::MAX,
                    actual: 0,
                })?;
        let mut pixels = vec![0u8; len];
        let bytes = color.to_array();
        for chunk in pixels.chunks_exact_mut(RGBA_BYTES_PER_PIXEL) {
            chunk.copy_from_slice(&bytes);
        }
        Ok(Self {
            pixels,
            width,
            height,
            stride,
        })
    }

    /// Constructs a buffer from raw pixel bytes laid out at the given
    /// stride.
    ///
    /// # Errors
    ///
    /// - [`Error::StrideTooSmall`] if `stride < width * 4`.
    /// - [`Error::ByteLengthMismatch`] if `pixels.len() != stride * height`.
    pub fn from_raw(width: u32, height: u32, stride: u32, pixels: Vec<u8>) -> Result<Self> {
        let min_stride =
            width
                .checked_mul(RGBA_BYTES_PER_PIXEL_U32)
                .ok_or(Error::StrideTooSmall {
                    stride,
                    min_stride: u32::MAX,
                })?;
        if stride < min_stride {
            return Err(Error::StrideTooSmall { stride, min_stride });
        }
        let expected =
            (stride as usize)
                .checked_mul(height as usize)
                .ok_or(Error::ByteLengthMismatch {
                    expected: usize::MAX,
                    actual: pixels.len(),
                })?;
        if pixels.len() != expected {
            return Err(Error::ByteLengthMismatch {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            pixels,
            width,
            height,
            stride,
        })
    }

    /// Width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Row stride in bytes. `>= width * 4`.
    #[must_use]
    pub const fn stride(&self) -> u32 {
        self.stride
    }

    /// Whether the buffer contains zero pixels.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Borrows the raw byte buffer. Includes any padding bytes between
    /// `width * 4` and `stride` per row.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.pixels
    }

    /// Mutably borrows the raw byte buffer. Same layout as [`Self::as_bytes`].
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }

    /// Returns the pixel at `(x, y)` or `None` if the coordinates fall
    /// outside the buffer.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<Rgba> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = (y as usize) * (self.stride as usize) + (x as usize) * RGBA_BYTES_PER_PIXEL;
        let bytes = self.pixels.get(offset..offset + RGBA_BYTES_PER_PIXEL)?;
        Some(Rgba::new(bytes[0], bytes[1], bytes[2], bytes[3]))
    }

    /// Writes `color` at `(x, y)`. No-op if the coordinates are out of
    /// bounds — the caller is responsible for clipping if a hard error
    /// is desired.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Rgba) {
        if x >= self.width || y >= self.height {
            return;
        }
        let offset = (y as usize) * (self.stride as usize) + (x as usize) * RGBA_BYTES_PER_PIXEL;
        if let Some(slot) = self.pixels.get_mut(offset..offset + RGBA_BYTES_PER_PIXEL) {
            slot.copy_from_slice(&color.to_array());
        }
    }

    /// Returns one row's worth of pixel bytes (length = `width * 4`,
    /// trimmed of any padding bytes).
    #[must_use]
    pub fn row(&self, y: u32) -> Option<&[u8]> {
        if y >= self.height {
            return None;
        }
        let row_start = (y as usize) * (self.stride as usize);
        let row_len = (self.width as usize) * RGBA_BYTES_PER_PIXEL;
        self.pixels.get(row_start..row_start + row_len)
    }

    /// Mutable counterpart to [`Self::row`].
    pub fn row_mut(&mut self, y: u32) -> Option<&mut [u8]> {
        if y >= self.height {
            return None;
        }
        let row_start = (y as usize) * (self.stride as usize);
        let row_len = (self.width as usize) * RGBA_BYTES_PER_PIXEL;
        self.pixels.get_mut(row_start..row_start + row_len)
    }

    /// Iterator over decoded pixels in row-major order. Padding bytes
    /// in the stride are skipped.
    pub fn pixels(&self) -> impl Iterator<Item = Rgba> + '_ {
        let width = self.width as usize;
        let stride = self.stride as usize;
        (0..self.height as usize).flat_map(move |y| {
            let start = y * stride;
            (0..width).map(move |x| {
                let off = start + x * RGBA_BYTES_PER_PIXEL;
                let bytes = &self.pixels[off..off + RGBA_BYTES_PER_PIXEL];
                Rgba::new(bytes[0], bytes[1], bytes[2], bytes[3])
            })
        })
    }

    /// Borrows the underlying [`image::RgbaImage`] view. Only available
    /// when `stride == width * 4`; tightly-packed buffers round-trip
    /// to and from the `image` crate without an extra copy.
    #[must_use]
    pub fn as_image(&self) -> Option<image::ImageBuffer<image::Rgba<u8>, &[u8]>> {
        if self.stride != self.width * RGBA_BYTES_PER_PIXEL_U32 {
            return None;
        }
        image::ImageBuffer::from_raw(self.width, self.height, self.pixels.as_slice())
    }

    /// Constructs a buffer from a tightly-packed [`image::RgbaImage`].
    /// Stride is `width * 4`.
    #[must_use]
    pub fn from_image(img: image::RgbaImage) -> Self {
        let (width, height) = img.dimensions();
        let pixels = img.into_raw();
        Self {
            pixels,
            width,
            height,
            stride: width * RGBA_BYTES_PER_PIXEL_U32,
        }
    }

    /// Fills every pixel with `color`. Padding bytes are left untouched.
    pub fn fill(&mut self, color: Rgba) {
        let bytes = color.to_array();
        let width = self.width as usize;
        let stride = self.stride as usize;
        for y in 0..self.height as usize {
            let row_start = y * stride;
            let row = &mut self.pixels[row_start..row_start + width * RGBA_BYTES_PER_PIXEL];
            for chunk in row.chunks_exact_mut(RGBA_BYTES_PER_PIXEL) {
                chunk.copy_from_slice(&bytes);
            }
        }
    }
}

impl Default for PixelBuffer {
    fn default() -> Self {
        Self::empty()
    }
}

/// 8-bit-per-pixel palette-index buffer.
///
/// One index per pixel; resolution to RGBA happens via
/// [`IndexedBuffer::resolve`] using the active palette. Stride is in
/// bytes (== width unless the buffer carries SIMD padding).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedBuffer {
    indices: Vec<u8>,
    width: u32,
    height: u32,
    stride: u32,
}

impl IndexedBuffer {
    /// Constructs an empty buffer.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            indices: Vec::new(),
            width: 0,
            height: 0,
            stride: 0,
        }
    }

    /// Constructs a buffer filled with `index`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::StrideTooSmall`] when `width == 0` but
    /// `height != 0`.
    pub fn filled(width: u32, height: u32, index: u8) -> Result<Self> {
        if width == 0 && height == 0 {
            return Ok(Self::empty());
        }
        if width == 0 {
            return Err(Error::StrideTooSmall {
                stride: 0,
                min_stride: 1,
            });
        }
        let stride = width;
        let len =
            (stride as usize)
                .checked_mul(height as usize)
                .ok_or(Error::ByteLengthMismatch {
                    expected: usize::MAX,
                    actual: 0,
                })?;
        Ok(Self {
            indices: vec![index; len],
            width,
            height,
            stride,
        })
    }

    /// Constructs a buffer from raw indices.
    ///
    /// # Errors
    ///
    /// - [`Error::StrideTooSmall`] if `stride < width`.
    /// - [`Error::ByteLengthMismatch`] if `indices.len() != stride * height`.
    pub fn from_raw(width: u32, height: u32, stride: u32, indices: Vec<u8>) -> Result<Self> {
        if stride < width {
            return Err(Error::StrideTooSmall {
                stride,
                min_stride: width,
            });
        }
        let expected =
            (stride as usize)
                .checked_mul(height as usize)
                .ok_or(Error::ByteLengthMismatch {
                    expected: usize::MAX,
                    actual: indices.len(),
                })?;
        if indices.len() != expected {
            return Err(Error::ByteLengthMismatch {
                expected,
                actual: indices.len(),
            });
        }
        Ok(Self {
            indices,
            width,
            height,
            stride,
        })
    }

    /// Width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Stride in bytes. `>= width`.
    #[must_use]
    pub const fn stride(&self) -> u32 {
        self.stride
    }

    /// Reads the palette index at `(x, y)`.
    #[must_use]
    pub fn index_at(&self, x: u32, y: u32) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = (y as usize) * (self.stride as usize) + (x as usize);
        self.indices.get(offset).copied()
    }

    /// Writes the palette index at `(x, y)`. No-op if out of bounds.
    pub fn set_index(&mut self, x: u32, y: u32, index: u8) {
        if x >= self.width || y >= self.height {
            return;
        }
        let offset = (y as usize) * (self.stride as usize) + (x as usize);
        if let Some(slot) = self.indices.get_mut(offset) {
            *slot = index;
        }
    }

    /// Borrows the raw index bytes (includes padding between `width`
    /// and `stride`).
    #[must_use]
    pub fn as_indices(&self) -> &[u8] {
        &self.indices
    }

    /// Resolves indices through `palette` into a [`PixelBuffer`]. The
    /// transparent index — usually 0 — should be set to
    /// [`Rgba::transparent`] in the palette so it composites correctly.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaletteIndexOutOfRange`] the first time an
    /// index falls outside `palette`.
    pub fn resolve(&self, palette: &[Rgba]) -> Result<PixelBuffer> {
        let mut out = PixelBuffer::new(self.width, self.height)?;
        let palette_len = palette.len();
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = self.index_at(x, y).ok_or(Error::ByteLengthMismatch {
                    expected: (self.stride as usize) * (self.height as usize),
                    actual: self.indices.len(),
                })?;
                let color = *palette
                    .get(idx as usize)
                    .ok_or(Error::PaletteIndexOutOfRange {
                        index: idx,
                        palette_len,
                    })?;
                out.set_pixel(x, y, color);
            }
        }
        Ok(out)
    }
}

impl Default for IndexedBuffer {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_has_zero_size() {
        let b = PixelBuffer::empty();
        assert_eq!(b.width(), 0);
        assert_eq!(b.height(), 0);
        assert!(b.is_empty());
        assert!(b.as_bytes().is_empty());
    }

    #[test]
    fn new_buffer_is_transparent() {
        let b = PixelBuffer::new(4, 4).unwrap();
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(b.pixel(x, y), Some(Rgba::transparent()));
            }
        }
    }

    #[test]
    fn filled_writes_every_pixel() {
        let red = Rgba::opaque(255, 0, 0);
        let b = PixelBuffer::filled(8, 8, red).unwrap();
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(b.pixel(x, y), Some(red));
            }
        }
    }

    #[test]
    fn out_of_bounds_pixel_returns_none() {
        let b = PixelBuffer::new(8, 8).unwrap();
        assert_eq!(b.pixel(8, 0), None);
        assert_eq!(b.pixel(0, 8), None);
        assert_eq!(b.pixel(100, 100), None);
    }

    #[test]
    fn set_pixel_round_trips() {
        let mut b = PixelBuffer::new(4, 4).unwrap();
        let c = Rgba::new(10, 20, 30, 40);
        b.set_pixel(1, 2, c);
        assert_eq!(b.pixel(1, 2), Some(c));
    }

    #[test]
    fn from_raw_rejects_short_stride() {
        let pixels = vec![0u8; 16];
        let err = PixelBuffer::from_raw(4, 4, 4, pixels).unwrap_err();
        assert_eq!(
            err,
            Error::StrideTooSmall {
                stride: 4,
                min_stride: 16
            }
        );
    }

    #[test]
    fn from_raw_rejects_wrong_length() {
        let pixels = vec![0u8; 32];
        let err = PixelBuffer::from_raw(4, 4, 16, pixels).unwrap_err();
        assert_eq!(
            err,
            Error::ByteLengthMismatch {
                expected: 64,
                actual: 32
            }
        );
    }

    #[test]
    fn from_raw_accepts_padding_stride() {
        let pixels = vec![0u8; 4 * 4 * 4 + 4 * 8];
        let b = PixelBuffer::from_raw(4, 4, 24, pixels).unwrap();
        assert_eq!(b.width(), 4);
        assert_eq!(b.height(), 4);
        assert_eq!(b.stride(), 24);
        assert_eq!(b.row(0).map(<[u8]>::len), Some(16));
    }

    #[test]
    fn pixels_iter_skips_padding() {
        let mut pixels = vec![0u8; 24 * 2];
        pixels[0..16].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let b = PixelBuffer::from_raw(4, 2, 24, pixels).unwrap();
        let collected: Vec<_> = b.pixels().take(4).collect();
        assert_eq!(
            collected,
            vec![
                Rgba::new(1, 2, 3, 4),
                Rgba::new(5, 6, 7, 8),
                Rgba::new(9, 10, 11, 12),
                Rgba::new(13, 14, 15, 16),
            ]
        );
    }

    #[test]
    fn from_image_round_trip() {
        let mut img = image::RgbaImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgba([1, 2, 3, 4]));
        img.put_pixel(1, 0, image::Rgba([5, 6, 7, 8]));
        img.put_pixel(0, 1, image::Rgba([9, 10, 11, 12]));
        img.put_pixel(1, 1, image::Rgba([13, 14, 15, 16]));
        let b = PixelBuffer::from_image(img);
        assert_eq!(b.pixel(0, 0), Some(Rgba::new(1, 2, 3, 4)));
        assert_eq!(b.pixel(1, 1), Some(Rgba::new(13, 14, 15, 16)));
    }

    #[test]
    fn as_image_returns_view_for_packed_buffer() {
        let b = PixelBuffer::filled(4, 4, Rgba::opaque(10, 20, 30)).unwrap();
        let view = b
            .as_image()
            .expect("packed buffer should expose image view");
        assert_eq!(view.dimensions(), (4, 4));
    }

    #[test]
    fn as_image_returns_none_for_padded_buffer() {
        let b = PixelBuffer::from_raw(2, 2, 16, vec![0u8; 32]).unwrap();
        assert!(b.as_image().is_none());
    }

    #[test]
    fn fill_overwrites_pixels() {
        let mut b = PixelBuffer::new(4, 4).unwrap();
        let c = Rgba::opaque(50, 100, 150);
        b.fill(c);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(b.pixel(x, y), Some(c));
            }
        }
    }

    #[test]
    fn indexed_buffer_resolves_through_palette() {
        let palette = vec![
            Rgba::transparent(),
            Rgba::opaque(255, 0, 0),
            Rgba::opaque(0, 255, 0),
        ];
        let mut idx = IndexedBuffer::filled(2, 2, 0).unwrap();
        idx.set_index(0, 0, 1);
        idx.set_index(1, 1, 2);
        let resolved = idx.resolve(&palette).unwrap();
        assert_eq!(resolved.pixel(0, 0), Some(Rgba::opaque(255, 0, 0)));
        assert_eq!(resolved.pixel(1, 0), Some(Rgba::transparent()));
        assert_eq!(resolved.pixel(0, 1), Some(Rgba::transparent()));
        assert_eq!(resolved.pixel(1, 1), Some(Rgba::opaque(0, 255, 0)));
    }

    #[test]
    fn indexed_resolve_surfaces_out_of_range_index() {
        let palette = vec![Rgba::transparent(), Rgba::opaque(255, 0, 0)];
        let mut idx = IndexedBuffer::filled(2, 1, 0).unwrap();
        idx.set_index(1, 0, 5);
        let err = idx.resolve(&palette).unwrap_err();
        assert_eq!(
            err,
            Error::PaletteIndexOutOfRange {
                index: 5,
                palette_len: 2
            }
        );
    }
}
