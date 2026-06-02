# The Spacing Chart ' Visual Notation for Time

It looks like a ruler with tick marks. It is the single most useful tool for thinking about motion.

## What it looks like

A simple horizontal line representing the path of motion, with tick marks for each drawing position:

```
EXTREME 1 EXTREME 2
 |---|--|-|||---------|-----------|-----------|-----------|
 1 2 3 4 5 6 7 8 9 10
 ^^^^^^^^^^^ ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
 Close spacing Wide spacing
 (slow start) (fast middle / end)
```

The chart is drawn next to a column of inbetween numbers. It tells the assistant (or AI) exactly *where* on the path each drawing should fall.

## How to read it

- **Tightly packed ticks** = slow motion (the drawings are close together in space, so each frame represents very little change)
- **Widely spaced ticks** = fast motion (the drawings cover a lot of ground per frame)
- **Even spacing** = constant speed (rare in good animation, mostly mechanical actions)
- **Tight → wide → tight** = slow-in, accelerate, slow-out (a natural motion)

## The classic chart shapes

### "Slow out" (acceleration from rest)
```
|||||----|----|------|----------|
1 2 3 4 5 6 7 8
```
Common at the start of every gesture, throw, kick, or punch.

### "Slow in" (deceleration to rest)
```
|----------|------|----|----|||||
1 2 3 4 5 6 7 8
```
Common at the end of every gesture — head turns, hand reaching for a cup.

### "Slow in + slow out" (the natural shape)
```
||||----|------|----||||
1 2 3 4 5 6 7 8 9 10
```
The default for natural motion. Sit, stand, turn, look — almost everything.

### "Constant speed"
```
|---|---|---|---|---|---|
```
Rare. Mechanical, machine-like. Use deliberately.

### "Snap then settle"
```
|||||| |---|--|----|
1 2 3 4 5 6 7 8 9 10
[6 frames, no movement] [settle with overshoot]
```
Common in cartoon takes — held in anticipation, then explosive release with follow-through.

## Where the breakdown sits

The breakdown drawing is where the chart's character is *most expressed*. A weak breakdown sits at the midpoint of the chart. A strong one offsets:

- **Closer to extreme 1** → the action starts fast and slows down (a thrown punch slowing as it lands)
- **Closer to extreme 2** → the action starts slow and accelerates (a wind-up swing)
- **Way off the midpoint** → produces character — a tentative reach, a frustrated jerk

## The most important "**

You can have beautiful drawings on every frame and dead animation. You can have rough drawings with perfect spacing and animation that feels alive. The chart is where the animation actually happens.

## Common mistakes the chart prevents

### Mistake 1 — putting the inbetween at the literal midpoint
For fast perspective changes (a telephone pole zipping past camera), the visual midpoint is not the timing midpoint. Most beginners (and most assistants) put it dead center and the motion feels mushy.

**Rule for fast perspective moves:** the inbetween goes about 2/3 of the way toward the camera-near extreme.

### Mistake 2 — linear interpolation between extremes
The mechanical "split the difference" approach kills the motion. Always ask: *should this be faster at the start or the end?* Then offset the breakdown accordingly.

### Mistake 3 — drawing a chart but ignoring it
Many animators draw the chart, then their assistant ignores it. **the chart is the law**. Every inbetween must hit its position on the chart.

## How to encode a spacing chart in an AI prompt

### Video models (Sora, Veo, Kling, Runway)

Translate the chart into natural-language timing:

| Chart shape | Prompt phrase |
|-------------|---------------|
| Slow out (acceleration) | "movement starts slowly then accelerates" |
| Slow in (deceleration) | "movement slows as it approaches its end position" |
| Slow in + slow out | "movement eases out of rest, accelerates through the middle, and eases to a stop" |
| Constant speed | "movement is mechanical, constant speed, no acceleration" |
| Snap then settle | "movement is held still, then explodes into action with a slight overshoot and settle" |
| Hesitant | "movement starts, pauses briefly, then continues — small uneven steps" |

### Code (CSS, GSAP)

Direct mapping to easing functions:

```javascript
const spacingToEasing = {
 "slow-out": "power2.in", // tight → wide
 "slow-in": "power2.out", // wide → tight
 "slow-in-slow-out": "power2.inOut", // tight → wide → tight
 "snap": "power4.in", // very gradual acceleration
 "settle": "elastic.out(1, 0.3)",
 "constant": "none",
 "anticipation": "back.in(2)", // overshoots backwards before going forward
 "follow-through": "back.out(2)", // overshoots past target and settles
};
```

### Keyframe image AI

You can't directly encode the chart in image-only AI. But you *can* generate uneven numbers of keyframes per second of target output:

- Slow-out: generate 6 keyframes in the first half-second, 2 in the second half-second
- Slow-in: opposite distribution
- Constant: equal keyframe distribution

Then feed these to an interpolator with the right "tween count" between each pair.

## A worked example: throwing a punch

The chart for a punch from rest to extension:

```
Anticipation: ||||||----|----| (slow draw-back, brief held wind-up)
Strike: |---|--|-|||| (slow release, accelerate, fast finish)
Recoil: |||||--|----| (snap back, settle)
```

The same punch in prompt form:

> "Character begins relaxed. Pulls fist back over 8 frames with deliberate weight (slow-in to wind-up). Holds wind-up for 2 frames. Strikes forward in 3 frames with maximum acceleration (no eases). Hits the target with a sharp impact frame. Recoils back over 5 frames in a quick settle."

## Linked concepts

- [[slow-in-slow-out]]
- [[extremes-breakdowns-inbetweens]]
- [[the-classic-spacing-mistakes]]
- [[mass-and-motion]]
