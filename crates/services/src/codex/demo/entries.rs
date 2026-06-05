//! The per-entry detail pass: header text, aliases, typed bodies, anchors, fragments,
//! and Canonical status for every hand-authored entry in the world.
//!
//! Split from the parent module so the world's bulkiest section reads on its own. The
//! shared command helpers (`update`, `anchor`, `fragments`, ...) and the negative
//! libraries live in `super`. The three spec-driven sub-passes (animations, rules,
//! recipes) live in their own sibling modules and are called by module path so the
//! dependency order stays explicit.

// Explicit imports rather than `use super::*`: the repo denies `clippy::wildcard_imports`
// outside test modules. The set is exactly what this section touches - the shared command
// helpers, the negative libraries, the sibling sub-pass modules, and the `core` types its
// hand-authored bodies name.
use super::{
    AddCodexAlias, AnchorKind, AnchorStrength, AntiAliasingRule, BuildError, CharacterDetails, CodexHandle, ColorRole, Command, DetailLevel, Document,
    EntryStatus, Handles, InclusionPriority, LineTreatment, NEG_ASSET, NEG_BIT_IDENTITY, NEG_STYLE, PaletteColor, PaletteDetails, PaletteRamp, QUALITY_POLISH,
    SetCharacterDetails, SetPaletteDetails, SetStyleDetails, StyleDetails, anchor, animations, delta, frag, fragments, generic, id, negatives, negatives_from,
    recipes, rules, status, update,
};

/// Fills in every entry's detail: header text, aliases, typed bodies, anchors,
/// fragments, and Canonical status. One long, flat pass per entry - the world is data,
/// and keeping it linear keeps the fixture auditable against the spec.
#[allow(clippy::too_many_lines)]
pub(super) fn detail_entries(doc: &mut Document, handles: &Handles) -> Result<(), BuildError> {
    // --- Bit (Character, hero) ---
    let bit = id(handles, "bit")?;
    update(
        doc,
        bit,
        delta(
            "Bit is the Pixhaus mascot: a small, friendly retro robot who guides the player and narrates the world. A boxy CRT/floppy-disk head with a glowing pixel-face screen, a stubby antenna with a blinking pixel, chunky rounded limbs, friendly proportions. Reads cleanly at 32px.",
            "Bit booted up in a forgotten arcade cabinet, wandered out of its own attract loop, and now explores the circuit-board world with stubborn optimism. Small, quick, never grim - it treats every dead end as the next thing to figure out.",
            "Round-over-square silhouette: a boxy CRT head about as tall as the torso, sitting on a chunky rounded biped body roughly two heads tall. The face is a single glowing pixel screen that shows the current expression; one stubby antenna with a blinking pixel on top. Stubby rounded arms and legs. No mouth - the screen carries all expression.",
            &["mascot", "hero", "robot", "retro", "platformer"],
        ),
    )?;
    let mut alias = AddCodexAlias::new(bit, CodexHandle::new("the_mascot")?);
    alias.apply(doc)?;
    let bit_body = CharacterDetails {
        proportions: "Two heads tall; the CRT head is about the same size as the torso. Chunky, rounded, stable stance.".to_owned(),
        silhouette_notes: "Round-over-square: a boxy head on a stubby rounded body reads at any zoom, holds at 32px. The antenna and the screen are the two silhouette landmarks. Body plan: upright biped, two arms and two legs, ~2 heads tall, chunky rounded proportions, legs about half the figure height. In side or three-quarter views, render the near-side arm and leg one value lighter and the far-side one value darker with a dark separation edge so overlapping limbs never merge into one shape.".to_owned(),
        palette_ref: Some(CodexHandle::new("bit_default")?),
        allowed_styles: vec![CodexHandle::new("pixel_art")?],
        forbidden_styles: vec![CodexHandle::new("flat_3d_render")?],
        animation_set: vec![
            CodexHandle::new("idle")?,
            CodexHandle::new("walk")?,
            CodexHandle::new("run")?,
            CodexHandle::new("jump")?,
            CodexHandle::new("fall")?,
            CodexHandle::new("attack")?,
            CodexHandle::new("hurt")?,
        ],
    };
    let mut bit_details = SetCharacterDetails::new(bit, bit_body);
    bit_details.apply(doc)?;
    anchor(
        doc,
        bit,
        AnchorKind::Identity,
        AnchorStrength::Locked,
        "Bit is always friendly and optimistic, never menacing or grimdark; a small retro robot that narrates the world.",
    )?;
    anchor(
        doc,
        bit,
        AnchorKind::Visual,
        AnchorStrength::Locked,
        "Bit is one specific robot in every frame: an upright chunky biped about two heads tall, a boxy CRT/floppy-disk head roughly the size of the torso, a single glowing pixel-face screen as the only expression (no mouth), one stubby antenna with a blinking pixel on top, stubby rounded arms and legs. Legs are about half the figure height, the head about a third; a small hip and shoulder offset keeps near and far limbs separable in side view. Faces right in the canonical view; the near-side limb reads one value lighter, the far-side one value darker, with a dark separation edge.",
    )?;
    anchor(
        doc,
        bit,
        AnchorKind::Palette,
        AnchorStrength::Strong,
        "Uses the Bit Default 6-colour 8-bit palette: charcoal body, cyan screen glow, off-white highlights, warm rust and sage-green accents.",
    )?;
    anchor(
        doc,
        bit,
        AnchorKind::Style,
        AnchorStrength::Strong,
        "Clean crisp pixel art, flat solid colour, strong silhouette, no anti-aliasing.",
    )?;
    anchor(
        doc,
        bit,
        AnchorKind::Scale,
        AnchorStrength::Normal,
        "Two heads tall, proportions held as fractions of total height so the figure stays on-model at any zoom; the round-over-square silhouette, the antenna, and the screen must each read at 32px on the 512x512 canvas.",
    )?;
    anchor(
        doc,
        bit,
        AnchorKind::Lore,
        AnchorStrength::Normal,
        "Booted from a forgotten arcade cabinet; explores a circuit-board world with stubborn optimism.",
    )?;
    anchor(
        doc,
        bit,
        AnchorKind::Negative,
        AnchorStrength::Locked,
        "No extra limbs, no mouth, no sharp teeth, no grimdark tone, no motion blur.",
    )?;
    fragments(
        doc,
        bit,
        vec![
            frag(
                "Bit, the Pixhaus mascot: an upright chunky retro robot about two heads tall with a boxy CRT/floppy-disk head, a single glowing pixel-face screen as its only expression (no mouth), one stubby antenna with a blinking pixel on top, stubby rounded arms and legs - a friendly round-over-square silhouette.",
                InclusionPriority::Critical,
            ),
            frag(
                "clean readable silhouette, the antenna and screen as the two landmarks, near-side limbs lighter and far-side darker with a dark separation edge so limbs stay clear of the body; reads at 32px",
                InclusionPriority::Important,
            ),
            frag("using @palette.bit_default in @style.pixel_art", InclusionPriority::Normal),
            frag("in the @vibe.retro_arcade world", InclusionPriority::Normal),
            frag(QUALITY_POLISH, InclusionPriority::Optional),
        ],
    )?;
    negatives_from(doc, bit, &[NEG_BIT_IDENTITY, NEG_STYLE], &["motion blur", "duplicate character or second Bit"])?;
    status(doc, bit, EntryStatus::Canonical)?;

    // --- Byte (Npc, companion) ---
    let byte = id(handles, "byte")?;
    update(
        doc,
        byte,
        delta(
            "Byte, Bit's companion - a small floating drone bot with a single round glowing lens-eye, a little spinning propeller on top, and slim arms. Shares Bit's crisp 8-bit palette.",
            "Where Bit walks, Byte hovers. A quieter machine, mostly lens and propeller, it tags along to light the dark corners of the circuit-board world and hand Bit the occasional floppy.",
            "A compact floating drone: one big round glowing lens-eye dominating the body, a small spinning propeller on top keeping it aloft, two slim arms. No legs - it never touches the floor.",
            &["companion", "npc", "drone", "robot", "retro"],
        ),
    )?;
    // Byte is an Npc, whose model body is generic (the rich Character body is reserved
    // for the Character type), so its character-style notes go in as generic fields.
    generic(
        doc,
        byte,
        &[
            ("proportions", "Compact and lens-dominated, about one and a half heads tall, smaller than Bit."),
            (
                "silhouette_notes",
                "A round lens body with a propeller nub on top and two slim arms - a floating circle reads instantly against Bit's boxy head.",
            ),
            (
                "body_plan",
                "Floating drone, no legs: hovers permanently, never planted to a ground baseline. The propeller reads as the propulsion surface - a filled disc with a clear leading edge, near side lighter, far side darker.",
            ),
            (
                "rest_state",
                "Resting state is a hover with a gentle vertical bob and a slight nose-up tilt; any flap or spin is low-amplitude in place, contrasting Bit's grounded idle.",
            ),
            ("palette_ref", "bit_default"),
            ("allowed_styles", "pixel_art"),
            ("animation_set", "idle"),
        ],
    )?;
    anchor(
        doc,
        byte,
        AnchorKind::Identity,
        AnchorStrength::Locked,
        "Byte is Bit's friendly floating companion drone - calm, helpful, never threatening.",
    )?;
    anchor(
        doc,
        byte,
        AnchorKind::Visual,
        AnchorStrength::Strong,
        "Byte is a compact floating drone: one big round glowing lens-eye, a small spinning propeller on top, two slim arms, no legs. It hovers - never planted on a ground line - resting with a gentle bob and a slight nose-up tilt.",
    )?;
    anchor(
        doc,
        byte,
        AnchorKind::Palette,
        AnchorStrength::Strong,
        "Same Bit Default 8-bit palette as Bit; the lens glows in the cyan screen-glow colour.",
    )?;
    anchor(
        doc,
        byte,
        AnchorKind::Negative,
        AnchorStrength::Strong,
        "No legs, no feet on the ground, no menacing red eye.",
    )?;
    fragments(
        doc,
        byte,
        vec![
            frag(
                "Byte, Bit's companion: a small floating drone bot with a single round glowing lens-eye, a little spinning propeller on top, two slim arms, no legs - it hovers, never touching the floor.",
                InclusionPriority::Critical,
            ),
            frag(
                "rests in a hover with a gentle bob and slight nose-up tilt; the propeller reads as a filled disc with a leading edge, near side lighter and far side darker",
                InclusionPriority::Important,
            ),
            frag(
                "the same crisp 8-bit palette as @character.bit, using @palette.bit_default in @style.pixel_art",
                InclusionPriority::Normal,
            ),
            frag(QUALITY_POLISH, InclusionPriority::Optional),
        ],
    )?;
    negatives_from(
        doc,
        byte,
        &[NEG_BIT_IDENTITY, NEG_STYLE],
        &["legs", "feet on the ground", "menacing red eye", "motion blur"],
    )?;
    status(doc, byte, EntryStatus::Canonical)?;

    // --- Bit Default Palette ---
    let palette = id(handles, "bit_default")?;
    update(
        doc,
        palette,
        delta(
            "The crisp 6-colour 8-bit palette the whole Bit world shares: a dark charcoal neutral, a cyan screen glow, an off-white highlight, a warm rust accent, a sage-green accent, and a near-black outline.",
            "",
            "Six flat, saturated 8-bit colours. Charcoal reads as the body, cyan as the live screen/lens glow, off-white as highlights and rim, rust and sage as the two warm/cool accents, and a near-black outline keeps every shape crisp.",
            &["palette", "8bit", "retro", "canonical"],
        ),
    )?;
    let palette_body = PaletteDetails {
        colors: vec![
            PaletteColor::new([24, 24, 32, 255], ColorRole::Shadow),
            PaletteColor::new([64, 200, 220, 255], ColorRole::MagicGlow),
            PaletteColor::new([240, 240, 245, 255], ColorRole::Highlight),
            PaletteColor::new([220, 90, 70, 255], ColorRole::Danger),
            PaletteColor::new([120, 200, 90, 255], ColorRole::Healing),
            PaletteColor::new([12, 12, 16, 255], ColorRole::Outline),
        ],
        ramps: vec![
            PaletteRamp {
                name: "Body charcoal ramp (outline -> shadow -> highlight)".to_owned(),
                color_indices: vec![5, 0, 2],
            },
            PaletteRamp {
                name: "Screen glow ramp (shadow -> cyan glow -> highlight)".to_owned(),
                color_indices: vec![0, 1, 2],
            },
        ],
        allow_generated_colors: false,
    };
    let mut palette_details = SetPaletteDetails::new(palette, palette_body);
    palette_details.apply(doc)?;
    anchor(
        doc,
        palette,
        AnchorKind::Palette,
        AnchorStrength::Locked,
        "Exactly these six 8-bit colours, by role: charcoal body, cyan screen glow, off-white highlight, warm rust accent, sage-green accent, near-black outline. Shade along the named ramps only. One cohesive limited palette, locked across every asset in the world - no new colours, no gradients.",
    )?;
    fragments(
        doc,
        palette,
        vec![
            frag(
                "a crisp 6-colour 8-bit palette by role: charcoal body, cyan screen glow, off-white highlights, warm rust and sage-green accents, a near-black outline",
                InclusionPriority::Important,
            ),
            frag(
                "shade by stepping along the ramp (shadow to base to highlight), not by blending; one cohesive limited palette locked across the whole world",
                InclusionPriority::Normal,
            ),
        ],
    )?;
    negatives(
        doc,
        palette,
        &[
            "off-palette colours",
            "gradients",
            "more than six colours",
            "blended or dithered mid-tones outside the ramp",
        ],
    )?;
    status(doc, palette, EntryStatus::Canonical)?;

    // --- Pixel Art (Style) ---
    let pixel_art = id(handles, "pixel_art")?;
    update(
        doc,
        pixel_art,
        delta(
            "The house look: clean crisp pixel art with a limited palette, strong silhouette, flat solid colour, crisp edges, and consistent lighting. No anti-aliasing.",
            "",
            "Single-weight clean outlines, flat fills, hard pixel edges, even lighting across every frame. The silhouette does the work; detail stays minimal so sprites read at small sizes.",
            &["style", "pixel-art", "8bit", "canonical"],
        ),
    )?;
    let style_body = StyleDetails {
        rendering_rules: "Clean 8-bit pixel art on a fixed grid. Selective dark outline on the outer silhouette only; interior form reads by value, not by line. Flat solid fills from a limited palette, no more colours than the ramp allows. Even, flat lighting across every frame - no directional cast shadow, no rim light, no spotlight. In side and three-quarter views, overlapping limbs carry a near/far value split (near limb one step lighter, far limb one step darker) with a dark separation edge so the two never merge into one shape. Proportions hold as fractions of total height so the figure stays on-model at any zoom and reads at 32px.".to_owned(),
        line_treatment: LineTreatment::Selective,
        detail_level: DetailLevel::Low,
        anti_aliasing: AntiAliasingRule::Manual,
        resolution: Some((512, 512)),
        negative_rules: vec![
            "automatic anti-aliasing".to_owned(),
            "smooth gradients or soft shading".to_owned(),
            "blur or motion blur".to_owned(),
            "3d render or photo-realism".to_owned(),
            "off-grid sub-pixel detail".to_owned(),
            "interior outline scribble (outline the silhouette, not every interior shape)".to_owned(),
            "more colours than the palette ramp".to_owned(),
        ],
    };
    let mut style_details = SetStyleDetails::new(pixel_art, style_body);
    style_details.apply(doc)?;
    anchor(
        doc,
        pixel_art,
        AnchorKind::Style,
        AnchorStrength::Locked,
        "Clean 8-bit pixel art on a fixed grid: a selective dark outline on the outer silhouette only, interior read by value, flat solid fills from a limited palette, even flat lighting, hard pixel edges, manual hand-placed anti-aliasing only - never automatic.",
    )?;
    anchor(
        doc,
        pixel_art,
        AnchorKind::Negative,
        AnchorStrength::Strong,
        "No painterly gradients, no automatic anti-aliasing, no soft shading, no interior outline scribble, no off-grid sub-pixel detail.",
    )?;
    fragments(
        doc,
        pixel_art,
        vec![
            frag(
                "clean 8-bit pixel art on a fixed grid: a selective dark outline on the outer silhouette only, interior form read by value not by line, flat solid fills, hard pixel edges, even flat lighting",
                InclusionPriority::Important,
            ),
            frag(
                "overlapping limbs carry a near/far value split with a dark separation edge so they never merge; limited palette, manual anti-aliasing only",
                InclusionPriority::Normal,
            ),
            frag(QUALITY_POLISH, InclusionPriority::Optional),
        ],
    )?;
    negatives_from(
        doc,
        pixel_art,
        &[NEG_STYLE],
        &["interior outline scribble", "same-value overlapping limbs reading as a blob"],
    )?;
    status(doc, pixel_art, EntryStatus::Canonical)?;

    // --- Retro-tech arcade (Vibe) ---
    let vibe = id(handles, "retro_arcade")?;
    update(
        doc,
        vibe,
        delta(
            "The mood of Bit's world: friendly retro-tech arcade. Glowing neon pixel screens, a circuit-board world, warm and optimistic - never grimdark.",
            "Everything hums with low-fi electricity: attract-mode glow, blinking node lights, the soft whine of a CRT. The feeling is a friendly arcade after hours, not a dystopia.",
            "Glowing cyan screens against charcoal, blueprint-grid floors, blinking node junctions. Light is warm and even; the palette stays bright and friendly.",
            &["vibe", "retro-tech", "arcade", "friendly", "canonical"],
        ),
    )?;
    generic(
        doc,
        vibe,
        &[
            ("mood", "friendly, optimistic, playful retro-tech - an arcade after hours, never a dystopia"),
            (
                "palette_cues",
                "saturated but limited: charcoal grounds, cyan screen glow, off-white highlights, warm rust and sage-green accents - the Bit Default set",
            ),
            (
                "lighting",
                "even and flat with a soft CRT bloom around lit screens and nodes; horizontally uniform on backgrounds so a layer can tile left-to-right without a hot side",
            ),
            (
                "setting",
                "the interior circuit-board world of a forgotten arcade cabinet: blueprint-grid floors, blinking node junctions, banks of pixel screens",
            ),
            ("era", "1980s-90s arcade register - scanline glow, attract-mode shimmer, chunky pixels"),
            ("tone_forbidden", "grimdark, horror, dystopian, gritty, photo-real"),
        ],
    )?;
    anchor(
        doc,
        vibe,
        AnchorKind::Lore,
        AnchorStrength::Strong,
        "Friendly retro-tech arcade: the interior circuit-board world of a forgotten cabinet, charcoal grounds under even flat light with a soft CRT bloom on cyan screens and nodes, saturated-but-limited 8-bit colour, warm and optimistic.",
    )?;
    anchor(
        doc,
        vibe,
        AnchorKind::Negative,
        AnchorStrength::Strong,
        "Never grimdark, never horror, never dystopian.",
    )?;
    fragments(
        doc,
        vibe,
        vec![
            frag(
                "friendly retro-tech arcade mood: a circuit-board world inside an old cabinet, glowing cyan pixel screens against charcoal, blinking node junctions, warm and optimistic",
                InclusionPriority::Normal,
            ),
            frag(
                "even flat lighting with a soft CRT bloom; saturated but limited 8-bit colour from @palette.bit_default; backgrounds lit horizontally evenly so they tile without a hot side",
                InclusionPriority::Normal,
            ),
        ],
    )?;
    negatives(
        doc,
        vibe,
        &["grimdark", "horror", "dystopian", "dark and gritty", "photo-real", "neon-noir rain"],
    )?;
    status(doc, vibe, EntryStatus::Canonical)?;

    // --- The seven animations ---
    animations::detail_animations(doc, handles)?;

    // --- Turnaround (Pose / reference entry) ---
    //
    // The turnaround is a Pose entry, which holds a Generic body, so its four-view
    // breakdown and timing notes go in as generic fields (SetAnimationDetails would
    // reject a Pose entry). The description, fragments, negatives, and the three
    // Locked anchors carry the authored model-sheet spec. This is the identity
    // reference every other Bit animation is checked against (solid drawing).
    let turnaround = id(handles, "turnaround")?;
    update(
        doc,
        turnaround,
        delta(
            "Bit's model-sheet turnaround - front, three-quarter, side, and back views at identical scale and volume. Not a motion cycle: the identity reference every animation entry is built against.",
            "",
            "Four orthographic views in a row at identical scale and volume - front, three-quarter, side profile, back - the master reference the directional sprites and every animation derive from. Only the viewing angle changes (solid drawing).",
            &["pose", "turnaround", "reference", "model-sheet"],
        ),
    )?;
    generic(
        doc,
        turnaround,
        &[
            (
                "purpose",
                "Lock Bit's identity across viewing angles so every other animation stays on-model (solid drawing): identical volume and proportion across all views, one clear view per cell, a strong silhouette per view, the profile rule for legibility. A static multi-view reference, not a motion cycle.",
            ),
            ("loop_behavior", "Once - a reference sheet, not a played clip"),
            ("recommended_frame_count", "4 - one canonical view per cell"),
            ("fps", "2 - if ever cycled, a slow view-to-view flip, not motion"),
            (
                "view.front",
                "Bit square to camera, both feet planted, arms relaxed at the sides but clear of the torso, antenna straight up with a slight natural lean, the screen showing the neutral level eyes. Baseline proportions: ~2 heads tall, boxy CRT head over a chunky rounded biped body, a round-over-square silhouette.",
            ),
            (
                "view.three_quarter",
                "Rotated ~45 degrees, showing the depth of the CRT head and the body volume; antenna and screen visible; same height and mass as the front (solid drawing - volume constant).",
            ),
            (
                "view.side",
                "Full side profile: the profile rule for legibility, the depth of the head and the stubby antenna's attachment clear, one arm and one leg reading against the body, the screen edge-on or angled to stay readable. Same height line as the other views. Apply the near/far value split on the visible arm and leg (near lighter, far darker) with a dark separation edge.",
            ),
            (
                "view.back",
                "Rear view: head and body shape from behind, antenna from the back, no screen (the back of the CRT head), proportions identical to the front. Confirms the silhouette closes from every angle.",
            ),
        ],
    )?;
    anchor(
        doc,
        turnaround,
        AnchorKind::Animation,
        AnchorStrength::Locked,
        "Turnaround is a static four-view reference (front / three-quarter / side / back), not a motion cycle: identical height, scale, volume, and proportion across all views, a neutral pose, only the angle changes.",
    )?;
    anchor(
        doc,
        turnaround,
        AnchorKind::Scale,
        AnchorStrength::Locked,
        "Every view must read as a closed, on-model silhouette at 32px; this sheet is the identity all other Bit animations are checked against.",
    )?;
    anchor(
        doc,
        turnaround,
        AnchorKind::Style,
        AnchorStrength::Locked,
        "@style.pixel_art on @palette.bit_default - crisp 8-bit, no anti-aliasing, consistent across all four views.",
    )?;
    fragments(
        doc,
        turnaround,
        vec![
            frag(
                "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen as the only expression, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall, round-over-square silhouette), model-sheet turnaround.",
                InclusionPriority::Critical,
            ),
            frag(
                "Four views - front, three-quarter, side profile, back - at identical height, scale, volume, and proportion; only the viewing angle changes (solid drawing).",
                InclusionPriority::Important,
            ),
            frag(
                "A neutral standing pose in every view, arms relaxed but clear of the torso silhouette, antenna upright; the side view follows the profile rule for legibility; the back view shows the CRT head from behind with no screen.",
                InclusionPriority::Important,
            ),
            frag(
                "Each view is a strong, closed, readable silhouette at 32px; consistent feature placement (head, screen, antenna, limbs) across all four.",
                InclusionPriority::Important,
            ),
            frag(
                "Reference sheet: 4 views in a left-to-right grid, identical cell size, identical character scale, identical front lighting and camera distance per cell, a shared ground line.",
                InclusionPriority::Normal,
            ),
            frag(
                "crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background",
                InclusionPriority::Normal,
            ),
        ],
    )?;
    negatives(
        doc,
        turnaround,
        &[
            "scale, height, or proportion change between views - identical volume across angles",
            "an expression change or action pose - neutral standing only",
            "a screen visible on the back view",
            "extra limbs, a duplicate character beyond the four intended views, background",
            "motion blur, action smears",
            "mouth, facial features beyond the pixel-face screen",
            "anti-aliasing, painterly gradients, soft shading",
            "perspective or lighting drift between cells",
        ],
    )?;
    status(doc, turnaround, EntryStatus::Canonical)?;

    // --- Floppy (Item) ---
    let floppy = id(handles, "floppy")?;
    update(
        doc,
        floppy,
        delta(
            "A retro floppy-disk power-up from Bit's world - a chunky 3.5-inch floppy disk with a glowing label and a pixel shine.",
            "Scattered through the circuit-board world, a Floppy is a fragment of forgotten data Bit can pick up. Byte often hands them over.",
            "A chunky 3.5-inch floppy disk seen front-on, sliding metal shutter at the top, a bright label across the middle with a soft glow, and a single pixel shine on one corner. 8-bit palette.",
            &["item", "power-up", "collectible", "retro", "floppy"],
        ),
    )?;
    generic(
        doc,
        floppy,
        &[
            ("kind", "power-up"),
            ("rarity", "common"),
            ("effect", "data fragment pickup"),
            (
                "silhouette",
                "a square 3.5-inch floppy-disk shell, sliding metal shutter across the top, a label strip across the middle",
            ),
            ("material", "matte plastic shell with a brushed-metal shutter, flat 8-bit fills - not glossy"),
            (
                "composition",
                "one object centred, filling about three-quarters of the frame, clear margin all around, not touching the edges",
            ),
            ("view", "flat 2D front-on, even ambient lighting, a single pixel shine - no perspective"),
        ],
    )?;
    anchor(
        doc,
        floppy,
        AnchorKind::Visual,
        AnchorStrength::Strong,
        "A chunky 3.5-inch floppy disk seen flat and front-on: square shell, a sliding metal shutter across the top, a glowing label strip across the middle, one pixel shine on a corner. One clean readable silhouette, centred, no perspective.",
    )?;
    anchor(
        doc,
        floppy,
        AnchorKind::Palette,
        AnchorStrength::Strong,
        "Bit Default 8-bit palette; the label glows in the cyan screen-glow colour.",
    )?;
    fragments(
        doc,
        floppy,
        vec![
            frag(
                "a chunky 3.5-inch floppy-disk power-up, flat and front-on: square shell, a sliding metal shutter across the top, a glowing cyan label strip, one pixel shine",
                InclusionPriority::Important,
            ),
            frag(
                "one object centred and filling about three-quarters of the frame with a clear margin, even ambient lighting, a clean keyable silhouette",
                InclusionPriority::Important,
            ),
            frag(
                "from @character.bit's world, using @palette.bit_default in @style.pixel_art",
                InclusionPriority::Normal,
            ),
            frag(QUALITY_POLISH, InclusionPriority::Optional),
        ],
    )?;
    negatives_from(
        doc,
        floppy,
        &[NEG_STYLE, NEG_ASSET],
        &[
            "modern USB drive",
            "object touching the frame edge",
            "more than one object",
            "glossy reflection",
        ],
    )?;
    status(doc, floppy, EntryStatus::Canonical)?;

    // --- Circuit Tiles (Material) ---
    let tiles = id(handles, "circuit_tiles")?;
    update(
        doc,
        tiles,
        delta(
            "A top-down circuit-board floor tileset for Bit's world - blueprint-grid lines, solder traces, glowing node junctions, seamless edges.",
            "",
            "A seamless top-down floor: blueprint-grid lines on charcoal, copper-style solder traces routing between glowing cyan node junctions. Tiles align on a grid with seamless edges so they repeat without seams.",
            &["material", "tileset", "circuit-board", "floor", "seamless"],
        ),
    )?;
    generic(
        doc,
        tiles,
        &[
            (
                "tiling",
                "seamless on all four edges: top matches bottom, left matches right, so tiles repeat with no visible seam",
            ),
            ("surface", "top-down circuit-board floor: a dark PCB-green substrate over charcoal"),
            (
                "detail",
                "fine copper-style solder traces routing between small silver solder-pad nodes that glow cyan; detail spread evenly so any patch looks interchangeable - no hero chip, no full-width trace run",
            ),
            (
                "node_language",
                "nodes sit at trace junctions, evenly distributed; vias glow in the cyan screen-glow colour",
            ),
            ("lighting", "even ambient light, no directional cast shadow, no center hotspot"),
            ("edges", "crisp anti-alias-free tile edges, no colour fringe"),
        ],
    )?;
    anchor(
        doc,
        tiles,
        AnchorKind::Visual,
        AnchorStrength::Strong,
        "A seamless top-down circuit-board floor: a dark PCB-green substrate, fine copper solder traces routing between small silver solder-pad nodes that glow cyan, blueprint-grid lines. Trace and node detail spread evenly so any region looks interchangeable - no hero chip, one continuous board.",
    )?;
    anchor(
        doc,
        tiles,
        AnchorKind::Palette,
        AnchorStrength::Strong,
        "Bit Default 8-bit palette; node junctions glow in the cyan screen-glow colour.",
    )?;
    anchor(
        doc,
        tiles,
        AnchorKind::Negative,
        AnchorStrength::Strong,
        "No visible seam or grid line between tiles, no cell-sized panel, no full-width or full-height trace run, no single dominant chip, no edge fringe, no directional shadow.",
    )?;
    fragments(
        doc,
        tiles,
        vec![
            frag(
                "a seamless top-down circuit-board floor tile: dark PCB-green substrate, fine copper solder traces, small silver solder-pad nodes glowing cyan, blueprint-grid lines",
                InclusionPriority::Important,
            ),
            frag(
                "tiles seamlessly on all four edges (top matches bottom, left matches right); trace and node detail spread evenly so any patch is interchangeable - no hero chip, no full-width trace run, one continuous board",
                InclusionPriority::Important,
            ),
            frag(
                "even ambient lighting, crisp pixel edges; for @location.arcade_world, using @palette.bit_default in @style.pixel_art",
                InclusionPriority::Normal,
            ),
        ],
    )?;
    negatives_from(
        doc,
        tiles,
        &[NEG_STYLE],
        &[
            "visible tile seam",
            "grid line between tiles",
            "cell-sized panel or beveled block",
            "full-width or full-height trace run",
            "single dominant hero chip",
            "uneven detail density across the surface",
            "directional cast shadow",
            "edge fringe or halo",
        ],
    )?;
    status(doc, tiles, EntryStatus::Canonical)?;

    // --- The Arcade Cabinet World (Location) ---
    let world = id(handles, "arcade_world")?;
    update(
        doc,
        world,
        delta(
            "The circuit-board world inside a forgotten arcade cabinet where Bit lives - glowing screens, blueprint-grid floors, and humming node junctions.",
            "Behind the cabinet's dark screen is a whole world built from its own circuitry: floors of circuit-board, walls of stacked components, and the steady hum of a machine that never quite powered down. Bit calls it home; Byte lights its corners.",
            "Interiors of charcoal and glowing cyan: circuit-board floors (the Circuit Tiles), banks of pixel screens, blinking node junctions, all under warm even arcade light. Friendly, not grim.",
            &["location", "arcade", "circuit-board", "world", "canonical"],
        ),
    )?;
    generic(
        doc,
        world,
        &[
            ("type", "interior circuit-board world inside a forgotten arcade cabinet"),
            (
                "scene_brief",
                "Inside a dark cabinet, a whole world built from its own circuitry. Even flat light with a soft CRT bloom on cyan screens and nodes; charcoal grounds, saturated-but-limited 8-bit colour; warm, friendly, optimistic - never grim.",
            ),
            (
                "layer_sky",
                "opaque charcoal back wall with a faint blueprint grid; tonal gradient runs top-to-bottom only so it tiles horizontally without a hot side",
            ),
            ("layer_far", "distant silhouettes of stacked components and screen banks, dim cyan glow"),
            ("layer_mid", "rows of arcade-cabinet shapes and node junctions, the readable middle band"),
            ("layer_near", "foreground circuit-board floor (the Circuit Tiles) and nearby props"),
            ("lighting", "ambient and horizontally even; no sun or single light source on one side"),
        ],
    )?;
    anchor(
        doc,
        world,
        AnchorKind::Lore,
        AnchorStrength::Strong,
        "A circuit-board world inside a forgotten arcade cabinet, read in depth bands: a charcoal back wall, distant component silhouettes, a middle band of cabinets and node junctions, a foreground circuit-board floor. Even flat light with a soft CRT bloom, warm and friendly, never grim.",
    )?;
    anchor(doc, world, AnchorKind::Palette, AnchorStrength::Normal, "Bit Default 8-bit palette throughout.")?;
    fragments(
        doc,
        world,
        vec![
            frag(
                "the circuit-board world inside a forgotten arcade cabinet: a charcoal back wall, distant component silhouettes, a middle band of cabinets and blinking node junctions, a foreground floor of @material.circuit_tiles",
                InclusionPriority::Normal,
            ),
            frag(
                "ambient, horizontally even lighting with a soft CRT bloom (no light on one side); @palette.bit_default in @style.pixel_art, the @vibe.retro_arcade mood",
                InclusionPriority::Normal,
            ),
        ],
    )?;
    negatives(
        doc,
        world,
        &[
            "grimdark",
            "horror",
            "outdoor landscape",
            "realistic photo",
            "a single directional light source",
            "characters baked into the background",
        ],
    )?;
    status(doc, world, EntryStatus::Canonical)?;

    // --- Rules ---
    rules::detail_rules(doc, handles)?;

    // --- Start Button (UiElement) ---
    let button = id(handles, "start_button")?;
    update(
        doc,
        button,
        delta(
            "The title-screen Start button - a chunky pixel-art button with a glowing label, in Bit's house look.",
            "",
            "A chunky rounded rectangular button with a near-black outline, charcoal fill, an off-white pixel label reading START, and a soft cyan glow on its active state. Hard pixel edges.",
            &["ui", "button", "hud", "title-screen"],
        ),
    )?;
    generic(doc, button, &[("shape", "chunky rounded rectangle"), ("label", "START")])?;
    anchor(
        doc,
        button,
        AnchorKind::Style,
        AnchorStrength::Strong,
        "Clean pixel-art button, hard edges, flat fills, cyan glow on the active state.",
    )?;
    anchor(
        doc,
        button,
        AnchorKind::Palette,
        AnchorStrength::Normal,
        "Bit Default 8-bit palette; glow in the cyan screen-glow colour.",
    )?;
    fragments(
        doc,
        button,
        vec![frag(
            "a chunky pixel-art Start button with a glowing label, using @palette.bit_default in @style.pixel_art",
            InclusionPriority::Normal,
        )],
    )?;
    negatives(doc, button, &["3d bevel", "gradient fill", "drop shadow blur"])?;
    status(doc, button, EntryStatus::Canonical)?;

    // --- Bit idle cycle (Recipe) ---
    let recipe = id(handles, "bit_idle_cycle")?;
    update(
        doc,
        recipe,
        delta(
            "A reusable recipe for generating Bit's idle breathing loop: the idle animation of Bit in the house style and palette.",
            "",
            "An 8-frame idle loop sheet of Bit breathing in place, consistent scale, transparent background.",
            &["recipe", "idle", "animation", "workflow"],
        ),
    )?;
    generic(
        doc,
        recipe,
        &[
            ("character", "bit"),
            ("animation", "idle"),
            ("style", "pixel_art"),
            ("palette", "bit_default"),
            ("frames", "8"),
            ("fps", "8"),
            ("canvas", "512x512 cells, pin the resolution explicitly"),
            (
                "step_1_anchor",
                "Lock identity first: one neutral Bit reference on a flat key (the turnaround front view), the canonical on-model image every cell is matched against.",
            ),
            (
                "step_2_pose_table",
                "Author the 8 idle pose beats (rest, inhale rise, top-of-breath moving hold, blink, exhale settle) as the pose map the model skins - do not let the model invent the motion.",
            ),
            (
                "step_3_skin",
                "Render the 8-frame sheet with the anchor attached so every cell stays the same robot; lowest temperature for an identity-critical multi-cell sheet.",
            ),
            (
                "step_4_normalize",
                "Align and scale-normalize the cells to one baseline and one scale; key the background to transparent.",
            ),
            (
                "step_5_review",
                "Check the sheet against the Rules folder: on-model identity, in-place stability, clean keyed silhouette, no off-palette colour.",
            ),
        ],
    )?;
    anchor(
        doc,
        recipe,
        AnchorKind::Animation,
        AnchorStrength::Normal,
        "One full slow breath across an 8-frame loop; feet planted, antenna lag.",
    )?;
    fragments(
        doc,
        recipe,
        vec![frag(
            "generate @character.bit doing @animation.idle in @style.pixel_art using @palette.bit_default, an 8-frame loop on a 512x512 transparent canvas, every cell matched to the @pose.turnaround front view, checked against @rule.identity_lock and @rule.clean_silhouette",
            InclusionPriority::Normal,
        )],
    )?;
    negatives(doc, recipe, &["drift between frames", "scale change", "background"])?;
    status(doc, recipe, EntryStatus::Canonical)?;

    // --- The forbidden alternative style, the new rules, and the two recipes ---
    recipes::detail_forbidden_style(doc, handles)?;
    recipes::detail_new_recipes(doc, handles)?;

    Ok(())
}
