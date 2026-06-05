//! Stable, typed ids for the domain model.
//!
//! Ids are monotonic `u64` newtypes minted from an [`IdCounter`]. A counter never
//! returns the same value twice, even after the thing it identified is deleted
//! (tombstone-on-delete), so a stale handle can never alias a live one. The width is
//! `u64` so the no-reuse guarantee holds for the life of any document (minting one id
//! per nanosecond would take ~585 years to exhaust the space). Newtypes keep a
//! [`SpriteId`] from ever being passed where a [`LayerId`] is expected.

use serde::{Deserialize, Serialize};

/// Stable identifier for a [`Sprite`](crate::Sprite) within one
/// [`Document`](crate::Document).
///
/// Each id type is a distinct newtype, so passing one where another is expected fails to
/// compile — the guarantee a same-typed `u64` could not give:
///
/// ```compile_fail
/// use pixhaus_core::ids::{LayerId, SpriteId};
/// fn takes_sprite(_: SpriteId) {}
/// takes_sprite(LayerId(7)); // a LayerId is not a SpriteId
/// ```
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct SpriteId(pub u64);

/// Stable identifier for a [`Layer`](crate::Layer) within one frame.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct LayerId(pub u64);

/// Stable identifier for a [`Frame`](crate::Frame) within one sprite.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct FrameId(pub u64);

/// Stable identifier for an [`AnimationClip`](crate::AnimationClip) within one sprite.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ClipId(pub u64);

/// Handle into the [`PixelBufferStore`](crate::PixelBufferStore). Structural data
/// references pixel bytes by this handle, never by value, so cloning a sprite for an
/// undo snapshot copies handles, not megabytes of pixels.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct PixelBufferId(pub u64);

/// Mints monotonic `u64` ids. Never reuses a value, so a deleted id stays retired
/// and a dangling handle can never collide with a freshly minted one. The `u64` space
/// makes exhaustion unreachable for any real document, so the no-reuse guarantee holds
/// without an overflow guard on the increment.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IdCounter {
    next: u64,
}

impl IdCounter {
    /// Returns a fresh id, never previously returned by this counter.
    pub fn mint(&mut self) -> u64 {
        let value = self.next;
        self.next += 1;
        value
    }

    /// Ensures the next minted id is strictly greater than `value`.
    ///
    /// Restoring a value under an id minted elsewhere (undo bringing a removed
    /// buffer back under its prior handle) would leave the counter free to mint
    /// that same value again and collide. Bumping past the restored id keeps the
    /// no-reuse guarantee even when ids enter from outside `mint`.
    // Only caller is `PixelBufferStore::insert_with_id`, itself unreachable outside
    // tests until the restore-under-prior-handle undo path is wired (bible section 26).
    // Allowed in non-test builds only; remove once `insert_with_id` gains a real caller.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn bump_past(&mut self, value: u64) {
        // saturating add keeps the no-overflow stance of mint: u64 exhaustion is
        // unreachable for any real document, so a guard here would be dead code that
        // only obscures the invariant.
        self.next = self.next.max(value.saturating_add(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_is_monotonic_and_never_repeats() {
        let mut counter = IdCounter::default();
        let first = counter.mint();
        let second = counter.mint();
        let third = counter.mint();
        assert_eq!((first, second, third), (0, 1, 2));
    }

    #[test]
    fn typed_ids_preserve_inner_value() {
        // The runtime side of the newtype guarantee: each id round-trips its inner u64 and
        // keys a set independently of the others. The compile-time distinctness (a LayerId
        // cannot be passed where a SpriteId is expected) is pinned by the compile_fail
        // doctest on SpriteId, which a runtime assert cannot exercise.
        use std::collections::HashSet;

        let sprite = SpriteId(7);
        let layer = LayerId(7);
        assert_eq!(sprite.0, 7);
        assert_eq!(layer.0, 7);

        let mut sprites = HashSet::new();
        sprites.insert(SpriteId(7));
        sprites.insert(SpriteId(8));
        assert!(sprites.contains(&SpriteId(7)));
        assert!(!sprites.contains(&SpriteId(9)));

        let mut layers = HashSet::new();
        layers.insert(LayerId(7));
        assert!(layers.contains(&LayerId(7)));
        assert_eq!(layers.len(), 1);
    }
}
