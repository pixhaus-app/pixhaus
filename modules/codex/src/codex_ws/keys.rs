//! Enum-to-i18n-key mappers and the relation-kind table.
//!
//! The Codex stores the enum; the shell localizes it. Each mapper returns a stable
//! `codex.*` key the shell resolves to display text at render time, so a renamed label
//! never invalidates a saved project. Kept in one place because the panel, editor,
//! details, coverage, and inspector areas all read these keys.

// Explicit imports rather than `use super::*`: the repo denies `clippy::wildcard_imports`
// outside test modules. The set is exactly the enum types these mappers match on, reached
// through the parent's re-exports.
use super::{AnchorKind, AnchorStrength, ColorRole, CoverageItemStatus, EntryStatus, EntryType, InclusionPriority, RelationKind};

/// The i18n key for an entry status (`codex.status.*`), resolved to display text at
/// render time. The Codex stores the enum; the shell localizes it.
pub(super) fn status_key(status: EntryStatus) -> &'static str {
    match status {
        EntryStatus::Draft => "codex.status.draft",
        EntryStatus::Candidate => "codex.status.candidate",
        EntryStatus::Canonical => "codex.status.canonical",
        EntryStatus::Deprecated => "codex.status.deprecated",
        EntryStatus::Archived => "codex.status.archived",
        EntryStatus::Rejected => "codex.status.rejected",
    }
}

/// The i18n key for a relationship kind (`codex.relation.*`).
pub(super) fn relation_key(kind: RelationKind) -> &'static str {
    match kind {
        RelationKind::Uses => "codex.relation.uses",
        RelationKind::BelongsTo => "codex.relation.belongs_to",
        RelationKind::AppearsIn => "codex.relation.appears_in",
        RelationKind::CompatibleWith => "codex.relation.compatible_with",
        RelationKind::IncompatibleWith => "codex.relation.incompatible_with",
        RelationKind::InheritsFrom => "codex.relation.inherits_from",
        RelationKind::VariantOf => "codex.relation.variant_of",
        RelationKind::Requires => "codex.relation.requires",
        RelationKind::Contains => "codex.relation.contains",
        RelationKind::Replaces => "codex.relation.replaces",
        RelationKind::InspiredBy => "codex.relation.inspired_by",
    }
}

/// The i18n key for an entry type (`codex.entry_type.*`).
pub(super) fn entry_type_key(entry_type: EntryType) -> &'static str {
    match entry_type {
        EntryType::Character => "codex.entry_type.character",
        EntryType::Enemy => "codex.entry_type.enemy",
        EntryType::Npc => "codex.entry_type.npc",
        EntryType::Creature => "codex.entry_type.creature",
        EntryType::Prop => "codex.entry_type.prop",
        EntryType::Item => "codex.entry_type.item",
        EntryType::Weapon => "codex.entry_type.weapon",
        EntryType::Material => "codex.entry_type.material",
        EntryType::Palette => "codex.entry_type.palette",
        EntryType::Style => "codex.entry_type.style",
        EntryType::Vibe => "codex.entry_type.vibe",
        EntryType::Location => "codex.entry_type.location",
        EntryType::Biome => "codex.entry_type.biome",
        EntryType::Faction => "codex.entry_type.faction",
        EntryType::Animation => "codex.entry_type.animation",
        EntryType::Pose => "codex.entry_type.pose",
        EntryType::Vfx => "codex.entry_type.vfx",
        EntryType::UiElement => "codex.entry_type.ui",
        EntryType::Rule => "codex.entry_type.rule",
        EntryType::Recipe => "codex.entry_type.recipe",
        EntryType::ReferenceBoard => "codex.entry_type.board",
    }
}

/// The i18n key for an inclusion priority (`codex.priority.*`), for the prompt
/// composer's per-fragment priority chip.
pub(super) fn priority_key(priority: InclusionPriority) -> &'static str {
    match priority {
        InclusionPriority::Critical => "codex.priority.critical",
        InclusionPriority::Important => "codex.priority.important",
        InclusionPriority::Normal => "codex.priority.normal",
        InclusionPriority::Optional => "codex.priority.optional",
        InclusionPriority::NeverInPrompt => "codex.priority.never_in_prompt",
    }
}

/// The i18n key for an anchor kind (`codex.anchor.kind.*`).
pub(super) fn anchor_kind_key(kind: AnchorKind) -> &'static str {
    match kind {
        AnchorKind::Identity => "codex.anchor.kind.identity",
        AnchorKind::Visual => "codex.anchor.kind.visual",
        AnchorKind::Palette => "codex.anchor.kind.palette",
        AnchorKind::Style => "codex.anchor.kind.style",
        AnchorKind::Animation => "codex.anchor.kind.animation",
        AnchorKind::Scale => "codex.anchor.kind.scale",
        AnchorKind::Lore => "codex.anchor.kind.lore",
        AnchorKind::Negative => "codex.anchor.kind.negative",
    }
}

/// The i18n key for an anchor strength (`codex.anchor.strength.*`).
pub(super) fn anchor_strength_key(strength: AnchorStrength) -> &'static str {
    match strength {
        AnchorStrength::Loose => "codex.anchor.strength.loose",
        AnchorStrength::Normal => "codex.anchor.strength.normal",
        AnchorStrength::Strong => "codex.anchor.strength.strong",
        AnchorStrength::Locked => "codex.anchor.strength.locked",
    }
}

/// The i18n key for a palette color role (`codex.color_role.*`).
pub(super) fn color_role_key(role: ColorRole) -> &'static str {
    match role {
        ColorRole::Shadow => "codex.color_role.shadow",
        ColorRole::Midtone => "codex.color_role.midtone",
        ColorRole::Highlight => "codex.color_role.highlight",
        ColorRole::Outline => "codex.color_role.outline",
        ColorRole::Skin => "codex.color_role.skin",
        ColorRole::Cloth => "codex.color_role.cloth",
        ColorRole::Metal => "codex.color_role.metal",
        ColorRole::MagicGlow => "codex.color_role.magic_glow",
        ColorRole::Danger => "codex.color_role.danger",
        ColorRole::Healing => "codex.color_role.healing",
        ColorRole::UiAccent => "codex.color_role.ui_accent",
    }
}

/// The localization key for a coverage status, used to label the per-slot status-cycle
/// button's tooltip with the status the click will move to.
pub(super) fn coverage_status_key(status: CoverageItemStatus) -> &'static str {
    match status {
        CoverageItemStatus::Missing => "codex.coverage.status.missing",
        CoverageItemStatus::Draft => "codex.coverage.status.draft",
        CoverageItemStatus::Generated => "codex.coverage.status.generated",
        CoverageItemStatus::NeedsReview => "codex.coverage.status.needs_review",
        CoverageItemStatus::Approved => "codex.coverage.status.approved",
        CoverageItemStatus::ManuallyFinalized => "codex.coverage.status.manually_finalized",
        CoverageItemStatus::Deprecated => "codex.coverage.status.deprecated",
    }
}

/// Every relationship kind, for the add-relationship picker.
pub(super) const RELATION_KINDS: [RelationKind; 11] = [
    RelationKind::Uses,
    RelationKind::BelongsTo,
    RelationKind::AppearsIn,
    RelationKind::CompatibleWith,
    RelationKind::IncompatibleWith,
    RelationKind::InheritsFrom,
    RelationKind::VariantOf,
    RelationKind::Requires,
    RelationKind::Contains,
    RelationKind::Replaces,
    RelationKind::InspiredBy,
];
