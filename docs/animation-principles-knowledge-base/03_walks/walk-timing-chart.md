# Walk Timing Chart — Frame Counts by Feel

the definitive table for walk timing. Memorize this. Every variation is a deviation from one of these baselines.

All counts assume **24 fps**. Adjust proportionally for other frame rates.

## The master walk timing table

| Frames per step | Steps per second | Walk feel | Notes |
|-----------------|------------------|-----------|-------|
| **4** | 6 | Very fast run | Both feet airborne, on ones |
| **6** | 4 | Fast run / very fast walk | Sprint, on ones |
| **8** | 3 | Brisk cartoon walk / slow run | Animation standard for "energetic" |
| **12** | 2 | **Natural walk** | **the default — memorize this** |
| **16** | 1.5 | Leisurely stroll | Casual, weekend pace |
| **20** | 1.2 | Older / tired | A walk with weariness |
| **24** | 1 | Slow / heavy / deliberate | Each step takes a second |
| **32** | 0.75 | "Show me the way home" | Very slow, dejected, sneaking |
| **48** | 0.5 | Extreme slow motion | Cinematic / supernatural |

## Why 8s and 16s instead of 12s

Half of 12 is 6, but a third of 12 is 4 — you can't get clean inbetweens.

**Animators prefer 8s or 16s** because they divide cleanly:
- 16: halves at 8, quarters at 4 and 12
- 8: halves at 4, quarters at 2 and 6

So most "natural-feeling" walks in cartoons are actually animated on 8s (slightly fast) or 16s (slightly slow), not the truly natural 12.

Hanna-Barbera, Tom & Jerry, and most TV animation: walks are on 8s.

## a master character animator's reference

**Result: standard walking pace is almost exactly 12 frames per step.** He used 12 as his baseline reference forever after.

## Chuck Jones' musical timing

Chuck Jones (Road Runner cartoons) had a "tempo" built into his films. He'd tap things at a fixed beat — usually 8 or 16 frames — so the music could sync without effort.

Special X-sheets:
- **Sheet 12** — colored line every 12 frames
- **Sheet 16** — colored line every 16 frames
- **Sheet 8** — colored line every 8 frames (standard for tight cartoon timing)

## How to choose the right timing for a character

### Match timing to personality

| Timing | Personality |
|--------|------------|
| 8 frames/step | Energetic, optimistic, military, kid, hero |
| 12 frames/step | Average adult, neutral, "everyman" |
| 16 frames/step | Casual, relaxed, content, friendly |
| 20 frames/step | Tired, older, defeated |
| 24 frames/step | Heavy, deliberate, ominous, contemplative |
| 32+ frames/step | Extremely sad, sneaking, supernatural, dying |

### Match timing to emotion

Same character can have all these:
- **Happy** = faster than baseline (subtract 2-4 frames)
- **Sad** = slower than baseline (add 4-8 frames)
- **Angry** = same speed but with bigger up/down and bigger lean
- **Sneaking** = much slower, held passing poses
- **Determined** = faster + more lean

## How Also, acting it with a metronome is a great help. Naturally, I think in seconds — 'One Mississippi' or 'One little monkey' or 'A thousand and one' etc."*

For animators (and AI prompters): walk across the room as the character. Time it. That's your reference.

## Step length vs. timing

These are independent variables:
- **Step length** = how far each foot moves per step (in screen units)
- **Step timing** = how many frames per step

A character can have:
- **Long steps + fast timing** = running, determined, eager
- **Long steps + slow timing** = giant, deliberate, dramatic
- **Short steps + fast timing** = nervous, hurried, mincing
- **Short steps + slow timing** = sneaking, hesitant, fragile

Match the combination to character intent.

## Translating to AI prompts

### Video models
Use seconds, not frame counts, for clarity:

> "12-frame natural walk at 24fps = 0.5 seconds per step = roughly 2 steps per second"

> "Slow heavy walk, 24 frames per step at 24fps = 1 second per step = 1 step per second"

> "Sneaking, very slow — 32 frames per step at 24fps = 1.33 seconds per step, with held passing pose between steps"

### Code
```javascript
const WALK_TIMING = {
 sprint: { framesPerStep: 4, stepDuration: 4/24 },
 fastRun: { framesPerStep: 6, stepDuration: 6/24 },
 cartoonWalk: { framesPerStep: 8, stepDuration: 8/24 },
 naturalWalk: { framesPerStep: 12, stepDuration: 12/24 },
 stroll: { framesPerStep: 16, stepDuration: 16/24 },
 tired: { framesPerStep: 20, stepDuration: 20/24 },
 slowHeavy: { framesPerStep: 24, stepDuration: 24/24 },
 veryslow: { framesPerStep: 32, stepDuration: 32/24 },
};
```

### Image keyframes
Decide your timing first, then generate keyframes per step at that density. For a 12-frame walk, you need 4 keyframes per step minimum (contact, down, passing, up).

## Final advice from the classical tradition

> It is again, not how they look
There is no universal "correct" walk speed — only the right speed for *this character feeling this thing right now.*

## Linked concepts

- [[the-standard-walk]]
- [[frame-counts-by-feel]]
- [[variations/]]
