# Hard Accent — The Accent That Bounces Back

A hard accent is a sharp, decisive movement that **bounces back** after hitting its target. The energy doesn't dissipate smoothly — it rebounds.

## What it looks like

```
[Speed in] → [STRIKE] → [BOUNCE BACK] → [SETTLE]
```

A finger snapping forward to point emphatically. The pointing arm reaches its target, then **recoils back slightly**, like a steel hammer hitting an anvil.

## the analogy

> If we hit an anvil with a steel hammer, obviously the anvil won't be affected by the hammer and the hammer will bounce back.
The hard accent says: "this action was decisive, sharp, and the world doesn't give." The energy returns to the character.

## When to use hard accents

- **Pointing emphatically** ("THERE!", "YOU!", "NO!")
- **Punches with full commitment**
- **Karate chops, kicks**
- **Slamming a door**
- **Stomping foot**
- **Declarative speech accents** ("Of course!")
- **Decisive head turns** (snap, then hold)
- **Sharp body accents** ("AHA!", surprise reveal)

## Timing of a hard accent

the classical tradition discovered: **6 frames minimum** to mark any accent clearly.

Tex Avery used 5 frames in his fast cartoons, but | Component | Frames (at 24fps) |
|-----------|-------------------|
| Anticipation | 4-8 (slow buildup) |
| Strike (action) | 2-3 (very fast) |
| Bounce back | 2-3 (overshoot opposite direction) |
| Settle / held position | 6+ (to mark the accent) |

## The "where the accent is felt"

the audience doesn't feel the accent at the moment of contact. They feel it **at the recoil**.

```
Frame 1: arm speeding forward (no accent felt yet)
Frame 2: arm at max extension (still no accent felt)
Frame 3: arm bouncing back (THIS is where the audience feels the accent)
Frame 4: arm settling
```

The "snap" of the accent is the bounce. Without the bounce, the accent feels soft and unimportant.

## Hard accent in pointing

A character pointing emphatically:

```
Frame 1-6: arm swinging up toward point direction (antic + action build)
Frame 7: arm at full extension toward target
Frame 8: arm RECOILS slightly toward body (hard bounce)
Frame 9-10: arm settles back to extended position
Frame 11+: held pointing pose
```

The frame 8 recoil is what makes it "hard."

## Hard accent in punching

> If we hit a nail with a hammer, the accent is NOT at the moment the hammer contacts the nail. The sound is on the bounce
The contact frame itself is invisible/instantaneous. The audience feels the punch when the hammer rebounds.

This is why audio sync for hard accents is offset:
- **Visual contact frame:** frame N
- **Audio impact sound:** frame N+1 (the sound is on the bounce)

## Specific examples

### "OF COURSE!" — emphatic agreement
Hard accent. Body and head SNAP forward into pose. Brief bounce back. Held with conviction.

### "NO!" — emphatic refusal
Hard accent. Head SNAPS to one side with sharp emphasis. Bounces back. Held.

### Pointing at the camera
Hard accent. Arm WHIPS forward. Bounces back. Held.

### Foot stomping
Hard accent. Foot drives down. Body bounces UP off the impact (recoil). Settles.

## Prompt-ready language

### Video model — hard accent pointing
> "Character points at the door emphatically with a hard accent. Arm swings up fast (4 frames), reaches full extension (1 frame), then RECOILS back slightly (2 frames), then settles into the held pointing pose. The slight bounce-back at the moment of pointing is what makes the gesture feel decisive."

### Video model — hard accent in speech
> "Character delivers the word 'NO!' with a hard accent. Head and body anticipate by pulling slightly back (4 frames). Then SNAP forward on the word 'NO' (2 frames). The body then bounces back from the snap (2 frames). Held in final pose with eyes locked. The recoil makes the line feel definitive."

### Code — hard accent
```javascript
const hardAccentPoint = gsap.timeline();
hardAccentPoint
 // Antic: arm draws back
 .to(arm, { rotation: -30, duration: 0.2, ease: "power2.out" })
 // STRIKE: arm shoots forward fast
 .to(arm, { rotation: 60, duration: 0.1, ease: "power3.in" })
 // BOUNCE: arm recoils 10% back
 .to(arm, { rotation: 50, duration: 0.1, ease: "power2.out" })
 // SETTLE: held with slight oscillation
 .to(arm, { rotation: 55, duration: 0.15, ease: "elastic.out(2, 0.5)" });
```

Or simpler — use `back.out`:
```javascript
gsap.to(arm, { 
 rotation: 60, 
 duration: 0.3, 
 ease: "back.out(2.5)" // strong overshoot then return
});
```

## The "violent stop" variant

Sometimes a hard accent doesn't bounce — it just **STOPS** abruptly. This is also valid for declarative actions where the body doesn't recoil:

> Although sometimes a hard action stops abruptly, as abruptly as it can.
Used for:
- A character coming to a frozen stop in shock
- "Halt!" command
- Sudden realization (everything freezes)

The audience reads the violent stop as just as decisive as a bounce.

## Linked concepts

- [[soft-accent-continues]]
- [[double-takes]]
- [[basic-anticipation]]
- [[the-AAR-formula]]
