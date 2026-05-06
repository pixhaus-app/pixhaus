//! Pixel-level undo/redo stack for canvas drawing operations.
//!
//! The core `History` type (from `pixhaus-core`) operates on `Project`
//! data: structural changes like adding layers or renaming sprites.
//! Pixel drawing operations instead mutate `PixelBufferEntry` bytes in
//! `DocumentStore.pixel_buffers`, which sit outside `Project`.
//!
//! `PixelHistory` is a simple linear stack of `PixelOpBatch` entries.
//! Each batch represents one user action (a stroke, a fill) and carries
//! before/after snapshots of every buffer it touched. Undo/redo swap the
//! snapshots back in place.
//!
//! The stack prunes the redo branch on every push, matching the linear
//! undo model most drawing tools offer.

use pixhaus_io::pixhaus::PixelBufferEntry;

/// A snapshot of one pixel buffer before and after a drawing operation.
pub struct PixelOp {
    /// Raw `PixelBufferId` value.
    pub buffer_id: u32,
    /// Buffer width — needed when rebuilding tile dirty payloads on
    /// undo/redo so the caller can re-emit the right tile boundaries.
    pub buf_width: u32,
    /// Buffer height.
    pub buf_height: u32,
    /// Buffer stride.
    pub buf_stride: u32,
    /// Sprite this buffer belongs to (for tile-dirty event emission).
    pub sprite_id: u32,
    /// Frame index (for tile-dirty event emission).
    pub frame_index: u32,
    /// Pixel bytes before the operation.
    pub before: Vec<u8>,
    /// Pixel bytes after the operation.
    pub after: Vec<u8>,
}

/// A group of pixel ops that undo and redo as a single unit.
pub struct PixelOpBatch {
    /// Human-readable label shown in a future history panel.
    pub label: String,
    /// Individual buffer changes in this batch.
    pub ops: Vec<PixelOp>,
}

/// Linear pixel undo/redo stack.
///
/// `cursor` points to the batch that a `redo` would reapply, or
/// equivalently one past the last applied batch. `cursor == 0` means the
/// history is at the beginning (nothing to undo); `cursor == stack.len()`
/// means everything has been applied (nothing to redo).
pub struct PixelHistory {
    stack: Vec<PixelOpBatch>,
    cursor: usize,
}

impl PixelHistory {
    /// Creates an empty pixel history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            cursor: 0,
        }
    }

    /// Whether there is a pixel op available to undo.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    /// Whether there is a pixel op available to redo.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.cursor < self.stack.len()
    }

    /// Label of the most recently applied batch, or `None` at the start.
    #[must_use]
    pub fn undo_label(&self) -> Option<&str> {
        if self.cursor == 0 {
            return None;
        }
        self.stack.get(self.cursor - 1).map(|b| b.label.as_str())
    }

    /// Push a new batch onto the stack.
    ///
    /// Truncates any redo branch that existed after the current cursor
    /// position — the same behaviour as most drawing apps.
    pub fn push(&mut self, batch: PixelOpBatch) {
        self.stack.truncate(self.cursor);
        self.stack.push(batch);
        self.cursor = self.stack.len();
    }

    /// Return the batch that was just undone (cursor decremented),
    /// or `None` when already at the beginning.
    ///
    /// The caller is responsible for applying the `before` bytes back
    /// into `DocumentStore.pixel_buffers`.
    pub fn undo(&mut self) -> Option<&PixelOpBatch> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        self.stack.get(self.cursor)
    }

    /// Return the batch that was just redone (cursor incremented),
    /// or `None` when already at the end.
    ///
    /// The caller is responsible for applying the `after` bytes back
    /// into `DocumentStore.pixel_buffers`.
    pub fn redo(&mut self) -> Option<&PixelOpBatch> {
        if self.cursor >= self.stack.len() {
            return None;
        }
        let batch = self.stack.get(self.cursor);
        self.cursor += 1;
        batch
    }
}

impl Default for PixelHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply the `before` bytes of each op in `batch` back into `buffers`.
///
/// Called by the undo path. Buffers not found in `buffers` are silently
/// skipped — a missing buffer means the original pixel data was never
/// materialised, which makes the undo a no-op for that buffer.
pub fn apply_before(batch: &PixelOpBatch, buffers: &mut [PixelBufferEntry]) {
    for op in &batch.ops {
        if let Some(entry) = buffers.iter_mut().find(|e| e.id == op.buffer_id) {
            entry.pixels.clone_from(&op.before);
        }
    }
}

/// Apply the `after` bytes of each op in `batch` into `buffers`.
///
/// Called by the redo path.
pub fn apply_after(batch: &PixelOpBatch, buffers: &mut [PixelBufferEntry]) {
    for op in &batch.ops {
        if let Some(entry) = buffers.iter_mut().find(|e| e.id == op.buffer_id) {
            entry.pixels.clone_from(&op.after);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_batch(label: &str, buffer_id: u32, before: &[u8], after: &[u8]) -> PixelOpBatch {
        PixelOpBatch {
            label: label.to_owned(),
            ops: vec![PixelOp {
                buffer_id,
                buf_width: 1,
                buf_height: 1,
                buf_stride: 4,
                sprite_id: 1,
                frame_index: 0,
                before: before.to_vec(),
                after: after.to_vec(),
            }],
        }
    }

    #[test]
    fn push_advances_cursor() {
        let mut h = PixelHistory::new();
        h.push(make_batch("stroke", 1, &[0; 4], &[255, 0, 0, 255]));
        assert!(h.can_undo());
        assert!(!h.can_redo());
    }

    #[test]
    fn undo_decrements_cursor() {
        let mut h = PixelHistory::new();
        h.push(make_batch("stroke", 1, &[0; 4], &[255, 0, 0, 255]));
        let batch = h.undo();
        assert!(batch.is_some());
        assert!(!h.can_undo());
        assert!(h.can_redo());
    }

    #[test]
    fn redo_increments_cursor() {
        let mut h = PixelHistory::new();
        h.push(make_batch("stroke", 1, &[0; 4], &[255, 0, 0, 255]));
        h.undo();
        let batch = h.redo();
        assert!(batch.is_some());
        assert!(h.can_undo());
        assert!(!h.can_redo());
    }

    #[test]
    fn push_after_undo_truncates_redo() {
        let mut h = PixelHistory::new();
        h.push(make_batch("a", 1, &[0; 4], &[1; 4]));
        h.push(make_batch("b", 1, &[1; 4], &[2; 4]));
        h.undo(); // cursor at 1, redo branch is "b"
        h.push(make_batch("c", 1, &[1; 4], &[3; 4])); // discards "b"
        assert_eq!(h.stack.len(), 2);
        assert_eq!(h.stack[1].label, "c");
    }

    #[test]
    fn apply_before_restores_buffer() {
        let batch = make_batch("stroke", 7, &[0, 0, 0, 0], &[255, 0, 0, 255]);
        let mut buffers = vec![PixelBufferEntry {
            id: 7,
            width: 1,
            height: 1,
            stride: 4,
            pixels: vec![255, 0, 0, 255], // currently shows "after"
        }];
        apply_before(&batch, &mut buffers);
        assert_eq!(buffers[0].pixels, vec![0, 0, 0, 0]);
    }

    #[test]
    fn apply_after_sets_buffer() {
        let batch = make_batch("stroke", 3, &[0, 0, 0, 0], &[0, 255, 0, 255]);
        let mut buffers = vec![PixelBufferEntry {
            id: 3,
            width: 1,
            height: 1,
            stride: 4,
            pixels: vec![0, 0, 0, 0],
        }];
        apply_after(&batch, &mut buffers);
        assert_eq!(buffers[0].pixels, vec![0, 255, 0, 255]);
    }
}
