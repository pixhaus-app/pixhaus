//! Pixhaus creative core: the domain model and pure operations.
//!
//! `core` owns the authoritative project data — sprites, layers, palettes, the
//! pixel buffers behind them, the typed ids that key them, and the [`Command`] trait
//! through which every mutation flows. It is pure data and operations: no egui, no
//! wgpu, no I/O. Everything else in the workspace depends on it; it depends on
//! nothing in the workspace.
//!
//! The shape, foundation stage:
//! - [`PixelBuffer`] — RGBA8 bytes with explicit stride; the only pixel container.
//! - [`Document`] — the single [`Command`] target, bundling structural data and the
//!   [`PixelBufferStore`] so one command type covers structural and pixel edits.
//! - [`Command`] + [`commands`] — the mutation boundary and the fundamental commands
//!   ([`commands::AddSprite`], [`commands::ApplyGeneratedAsset`]).
//! - [`composite_sprite`] / [`composite_active`] — the pure CPU compositor the
//!   renderer uploads as a texture.

#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic, clippy::float_cmp)
)]

pub mod animation;
pub mod buffer_store;
pub mod command;
pub mod commands;
pub mod composite;
pub mod document;
pub mod ids;
pub mod pixel;

pub use animation::{AnimationClip, LoopMode};
pub use buffer_store::PixelBufferStore;
pub use command::{Command, CommandError};
pub use commands::{AddSprite, ApplyGeneratedAnimation, ApplyGeneratedAsset, GeneratedFrameData, SpriteProto};
pub use composite::{CompositeError, composite_active, composite_frame, composite_sprite};
pub use document::{DEFAULT_CANVAS_SIZE, DEFAULT_FRAME_DURATION_MS, Document, Frame, Layer, Sprite};
pub use ids::{ClipId, FrameId, IdCounter, LayerId, PixelBufferId, SpriteId};
pub use pixel::{BlendMode, PixelBuffer, PixelError, Rgba};
