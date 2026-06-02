# Simple Overlap — Hair, Coat, Ears, Tails

Anything attached to a moving body lags behind. Hair drags. Coats trail. Ears flop. Tails sweep. This delay is overlap — and it's what makes characters feel like real bodies in real worlds.

## The principle

When the main body changes direction or stops, attached secondary parts **continue their previous motion for a few frames** before catching up.

```
Frame 1: body and hair moving forward together
Frame 5: body stops
Frame 6: body still stopped, hair still moving forward (overlap)
Frame 8: hair reaches its overshoot peak (past body position)
Frame 12: hair settling back toward body
Frame 18: hair finally at rest
```

The hair "drag" makes the body's motion feel weighted. Without overlap, a character looks like a paper cutout.

## The list of overlap-prone parts

| Body part | Lag (frames at 24fps) | Notes |
|-----------|----------------------|-------|
| Short hair | 2-4 | Subtle drag |
| Long hair / ponytail | 5-10 | Significant trail |
| Loose shirt | 3-5 | Wobble visible |
| Heavy coat | 5-8 | Slow drag |
| Skirt / dress | 4-7 | Sway and settle |
| Cape | 6-12 | Dramatic trail |
| Ears (cartoon) | 4-8 | Flop motion |
| Tail | 5-10 | Significant sweep |
| Jowls (heavy character) | 4-6 | Jiggle |
| Belly | 3-5 | Small wobble |
| Breasts | 4-6 | Bounce |
| Backpack | 5-8 | Sway |
| Belt / chain | 4-7 | Whip-like |
| Hat / beanie | 2-5 | Bounce on head |

## How to animate overlap

For any moving character with attached parts:

1. **Animate the body normally** (the primary action)
2. **Animate each attached part as a separate sub-animation**
3. **Delay each part by its lag amount**
4. **Add overshoot at the end of body motion**

The result: secondary parts trail behind, overshoot at the end, settle with diminishing oscillation.

## For long flowing things (hair, capes, tails), the motion creates a wave through the part:

```
Frame 1: tip of cape at rest position
Frame 2: body moves, cape root starts to follow
Frame 4: middle of cape catching up to root motion
Frame 6: tip of cape just starting to move (delay)
Frame 10: cape tip flying past body position (overshoot)
Frame 14: cape tip swinging back
Frame 20: full settle
```

The motion ripples through the cape from root to tip. Each segment lags slightly behind the segment closer to the body.

## The "wave action" formula

For a cape, tail, or hair that's swinging:

1. The root of the part moves first
2. Each segment further from the root moves with a 1-2 frame delay
3. The tip moves last, with the largest delay
4. As the motion peaks, segments reverse direction in a wave pattern
5. The wave continues to oscillate with decreasing amplitude

```
Time: Frame 1 Frame 3 Frame 5 Frame 7 Frame 9
Root: ↑→ ← ↑→ ← ↑→
Middle: → ↑→ ← ↑→ ←
Tip: ← → ↑→ ← ↑→
```

The wave propagates through the part. This is the "whip" action applied to flexible structures.

## the Tiny Tim laugh example

Animator an animator created a Tiny Tim laugh where the WHOLE BODY laughed with overlap:

- Shoulders bounce up and down (the laugh action)
- Belly follows shoulders with a 2-frame delay
- Hands rise to face but lag behind
- Hair bobs with body but delayed
- Each accent of laughter creates a wave through the body

The result: a laugh that felt physically embodied, not just facial.

## How to break a part into segments

For animation:

### Simple parts (2-3 segments)
- Hair on head: root + tip (just 2 segments)
- Short ponytail: root + middle + tip (3 segments)

### Complex parts (5+ segments)
- Long flowing hair: 5-7 segments along length
- Cape: 3-5 segments vertically
- Tail: 4-6 segments along length

More segments = more wave-like motion. Fewer segments = simpler drag.

For physics-based animation (cloth sim, hair sim), the number of segments is automatic. For traditional or keyframe animation, the animator chooses.

## Prompt-ready language

### Video model — overlap on long hair
> "Character with long flowing hair turns head sharply to the left. Head completes turn in 4 frames. Hair lags behind — closest to scalp follows head with 2-frame delay, middle of hair with 5-frame delay, tips of hair with 8-frame delay. Hair tips overshoot past the new head position, then sway back, then settle. Total hair motion takes 18 frames after the head turn."

### Video model — overlap on cape
> "Character in a long cape stops running suddenly. Body comes to halt in 6 frames. Cape continues forward — bottom of cape sweeps past the body position (overshoot 30% past). Cape then swings back, oscillates twice with diminishing amplitude, finally settles to drape behind character. Total cape motion: 24 frames after body stops."

### Video model — heavy character with jiggle
> "Heavy character takes a step and stops. As body stops, belly continues forward for 3 frames, then jiggles back. Jowls jiggle independently — 4 frames of small oscillation. Heavy character's mass continues to settle for 8 frames after the body has stopped moving."

### Code (overlapping animation)
```javascript
// Body stops abruptly
gsap.to(body, { x: 100, duration: 0.3, ease: "power3.in" });

// Hair lags — starts 2 frames later, continues longer
gsap.to(hair, { 
 x: 110, // overshoot by 10
 duration: 0.5, 
 ease: "elastic.out(1, 0.3)",
 delay: 0.08 // 2-frame delay
});

// Cape lags more — starts 4 frames later
gsap.to(cape, { 
 x: 115, // overshoot by 15
 duration: 0.7, 
 ease: "elastic.out(1, 0.25)",
 delay: 0.16 // 4-frame delay
});
```

### Code (wave through a multi-segment part)
```javascript
const hairSegments = [hairRoot, hairMid1, hairMid2, hairTip];

// Each segment follows with progressive delay
hairSegments.forEach((segment, index) => {
 gsap.to(segment, {
 x: targetX + (index * 2), // each segment overshoots more
 duration: 0.5 + (index * 0.1),
 ease: "elastic.out(1, 0.3)",
 delay: index * 0.04 // 1-frame delay per segment
 });
});
```

## Linked concepts

- [[counter-reaction]]
- [[breaking-joints]]
- [[wave-action]]
- [[whip-action]]
