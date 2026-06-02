# Soft Accent — The Accent That Continues

A soft accent is the opposite of a hard accent. After hitting its emphasis point, the motion **continues smoothly** rather than bouncing back. Like a conductor leading a waltz.

## What it looks like

```
[Speed in] → [PEAK ACCENT] → [CONTINUES SMOOTHLY]
```

The arm reaches its emphasis point and then keeps flowing through to the next position. The energy doesn't return; it propagates onward.

## the analogy

> A soft accent will stay
Imagine a conductor's hand smoothly moving up-down-up-down for three-quarter time. Each beat has a clear accent, but the hand never bounces. It flows continuously through each emphasis.

## When to use soft accents

- **Gentle pointing** ("It's over there.")
- **Polite gestures**
- **Conducting / musical timing**
- **Friendly speech accents** ("Hello there...")
- **Wistful, contemplative moments**
- **Smooth dance moves**
- **Caressing motion**
- **"Would you like..." type questions**
- **Tired, slow speech**

## The "where the accent is felt" — soft version

In a soft accent, the audience feels the emphasis as a **brief peak in the motion**, not as a snap. The motion accelerates into the peak, then decelerates as it continues.

```
Frame 1: arm accelerating toward peak
Frame 2: arm peaking (THIS is where the audience feels the soft accent)
Frame 3: arm beginning to slow as it passes peak
Frame 4: arm continuing slowly to next position
```

No recoil. No bounce. Just smooth flow with a moment of emphasis.

## Soft accent in pointing

Compare with hard accent:

**Hard accent point:**
```
Arm swings up → reaches target → BOUNCES BACK → settles
```

**Soft accent point:**
```
Arm swings up → reaches target → CONTINUES PAST → flows back to neutral
```

The soft point doesn't lock at the target. It indicates and then continues. Used for casual or polite pointing.

## Specific examples

### "I'd be happy to..." (soft accent)
Gentle hand gesture flowing forward, no bounce, continues into next motion.

### "Hello there, cutie..." (could be either)
Hand wave that flows side-to-side smoothly. Soft accent.

### Wistful pointing
"The sunset is over there..." Hand floats up, gestures toward sunset, drifts back down.

### Conducting time
Each beat has a clear accent in the motion but no bounce. The hand keeps flowing in 3/4 or 4/4 time.

## Timing of soft accents

| Component | Frames (at 24fps) |
|-----------|-------------------|
| Anticipation | 6-12 (longer, gentler) |
| Build to peak | 4-8 |
| Brief peak | 1-2 |
| Continuation | 8-12 (slows but continues) |

Soft accents are generally **longer overall** than hard accents — they're meant to feel relaxed.

## Hard vs. Soft accent comparison

| Aspect | Hard accent | Soft accent |
|--------|-------------|-------------|
| Energy | Returns (bounce) | Continues (flow) |
| Feel | Decisive, sharp | Relaxed, fluid |
| Timing | Fast peak | Slow peak |
| End | Recoil + held | Continued motion |
| Examples | "NO!", punch | "Yes...", caress |
| Easing | `back.out(2.5)` | `power2.inOut` |

## Prompt-ready language

### Video model — soft accent gesture
> "Character gestures softly with their hand to indicate the picture on the wall. The hand swings up gently (8 frames), peaks briefly at the gesture position (1 frame), then continues flowing past as the arm slowly returns to rest position (10 frames). No bounce — the gesture flows smoothly throughout."

### Video model — soft accent in dialogue
> "Character says 'Welcome home...' with soft accents. As each word is emphasized, the head tilts slightly into the emphasis, then continues smoothly through the motion — no head snap or bounce. Body language is relaxed and fluid throughout."

### Code — soft accent
```javascript
const softAccentGesture = gsap.timeline();
softAccentGesture
 // Antic: slow gentle prep
 .to(arm, { rotation: -10, duration: 0.4, ease: "power2.out" })
 // Build to peak (smooth flow, no bounce)
 .to(arm, { rotation: 50, duration: 0.4, ease: "power2.inOut" })
 // Continue smoothly past peak — no recoil
 .to(arm, { rotation: 30, duration: 0.5, ease: "power2.inOut" });
```

Or simpler — sine ease:
```javascript
gsap.to(arm, { 
 rotation: 50, 
 duration: 0.8, 
 ease: "sine.inOut", // smooth in and out
 yoyo: true,
 repeat: 1 // returns smoothly after reaching peak
});
```

## Combining hard and soft accents

A character's speech rhythm is built from a mix:

```
"Hello there! [SOFT] How are you today? [SOFT] 
Oh no! [HARD] That's terrible! [HARD] 
Oh, but I see... [SOFT] you'll be fine. [SOFT]"
```

The accents follow the emotional content. Hard for emphatic/declarative, soft for casual/contemplative.

A great dialogue performance has variety — mostly soft, with hard accents on key beats. All-hard reads as aggressive. All-soft reads as lifeless.

## Linked concepts

- [[hard-accent-bounces]]
- [[double-takes]]
- [[dialogue-accents]]
- [[the-AAR-formula]]
