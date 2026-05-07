//! Little-endian byte readers for the Aseprite format.
//!
//! Aseprite stores every multi-byte field little-endian. This module is
//! a thin, allocation-free cursor over a borrowed slice. Each reader
//! returns [`Error::Truncated`] when the requested number of bytes is
//! not available rather than panicking on slice indexing.

use crate::error::{Error, Result};

/// Cursor over a borrowed byte slice with little-endian readers.
///
/// `position` advances on every successful read; failures leave the
/// cursor unchanged so the caller can decide whether to bail or skip.
#[derive(Debug)]
pub struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    /// Constructs a cursor positioned at the start of `bytes`.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    /// Returns the current byte offset.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    /// Returns the number of bytes still available.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    /// Returns `true` when the cursor has consumed every byte.
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        self.position >= self.bytes.len()
    }

    /// Sets the cursor's position. Used by the chunk-level parser to
    /// jump past chunks the reader chose to ignore.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Truncated`] when `position` exceeds the buffer.
    pub fn set_position(&mut self, position: usize) -> Result<()> {
        if position > self.bytes.len() {
            return Err(Error::Truncated);
        }
        self.position = position;
        Ok(())
    }

    /// Advances the cursor by `count` bytes without returning the
    /// skipped slice.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Truncated`] when fewer than `count` bytes remain.
    pub fn skip(&mut self, count: usize) -> Result<()> {
        let end = self.position.checked_add(count).ok_or(Error::Truncated)?;
        if end > self.bytes.len() {
            return Err(Error::Truncated);
        }
        self.position = end;
        Ok(())
    }

    /// Reads `count` bytes and returns them as a borrowed slice.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Truncated`] when fewer than `count` bytes remain.
    pub fn read_slice(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self.position.checked_add(count).ok_or(Error::Truncated)?;
        let slice = self.bytes.get(self.position..end).ok_or(Error::Truncated)?;
        self.position = end;
        Ok(slice)
    }

    /// Reads a single byte.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Truncated`] when the cursor is at EOF.
    pub fn read_u8(&mut self) -> Result<u8> {
        let bytes = self.read_slice(1)?;
        Ok(bytes[0])
    }

    /// Reads a little-endian `u16`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Truncated`] when fewer than 2 bytes remain.
    pub fn read_u16_le(&mut self) -> Result<u16> {
        let bytes = self.read_slice(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Reads a little-endian `i16`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Truncated`] when fewer than 2 bytes remain.
    pub fn read_i16_le(&mut self) -> Result<i16> {
        let bytes = self.read_slice(2)?;
        Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Reads a little-endian `u32`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Truncated`] when fewer than 4 bytes remain.
    pub fn read_u32_le(&mut self) -> Result<u32> {
        let bytes = self.read_slice(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Reads a little-endian `i32`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Truncated`] when fewer than 4 bytes remain.
    pub fn read_i32_le(&mut self) -> Result<i32> {
        let bytes = self.read_slice(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Reads a little-endian `u64`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Truncated`] when fewer than 8 bytes remain.
    pub fn read_u64_le(&mut self) -> Result<u64> {
        let bytes = self.read_slice(8)?;
        let mut buf = [0u8; 8];
        buf.copy_from_slice(bytes);
        Ok(u64::from_le_bytes(buf))
    }

    /// Reads an Aseprite STRING (LE u16 length prefix + UTF-8 bytes).
    /// Invalid UTF-8 is replaced with `U+FFFD` so a malformed file
    /// surfaces as a partly-readable string rather than a parse error.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Truncated`] when the prefix or the body lies
    /// past the buffer end.
    pub fn read_aseprite_string(&mut self) -> Result<String> {
        let len = self.read_u16_le()? as usize;
        let bytes = self.read_slice(len)?;
        Ok(String::from_utf8_lossy(bytes).to_string())
    }
}

/// Helpers for writing the same primitives little-endian.
pub mod write {
    /// Appends `value` to `out` as a little-endian `u16`.
    pub fn put_u16_le(out: &mut Vec<u8>, value: u16) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends `value` to `out` as a little-endian `i16`.
    pub fn put_i16_le(out: &mut Vec<u8>, value: i16) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends `value` to `out` as a little-endian `u32`.
    pub fn put_u32_le(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends `value` to `out` as a little-endian `i32`.
    pub fn put_i32_le(out: &mut Vec<u8>, value: i32) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends `value` as `count` bytes of zero. Used to satisfy the
    /// Aseprite spec's "reserved" fields.
    pub fn put_zeros(out: &mut Vec<u8>, count: usize) {
        for _ in 0..count {
            out.push(0);
        }
    }

    /// Writes an Aseprite STRING (LE u16 length prefix + UTF-8 body).
    /// Strings longer than `u16::MAX` bytes are silently truncated;
    /// callers that surface user-facing strings should validate length
    /// before calling.
    pub fn put_aseprite_string(out: &mut Vec<u8>, s: &str) {
        let len = u16::try_from(s.len()).unwrap_or(u16::MAX);
        let body = if usize::from(len) == s.len() {
            s.as_bytes()
        } else {
            // Truncate at a UTF-8 boundary at or below `len` so we never
            // emit a partial multi-byte sequence on the wire.
            let mut end = len as usize;
            while end > 0 && !s.is_char_boundary(end) {
                end -= 1;
            }
            &s.as_bytes()[..end]
        };
        put_u16_le(out, u16::try_from(body.len()).unwrap_or(u16::MAX));
        out.extend_from_slice(body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_le_primitives() {
        let bytes = [0x01u8, 0x00, 0x02, 0x00, 0x00, 0x00];
        let mut c = Cursor::new(&bytes);
        assert_eq!(c.read_u16_le().unwrap(), 1);
        assert_eq!(c.read_u32_le().unwrap(), 2);
        assert!(c.is_eof());
    }

    #[test]
    fn truncated_read_surfaces_error() {
        let bytes = [0u8];
        let mut c = Cursor::new(&bytes);
        let err = c.read_u32_le().unwrap_err();
        assert!(matches!(err, Error::Truncated));
        // Cursor unchanged on failure.
        assert_eq!(c.position(), 0);
    }

    #[test]
    fn aseprite_string_round_trips() {
        let mut out = Vec::new();
        write::put_aseprite_string(&mut out, "hello");
        let mut c = Cursor::new(&out);
        assert_eq!(c.read_aseprite_string().unwrap(), "hello");
    }

    #[test]
    fn aseprite_string_handles_utf8() {
        let mut out = Vec::new();
        write::put_aseprite_string(&mut out, "café 🎨");
        let mut c = Cursor::new(&out);
        assert_eq!(c.read_aseprite_string().unwrap(), "café 🎨");
    }

    #[test]
    fn skip_advances_position() {
        let bytes = [0u8; 16];
        let mut c = Cursor::new(&bytes);
        c.skip(7).unwrap();
        assert_eq!(c.position(), 7);
        assert_eq!(c.remaining(), 9);
    }

    #[test]
    fn skip_past_end_errors() {
        let bytes = [0u8; 4];
        let mut c = Cursor::new(&bytes);
        let err = c.skip(5).unwrap_err();
        assert!(matches!(err, Error::Truncated));
    }

    #[test]
    fn read_signed_primitives() {
        let mut out = Vec::new();
        write::put_i16_le(&mut out, -3);
        write::put_i32_le(&mut out, -1_000_000);
        let mut c = Cursor::new(&out);
        assert_eq!(c.read_i16_le().unwrap(), -3);
        assert_eq!(c.read_i32_le().unwrap(), -1_000_000);
    }
}
