//! Fixed/variable invariant clauses for anchor-conditioned generation.
//!
//! Every v2 generation that conditions on an anchor image (the neutral reset,
//! the directional anchors, the animation first frames) routes the anchor PNG
//! into the backend's `reference_images` slot but, without this clause, never
//! tells the model what about that reference stays fixed. A numeric IP-Adapter
//! weight pulls globally toward the reference but cannot distinguish "keep the
//! face and palette" from "let the pose change", so a directional turn or a seed
//! frame can quietly redraw the costume or rescale the character.
//!
//! [`identity_lock_clause`] is the single source of the locked-attribute
//! vocabulary: it names the reference image, lists the attributes that stay
//! identical, then the one axis that is free to change. Keeping the fixed list
//! in one private const stops it drifting between call sites — the verb, the
//! studio cascade, and the clip self-seed all read the same words.

/// The one axis a derivation is allowed to change while every other attribute
/// of the referenced character stays locked.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VariableAxis {
    /// The pose changes (a first frame, a neutral reset).
    Pose,
    /// The facing direction changes (a directional anchor turn).
    Facing,
    /// The animation phase changes (a clip self-seed).
    Phase,
}

/// The attributes that stay identical to the reference across every derivation.
/// One const so the verb, the cascade, and the clip self-seed cannot disagree.
const FIXED_ATTRS: &str = "identical silhouette, palette, face, costume, and scale";

impl VariableAxis {
    /// The free axis named in the clause, e.g. `"the facing direction"`.
    fn variable_phrase(self) -> &'static str {
        match self {
            VariableAxis::Pose => "the pose",
            VariableAxis::Facing => "the facing direction",
            VariableAxis::Phase => "the animation phase",
        }
    }
}

/// Builds the identity-lock clause for one variable axis.
///
/// The phrase opens by naming the reference image (so reference-visible-first
/// holds even when the model sees only text), lists the locked attributes from
/// [`FIXED_ATTRS`], then states the single axis that is free to change. Pure,
/// deterministic, and never empty — the same axis always yields the same bytes.
///
/// ```
/// # use pixhaus_ai::compose::{identity_lock_clause, VariableAxis};
/// let clause = identity_lock_clause(VariableAxis::Facing);
/// assert!(clause.contains("reference image"));
/// assert!(clause.contains("the facing direction"));
/// ```
#[must_use]
pub fn identity_lock_clause(axis: VariableAxis) -> String {
    format!(
        "keep the same character from the reference image: {FIXED_ATTRS}; change only {}",
        axis.variable_phrase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use rstest::rstest;

    #[rstest]
    #[case::pose(VariableAxis::Pose, "the pose")]
    #[case::facing(VariableAxis::Facing, "the facing direction")]
    #[case::phase(VariableAxis::Phase, "the animation phase")]
    fn names_the_reference_the_fixed_list_and_only_that_axis(#[case] axis: VariableAxis, #[case] variable: &str) {
        let clause = identity_lock_clause(axis);
        // Reference-visible-first: the clause names the reference image as input.
        assert!(clause.contains("reference image"), "clause must name the reference image: {clause}");
        // The full locked-attribute list rides every clause.
        for attr in ["silhouette", "palette", "face", "costume", "scale"] {
            assert!(clause.contains(attr), "clause must lock `{attr}`: {clause}");
        }
        // Exactly this axis's variable phrase is named as free to change.
        assert!(clause.contains(&format!("change only {variable}")), "clause must free only `{variable}`: {clause}");
    }

    #[rstest]
    #[case::pose(VariableAxis::Pose)]
    #[case::facing(VariableAxis::Facing)]
    #[case::phase(VariableAxis::Phase)]
    fn has_no_leading_comma_and_is_never_empty(#[case] axis: VariableAxis) {
        let clause = identity_lock_clause(axis);
        assert!(!clause.is_empty(), "the clause is never empty");
        assert!(!clause.starts_with(','), "the clause has no leading comma: {clause}");
        assert!(!clause.starts_with(' '), "the clause has no leading space: {clause}");
    }

    #[test]
    fn each_axis_names_only_its_own_variable_phrase() {
        // Pose's clause must not mention facing or phase, and vice versa, so a
        // mis-routed axis is caught rather than silently over-stating freedom.
        let pose = identity_lock_clause(VariableAxis::Pose);
        let facing = identity_lock_clause(VariableAxis::Facing);
        let phase = identity_lock_clause(VariableAxis::Phase);
        assert!(!pose.contains("facing direction"), "pose clause must not free facing: {pose}");
        assert!(!pose.contains("animation phase"), "pose clause must not free phase: {pose}");
        assert!(!facing.contains("animation phase"), "facing clause must not free phase: {facing}");
        assert!(!phase.contains("facing direction"), "phase clause must not free facing: {phase}");
    }

    #[test]
    fn clause_strings_are_frozen() {
        insta::assert_snapshot!("identity_lock_pose", identity_lock_clause(VariableAxis::Pose));
        insta::assert_snapshot!("identity_lock_facing", identity_lock_clause(VariableAxis::Facing));
        insta::assert_snapshot!("identity_lock_phase", identity_lock_clause(VariableAxis::Phase));
    }
}
