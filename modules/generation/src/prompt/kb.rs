//! Curated prompt-ready language distilled from the animation knowledge base.
//!
//! These are DATA (prompt content), not i18n keys. Each const cites the knowledge
//! base file(s) it is distilled from (in a plain comment, so the file paths do not
//! trip the doc linter), so a revision there has an obvious code counterpart. The
//! knowledge base itself is non-runtime reference (see the docs tree and
//! `docs/CLAUDE.md`); we bake the distilled lines as string consts rather than read
//! the markdown at runtime, so they are zero-cost, testable, and ship with the binary.

/// The breathing-loop description for an idle animation.
// distilled from: 01_foundations (slow-in/slow-out at the extremes) and
// 13_twelve-principles (#6 slow in and out, #9 timing).
pub const BREATHING_OSCILLATION: &str = "The body breathes in a slow, even cycle - the chest rises and falls and the motion eases at the top and bottom of each breath, with one full breath spread across the whole loop. The knees flex slightly with the breath and the feet stay planted.";

/// The secondary-motion / overlap description for an idle animation.
// distilled from: 06_flexibility-overlap-follow-through (the lag table) and
// 09_wiggles-waves-whips (the gentle sway formula).
pub const SECONDARY_OVERLAP: &str = "Loose and attached parts lag a few frames behind the body and settle a beat after it stops - a stubby antenna and small parts drift with a short delay, overshoot slightly, then ease back to rest. The blinking pixel on the antenna pulses gently with the loop.";

/// A clean, readable game-sprite style line.
// distilled from: 16_style-presets (a curated game-sprite line, not a film preset).
pub const GAME_SPRITE_STYLE: &str =
    "A clean, readable game-sprite look with a strong silhouette, flat solid colour, and crisp edges; consistent lighting across every frame.";

/// A short motion-feel note that pairs with the style line.
// distilled from: 16_style-presets (the frame-rate-feel notes).
pub const GAME_SPRITE_FEEL: &str = "Smooth, even motion so the loop reads continuously.";

/// The "what must not happen" line that guards drift, duplicates, and clutter.
// distilled from: 14_staging-silhouette (read the silhouette) and
// 15_prompt-templates (state what should not happen).
pub const IDLE_NEGATIVES: &str = "No drift, no scale change between cells, no extra limbs or duplicate characters, no background detail, no motion blur.";
