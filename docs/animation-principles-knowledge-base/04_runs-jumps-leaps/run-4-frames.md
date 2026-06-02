# The 4-Drawing Run ' Formula

The most efficient cartoon run. Four unique drawings, looped, animated on ones. Six steps per second.

## The four poses (memorize these)

Each step in a 4-frame run is a single drawing:

| Frame | Pose | Description |
|-------|------|-------------|
| 1 | **Contact (down-ish)** | Front foot lands, knee bent, body low |
| 2 | **Recoil/Down → start of up** | Push-off begins, body starts rising |
| 3 | **Passing (airborne)** | Both feet off ground, body highest |
| 4 | **Pre-contact** | Front leg extending for next landing |

Then the cycle repeats with the OTHER leg leading.

## What the eye sees

At 6 steps per second (4 frames per step on ones), the eye reads the pattern as:
- **Bounce-bounce-bounce-bounce** with smooth fluidity
- The "blur" of motion fills in the gaps between drawings
- Body shape changes radically between frames — that's correct

## The the classical tradition trick: don't worry about exact symmetry

Change it slightly — make it a little higher or lower. The eye doesn't catch the asymmetry, and the asymmetry adds life.

```
Step 1: drawing 1, 2, 3, 4
Step 2: drawing 5, 6, 7, 8

Drawing 5 should NOT be identical to drawing 1.
Make it slightly different — maybe lower or higher.
```

This is what separates a "cycle" (mechanical) from "alive animation."

## When to use 4-frame runs

- **Cartoon-style runs** (Looney Tunes, classic Disney)
- **Fast hero motion**
- **Action scenes**
- **Chases**

## When NOT to use 4-frame runs

- **Realistic human running** — 6 frames is more believable
- **Very long character legs** — 6 frames gives them room to extend properly
- **Specific timing needs** — 4 frames is locked at ~6 steps/sec

## Prompt-ready language

### Video model
> "Cartoon-style 4-frame run cycle at 24fps, animated on ones (6 steps per second). Four distinct poses per step: contact-low, push-off, airborne-passing, pre-contact-extension. Body leans forward dramatically. Arms swing in tight opposition. Hair and clothes flow behind. Motion is rhythmic and energetic."

### Code (GSAP — 4-frame run timing)
```javascript
const FPS = 24;
const FRAMES_PER_STEP = 4;
const STEP_DURATION = FRAMES_PER_STEP / FPS; // ~0.167s

const runCycle = gsap.timeline({ repeat: -1 });

// One full step (one foot leading)
runCycle
 .to(body, { y: 6, duration: STEP_DURATION * 0.25, ease: "power2.in" }) // contact-down
 .to(body, { y: -2, duration: STEP_DURATION * 0.25, ease: "power2.out" }) // push-off
 .to(body, { y: -10, duration: STEP_DURATION * 0.25, ease: "power2.out" }) // airborne
 .to(body, { y: -2, duration: STEP_DURATION * 0.25, ease: "power2.in" }); // pre-contact

// (then repeat with mirrored leg positions)
```

## Linked concepts

- [[run-cycle-fundamentals]]
- [[run-3-frames]]
- [[ones-vs-twos]]
