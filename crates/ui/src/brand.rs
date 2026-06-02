//! The Pixhaus brand assets, baked into the binary.
//!
//! Three pixel-art PNGs live in `crates/ui/assets/` and are included at compile
//! time so the app carries its identity with no runtime file load: [`ICON`] (the
//! square Bit mark) for the top-bar brand and the OS window icon, [`WORDMARK`] for
//! the About dialog, and [`LOGO`] for the startup splash.
//!
//! The [`egui::ImageSource`]s render only once `egui_extras`' image loaders are
//! installed on the context - see [`crate::install_image_loaders`]. Paint them with
//! [`egui::TextureOptions::NEAREST`] so the pixel art stays crisp at any scale.

/// The square Bit mark - the top-bar brand and the basis for the OS window icon.
pub const ICON: egui::ImageSource<'_> = egui::include_image!("../assets/icon.png");

/// The "PIXHAUS" pixel wordmark, shown in the About dialog.
pub const WORDMARK: egui::ImageSource<'_> = egui::include_image!("../assets/wordmark.png");

/// The full brand banner (Bit + wordmark + sparkles), shown on the startup splash.
pub const LOGO: egui::ImageSource<'_> = egui::include_image!("../assets/logo.png");

/// Raw icon PNG bytes, for the app to decode into an OS window `IconData`.
///
/// Shares the one asset file with [`ICON`] rather than duplicating it: the egui
/// image source path and these bytes both point at `assets/icon.png`.
pub const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");
