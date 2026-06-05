//! Type-specific detail commands: replace an entry's body, reversibly.
//!
//! Each command targets one [`EntryDetails`](crate::codex::EntryDetails) variant. It
//! validates that the entry currently holds that variant (else
//! [`CommandError::InvalidState`]), swaps in the new body, and keeps the prior body
//! for a faithful undo. Replacing the whole struct is the simplest reverse: the UI
//! edits a working copy and submits it.

use crate::codex::CodexEntryId;
use crate::codex::details::{AnimationDetails, CharacterDetails, EntryDetails, GenericDetails, PaletteDetails, StyleDetails};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Generates a command that replaces one [`EntryDetails`] variant's body.
///
/// `$cmd` is the command type, `$body` the body struct it carries, `$variant` the
/// matching [`EntryDetails`] variant, and `$label` the stable history label key.
macro_rules! detail_command {
    ($(#[$meta:meta])* $cmd:ident, $body:ty, $variant:ident, $label:literal) => {
        $(#[$meta])*
        pub struct $cmd {
            id: CodexEntryId,
            body: Option<$body>,
        }

        impl $cmd {
            /// A command that will set the entry `id`'s body to `body`. The entry must
            /// already hold the matching detail variant.
            pub fn new(id: CodexEntryId, body: $body) -> Self {
                Self { id, body: Some(body) }
            }
        }

        impl Command for $cmd {
            fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
                let body = self.body.take().ok_or(CommandError::InvalidState)?;
                let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
                match &mut entry.details {
                    EntryDetails::$variant(current) => {
                        self.body = Some(std::mem::replace(current, body));
                    }
                    _ => {
                        // Wrong type for this command; hand the body back and report it.
                        self.body = Some(body);
                        return Err(CommandError::InvalidState);
                    }
                }
                doc.bump_revision();
                Ok(())
            }

            fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
                // apply and undo are symmetric: each swaps the held body with the live one.
                self.apply(doc)
            }

            fn label_key(&self) -> &'static str {
                $label
            }

            fn estimated_size_bytes(&self) -> usize {
                std::mem::size_of::<Self>() + std::mem::size_of::<$body>()
            }
        }
    };
}

detail_command!(
    /// Replaces a Character entry's body. Undo restores the prior body.
    SetCharacterDetails,
    CharacterDetails,
    Character,
    "command.codex.set_character_details"
);

detail_command!(
    /// Replaces a Palette entry's body (colors, ramps, generation rule). Undo restores
    /// the prior body.
    SetPaletteDetails,
    PaletteDetails,
    Palette,
    "command.codex.set_palette_details"
);

detail_command!(
    /// Replaces a Style entry's body. Undo restores the prior body.
    SetStyleDetails,
    StyleDetails,
    Style,
    "command.codex.set_style_details"
);

detail_command!(
    /// Replaces an Animation entry's body (pose beats, fps, loop). Undo restores the
    /// prior body.
    SetAnimationDetails,
    AnimationDetails,
    Animation,
    "command.codex.set_animation_details"
);

detail_command!(
    /// Replaces a generic entry's key/value body. Undo restores the prior body.
    SetGenericDetails,
    GenericDetails,
    Generic,
    "command.codex.set_generic_details"
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::details::{GenericField, PaletteColor};
    use crate::codex::{ColorRole, EntryType};
    use crate::test_support::seed_entry;

    // Local wrapper: these tests use a throwaway name and vary only the type, so they
    // pin "x" into the shared four-field builder.
    fn seed(doc: &mut Document, handle: &str, ty: EntryType) -> CodexEntryId {
        seed_entry(doc, handle, "x", ty)
    }

    #[test]
    fn character_details_round_trip() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "bit", EntryType::Character);
        let body = CharacterDetails {
            proportions: "2 heads tall".to_owned(),
            ..CharacterDetails::default()
        };
        let mut cmd = SetCharacterDetails::new(id, body);

        cmd.apply(&mut doc).unwrap();
        assert!(matches!(
            &doc.codex().entry(id).unwrap().details,
            EntryDetails::Character(c) if c.proportions == "2 heads tall"
        ));

        cmd.undo(&mut doc).unwrap();
        assert!(matches!(
            &doc.codex().entry(id).unwrap().details,
            EntryDetails::Character(c) if c.proportions.is_empty()
        ));
    }

    #[test]
    fn palette_details_round_trip() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "pal", EntryType::Palette);
        let mut body = PaletteDetails::default();
        body.colors.push(PaletteColor::new([1, 2, 3, 255], ColorRole::Shadow));
        let mut cmd = SetPaletteDetails::new(id, body);

        cmd.apply(&mut doc).unwrap();
        assert!(matches!(
            &doc.codex().entry(id).unwrap().details,
            EntryDetails::Palette(p) if p.colors.len() == 1
        ));
        cmd.undo(&mut doc).unwrap();
        assert!(matches!(
            &doc.codex().entry(id).unwrap().details,
            EntryDetails::Palette(p) if p.colors.is_empty()
        ));
    }

    #[test]
    fn style_details_round_trip() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "sty", EntryType::Style);
        let body = StyleDetails {
            rendering_rules: "hard edges".to_owned(),
            ..StyleDetails::default()
        };
        let mut cmd = SetStyleDetails::new(id, body);
        cmd.apply(&mut doc).unwrap();
        cmd.undo(&mut doc).unwrap();
        assert!(matches!(
            &doc.codex().entry(id).unwrap().details,
            EntryDetails::Style(s) if s.rendering_rules.is_empty()
        ));
    }

    #[test]
    fn animation_details_round_trip() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "anim", EntryType::Animation);
        let body = AnimationDetails {
            fps: 12,
            ..AnimationDetails::default()
        };
        let mut cmd = SetAnimationDetails::new(id, body);
        cmd.apply(&mut doc).unwrap();
        assert!(matches!(
            &doc.codex().entry(id).unwrap().details,
            EntryDetails::Animation(a) if a.fps == 12
        ));
        cmd.undo(&mut doc).unwrap();
        assert!(matches!(
            &doc.codex().entry(id).unwrap().details,
            EntryDetails::Animation(a) if a.fps == 0
        ));
    }

    #[test]
    fn generic_details_round_trip() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "prop", EntryType::Prop);
        let body = GenericDetails {
            fields: vec![GenericField {
                key: "species".to_owned(),
                value: "slime".to_owned(),
            }],
        };
        let mut cmd = SetGenericDetails::new(id, body);
        cmd.apply(&mut doc).unwrap();
        assert!(matches!(
            &doc.codex().entry(id).unwrap().details,
            EntryDetails::Generic(g) if g.fields.len() == 1
        ));
        cmd.undo(&mut doc).unwrap();
        assert!(matches!(
            &doc.codex().entry(id).unwrap().details,
            EntryDetails::Generic(g) if g.fields.is_empty()
        ));
    }

    #[test]
    fn wrong_type_is_invalid_state() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "bit", EntryType::Character);
        let mut cmd = SetPaletteDetails::new(id, PaletteDetails::default());
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = SetCharacterDetails::new(CodexEntryId(9), CharacterDetails::default());
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }
}
