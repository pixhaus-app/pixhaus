//! Lua binding registration.
//!
//! Each submodule installs one host-API namespace into a `Lua` VM. The
//! runtime calls them in order: `color::register` first (so `Color` is
//! available as a constructor), then `app::register`.

pub mod app;
pub mod color;
pub mod frame;
pub mod layer;
pub mod palette;
pub mod sprite;

pub use app::OutputCollectors;
