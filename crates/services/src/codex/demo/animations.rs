//! The seven Bit animations: a principle-grounded teaching set.
//!
//! Split from the parent because the animation table is the world's longest spec-driven
//! section. Shared command helpers come from `super`; this file is called by
//! [`detail_entries`](super::entries::detail_entries) in entry order.

// Explicit imports rather than `use super::*`: the repo denies `clippy::wildcard_imports`
// outside test modules. The set is exactly what this section touches - the shared command
// helpers and the `core` animation types its specs name.
use super::{
    AnchorKind, AnchorStrength, AnimationDetails, BuildError, CodexHandle, Command, Document, EntryStatus, Handles, InclusionPriority, LoopBehavior, PoseBeat,
    PromptFragment, QUALITY_POLISH, SetAnimationDetails, anchor, delta, frag, fragments, id, negatives, status, update,
};

/// One animation's full specification, grounded in the animation-principles knowledge
/// base so the demo Codex teaches as it generates. Each field maps to a public Codex
/// command: `description` to [`UpdateCodexEntry`](pixhaus_core::commands::UpdateCodexEntry),
/// the timing/loop/`beats` to [`SetAnimationDetails`], `fragments` to
/// [`SetPromptFragments`](pixhaus_core::commands::SetPromptFragments), `negatives` to
/// [`SetNegativeFragments`](pixhaus_core::commands::SetNegativeFragments), and the three
/// anchors to [`SetAnchor`](pixhaus_core::commands::SetAnchor).
struct AnimSpec {
    handle: &'static str,
    description: &'static str,
    purpose: &'static str,
    loop_behavior: LoopBehavior,
    frames: u32,
    fps: u16,
    /// The key poses, in playback order; each beat names the principle it embodies.
    beats: &'static [(&'static str, &'static str)],
    /// Priority-tagged positive fragments: a Critical identity+action line, the
    /// Important motion/timing/arc/weight lines, then Normal sheet-framing and
    /// `@`-references that resolve against the world.
    fragments: &'static [(&'static str, InclusionPriority)],
    /// Per-entry negative fragments (drift, off-model, clipping, scale-change, etc.).
    negatives: &'static [&'static str],
    /// The motion-intent Animation anchor, at the given strength.
    animation_anchor: (AnchorStrength, &'static str),
    /// The Scale anchor that keeps the motion readable at 32px.
    scale_anchor: (AnchorStrength, &'static str),
    /// The Style anchor pinning the house look.
    style_anchor: (AnchorStrength, &'static str),
}

/// Fills in the seven Bit animations as a best-in-class, principle-grounded teaching
/// set. Each spec is self-contained: its own principled fps and frame count (from the
/// timing/spacing KB, not a shared constant), pose beats that walk the key poses and
/// name the principle each one embodies, priority-tagged prompt fragments (a Critical
/// identity+action line, Important motion/timing/arc/weight/anticipation/overlap
/// lines, Normal sheet-framing and `@`-references), per-entry negatives, and three
/// anchors (Animation motion-intent, Scale 32px-read, Style house-look).
///
/// Two families share discipline: the locomotion family (idle / walk / run) shares
/// ground-contact, weight read through the up/down of mass, and antenna-lag overlap;
/// the impact/reaction family (attack / hurt) shares the AAR shape (anticipation,
/// snap action, overshoot, held settle) and the screen-switch-on-settle rule.
///
/// One long body on purpose: the seven specs are flat data with no branching to
/// factor out, and keeping them in one table keeps the fixture auditable against the
/// authored spec.
#[allow(clippy::too_many_lines)]
pub(super) fn detail_animations(doc: &mut Document, handles: &Handles) -> Result<(), BuildError> {
    use AnchorStrength::{Locked, Strong};
    use InclusionPriority::{Critical, Important, Normal};

    let specs = [
        AnimSpec {
            handle: "idle",
            description: "Bit at rest, alive but still - a slow breathing oscillation with a periodic blink and a trailing antenna sway. The base loop every other state returns to.",
            purpose: "Keep Bit reading as alive while idle by replacing a dead static hold with a moving hold (a static hold past ~8 frames looks dead). Governing principles: moving-hold timing, slow-in/slow-out on the breath, secondary action and overlap/follow-through on the antenna, appeal. The body breathes; the antenna lags and settles; the screen blinks as punctuation.",
            loop_behavior: LoopBehavior::Loop,
            frames: 8,
            fps: 6,
            beats: &[
                (
                    "Rest low (neutral)",
                    "Settled square on both feet, shoulders at their lowest, screen showing soft level eyes (content glyph), antenna upright with a hair of lean. Line of action is a gentle resting S-curve (line-of-action).",
                ),
                (
                    "Inhale rise (slow-out)",
                    "Torso eases upward, shoulders lift, the body stretches a touch taller without changing volume; spacing tightens at the start of the rise (squash/stretch volume-preserved, slow-out).",
                ),
                (
                    "Top of breath (moving hold)",
                    "Highest point, briefly held; the antenna still catches up, leaning back as it lags the rise by ~2-3 frames (wave action, overlap). No dead freeze - this is the moving hold, not a stop.",
                ),
                (
                    "Blink accent",
                    "A single-frame screen blink as punctuation, placed at the top so it reads as a calm beat (acting: blinks mark beats).",
                ),
                (
                    "Exhale settle low (slow-in)",
                    "Torso eases back down to rest, decelerating into the low pose; the antenna overshoots forward slightly then settles (slow-in, follow-through with decaying amplitude).",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen as the only expression, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), standing idle and breathing in place.",
                    Critical,
                ),
                (
                    "Slow breathing oscillation: torso rises and falls a few pixels over the loop, slow-in and slow-out, never a frozen pose (a moving hold, not a static hold).",
                    Important,
                ),
                (
                    "Antenna lags the breath by 2-3 frames and overshoots slightly before settling - overlap and follow-through; the blinking pixel rides the tip.",
                    Important,
                ),
                (
                    "Pixel-face screen holds a soft, level, content expression with one single-frame blink as punctuation; expression states switch crisply, never blur or morph.",
                    Important,
                ),
                (
                    "Feet stay planted flat on the ground line across all frames; weight even on both feet; a gentle resting S-curve in the body silhouette, readable at 32px.",
                    Important,
                ),
                (
                    "Sprite sheet: 8 frames in a left-to-right, top-to-bottom grid, identical cell size, identical character scale, identical lighting and camera in every cell.",
                    Normal,
                ),
                (
                    "crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background for clean keying",
                    Normal,
                ),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "static frozen pose, dead hold",
                "scale change between cells, character changing size between frames",
                "extra limbs, duplicate character, a second Bit in a cell",
                "background scenery, background changes between cells",
                "motion blur, smear frames",
                "mouth, facial features beyond the pixel-face screen",
                "anti-aliasing, painterly gradients, soft shading",
                "antenna stapled rigid to the head",
            ],
            animation_anchor: (
                Locked,
                "Idle is an 8-frame loop at 6 fps: breathe up on slow-out, hold at the top as a moving hold, settle down on slow-in; the antenna lags 2-3 frames and overshoots; one blink per loop. The loop seam is seamless - the last frame eases into the first.",
            ),
            scale_anchor: (
                Strong,
                "The breathing rise and antenna sway must stay readable in the 32px silhouette; no detail that only reads when zoomed in.",
            ),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default - crisp 8-bit, no anti-aliasing."),
        },
        AnimSpec {
            handle: "walk",
            description: "Bit's standard walk loop - a brisk, energetic four-pose step cycle with a clear weight shift, counter-swinging arms, and a bobbing head the antenna trails behind. The locomotion backbone.",
            purpose: "A readable, weight-bearing walk on the four-pose foundation (contact / down / passing / up), conveying weight through the head/body bob and the belt-line tilt. Governing principles: arcs, slow-in/slow-out, follow-through on the antenna, timing, weight-shift on the belt line, arm counter-swing in opposition. Encodes contralateral motion and avoids the lockstep-arms robot tell.",
            loop_behavior: LoopBehavior::Loop,
            frames: 8,
            fps: 10,
            beats: &[
                (
                    "Contact (right lead)",
                    "Right foot lands extended forward, left foot extended behind, body just above mid-height. Left arm forward, right arm back (opposition). Belt line tilts toward the off-weight leg (weight-shift). Feet travel on a clear arc, not a straight slide.",
                ),
                (
                    "Down / recoil",
                    "Weight transfers onto the front leg, knee bends, body at its lowest point - where weight reads. The arm reaches its peak swing here, one frame after contact (the arm lags the leg). Belt line tilts strongly toward the bent weight-bearing leg.",
                ),
                (
                    "Passing",
                    "The free leg passes the standing leg, the standing leg straight, body at/near its highest. Belt line briefly level - the personality pose. Light S-curve through the spine, shoulders counter-tilting the hips.",
                ),
                (
                    "Up / push-off",
                    "The standing leg straightens fully, pushing the body to peak height, the free foot reaching forward into the next contact. Belt line tilts toward the push-off leg. The antenna trails the bob, lagging ~2-4 frames, overshooting at the top before settling (overlap/follow-through). Screen neutral-content, eyes forward.",
                ),
                (
                    "Contact (left lead)",
                    "Mirror of the first contact for the second step: left foot lands, right behind, arms swap to the opposite opposition. The remaining beats mirror down / passing / up.",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), walking in a side-view step cycle.",
                    Critical,
                ),
                (
                    "Four-pose walk per step - contact, down (lowest, knee bent, weight reads here), passing (highest, body lifts), up (push-off) - over two steps, left and right mirrored.",
                    Important,
                ),
                (
                    "Arms swing in opposition to the legs (left arm forward when the right leg leads) and peak one frame after the foot contacts; never in lockstep with the legs.",
                    Important,
                ),
                (
                    "A clear head and body up-down bob conveys weight; the belt line tilts toward the weight-bearing leg every step and is never level for long - no sliding feet.",
                    Important,
                ),
                (
                    "Feet travel on arcs and plant firmly on the ground line at contact; the antenna trails the head bob by 2-4 frames and overshoots at the top (overlap and follow-through).",
                    Important,
                ),
                (
                    "A light S-curve through the body, shoulders counter-tilting the hips; an asymmetric pose, never twinned left-right; silhouette readable at 32px with limbs clear of the torso.",
                    Important,
                ),
                (
                    "Sprite sheet: 8 frames in a left-to-right, top-to-bottom grid, identical cell size, identical character scale, identical lighting and camera per cell.",
                    Normal,
                ),
                ("crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background", Normal),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "floating or skating feet, a sliding walk",
                "a level belt line, flat hips across the cycle",
                "arms moving in lockstep with the legs",
                "twinned symmetric left-right poses",
                "straight-line, arc-less limb paths",
                "scale change between cells, character changing size between frames",
                "extra limbs, duplicate character, background",
                "motion blur, smear frames",
                "mouth, anti-aliasing, painterly gradients",
                "antenna stapled rigid to the head",
            ],
            animation_anchor: (
                Locked,
                "Walk is an 8-frame loop at 10 fps: two steps, four poses each (contact / down / passing / up), arms in opposition peaking one frame after foot contact, the belt line tilting toward the weight leg, the antenna lagging 2-4 frames. Body lowest at down, highest at passing.",
            ),
            scale_anchor: (
                Strong,
                "The bob, weight shift, and stride must read in the 32px silhouette; limbs held clear of the torso outline.",
            ),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default."),
        },
        AnimSpec {
            handle: "run",
            description: "Bit's run loop - a fast, leaning four-pose cycle with a longer stride and an airborne passing frame where both feet leave the ground. The high-energy sibling of the walk.",
            purpose: "A believable cartoon run that obeys the one rule separating a run from a fast walk: at least one frame with both feet off the ground at passing. Governing principles: timing (fewer frames, on ones), arcs, exaggeration (the forward lean), weight, follow-through (the antenna streams back). Minimal head bob - over-bobbing reads as jumping.",
            loop_behavior: LoopBehavior::Loop,
            frames: 4,
            fps: 12,
            beats: &[
                (
                    "Contact",
                    "Front foot lands, knee deeply bent absorbing, body low and pitched forward ~20-30 degrees (the run lean; exaggeration sells speed). Arms bent ~90 degrees in fists, in opposition, kept close to the body.",
                ),
                (
                    "Down / recoil",
                    "The knee absorbs and the push-off begins, body starting to rise, the rear leg loading. The stride is longer than the walk's.",
                ),
                (
                    "Passing - AIRBORNE (the money frame)",
                    "Both feet off the ground, body at its highest, briefly weightless; the front leg extending forward, the back leg trailing fully extended. This is the frame that makes it a run, not a walk. Antenna pinned back, streaming with momentum (~4-8 frame lag).",
                ),
                (
                    "Up / push-off",
                    "The back leg extends explosively, launching into the next stride, the front foot reaching forward. The head rises only a third-to-half a head height - minimal bob. Screen determined/focused, eyes fixed forward. The antenna whips on the push-off then trails.",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), running in a fast side-view cycle.",
                    Critical,
                ),
                (
                    "Four-pose run on ones - contact, down, airborne passing, push-off - with at least one frame where BOTH feet are off the ground at passing; this is what makes it a run, not a fast walk.",
                    Important,
                ),
                (
                    "A strong forward body lean (20-30 degrees) sells the speed; a longer stride than a walk; arms bent ~90 degrees in loose fists, a short swing, kept close, in opposition to the legs.",
                    Important,
                ),
                (
                    "Minimal head bob - the head rises only a third to half a head height; over-bobbing reads as jumping, not running.",
                    Important,
                ),
                (
                    "The antenna streams backward with momentum, pinned back through the airborne frame and whipping on push-off (overlap/follow-through, more lag than the walk); the blinking pixel trails the tip.",
                    Important,
                ),
                (
                    "An asymmetric leg pose, feet on arcs, a dynamic silhouette readable at 32px with limbs clear of the torso.",
                    Important,
                ),
                (
                    "Sprite sheet: 4 frames in a left-to-right grid, identical cell size, identical character scale, identical lighting and camera per cell.",
                    Normal,
                ),
                ("crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background", Normal),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "a both-feet-down run - the passing frame must be airborne",
                "an upright vertical body, a missing forward lean",
                "excessive head bob",
                "arms in lockstep with the legs, wide loose walk-arms",
                "twinned symmetric poses",
                "scale change between cells, character changing size between frames",
                "extra limbs, duplicate character, background",
                "motion blur, smear frames",
                "mouth, anti-aliasing, painterly gradients",
            ],
            animation_anchor: (
                Locked,
                "Run is a 4-frame loop at 12 fps on ones: contact, down, AIRBORNE passing (both feet off the ground), push-off. Forward lean 20-30 degrees, minimal head bob, arms bent ~90 degrees in opposition, the antenna streaming back.",
            ),
            scale_anchor: (Strong, "The airborne passing frame and the forward lean must be unambiguous at 32px."),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default."),
        },
        AnimSpec {
            handle: "jump",
            description: "Bit's standing jump - anticipation crouch, explosive stretch launch, weightless apex, heavy squash landing, and a settle. A full five-phase weight study, played once.",
            purpose: "Teach weight and force through the five jump phases, where the anticipation crouch depth telegraphs the jump size before it happens and the landing squash plus held compression sells the impact. Governing principles: anticipation, squash/stretch (volume-preserved), arcs (parabolic, symmetric about the apex), follow-through, the contact-before-squash weight trick, decay easing on the fall.",
            loop_behavior: LoopBehavior::Once,
            frames: 12,
            fps: 12,
            beats: &[
                (
                    "Anticipation crouch",
                    "Knees bend deep, the body drops and squashes wider (volume preserved), arms draw back and down, the head dips, the screen narrows to a loaded/focused glyph. Forward C-curve. The crouch depth telegraphs the jump height - the soul of the jump. The antenna whips down/back.",
                ),
                (
                    "Launch / push-off stretch",
                    "Legs extend explosively, the body stretches tall, arms swing up. A straight-diagonal line of action (power). Hold one foot in contact one extra frame while extending (the contact-before-squash trick) for push-off weight. The antenna snaps up and trails the stretch.",
                ),
                (
                    "Apex",
                    "Both feet off the ground, the body extended and reaching up, briefly weightless; the top of a parabolic arc symmetric about the apex. Screen bright/wide (excited). Hands lead. The antenna streams.",
                ),
                (
                    "Landing contact (squash)",
                    "Feet plant, one frame of contact with the body still extended BEFORE the knees squash (contact-before-squash makes it land heavy), then the knees absorb and the body squashes down and wider, arms forward for balance, head/screen bobs down, screen squints on impact. Decay easing - gravity accelerates the fall, slow-out only, no slow-in.",
                ),
                (
                    "Recover / settle",
                    "Hold the low landing pose a beat (chunky weight), then rise back to standing with follow-through; the antenna overshoots down past the body then oscillates up with decaying amplitude and settles. Skip the recovery and the jump feels glued.",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), performing a standing jump.",
                    Critical,
                ),
                (
                    "Five phases in order - anticipation crouch, launch stretch, weightless apex, landing squash, recover - each a distinct key pose; no phase skipped.",
                    Important,
                ),
                (
                    "Anticipation: a deep knee bend and body crouch (forward C-curve) before launch; the crouch depth telegraphs the jump height. No jump without the crouch.",
                    Important,
                ),
                (
                    "Squash and stretch with volume preserved - squash wider on the crouch and the landing, stretch taller on the launch and apex; the rigid head and limbs hold their shape, never gummy.",
                    Important,
                ),
                (
                    "The body follows a parabolic arc symmetric about the apex; the fall accelerates (slow-out only, decay easing); land heavy with one contact frame before the knees squash, then hold the low pose a beat.",
                    Important,
                ),
                (
                    "The antenna whips down on the crouch, snaps up trailing the launch, streams at the apex, overshoots down on impact and oscillates to a settle (overlap/follow-through); the screen reads loaded on the crouch, bright-wide at the apex, squint on impact.",
                    Important,
                ),
                (
                    "Feet are off the ground at the apex (airborne); a strong readable silhouette at 32px in every phase.",
                    Important,
                ),
                (
                    "Sprite sheet: 12 frames in a left-to-right, top-to-bottom grid, identical cell size, identical character scale, identical lighting and camera per cell.",
                    Normal,
                ),
                ("crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background", Normal),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "feet still grounded at the apex - Bit must be airborne at the top",
                "a missing anticipation crouch, a launch without a wind-up",
                "gummy or rubber-hose limbs, volume change on the head",
                "scale change between cells, character changing size between frames",
                "extra limbs, duplicate character, background",
                "motion blur, smear frames",
                "mouth, anti-aliasing, painterly gradients",
                "antenna stapled rigid to the head",
            ],
            animation_anchor: (
                Strong,
                "Jump plays once across 12 frames at 12 fps through five phases: crouch (telegraph), launch stretch, airborne apex, landing squash (contact-before-squash, a held low beat), recover. A parabolic arc symmetric about the apex; the fall on decay easing.",
            ),
            scale_anchor: (Strong, "The crouch depth, the airborne apex, and the landing squash must each read at 32px."),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default."),
        },
        AnimSpec {
            handle: "fall",
            description: "Bit losing balance and dropping - a tip past the point of no return, a flailing descent, a heavy impact, and an overshoot settle. Built from balance loss and counter-reaction, played once.",
            purpose: "Show an involuntary fall reading as weight surrendered to gravity, with the centre of gravity tipping past the foot base, counter-reaction staggering the parts, and a heavy impact-then-settle. Governing principles: balance/counterbalance, counter-reaction (body leads, head lags, antenna lags most), arcs, follow-through, decay easing (constant gravity, slow-out only, no slow-in), contact-before-squash on impact.",
            loop_behavior: LoopBehavior::Once,
            frames: 12,
            fps: 12,
            beats: &[
                (
                    "Off-balance tip",
                    "The centre-of-gravity line passes outside the foot base; the body leans past its support, one leg extends back in a last-moment balance attempt, arms windmill forward and out (not opposing the legs). The screen flashes to alarm - wide/dilated eyes. The CoG outside the base is what reads as about to fall.",
                ),
                (
                    "Free-fall descent",
                    "Arms out for balance, legs trailing, the body tipping along its line of action and accelerating (decay, slow-out only). Counter-reaction: the head/screen lags the body, the antenna lags most, trailing up and back. A forward/tumbling C-curve.",
                ),
                (
                    "Impact contact (squash)",
                    "The body hits, one contact frame still extended BEFORE the squash (contact-before-squash for weight), then the knees/body absorb and squash down, arms come forward. The screen squints on impact.",
                ),
                (
                    "Settle / recovery",
                    "Overshoot then settle; the antenna whips forward/down past the body (overshoot 20-30%), swings back, oscillates twice with diminishing amplitude before rest. The screen does a small recovery blink back toward neutral. A stop is an event: decelerate, overshoot, settle - never a hard freeze.",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), losing balance and falling.",
                    Critical,
                ),
                (
                    "Sequence - off-balance tip, free-fall descent, heavy impact, settle. The tip reads because the centre of gravity leans past the feet; one leg kicks back, arms windmill out for balance.",
                    Important,
                ),
                (
                    "The descent accelerates under gravity (decay easing, slow-out only, no slow-in); counter-reaction staggers the parts - the body leads, the head/screen lags, the antenna lags the most, trailing back.",
                    Important,
                ),
                (
                    "Land heavy: one contact frame with the body still extended before it squashes, then absorb and squash down (volume preserved); arms forward.",
                    Important,
                ),
                (
                    "The settle is an event, not a freeze - overshoot then settle; the antenna whips past the body 20-30% and oscillates down to rest; the screen flashes alarm (wide eyes) on the tip and descent, squints on impact, blinks on recovery.",
                    Important,
                ),
                (
                    "A forward/tumbling C-curve line of action; a silhouette readable at 32px through the tip and the impact.",
                    Important,
                ),
                (
                    "Sprite sheet: 12 frames in a left-to-right, top-to-bottom grid, identical cell size, identical character scale, identical lighting and camera per cell.",
                    Normal,
                ),
                ("crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background", Normal),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "a balanced upright pose during the fall - the centre of gravity must be past the feet",
                "a constant-speed descent - the fall accelerates",
                "a hard frozen stop on landing - it overshoots and settles",
                "arms opposing the legs like a walk - arms windmill out",
                "scale change between cells, character changing size between frames",
                "extra limbs, duplicate character, background",
                "motion blur, smear frames",
                "mouth, anti-aliasing, painterly gradients",
                "antenna stapled rigid to the head",
            ],
            animation_anchor: (
                Strong,
                "Fall plays once across 12 frames at 12 fps: off-balance tip (CoG past the feet), accelerating descent (decay easing), heavy impact (contact-before-squash), overshoot settle. Counter-reaction staggers body, head, antenna; the antenna overshoots and oscillates to rest.",
            ),
            scale_anchor: (Strong, "The tip past balance and the heavy impact must read at 32px."),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default."),
        },
        AnimSpec {
            handle: "attack",
            description: "Bit's melee swing - a coiled wind-up away from the target, a one-to-two-frame strike, a recoil bounce where the hit lands for the viewer, and a long settle. A hard accent wrapped in anticipation-action-reaction, played once.",
            purpose: "Land a decisive melee strike as a hard accent inside the AAR formula, where the audience feels the hit on the recoil/bounce, not at contact. Governing principles: anticipation (opposite direction, ~2x the action), the hard-accent bounce-back, snap action (no slow-in, 1-2 frames), arcs, follow-through, a held settle (6+ frames at speed) so the accent reads, exaggeration. Pairs with hurt as the impact/reaction family.",
            loop_behavior: LoopBehavior::Once,
            frames: 8,
            fps: 12,
            beats: &[
                (
                    "Ready / neutral",
                    "Settled, weight centred, the screen showing a focused glyph (small constricted pupils = focus/aggression).",
                ),
                (
                    "Wind-up anticipation (the longest beat)",
                    "The body coils AWAY from the strike target, the striking limb drawn back, weight shifts to the back foot, the screen narrows to a determined squint. A backward C-curve. The antic runs ~2x the action time - go back before you go forward.",
                ),
                (
                    "Strike / contact (1-2 frames, fast)",
                    "The limb drives through along a straight-diagonal line of action (power); shown having passed through the target, not dwelling on contact. A single elongated streak on this frame reads as speed. No slow-in - snap from the held wind-up.",
                ),
                (
                    "Overshoot / bounce-back",
                    "The limb passes max extension then recoils slightly toward the body; this is where the hit lands for the viewer (a hard accent - the snap is the bounce). Any impact spark sits one frame after contact, on the bounce. The antenna whips hardest here.",
                ),
                (
                    "Settle (held, long)",
                    "The body returns to balance over several frames; the screen relaxes from squint back to neutral/satisfied (switching on the settle frame, never a crossfade); the antenna does one last overshoot-and-settle. A hard accent must hold or the hit feels unimportant.",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), performing a decisive melee attack swing.",
                    Critical,
                ),
                (
                    "Anticipation-action-reaction: a long wind-up coiling AWAY from the target (about twice the strike's length), a fast 1-2 frame strike, then a recoil bounce - the hit reads on the bounce, not at contact.",
                    Important,
                ),
                (
                    "The wind-up is a backward C-curve with weight on the back foot; the strike drives through on a straight-diagonal line of action and is shown having passed through the target, never dwelling on the contact frame.",
                    Important,
                ),
                (
                    "Snap the strike with no slow-in (held wind-up, then sudden release); a single elongated streak on the strike frame reads as speed; the limb overshoots past full extension then recoils toward the body.",
                    Important,
                ),
                (
                    "The antenna whips hardest on the strike-and-bounce and settles last; any impact spark appears one frame after contact, on the recoil (overlap/follow-through).",
                    Important,
                ),
                (
                    "The screen reads focused (small pupils) through the wind-up and switches crisply to neutral/satisfied on the held settle frame - never a blur or morph; the settle is held long enough that the hit feels important.",
                    Important,
                ),
                (
                    "A strong silhouette at 32px in the wind-up and the strike, limbs clear of the torso.",
                    Important,
                ),
                (
                    "Sprite sheet: 8 frames in a left-to-right, top-to-bottom grid, identical cell size, identical character scale, identical lighting and camera per cell.",
                    Normal,
                ),
                ("crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background", Normal),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "a strike without a wind-up - a teleporting attack",
                "dwelling on the contact frame, ending on the strike with no settle",
                "slow-in on the strike - it snaps",
                "scale change between cells, character changing size between frames",
                "extra limbs, duplicate character, background",
                "motion blur as a soft smear across cells (a single crisp streak frame only), painterly gradients",
                "mouth, anti-aliasing",
                "antenna stapled rigid to the head",
            ],
            animation_anchor: (
                Strong,
                "Attack plays once across 8 frames at 12 fps as a hard accent in AAR: a long wind-up away from the target (~2x the action), a 1-2 frame snap strike on a straight diagonal, a recoil bounce where the hit lands, a held settle. The screen switches focused to neutral on the settle frame.",
            ),
            scale_anchor: (Strong, "The wind-up coil and the strike extension must read at 32px."),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default."),
        },
        AnimSpec {
            handle: "hurt",
            description: "Bit taking a hit - a brief compress into the blow, a sharp recoil away, an overshoot past a believable position, and a decaying settle back toward neutral. A take driven by contrast, played once. The reaction sibling of the attack.",
            purpose: "Sell an impact reaction as a take built on contrast - the recoil reads hardest springing from a content/neutral state, following the take's universal DOWN-then-UP pattern. Governing principles: anticipation (the down-compress), takes/accents (a 1-2 frame snap), follow-through/overshoot, contrast (start opposite the destination), the moving-hold settle, a restrained volume-preserving squash (a chunky robot, not gummy). Pairs with attack as the impact/reaction family.",
            loop_behavior: LoopBehavior::Once,
            frames: 8,
            fps: 12,
            beats: &[
                (
                    "Pre-hit contrast pose",
                    "Caught content/neutral, the screen soft and level. The wider the swing from fine to hurt, the harder the recoil lands (look for contrasts).",
                ),
                (
                    "Down anticipation (compress)",
                    "The body crouches and compresses INTO the hit direction for a beat before reacting; the take is DOWN then UP. A restrained squash, volume preserved (a chunky robot - widen what flattens, do not shrink or grow him).",
                ),
                (
                    "Accent / impact (1-2 frames)",
                    "A sharp recoil AWAY from the hit: the head and body snap back, the screen flashes to an alarm glyph (wide/dilated eyes, or a jagged glyph). A backward C-curve. A single smear or doubled silhouette sells the violence. The hardest single frame.",
                ),
                (
                    "Overshoot",
                    "The body travels PAST a believable recoil position; the antenna whips back hardest of all (whip action, a one-frame crack to an impossibly-far lean).",
                ),
                (
                    "Settle back (held, decaying)",
                    "The body springs back through balance with a small decaying oscillation; the screen switches (never a crossfade) from alarm back toward neutral/dazed on the settle frame; the antenna does one last overshoot-and-settle. A moving hold under the settle keeps it alive.",
                ),
            ],
            fragments: &[
                (
                    "Bit, the small chunky retro robot mascot (boxy CRT head, glowing pixel-face screen, one stubby antenna with a blinking pixel tip, rounded biped limbs, ~2 heads tall), recoiling from taking a hit.",
                    Critical,
                ),
                (
                    "A take built on contrast: start in a content/neutral pose, compress DOWN into the hit for a beat, then snap UP and back in a sharp 1-2 frame recoil - the down-then-up pattern.",
                    Important,
                ),
                (
                    "The recoil is a backward C-curve away from the hit; the body overshoots past a believable position before springing back; a single smear or doubled silhouette on the accent frame sells the violence.",
                    Important,
                ),
                (
                    "The squash is restrained and volume-preserved - widen what flattens, never shrink or grow Bit; the chunky rigid body is not gummy.",
                    Important,
                ),
                (
                    "The antenna whips back hardest of all on the accent (a one-frame crack to an extreme lean) and settles last; the screen flashes an alarm glyph (wide/dilated eyes) on impact and switches crisply back toward neutral/dazed on the held settle frame - never a morph.",
                    Important,
                ),
                (
                    "The settle springs back with a small decaying oscillation and a moving hold underneath (no dead freeze); a strong silhouette at 32px through the recoil.",
                    Important,
                ),
                (
                    "Sprite sheet: 8 frames in a left-to-right, top-to-bottom grid, identical cell size, identical character scale, identical lighting and camera per cell.",
                    Normal,
                ),
                ("crisp 8-bit pixel art, @style.pixel_art, @palette.bit_default, flat key background", Normal),
                (QUALITY_POLISH, Normal),
            ],
            negatives: &[
                "a recoil without the down-compress beat - a flat snap from neutral",
                "scale change between cells - Bit does not shrink or grow when squashed",
                "gummy or rubber-hose deformation on the rigid body",
                "a morphing or crossfading screen expression - it switches crisply",
                "a hard frozen settle - it oscillates and decays under a moving hold",
                "extra limbs, duplicate character, background",
                "motion blur as a soft smear across cells (a single crisp streak frame only), painterly gradients",
                "mouth, anti-aliasing",
                "antenna stapled rigid to the head",
            ],
            animation_anchor: (
                Strong,
                "Hurt plays once across 8 frames at 12 fps as a take built on contrast: a content pose, a down-compress into the hit, a 1-2 frame snap recoil (backward C), an overshoot, a decaying settle under a moving hold. The screen switches content to alarm to neutral on key frames, never morphs.",
            ),
            scale_anchor: (Strong, "The compress and the recoil overshoot must read at 32px without scale drift."),
            style_anchor: (Strong, "@style.pixel_art on @palette.bit_default."),
        },
    ];

    for spec in specs {
        let entry = id(handles, spec.handle)?;
        update(doc, entry, delta(spec.description, "", "", &["animation", "bit", spec.handle]))?;
        let body = AnimationDetails {
            purpose: spec.purpose.to_owned(),
            loop_behavior: spec.loop_behavior,
            recommended_frame_count: spec.frames,
            fps: spec.fps,
            pose_beats: spec
                .beats
                .iter()
                .map(|(label, description)| PoseBeat {
                    label: (*label).to_owned(),
                    description: (*description).to_owned(),
                })
                .collect(),
            character_compatibility: vec![CodexHandle::new("bit")?],
        };
        let mut details = SetAnimationDetails::new(entry, body);
        details.apply(doc)?;
        let (anim_strength, anim_statement) = spec.animation_anchor;
        anchor(doc, entry, AnchorKind::Animation, anim_strength, anim_statement)?;
        let (scale_strength, scale_statement) = spec.scale_anchor;
        anchor(doc, entry, AnchorKind::Scale, scale_strength, scale_statement)?;
        let (style_strength, style_statement) = spec.style_anchor;
        anchor(doc, entry, AnchorKind::Style, style_strength, style_statement)?;
        let frags: Vec<PromptFragment> = spec.fragments.iter().map(|(text, priority)| frag(text, *priority)).collect();
        fragments(doc, entry, frags)?;
        negatives(doc, entry, spec.negatives)?;
        status(doc, entry, EntryStatus::Canonical)?;
    }
    Ok(())
}
