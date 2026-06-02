//! The pixel-buffer store: pixel bytes keyed by handle, decoupled from metadata.
//!
//! Structural data (sprites, layers) references buffers by [`PixelBufferId`], so a
//! structural undo snapshot copies handles, not pixels. The store is part of
//! [`Document`](crate::Document) so a single [`Command`](crate::Command) target
//! covers both structural and pixel edits.

use std::collections::HashMap;

use crate::ids::{IdCounter, PixelBufferId};
use crate::pixel::PixelBuffer;

/// Owns the pixel bytes for a document, keyed by [`PixelBufferId`].
#[derive(Clone, Debug, Default)]
pub struct PixelBufferStore {
    buffers: HashMap<PixelBufferId, PixelBuffer>,
    counter: IdCounter,
}

impl PixelBufferStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a buffer under a freshly minted handle and returns it.
    pub fn insert(&mut self, buffer: PixelBuffer) -> PixelBufferId {
        let id = PixelBufferId(self.counter.mint());
        self.buffers.insert(id, buffer);
        id
    }

    /// Restores a buffer under an exact prior handle (used by undo to bring a
    /// removed buffer back under the id structural data still references).
    pub fn insert_with_id(&mut self, id: PixelBufferId, buffer: PixelBuffer) {
        self.buffers.insert(id, buffer);
    }

    /// Borrows the buffer for `id`, or `None` if absent.
    pub fn get(&self, id: PixelBufferId) -> Option<&PixelBuffer> {
        self.buffers.get(&id)
    }

    /// Removes and returns the buffer for `id`, or `None` if absent.
    pub fn remove(&mut self, id: PixelBufferId) -> Option<PixelBuffer> {
        self.buffers.remove(&id)
    }

    /// The number of stored buffers.
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// Whether the store holds no buffers.
    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf() -> PixelBuffer {
        PixelBuffer::new(1, 1).unwrap()
    }

    #[test]
    fn insert_mints_distinct_handles() {
        let mut store = PixelBufferStore::new();
        let a = store.insert(buf());
        let b = store.insert(buf());
        assert_ne!(a, b);
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn remove_then_restore_under_same_handle() {
        let mut store = PixelBufferStore::new();
        let id = store.insert(buf());
        let removed = store.remove(id).unwrap();
        assert!(store.get(id).is_none());
        store.insert_with_id(id, removed);
        assert!(store.get(id).is_some());
    }
}
