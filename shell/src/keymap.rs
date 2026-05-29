//! The keybinding subsystem: the command catalogue, chords, presets, and the
//! resolution that turns a pressed key into a [`CommandId`].
//!
//! This replaces the hardcoded `key_pressed` checks that lived in
//! `app.rs::handle_shortcuts`. A [`Keymap`] holds the active [`KeybindPreset`]
//! plus a sparse map of per-command custom overrides; it persists through
//! eframe Storage under the `"keymap"` key. Resolution order is custom override
//! -> active preset -> unbound, mirroring the legacy
//! `ui/src/keybinds/keybind-manager.ts`.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use eframe::egui::{Key, Modifiers};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Every keyboard-invokable action the shell can dispatch today.
///
/// Each command carries a display label and a [`CommandCategory`] for grouping
/// in the rebinding UI. The registry lists only commands v2 can actually run;
/// extend it as features land.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CommandId {
    // File
    /// Create a new sprite.
    NewSprite,
    // Edit
    /// Undo the last edit.
    Undo,
    /// Redo the next edit.
    Redo,
    /// Swap the foreground and background colours.
    SwapColors,
    /// Decrease the brush size by one pixel.
    BrushSizeDecrease,
    /// Increase the brush size by one pixel.
    BrushSizeIncrease,
    // Selection
    /// Select the whole canvas.
    SelectAll,
    /// Drop the current selection.
    Deselect,
    /// Invert the current selection.
    InvertSelection,
    /// Clear the selected pixels on the active cel.
    DeleteSelection,
    // View
    /// Zoom in one step, keeping the canvas centred.
    ZoomIn,
    /// Zoom out one step, keeping the canvas centred.
    ZoomOut,
    /// Fit the sprite to the viewport.
    ZoomFit,
    /// Reset the zoom to 100%.
    ZoomReset,
    /// Start or stop playback.
    PlayPause,
    // Tools
    /// Select the pencil tool.
    ToolPencil,
    /// Select the eraser tool.
    ToolEraser,
    /// Select the flood-fill tool.
    ToolFill,
    /// Select the line tool.
    ToolLine,
    /// Select the rectangle tool.
    ToolRectangle,
    /// Select the ellipse tool.
    ToolEllipse,
    /// Select the colour-picker tool.
    ToolPicker,
    /// Select the rectangular-marquee tool.
    ToolSelectRect,
    /// Select the lasso tool.
    ToolLasso,
    /// Select the magic-wand tool.
    ToolWand,
    /// Select the colour-range (select-by-colour) tool.
    ToolColorRange,
    /// Select the move tool.
    ToolMove,
    /// Select the free-transform tool.
    ToolTransform,
    // Window
    /// Open the settings window.
    OpenSettings,
}

impl CommandId {
    /// Every command, in catalogue order. The dispatcher and the rebinding UI
    /// both iterate this.
    pub const ALL: &'static [CommandId] = &[
        CommandId::NewSprite,
        CommandId::Undo,
        CommandId::Redo,
        CommandId::SwapColors,
        CommandId::BrushSizeDecrease,
        CommandId::BrushSizeIncrease,
        CommandId::SelectAll,
        CommandId::Deselect,
        CommandId::InvertSelection,
        CommandId::DeleteSelection,
        CommandId::ZoomIn,
        CommandId::ZoomOut,
        CommandId::ZoomFit,
        CommandId::ZoomReset,
        CommandId::PlayPause,
        CommandId::ToolPencil,
        CommandId::ToolEraser,
        CommandId::ToolFill,
        CommandId::ToolLine,
        CommandId::ToolRectangle,
        CommandId::ToolEllipse,
        CommandId::ToolPicker,
        CommandId::ToolSelectRect,
        CommandId::ToolLasso,
        CommandId::ToolWand,
        CommandId::ToolColorRange,
        CommandId::ToolMove,
        CommandId::ToolTransform,
        CommandId::OpenSettings,
    ];

    /// Human-readable label shown in the rebinding UI.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            CommandId::NewSprite => "New sprite",
            CommandId::Undo => "Undo",
            CommandId::Redo => "Redo",
            CommandId::SwapColors => "Swap colours",
            CommandId::BrushSizeDecrease => "Decrease brush size",
            CommandId::BrushSizeIncrease => "Increase brush size",
            CommandId::SelectAll => "Select all",
            CommandId::Deselect => "Deselect",
            CommandId::InvertSelection => "Invert selection",
            CommandId::DeleteSelection => "Delete selection",
            CommandId::ZoomIn => "Zoom in",
            CommandId::ZoomOut => "Zoom out",
            CommandId::ZoomFit => "Zoom to fit",
            CommandId::ZoomReset => "Zoom to 100%",
            CommandId::PlayPause => "Play / pause",
            CommandId::ToolPencil => "Pencil",
            CommandId::ToolEraser => "Eraser",
            CommandId::ToolFill => "Fill",
            CommandId::ToolLine => "Line",
            CommandId::ToolRectangle => "Rectangle",
            CommandId::ToolEllipse => "Ellipse",
            CommandId::ToolPicker => "Colour picker",
            CommandId::ToolSelectRect => "Rectangular marquee",
            CommandId::ToolLasso => "Lasso",
            CommandId::ToolWand => "Magic wand",
            CommandId::ToolColorRange => "Colour range",
            CommandId::ToolMove => "Move",
            CommandId::ToolTransform => "Free transform",
            CommandId::OpenSettings => "Settings…",
        }
    }

    /// The category this command groups under in the rebinding UI.
    #[must_use]
    pub fn category(self) -> CommandCategory {
        match self {
            CommandId::NewSprite => CommandCategory::File,
            CommandId::Undo | CommandId::Redo | CommandId::SwapColors | CommandId::BrushSizeDecrease | CommandId::BrushSizeIncrease => CommandCategory::Edit,
            CommandId::SelectAll | CommandId::Deselect | CommandId::InvertSelection | CommandId::DeleteSelection => CommandCategory::Selection,
            CommandId::ZoomIn | CommandId::ZoomOut | CommandId::ZoomFit | CommandId::ZoomReset | CommandId::PlayPause => CommandCategory::View,
            CommandId::ToolPencil
            | CommandId::ToolEraser
            | CommandId::ToolFill
            | CommandId::ToolLine
            | CommandId::ToolRectangle
            | CommandId::ToolEllipse
            | CommandId::ToolPicker
            | CommandId::ToolSelectRect
            | CommandId::ToolLasso
            | CommandId::ToolWand
            | CommandId::ToolColorRange
            | CommandId::ToolMove
            | CommandId::ToolTransform => CommandCategory::Tools,
            CommandId::OpenSettings => CommandCategory::Window,
        }
    }
}

/// Grouping for the rebinding UI, mirroring the legacy command-registry split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandCategory {
    /// Document-level commands.
    File,
    /// Editing commands.
    Edit,
    /// Selection commands.
    Selection,
    /// Viewport and playback commands.
    View,
    /// Tool selectors.
    Tools,
    /// Window commands.
    Window,
}

impl CommandCategory {
    /// Every category, in display order.
    pub const ALL: &'static [CommandCategory] = &[
        CommandCategory::File,
        CommandCategory::Edit,
        CommandCategory::Selection,
        CommandCategory::View,
        CommandCategory::Tools,
        CommandCategory::Window,
    ];

    /// Heading shown above the category's commands.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            CommandCategory::File => "File",
            CommandCategory::Edit => "Edit",
            CommandCategory::Selection => "Selection",
            CommandCategory::View => "View",
            CommandCategory::Tools => "Tools",
            CommandCategory::Window => "Window",
        }
    }
}

/// A modifier-plus-key chord, e.g. `Ctrl+Shift+Z`.
///
/// `ctrl` is the platform "command-or-control" modifier (`Cmd` on macOS,
/// `Ctrl` elsewhere), matching the legacy combo grammar. The chord round-trips
/// through serde as its display string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    /// Command (macOS) or Control.
    pub ctrl: bool,
    /// Shift.
    pub shift: bool,
    /// Alt / Option.
    pub alt: bool,
    /// The non-modifier key.
    pub key: Key,
}

impl Chord {
    /// A bare key chord with no modifiers.
    #[must_use]
    pub const fn key(key: Key) -> Self {
        Self {
            ctrl: false,
            shift: false,
            alt: false,
            key,
        }
    }

    /// A `Ctrl`/`Cmd`-modified chord.
    #[must_use]
    pub const fn ctrl(key: Key) -> Self {
        Self {
            ctrl: true,
            shift: false,
            alt: false,
            key,
        }
    }

    /// A `Ctrl`/`Cmd`+`Shift`-modified chord.
    #[must_use]
    pub const fn ctrl_shift(key: Key) -> Self {
        Self {
            ctrl: true,
            shift: true,
            alt: false,
            key,
        }
    }

    /// Builds a chord from egui modifiers and a key, collapsing `command` and
    /// `ctrl` into the single platform modifier.
    #[must_use]
    pub fn from_modifiers(modifiers: Modifiers, key: Key) -> Self {
        Self {
            ctrl: modifiers.command || modifiers.ctrl,
            shift: modifiers.shift,
            alt: modifiers.alt,
            key,
        }
    }

    /// Whether `modifiers` exactly match this chord's modifier set. The key is
    /// checked separately so the caller can use `key_pressed`.
    #[must_use]
    pub fn modifiers_match(self, modifiers: Modifiers) -> bool {
        let ctrl = modifiers.command || modifiers.ctrl;
        ctrl == self.ctrl && modifiers.shift == self.shift && modifiers.alt == self.alt
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        f.write_str(self.key.name())
    }
}

/// The error returned when a chord string cannot be parsed.
#[derive(Debug, PartialEq, Eq)]
pub struct ParseChordError(String);

impl fmt::Display for ParseChordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid chord: {}", self.0)
    }
}

impl std::error::Error for ParseChordError {}

impl FromStr for Chord {
    type Err = ParseChordError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (mut ctrl, mut shift, mut alt) = (false, false, false);
        let mut key = None;
        for part in s.split('+') {
            match part {
                "Ctrl" | "Cmd" | "Command" => ctrl = true,
                "Shift" => shift = true,
                "Alt" | "Option" => alt = true,
                other => {
                    if key.is_some() {
                        return Err(ParseChordError(format!("more than one key in {s:?}")));
                    }
                    key = Some(Key::from_name(other).ok_or_else(|| ParseChordError(format!("unknown key {other:?}")))?);
                }
            }
        }
        let key = key.ok_or_else(|| ParseChordError(format!("no key in {s:?}")))?;
        Ok(Self { ctrl, shift, alt, key })
    }
}

impl Serialize for Chord {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Chord {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Which built-in default table a [`Keymap`] resolves against.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeybindPreset {
    /// Aseprite-style defaults (the v2 default).
    #[default]
    Aseprite,
    /// Photoshop-style defaults.
    Photoshop,
    /// No preset table; only custom overrides bind. Falls back to the Aseprite
    /// table as a base, mirroring the legacy manager.
    Custom,
}

impl KeybindPreset {
    /// Display label for the preset dropdown.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            KeybindPreset::Aseprite => "Aseprite",
            KeybindPreset::Photoshop => "Photoshop",
            KeybindPreset::Custom => "Custom",
        }
    }
}

/// The user's keybindings: an active preset plus per-command overrides.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Keymap {
    /// Which default table to resolve against.
    pub preset: KeybindPreset,
    /// Per-command overrides that win over the preset.
    pub custom: BTreeMap<CommandId, Chord>,
}

impl Keymap {
    /// The effective chord for `command`: a custom override if present,
    /// otherwise the active preset's default, otherwise unbound.
    #[must_use]
    pub fn resolve(&self, command: CommandId) -> Option<Chord> {
        if let Some(chord) = self.custom.get(&command) {
            return Some(*chord);
        }
        preset_default(self.preset, command)
    }

    /// Whether `command` has a custom override.
    #[must_use]
    pub fn is_overridden(&self, command: CommandId) -> bool {
        self.custom.contains_key(&command)
    }

    /// Records a custom override for `command`.
    pub fn set_override(&mut self, command: CommandId, chord: Chord) {
        self.custom.insert(command, chord);
    }

    /// Removes any custom override for `command`, reverting it to the preset
    /// default.
    pub fn reset(&mut self, command: CommandId) {
        self.custom.remove(&command);
    }

    /// Clears every custom override.
    pub fn reset_all(&mut self) {
        self.custom.clear();
    }
}

/// The default chord for `command` under `preset`. `Custom` resolves against
/// the Aseprite base, so unbound-by-preset still has a sensible fallback.
fn preset_default(preset: KeybindPreset, command: CommandId) -> Option<Chord> {
    match preset {
        KeybindPreset::Photoshop => photoshop_default(command),
        KeybindPreset::Aseprite | KeybindPreset::Custom => Some(aseprite_default(command)),
    }
}

/// The Aseprite preset, restricted to commands v2 implements. Tool letters and
/// the edit/view commands track Aseprite's real bindings; the file/window
/// commands are ported from `ui/src/keybinds/defaults.ts`. Every command has an
/// Aseprite binding, so this is total.
fn aseprite_default(command: CommandId) -> Chord {
    use CommandId as C;
    match command {
        C::NewSprite => Chord::ctrl(Key::N),
        C::Undo => Chord::ctrl(Key::Z),
        C::Redo => Chord::ctrl_shift(Key::Z),
        C::SwapColors => Chord::key(Key::X),
        C::BrushSizeDecrease => Chord::key(Key::OpenBracket),
        C::BrushSizeIncrease => Chord::key(Key::CloseBracket),
        C::SelectAll => Chord::ctrl(Key::A),
        C::Deselect => Chord::key(Key::Escape),
        C::InvertSelection => Chord::ctrl_shift(Key::I),
        C::DeleteSelection => Chord::key(Key::Delete),
        C::ZoomIn => Chord::ctrl(Key::Equals),
        C::ZoomOut => Chord::ctrl(Key::Minus),
        C::ZoomFit => Chord::ctrl(Key::Num0),
        C::ZoomReset => Chord::ctrl(Key::Num1),
        C::PlayPause => Chord::key(Key::Enter),
        C::ToolPencil => Chord::key(Key::B),
        C::ToolEraser => Chord::key(Key::E),
        C::ToolFill => Chord::key(Key::G),
        C::ToolLine => Chord::key(Key::L),
        C::ToolRectangle => Chord::key(Key::U),
        C::ToolEllipse => Chord::key(Key::O),
        C::ToolPicker => Chord::key(Key::I),
        C::ToolSelectRect => Chord::key(Key::M),
        C::ToolLasso => Chord::key(Key::Q),
        C::ToolWand => Chord::key(Key::W),
        // No standard Aseprite letter; A is free in the preset.
        C::ToolColorRange => Chord::key(Key::A),
        C::ToolMove => Chord::key(Key::V),
        C::ToolTransform => Chord::key(Key::T),
        C::OpenSettings => Chord::ctrl(Key::Comma),
    }
}

/// The Photoshop preset. Shares the edit/view/file commands with Aseprite but
/// uses Photoshop's tool letters (and `Ctrl+D` for deselect), so switching
/// presets is observable. Tools without a standard Photoshop single-key binding
/// (line, ellipse) are left unbound under this preset.
fn photoshop_default(command: CommandId) -> Option<Chord> {
    use CommandId as C;
    match command {
        C::Deselect => Some(Chord::ctrl(Key::D)),
        C::ToolLine | C::ToolEllipse => None,
        C::ToolRectangle => Some(Chord::key(Key::U)),
        C::ToolLasso => Some(Chord::key(Key::L)),
        // Everything else matches Aseprite's binding.
        other => Some(aseprite_default(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chord_round_trips_through_its_string() {
        for chord in [
            Chord::key(Key::B),
            Chord::ctrl(Key::Z),
            Chord::ctrl_shift(Key::Z),
            Chord::key(Key::Escape),
            Chord::ctrl(Key::Comma),
            Chord::ctrl(Key::Equals),
            Chord::ctrl(Key::Num0),
            Chord {
                ctrl: true,
                shift: true,
                alt: true,
                key: Key::OpenBracket,
            },
        ] {
            let text = chord.to_string();
            let parsed: Chord = text.parse().expect("chord parses back");
            assert_eq!(parsed, chord, "round trip for {text}");
        }
    }

    #[test]
    fn chord_formats_modifiers_in_order() {
        assert_eq!(Chord::ctrl_shift(Key::Z).to_string(), "Ctrl+Shift+Z");
        assert_eq!(Chord::ctrl(Key::Comma).to_string(), "Ctrl+Comma");
        assert_eq!(Chord::key(Key::Escape).to_string(), "Escape");
    }

    #[test]
    fn parse_accepts_cmd_as_ctrl() {
        assert_eq!("Cmd+Z".parse::<Chord>().unwrap(), Chord::ctrl(Key::Z));
    }

    #[test]
    fn parse_rejects_unknown_key_and_missing_key() {
        assert!("Ctrl+Frobnicate".parse::<Chord>().is_err());
        assert!("Ctrl+Shift".parse::<Chord>().is_err());
    }

    #[test]
    fn serde_round_trips_a_keymap() {
        let mut keymap = Keymap {
            preset: KeybindPreset::Photoshop,
            ..Default::default()
        };
        keymap.set_override(CommandId::ToolPencil, Chord::key(Key::P));
        let json = serde_json::to_string(&keymap).expect("serialize");
        let back: Keymap = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.preset, KeybindPreset::Photoshop);
        assert_eq!(back.resolve(CommandId::ToolPencil), Some(Chord::key(Key::P)));
    }

    #[test]
    fn custom_override_wins_over_preset() {
        let mut keymap = Keymap::default();
        assert_eq!(keymap.resolve(CommandId::ToolPencil), Some(Chord::key(Key::B)));
        keymap.set_override(CommandId::ToolPencil, Chord::key(Key::P));
        assert_eq!(keymap.resolve(CommandId::ToolPencil), Some(Chord::key(Key::P)));
    }

    #[test]
    fn preset_resolves_when_no_override() {
        let keymap = Keymap::default();
        assert_eq!(keymap.resolve(CommandId::Undo), Some(Chord::ctrl(Key::Z)));
        assert_eq!(keymap.resolve(CommandId::Deselect), Some(Chord::key(Key::Escape)));
    }

    #[test]
    fn reset_reverts_to_the_preset_default() {
        let mut keymap = Keymap::default();
        keymap.set_override(CommandId::Undo, Chord::ctrl(Key::Y));
        assert_eq!(keymap.resolve(CommandId::Undo), Some(Chord::ctrl(Key::Y)));
        keymap.reset(CommandId::Undo);
        assert_eq!(keymap.resolve(CommandId::Undo), Some(Chord::ctrl(Key::Z)));
    }

    #[test]
    fn photoshop_preset_differs_from_aseprite() {
        let aseprite = Keymap::default();
        let photoshop = Keymap {
            preset: KeybindPreset::Photoshop,
            custom: BTreeMap::new(),
        };
        // Deselect differs: Escape vs Ctrl+D.
        assert_eq!(aseprite.resolve(CommandId::Deselect), Some(Chord::key(Key::Escape)));
        assert_eq!(photoshop.resolve(CommandId::Deselect), Some(Chord::ctrl(Key::D)));
        // Line has no Photoshop binding.
        assert_eq!(aseprite.resolve(CommandId::ToolLine), Some(Chord::key(Key::L)));
        assert_eq!(photoshop.resolve(CommandId::ToolLine), None);
    }

    #[test]
    fn custom_preset_falls_back_to_aseprite_base() {
        let keymap = Keymap {
            preset: KeybindPreset::Custom,
            custom: BTreeMap::new(),
        };
        assert_eq!(keymap.resolve(CommandId::Undo), Some(Chord::ctrl(Key::Z)));
    }

    #[test]
    fn undo_and_redo_chords_are_distinguishable() {
        let keymap = Keymap::default();
        let undo = keymap.resolve(CommandId::Undo).unwrap();
        let redo = keymap.resolve(CommandId::Redo).unwrap();
        // Both use Z, but the shift modifier separates them.
        assert_eq!(undo.key, Key::Z);
        assert_eq!(redo.key, Key::Z);
        assert!(!undo.modifiers_match(Modifiers::COMMAND | Modifiers::SHIFT));
        assert!(redo.modifiers_match(Modifiers::COMMAND | Modifiers::SHIFT));
    }

    #[test]
    fn every_command_has_a_label_and_appears_once() {
        for &command in CommandId::ALL {
            assert!(!command.label().is_empty());
            let count = CommandId::ALL.iter().filter(|&&c| c == command).count();
            assert_eq!(count, 1, "{command:?} listed once");
        }
        // The Selection category carries select-all and invert alongside the
        // pre-existing deselect / delete, each with an Aseprite binding.
        for command in [CommandId::SelectAll, CommandId::InvertSelection] {
            assert!(CommandId::ALL.contains(&command), "{command:?} is catalogued");
            assert_eq!(command.category(), CommandCategory::Selection);
        }
        assert_eq!(aseprite_default(CommandId::SelectAll), Chord::ctrl(Key::A));
        assert_eq!(aseprite_default(CommandId::InvertSelection), Chord::ctrl_shift(Key::I));
        // Photoshop falls through to the same select/invert chords.
        assert_eq!(photoshop_default(CommandId::SelectAll), Some(Chord::ctrl(Key::A)));
        assert_eq!(photoshop_default(CommandId::InvertSelection), Some(Chord::ctrl_shift(Key::I)));
    }
}
