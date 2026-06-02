# Ones vs Twos — How Many Drawings Per Second?

A foundational the classical tradition debate: should you draw on "ones" (a new drawing every frame, 24 per second) or on "twos" (one new drawing every two frames, 12 per second, each held for 2 frames)?

## The math

At 24 frames per second (standard film):
- **On ones:** 24 new drawings per second of screen time
- **On twos:** 12 new drawings per second of screen time (each shown for 2 frames)

## When to use ones

- **Fast action** — runs, sword swings, fast pans, anything with rapid spacing
- **Camera moves** — pans, trucks, anything where the background scrolls
- **Smooth fluids** — water, smoke detail
- **High-end feature work** — Disney features traditionally use a lot of ones for hero shots
- **Whenever spacing between frames exceeds about 1/3 of the character's body width** — at that point twos start to look jittery (strobe)

**** *"Runs should always be on ones."* Because the spacing per frame is so wide in a run that twos create visual stuttering.

## When to use twos

- **Most character acting** — dialogue, normal walks, gestures
- **Holds** — when the character is essentially still
- **Most commercial / TV animation** — saves 50% of drawings
- **Looney Tunes / Disney TV / Anime** — overwhelmingly on twos

**Why twos look fine for acting:** spacing is small enough that the strobe effect doesn't trigger. The eye accepts the 2-frame hold as continuous motion.

## When to use threes or fours (limited animation)

- **Hanna-Barbera TV cartoons** — on threes
- **Some anime** — varies between twos, threes, and even fours
- **Style choice** — gives a deliberate "limited" feel
- **Holds and slow drift** — perfectly fine on threes

## The hybrid approach (the standard professional method)

**Most animation is mixed.** A typical pro scene:
- Dialogue and acting on twos
- Fast head turn on ones for the windup, then twos
- Walk cycle on twos
- A run cycle on ones
- Holds on fours

This is called "shooting on different exposures" — the X-sheet shows which frames are single, double, or held.

## How this affects AI prompting

### Video models
Frame rate and "smoothness" are often controllable. To get a "twos" feel:
> "Animation style: classic 2D cartoon, drawn on twos, 12 unique drawings per second at 24fps. Slight pop between frames typical of hand-drawn animation."

To get a "ones" feel:
> "Animation style: smooth Disney feature animation, drawn on ones at 24fps, no strobing or popping."

To get anime feel:
> "Anime style with mixed exposures: holds on threes and fours, sudden action moments on ones for impact frames, then back to twos."

### Image keyframes
Decide your target output frame rate. If targeting 12fps "on twos" animation:
- 1 second = 12 keyframes
- 2 seconds = 24 keyframes
- Generate at this density, then duplicate frames or use simple frame-doubling to hit 24fps playback.

### Code
Pure CSS/GSAP runs at the browser's refresh rate (60fps or 120fps), which is effectively "on ones+." To simulate "on twos":
```js
gsap.ticker.fps(12); // force 12fps update — looks "drawn on twos"
```
Or use `steps()` easing in CSS:
```css
animation-timing-function: steps(12, end); /* 12 discrete steps per second */
```

## The strobing problem

When spacing per frame exceeds a certain threshold relative to character size, on-twos animation visibly stutters. The eye sees "two distinct positions" instead of continuous motion.

**** switch to ones for those frames. Don't worry about consistency across the shot — mix it.

**The threshold (rule of thumb):** if a body part moves more than its own diameter between frames, you need ones.

## The art-house argument

The "stutter" becomes a feature, not a bug. Stop-motion is essentially "on twos" or "on threes" by default for the same reason — it's part of the texture.

## Quick decision tree

```
Is the action FAST (run, swing, impact)?
 → ONES
Is the action a HOLD?
 → THREES or FOURS
Is the action normal acting / walking / dialogue?
 → TWOS
Is this anime?
 → TWOS by default, ONES on impact, THREES/FOURS on holds
Is this a fluid / smoke / water / fire?
 → ONES (and usually straight-ahead)
```

## Linked concepts

- [[time-and-space]]
- [[run-cycle-fundamentals]]
- [[frame-counts-by-feel]]
