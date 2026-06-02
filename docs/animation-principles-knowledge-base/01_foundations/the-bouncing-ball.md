# The Bouncing Ball — The Most Important Exercise in Animation

## Frame-by-frame breakdown of a single bounce

Assume 24 fps. One bounce from peak to next peak takes roughly 12 frames.

| Frame | Position | Notes |
|-------|----------|-------|
| 1 | Peak of arc | Hold this slowly — closely-spaced frames around the peak |
| 2 | Slightly below peak | Still slow, close spacing |
| 3-5 | Falling | Spacing grows rapidly — ball accelerating |
| 6 | Just before impact | Maximum stretch (vertical) — anticipation of squash |
| 7 | Impact frame | Squash — compressed against ground |
| 8 | Leaving ground | Stretch — opposite direction |
| 9-10 | Rising fast | Wide spacing — still moving fast |
| 11 | Approaching new peak | Spacing closes — decelerating |
| 12 | New peak | Slow, close spacing again |

**The pattern:** dense frames at the slow parts (top of arc), sparse frames at the fast parts (near impact).

## What every variable changes

### Change the material (visual style of the ball)
- **Rubber ball:** strong squash/stretch, bounces high relative to drop
- **Bowling ball:** minimal squash, low bounce, deep impact dent in floor
- **Beach ball:** light, floats, wide arcs, slow time
- **Golf ball:** rigid, no squash, fast spin, predictable arcs
- **Egg:** squash on impact + crack at peak compression (irreversible)

### Change the gravity / environment
- **Earth (1g):** standard timing
- **Moon (1/6 g):** stretch all arcs ~2.5x longer in time, less squash
- **Underwater:** ease everything, exponential decay, no clean impact
- **Heavy molasses:** linear deceleration, no bounce-back

### Change the personality
The ball can act like a character with no face:

- **Eager:** ball overshoots arcs, bounces forward enthusiastically
- **Tired:** weak bounces, low arcs, settles quickly
- **Cautious:** small first bounce, larger second bounce after "checking"
- **Drunk:** uneven horizontal spacing, arcs of inconsistent height
- **Sneaky:** held pauses at peaks, tiny anticipations before drops

## The "personality without drawing" insight

No face needed. This is why timing is the soul of animation, not the drawing.

## Prompt-ready language

**Use these phrases verbatim in video-model prompts:**

- "ball decelerates approaching the apex of each arc"
- "ball accelerates between apex and impact"
- "ball compresses on impact, then springs back to original shape"
- "each successive bounce is lower than the previous, with energy dissipating"
- "ball pauses briefly at the top of each arc"
- "spacing of motion-blurred frames is wide at impact, narrow at apex"

**For image-keyframe generation:**

- Generate 3 extremes: peak (hold), impact (squash), next peak (smaller)
- Optional stretch frames at frame 6 (falling) and frame 8 (rising)

**For code:**

```css
@keyframes bounce {
 0% { transform: translateY(0) scaleY(1); } /* peak */
 45% { transform: translateY(300px) scaleY(1.2); } /* stretch falling */
 50% { transform: translateY(310px) scaleY(0.7); } /* squash */
 55% { transform: translateY(300px) scaleY(1.2); } /* stretch rising */
 100% { transform: translateY(50px) scaleY(1); } /* lower peak */
}
animation-timing-function: cubic-bezier(0.4, 0, 0.7, 1);
```

## Why this exercise unlocks everything

If you can animate a believable bouncing ball with personality, you understand:
- Timing (impact frames)
- Spacing (acceleration through arc)
- Squash and stretch (optional, situational)
- Anticipation (stretch before impact)
- Follow-through (stretch after impact)
- Arcs (no straight lines)
- Slow-in / slow-out (at peaks)
- Weight (bounce height ratio)

Every walk, run, dialogue performance, and action shot is a generalization of these same ideas.

## Linked concepts

- [[time-and-space]]
- [[slow-in-slow-out]]
- [[squash-and-stretch]]
- [[arcs]]
