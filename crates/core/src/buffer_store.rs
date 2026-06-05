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
    ///
    /// `pub(crate)` on purpose: this is an internal restore/dedup helper, not public
    /// API. Inserting under an externally chosen id bypasses `mint`, so a caller
    /// outside `core` could desync the counter and reintroduce id collisions —
    /// keeping it crate-private confines that hazard to the undo path that needs it.
    // No production caller yet — the buffer-removing undo path that restores under a
    // prior handle isn't wired (bible section 26). Allowed only in non-test builds so
    // the test below still exercises it; drop this attribute the moment a real caller
    // lands so dead code can't hide behind it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn insert_with_id(&mut self, id: PixelBufferId, buffer: PixelBuffer) {
        // Advance the counter past the restored id so a later `insert` cannot mint the
        // same handle and alias this buffer. The id arrived from outside `mint`, so the
        // monotonic guarantee only holds if we re-establish it here.
        self.counter.bump_past(id.0);
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

    #[test]
    fn insert_with_id_advances_counter_past_restored_id() {
        // The collision the fix closes: restoring under a handle minted elsewhere must
        // push the counter past it, so the next mint cannot hand back the same id and
        // alias the restored buffer.
        let mut store = PixelBufferStore::new();
        let restored = PixelBufferId(42);
        store.insert_with_id(restored, buf());
        let next = store.insert(buf());
        assert_ne!(next, restored);
        assert!(next.0 > restored.0);
    }
}
