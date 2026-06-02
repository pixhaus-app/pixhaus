//! Theme token structs. Pure data; no logic. A `Theme` is a runtime value held in
//! `Host` so a variant change can re-derive and re-apply it. Panels and regions ask
//! for `theme.surfaces.panel` or `theme.surface(SurfaceTier::Elevated)`, never a hex
//! literal.

use egui::Color32;

/// The full token set for one resolved theme. Built by `Theme::for_variant`.
#[derive(Copy, Clone)]
pub struct Theme {
    /// Which variant produced this token set.
    pub variant: ThemeVariant,
    /// Per-region background tiers.
    pub surfaces: Surfaces,
    /// Semantic foreground and status roles.
    pub roles: Roles,
    /// Derived from a seed color; the separable preference axis (light/dark is one
    /// axis, accent color is another - the bible treats them as two preferences).
    pub accent: AccentTokens,
    /// Shadow tiers for raised cards and overlays.
    pub elevation: Elevation,
    /// The spacing scale.
    pub spacing: Spacing,
    /// The type-size scale.
    pub type_scale: TypeScale,
    /// The corner-radius scale.
    pub radius: Radii,
    /// Representative mock-content colors (palette swatches, timeline clip spans,
    /// preset thumbnails). These exist so mock panels read alive in the references'
    /// look without a real document; they are not semantic roles.
    pub mock: MockColors,
}

/// Representative colors for the mock content that stands in for a real document.
/// Pure data, no logic. Defined per variant in `palettes.rs` so module/widget code
/// paints from tokens rather than hex literals. Decorative only - these are not
/// gated by the role WCAG floors, so labels drawn over them use a dark/light ink
/// chosen for legibility, not a role color.
#[derive(Copy, Clone)]
pub struct MockColors {
    /// A 24-color "Bit" sprite palette spanning the families the references show:
    /// near-black outline, warm browns, a stone-gray ramp, golds, greens, blues, a
    /// red pair, an orange, and white.
    pub palette: [Color32; 24],
    /// Five muted clip-span hues for the timeline (green, blue, orange, violet, gold).
    pub clips: [Color32; 5],
    /// A small set of warm thumbnail tints so preset/sprite grids read with color
    /// instead of flat checker.
    pub thumbnails: [Color32; 6],
}

/// Which theme is active. `serde`-ready so it can round-trip through `Prefs`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ThemeVariant {
    /// The tuned dark theme (default).
    Dark,
    /// The light variant.
    Light,
    /// The accent-driven high-contrast variant.
    AccentHighContrast,
}

/// Per-region background tiers (UX 6.2), near-black warm slate in dark.
#[derive(Copy, Clone)]
pub struct Surfaces {
    /// Darkest - the app frame.
    pub app_frame: Color32,
    /// Dark charcoal/slate - panels, left rail, tray.
    pub panel: Color32,
    /// Slightly lighter - cards, top bars.
    pub elevated: Color32,
    /// Deepest neutral - behind the artboard.
    pub stage: Color32,
    /// Text fields, wells, HUD.
    pub inset: Color32,
}

/// Semantic foreground and status roles.
#[derive(Copy, Clone)]
pub struct Roles {
    /// Hairline borders and dividers.
    pub border: Color32,
    /// Primary text.
    pub text_primary: Color32,
    /// Secondary / muted text.
    pub text_secondary: Color32,
    /// Disabled text.
    pub text_disabled: Color32,
    /// Muted green.
    pub success: Color32,
    /// Muted amber.
    pub warning: Color32,
    /// Muted red.
    pub error: Color32,
}

/// All derived from one seed (default ~#7c6cef violet).
#[derive(Copy, Clone)]
pub struct AccentTokens {
    /// The seed the accent was derived from.
    pub seed: Color32,
    /// The accent.
    pub base: Color32,
    /// Hover state of the accent.
    pub hover: Color32,
    /// Opaque dark accent fill behind an active tool/tab/row. Stands in for a
    /// low-alpha accent overlay over `surfaces.panel` - precomputed opaque so it
    /// composites without an alpha pass and reads consistently regardless of what
    /// sits beneath it.
    pub muted: Color32,
    /// A brighter desaturated mid-violet for the single active tool cell in the
    /// rail, where `muted` reads too close to the panel to mark the selection. One
    /// step up from `muted` toward the seed, matching the references' active-tool fill.
    pub tool_active_bg: Color32,
    /// Near-white foreground for text/icons painted directly on `base` (egui's
    /// active-widget fill). `base` is too light to host `text_primary` at the
    /// 4.5 contrast floor, so active widgets get this dedicated high-contrast ink
    /// instead of the default override text color.
    pub on_accent: Color32,
    /// Sparkle marker tint (named for intent).
    pub ai: Color32,
    /// Softer halo behind AI affordances.
    pub ai_glow: Color32,
}

/// Shadow tiers (UX 6.2). The artboard "shadow" is painted manually in the canvas
/// stage - `Shadow` is not a paint primitive and is reserved for card `Frame`s and
/// the command-palette `Area`.
#[derive(Copy, Clone)]
pub struct Elevation {
    /// Card `Frame`s.
    pub raised: egui::epaint::Shadow,
    /// Command palette / windows.
    pub overlay: egui::epaint::Shadow,
}

/// Spacing scale: 2, 4, 8, 12, 16.
#[derive(Copy, Clone)]
pub struct Spacing {
    /// 2 px.
    pub xs: f32,
    /// 4 px.
    pub sm: f32,
    /// 8 px.
    pub md: f32,
    /// 12 px.
    pub lg: f32,
    /// 16 px.
    pub xl: f32,
}

/// Type scale: 11, 13, 13, 15, 12.
#[derive(Copy, Clone)]
pub struct TypeScale {
    /// Small labels - 11 px.
    pub label: f32,
    /// Body text - 13 px.
    pub body: f32,
    /// Section headers - 13 px.
    pub section_header: f32,
    /// Titles - 15 px.
    pub title: f32,
    /// Monospace - 12 px.
    pub mono: f32,
}

/// Corner radii: 2, 3 - a production cockpit, not rounded mobile.
#[derive(Copy, Clone)]
pub struct Radii {
    /// Small radius - 2 px.
    pub sm: f32,
    /// Medium radius - 3 px.
    pub md: f32,
}

/// Names a surface tier so callers can ask `theme.surface(tier)` at runtime.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SurfaceTier {
    /// The darkest app frame.
    AppFrame,
    /// Panels, rails, trays.
    Panel,
    /// Cards and top bars.
    Elevated,
    /// Behind the artboard.
    Stage,
    /// Text fields, wells, HUD.
    Inset,
}
