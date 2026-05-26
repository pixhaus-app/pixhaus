//! Pixhaus wgpu viewport renderer — vertical-slice subset.
//!
//! A minimal [`ViewportRenderer`] that owns wgpu resources and does two things:
//! display a composited sprite frame, and swap the frame texture when playback
//! advances. SPRITE program only — checkerboard background plus the frame
//! texture, alpha-composited. Stays egui-independent (depends on `core` +
//! `wgpu`) so it survives any later UI-framework change. The egui embedding
//! lives in the `shell` crate.
//!
//! [`viewport`] holds the pan/zoom math, ported from `ui/src/canvas/viewport.ts`.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::disallowed_methods,
        clippy::cast_lossless,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::print_stderr,
        clippy::items_after_statements
    )
)]

pub mod renderer;
pub mod viewport;

pub use renderer::ViewportRenderer;
pub use viewport::Viewport;
