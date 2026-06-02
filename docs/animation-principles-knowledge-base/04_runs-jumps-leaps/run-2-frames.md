# The 2-Drawing Run — The Impossible Cartoon Run

Two unique frames repeated. Twelve "steps" per second. **Anatomically impossible. Visually convincing.**

This is one of s — the cartoon run that breaks physics but works because of how the eye fills in motion.

## How it works

Just two drawings. They alternate.

| Frame | Pose |
|-------|------|
| 1 | **Contact-right-foot** with body in mid-flight position |
| 2 | **Contact-left-foot** with body in mirrored mid-flight position |

The PASSING pose (both feet airborne) is **omitted**. The eye sees two "contact" frames and infers the missing passing pose between them.

## Why this works (the explanation)

> The passing pose is omitted (implied) by the second closing drawing at each end. And the eye jumps to the space where the passing pose would normally be.
The brain fills in the missing middle frame. The result reads as 12 steps per second — impossible for a human, but perfect for a roadrunner-style cartoon dash.

## Variations on the 2-frame run

### Variation 1: Vary the silhouettes
Make each of the two drawings a *significantly different* silhouette. The eye reads them as a contrast — like a flicker between two states. Used for chaotic action.

### Variation 2: Wheel the arms
Both arms spin wildly around the body in circles. Use a single drawing of the spinning arms and just shift the body position. The arms blur into circles.

### Variation 3: Hide the legs in motion blur
Replace the legs with a wheel-like blur (a circle or smear). Just animate the body position. The audience accepts the legs are "moving too fast to see."

### Variation 4: 2 frames with blur cross-fade
Animate the legs on twos, but each frame is a 50/50 blend of two consecutive poses (motion blur). Result:
- Frame 1: focused pose A
- Frame 2: 50% A + 50% B (blurred)
- Frame 3: focused pose B
- Frame 4: 50% B + 50% A (blurred)

This is "double exposure blurring" — a real cinematic technique adapted to animation.

## When to use a 2-frame run

- **Impossible speed** moments (Road Runner-style)
- **Final escape** burst
- **Comedy "instant flee"**
- **Camera tracking shots** where the character runs frantically

## When NOT to use

- Realistic character work
- Anything requiring readable acting
- Closeups (looks broken at close range)

## Prompt-ready language

### Video model
> "Impossible cartoon-fast run, 2 unique poses per cycle, 12 steps per second. Legs blur into wheel-like circular motion. Arms wheel around body. Hair and clothes stretched horizontally behind character. Background streaks with speed lines. Body leans almost horizontally."

### Code
The 2-frame run is hard to fake with smooth easing — it relies on the visual flicker. Best done with stepped animation:

```javascript
gsap.to(legs, {
 rotation: -360,
 duration: 1/6, // one full cycle per 1/6 second
 ease: "none",
 repeat: -1,
 // legs should be rendered as a blur sprite, not an articulated leg
});
```

Or use CSS steps:
```css
.legs-blur {
 animation: legSpin 0.167s steps(2) infinite;
}
@keyframes legSpin {
 0% { /* pose A */ }
 50% { /* pose B */ }
 100% { /* pose A */ }
}
```

## The "circular motion" trick

For the most extreme 2-frame run, replace the legs entirely with a circular blur sprite:

```
[character torso + arms] over [spinning circle where legs should be]
```

This is the Road Runner / Wile E. Coyote standard. The legs become an abstraction.

## Linked concepts

- [[run-3-frames]]
- [[run-4-frames]]
- [[run-cycle-fundamentals]]
