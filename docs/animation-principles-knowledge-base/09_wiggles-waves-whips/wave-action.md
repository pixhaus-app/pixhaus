# Wave Action — Flags, Hair, Tails, Ropes

Wave action is when a motion travels through a flexible object from one end to the other in a continuous wave. Like a flag in wind, a rope wave, hair flowing, or a tail swishing.

## The principle

A wave action has these characteristics:
1. Motion enters at one end (the root)
2. Travels along the length of the object
3. The far end moves later than the near end
4. The motion is smooth, not snap

## the rope example

The rope:
- Root attached to ceiling (stationary)
- Body grips middle/bottom
- Rope shows wave pattern from root to body
- Body sways in arc, rope follows

```
Frame 1: rope straight, body at rest position
Frame 5: body swinging right, wave starts traveling down rope
Frame 10: wave at midpoint of rope
Frame 15: wave at the body, body at extreme right
Frame 20: body swinging back, new wave traveling down
```

## The wave-action pattern

For any flexible object, divide it into segments. Each segment lags behind the segment closer to the root:

```
Root segment: follows the driver (body, wind, hand) directly
Segment 2: lags 1-2 frames behind root
Segment 3: lags 2-3 frames behind root
Segment 4: lags 3-5 frames behind root
Tip segment: lags the most
```

When the root reverses direction, the new motion travels along the segments in sequence — creating the wave.

## Wave vs. whip

- **Wave**: smooth back-and-forth, like a flag or hair flowing
- **Whip**: sharp acceleration at the tip, like a whip crack (see `whip-action.md`)

The difference: a wave maintains smooth motion throughout. A whip has a sudden acceleration that's invisible to the eye but adds the "crack."

## Applications

### Hair flowing in motion
A character running has hair waving behind them:
- Root of hair at scalp follows head motion
- Middle of hair lags by 4-6 frames
- Tips of hair lag by 8-12 frames
- The hair forms a continuous wave shape, not segments jerking

### Flag in wind
- Pole stationary
- Cloth segment 1 follows pole
- Subsequent segments form a wave propagating outward
- As wind direction shifts, new wave starts at pole and travels

### Cape in motion
- Shoulders (root) follow body
- Cape body follows shoulders with delay
- Cape bottom follows cape body with more delay
- When character turns, wave propagates through cape

### Tail / pony tail
- Root attached to head/body
- Tail length forms a continuous curve
- Motion at root propagates as wave to tip
- Tip has largest amplitude (most overshoot)

## For a simple wave action, you can sometimes do it in just 3 drawings:

```
Drawing 1: object in "S-curve up" position
Drawing 2: object straight
Drawing 3: object in "S-curve down" position
```

Looping these three creates a continuous wave. For hair or a flag, this is the basic cycle. Add inbetweens for smoothness.

## Wave action in walks

A character's hair while walking has wave action that bobs UP and DOWN with each step:

```
Step 1 contact: hair high (overshoot from previous down step)
Step 1 down: hair starts to follow body down
Step 1 passing: hair at low point
Step 1 up: hair starts to rise
Step 2 contact: hair high again
```

The hair forms a continuous wave riding on top of the walk cycle. This is what makes long hair look alive in walking characters.

## Numerical control of waves

For animation systems, a wave can be parameterized:

- **Amplitude**: how far each segment moves (peak displacement)
- **Frequency**: how fast the wave oscillates
- **Wavelength**: distance between wave peaks
- **Phase offset per segment**: delay between consecutive segments
- **Damping**: how the wave dies down (or stays steady)

Adjusting these creates everything from gentle hair drift to violent whip.

## Prompt-ready language

### Video model — wave action through hair
> "Character with long hair turns head left. Hair shows wave action: scalp area follows head turn immediately, middle of hair lags 4 frames behind, tips of hair lag 8 frames behind. The hair forms a flowing S-curve during the turn, then continues to wave gently after the head settles, oscillating with decreasing amplitude over 2 seconds."

### Video model — flag in wind
> "Flag fluttering in steady wind. Wave action travels from pole outward to the flag's free edge. New waves form at the pole every 0.5 seconds and propagate outward. The free edge shows the largest displacement. Wave is continuous, not jerky — smooth flowing motion."

### Video model — tail wave
> "Cat walking with tail held high. Tail shows continuous wave action — root follows body motion, but the tip describes a slow figure-eight pattern. With each step, the tail makes one full wave cycle from base to tip. The tail tip is the most expressive — large amplitude, smooth curves."

### Code (wave through multiple segments)
```javascript
const hairSegments = [hair1, hair2, hair3, hair4, hair5];

function animateWave(amplitude, frequency, phaseOffset, duration) {
 hairSegments.forEach((seg, i) => {
 gsap.to(seg, {
 x: `+=${amplitude}`,
 duration: 1 / frequency,
 yoyo: true,
 repeat: -1,
 ease: "sine.inOut",
 delay: i * phaseOffset, // each segment offset
 });
 });
}

// Gentle wave (hair in breeze)
animateWave(amplitude=5, frequency=2, phaseOffset=0.08);

// Aggressive wave (running with hair flying)
animateWave(amplitude=15, frequency=4, phaseOffset=0.06);
```

### Code (continuous wave with sine curve)
```javascript
// Wave through segments using sine
gsap.ticker.add(() => {
 hairSegments.forEach((seg, i) => {
 const phase = (currentTime - i * 0.04) * frequency * Math.PI * 2;
 seg.y = baseY + Math.sin(phase) * amplitude;
 });
});
```

## Linked concepts

- [[whip-action]]
- [[simple-overlap]]
- [[side-to-side-wiggle-formula]]
- [[counter-reaction]]
