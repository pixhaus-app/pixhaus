# The Side-to-Side Wiggle Formula

**

## The setup

To make a head wobble side-to-side:

1. Draw a sequence of poses going from left to right (e.g., 17 drawings)
2. Trace those same drawings but with a slight offset in the opposite direction
3. Interleave them — drawing 1A, then 1B, then 2A, then 2B, etc.

The result: a hand or head that vibrates side-to-side while progressing through the main motion.

## The formula visualized

```
Series A (main motion): 1 - 2 - 3 - 4 - 5 - 6 - 7 - 8 - 9
Series B (offset trace): 1 - 2 - 3 - 4 - 5 - 6 - 7 - 8 - 9

Interleaved playback: A1, B1, A2, B2, A3, B3, A4, B4, A5...
```

When played back, the alternation between A and B drawings creates a high-frequency wobble overlaid on the main motion.

## s

### a master action animator's earthquake action
the analysis: Ken used two interleaved drawing series — the trembling is the alternation between them.

### Hand vibration
For a hand that's shaking with fear or rage:
- Series A: hand at natural position
- Series B: hand traced from A but shifted slightly left or right
- Alternate between them on ones (every frame)
- Result: a hand vibrating high-frequency side-to-side

### Triumphant arm raise
For an arm rising in triumph while vibrating with intensity:
- Series A: arm rising from down to up over 9 frames
- Series B: arm tracing the same path but offset
- Interleave: A1, B1, A2, B2... A9, B9
- Result: arm rising while vibrating

## Why this is more elegant than random shake

A truly random shake looks chaotic. creates a **structured** vibration that:
- Has consistent frequency
- Maintains the main motion underneath
- Produces a clean visual rhythm
- Is easy to control

The audience reads it as "controlled vibration" rather than "random chaos."

## Timing and frequency

The shake frequency depends on the alternation rate:

| Alternation | Effective frequency | Use |
|-------------|---------------------|-----|
| Every frame (ones) | 12 Hz at 24fps | Fast vibration, fear, rage |
| Every 2 frames (twos) | 6 Hz | Medium tremor, cold |
| Every 3 frames | 4 Hz | Slow wobble, dizzy |
| Every 4 frames | 3 Hz | Gentle sway |

## How to encode in code

### Pure shake (no main motion)
```javascript
// Shake a fixed-position object
gsap.to(element, {
 x: '+=2',
 duration: 1/24,
 yoyo: true,
 repeat: -1,
 ease: "none"
});
```

### Shake overlaid on motion ()
```javascript
// Main motion + shake overlay
const mainMotion = gsap.timeline();
mainMotion.to(element, { y: -100, duration: 0.4, ease: "power2.out" });

// Add shake on top
const shake = gsap.to(element, {
 x: '+=3',
 duration: 1/12, // 2 frames at 24fps
 yoyo: true,
 repeat: 20,
 ease: "none"
}, 0);
```

### Multi-segment wave (for tail, hair shake)
```javascript
const segments = [segment1, segment2, segment3, segment4];

segments.forEach((seg, i) => {
 gsap.to(seg, {
 x: '+=3',
 duration: 1/12,
 yoyo: true,
 repeat: -1,
 delay: i * 0.04, // each segment offset by 1 frame
 ease: "none"
 });
});
```

## s by use case

### Hand trembling with fear
- High frequency vibration (every frame)
- Small displacement (2-4 pixels)
- Random tiny variation in position
- Overlay on main motion

### Body shivering with cold
- Medium frequency (every 2-3 frames)
- Whole-body small displacement
- Add slight head bobble in addition to body
- Slower than fear-trembling

### Rage trembling
- High frequency, larger displacement
- Whole-body involvement
- Visible body tension
- Often accompanied by clenched fists, gritted teeth

### Earthquake / impact aftershock
- Initial big shake, decaying over time
- Whole environment shakes, not just one element
- Multi-segment wave through body

### Vibrato (for singing characters)
- Subtle high-frequency mouth motion
- Very small displacement
- Adds "musical" quality to dialogue animation

### Cartoon dizziness
- Slow circular motion (not just side-to-side)
- Head wobbles in a circle pattern
- Used with motion lines / stars

## Prompt-ready language

### Video model — controlled vibration
> "Character holds a glass with shaking hand. The hand vibrates with controlled high-frequency motion — about 8 oscillations per second, small displacement (a few pixels). The vibration is overlaid on a slow rising motion as the character lifts the glass. The shake is consistent and structured, not random chaos."

### Video model — fear trembling
> "Character experiences fear. Body trembles with high-frequency small-amplitude shake — entire body vibrating 10-12 times per second. Hands shake more visibly than torso. Head has small additional bobble. Trembling continues throughout the held scared pose."

### Video model — earthquake on character
> "Earthquake shakes the character. Initial big shake (large displacement), decaying over 3 seconds to small residual tremor. Whole body involved — head shakes, body wobbles, character struggling to maintain balance. Multi-segment wave through body from feet to head."

### Code ()
For a hand-drawn approach, you'd create two drawing series and interleave. For code:

```javascript
// "Interleaved series" approach — alternate between two states each frame
let toggle = false;
gsap.ticker.add(() => {
 toggle = !toggle;
 element.x = toggle ? mainX + offset : mainX - offset;
 // mainX is calculated from the underlying motion timeline
});
```

Or for cleaner implementation:
```javascript
// Sine wave overlay on main motion
gsap.to(state, { y: -100, duration: 1, ease: "power2.out", 
 onUpdate: () => {
 const shake = Math.sin(state.time * 50) * 3; // 50 = frequency, 3 = amplitude
 element.x = state.x + shake;
 }
});
```

## Linked concepts

- [[wave-action]]
- [[whip-action]]
- [[simple-overlap]]
- [[counter-reaction]]
