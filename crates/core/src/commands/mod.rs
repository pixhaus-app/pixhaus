//! Concrete undoable commands over the [`Document`](crate::Document).
//!
//! These are the fundamental commands the shared core ships. Module-specific
//! commands live in their owning module, but anything that mutates the base
//! sprite/layer/buffer model belongs here so it stays pure and testable.

mod add_sprite;
mod apply_generated_asset;

pub use add_sprite::{AddSprite, SpriteProto};
pub use apply_generated_asset::ApplyGeneratedAsset;
