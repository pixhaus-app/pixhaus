//! The canvas subsystem: the view transform and the input mapping the canvas stage
//! and (later) the drawing tools share.
//!
//! [`view`] is pure geometry — the camera math that turns a `zoom`/`pan` pair into an
//! on-screen artboard rect, maps screen points to sprite pixels and back, and keeps
//! zoom anchored under the cursor. It is egui-value-typed (`Vec2`/`Rect`/`Pos2`) but
//! frame-free, so it is unit-tested without a `Context`, like [`crate::playback`].

pub mod onion;
pub mod overlay;
pub mod view;
