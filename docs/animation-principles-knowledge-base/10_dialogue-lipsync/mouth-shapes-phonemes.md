# Mouth Shapes — The Phoneme Vocabulary

A reference for the standard mouth shapes used in dialogue animation. **But remember: phrasing matters more than phonemes.** Use this as a vocabulary, not a script.

## The basic mouth shapes

Most animation systems use a vocabulary of 8-12 standard mouth shapes ("visemes"). Here's the classical-flavored set:

### Closed shapes

#### M / B / P (lips together)
- Lips fully closed
- Slight tension in lips
- Used for: M, B, P, mm sounds
- **Hold for 2+ frames** to register

#### F / V (lip on teeth)
- Lower lip tucked under upper teeth
- Mouth slightly open
- Used for: F, V, ph sounds

### Open shapes

#### A / Ah (wide open)
- Mouth fully open
- Jaw dropped low
- Lips relaxed
- Used for: father, car, ah

#### E / Eh (open with smile)
- Mouth open horizontally
- Lips pulled to sides
- Teeth visible
- Used for: bed, set, eh

#### I / Ee (tight smile)
- Mouth slightly open
- Lips pulled wide to sides
- Teeth mostly visible
- Used for: see, bee, key

#### O / Oh (round, medium)
- Lips form a round shape
- Mouth medium open
- Used for: go, no, oh

#### U / Oo (round, tight)
- Lips tightly pursed
- Small round opening
- Used for: too, food, you

### Other shapes

#### L (tongue up)
- Mouth slightly open
- Tongue tip visible touching upper teeth
- Used for: L sounds, only flash through

#### T / D / N / S (closed/teeth)
- Teeth nearly touching
- Tongue position implied
- Used for: most consonants
- Often skipped between vowels

#### Rest / Silence
- Mouth slightly open, relaxed
- The default neutral position

## s

### Rule 1: Don't animate the tongue between positions
> Never inbetween the tongue in speech. Our tongues work so fast we only see it up or down, never on route (with a pause).
The tongue goes UP on the L sound, then DOWN on the next frame. No transition frame.

### Rule 2: Upper teeth don't move
> Remember that the upper teeth are anchored to the skull and don't animate.
When you draw an open mouth, the upper teeth stay still. The lower jaw moves down. The lips wrap around both.

### Rule 3: The jaw is mostly up/down
> Most of the jaw animation is up and down
Don't over-animate the jaw side-to-side. It moves vertically with occasional rotation. Lips do most of the shape work.

### Rule 4: Closures need 2+ frames
> We agreed we needed 2 frames to catch the important consonants like M, P, B, F, V or T. If not, the viewer won't see it.
If a closure is on the soundtrack for only 1 frame, **borrow a frame from the preceding sound** — close the mouth one frame early.

```
SOUND: [...vowel...] [M sound 1 frame] [next vowel...]
NAIVE: [mouth open] [closed 1 frame] [open] ← won't register
WORKING: [mouth open] [closed 2 frames] [open] ← steal a frame from previous vowel
```

## The 12-shape standard (used in production)

For real production work, animators use a more granular set. Here's the standard reference:

| # | Shape | Used for |
|---|-------|---------|
| 0 | Rest / silent | Pauses, breaths |
| 1 | Closed (M, B, P) | Lip closures |
| 2 | Closed Tight (puff) | P specifically |
| 3 | F / V | Lip-on-teeth |
| 4 | A / Ah / Aw | Wide open |
| 5 | E / Eh | Horizontal open |
| 6 | I / Ee | Tight smile |
| 7 | O / Oh | Round medium |
| 8 | U / Oo / W | Round tight |
| 9 | L / N / D / T | Tongue/teeth indicated |
| 10 | S / Z / Sh | Teeth showing, tight |
| 11 | R | Slight pucker |

In animation software (Maya, Toon Boom, After Effects with rigs), these are usually keyboard-shortcut blendshapes or X-sheet positions.

## When to skip shapes

Only the important ones get unique shapes.

### Example: "I love you"

The phonemes are: AY - L - UH - V - Y - OO

But you only need:
1. **AY shape** (the "I")
2. **V shape** (the "V" — critical lip-on-teeth closure)
3. **OO shape** (the "you")

The L, UH, and Y sounds happen in transition. Skip drawing them.

```
Naïve approach: 6 distinct mouth shapes (looks fluttery)
the classical tradition approach: 3 distinct mouth shapes (reads clean)
```

### Example: "Hello"

Phonemes: H - EH - L - OH

But you only need:
1. **EH shape** (the open vowel)
2. **L shape** (brief tongue flash, 1 frame)
3. **OH shape** (the round close)

The H is invisible (just an exhale through whatever shape is there). The transition is the L flash.

## a slow-articulating actor observation

But from the front, his face looked like a fish.

The lesson: **mouth shapes look very different from different angles.** When generating AI keyframes:
- Front view: round shapes (O, U) read most clearly
- Side view: open vowel shapes (A, E) read most clearly
- 3/4 view: hybrid — all shapes work

Most dialogue is animated in 3/4 view for this reason. It gives the best of both.

## Prompt-ready language

### Video model — character speaks a line
> "Character says 'I love you.' Mouth opens wide on the 'I' (AY shape), closes briefly with lower lip on upper teeth for the 'V' (1-2 frames), then closes into rounded 'OO' shape for 'you.' Three distinct mouth positions, with transitions handled by interpolation. Don't animate every letter."

### Video model — emphasized line
> "Character emphatically says 'NO!' Mouth opens wide on 'N' transitioning quickly to the round 'O' shape. Held open for 4 frames. Then closes slightly. Eyes wide. Body language reinforces the line."

### Video model — singing
> "Character sings 'la-la-la' clearly. For each 'L', tongue visible at upper teeth (1 frame). For each 'A', mouth wide open (3 frames held). Distinct articulation since singing requires clearer vowel shapes than speech."

### Code (simple lip sync system)
```javascript
// Lip sync from audio: trigger blendshapes based on phoneme markers
const lipSync = {
 AH: () => morphTo('mouth_open_wide', 0.08),
 EE: () => morphTo('mouth_smile', 0.06),
 OH: () => morphTo('mouth_round_med', 0.07),
 OO: () => morphTo('mouth_round_tight', 0.06),
 MM: () => morphTo('mouth_closed', 0.10), // hold longer for closure
 FV: () => morphTo('mouth_f', 0.08),
 REST: () => morphTo('mouth_rest', 0.12),
};

// Process phoneme timeline (from audio analysis)
phonemes.forEach(({ sound, timestamp }) => {
 setTimeout(() => lipSync[sound](), timestamp * 1000);
});
```

For AI keyframe generation:
- Use distinct mouth shape descriptions per important phoneme
- Don't generate transition frames — they're interpolated

## Linked concepts

- [[phrasing]]
- [[dialogue-accents]]
- [[image-sound-sync]]
- [[facial-flexibility]]
