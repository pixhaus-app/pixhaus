//! Theme token system: semantic roles, surfaces, accent, spacing, type, radii.
//! Dark-first; light and accent-high-contrast variants share the same role set.
//!
//! Filled by the theme layer: `Theme`, `ThemeVariant`, `apply_to_visuals`,
//! `install_fonts`, and the token structs in `tokens`/`palettes`/`contrast`.

pub mod tokens;

pub use tokens::{AccentTokens, Elevation, Radii, Roles, SurfaceTier, Surfaces, Theme, ThemeVariant, TypeScale};
