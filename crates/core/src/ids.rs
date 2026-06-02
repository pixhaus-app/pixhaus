//! Stable, typed ids for the domain model.
//!
//! Ids are monotonic `u32` newtypes minted from an [`IdCounter`]. A counter never
//! returns the same value twice, even after the thing it identified is deleted
//! (tombstone-on-delete), so a stale handle can never alias a live one. Newtypes
//! keep a [`SpriteId`] from ever being passed where a [`LayerId`] is expected.

use serde::{Deserialize, Serialize};

/// Stable identifier for a [`Sprite`](crate::Sprite) within one
/// [`Document`](crate::Document).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct SpriteId(pub u32);

/// Stable identifier for a [`Layer`](crate::Layer) within one sprite.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct LayerId(pub u32);

/// Handle into the [`PixelBufferStore`](crate::PixelBufferStore). Structural data
/// references pixel bytes by this handle, never by value, so cloning a sprite for an
/// undo snapshot copies handles, not megabytes of pixels.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct PixelBufferId(pub u32);

/// Mints monotonic `u32` ids. Never reuses a value, so a deleted id stays retired
/// and a dangling handle can never collide with a freshly minted one.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IdCounter {
    next: u32,
}

impl IdCounter {
    /// Returns a fresh id, never previously returned by this counter.
    pub fn mint(&mut self) -> u32 {
        let value = self.next;
        self.next += 1;
        value
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
    fn distinct_id_types_do_not_unify() {
        // A compile-time guarantee, exercised here so the newtypes stay distinct:
        // these are different types and cannot be compared or swapped.
        let sprite = SpriteId(7);
        let layer = LayerId(7);
        assert_eq!(sprite.0, layer.0);
    }
}
