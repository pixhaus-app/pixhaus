# Realistic / Naturalistic Style — cartoon/live-action hybrid, Rotoscoped, Photoreal CGI

The opposite of cartoon exaggeration. Motion matches real physics. Timing matches real human pace. Subtle acting. Photographic realism.

## Key characteristics

- **Drawn on ones for smooth motion**
- **Natural easing throughout** (no extreme snaps)
- **Minimal squash and stretch** (only on natural materials)
- **Subtle exaggeration** (just enough for camera to read)
- **Real-life proportions**
- **Realistic physics** (gravity, weight, friction)
- **Subtle facial acting** (no big expressions)
- **Voice-driven dialogue** (mouth shapes match phonemes carefully)

## His insights:

- The cartoon characters had to feel physically present in the real world
- Their motion had to obey real physics in shadow and light
- Their timing had to feel "convincing" not "cartoony" — even when exaggerated
- Real-world reference (live-action) was essential

For pure realism (no cartoon elements):

- Motion captures real movement
- Timing matches real human pace
- Exaggeration is minimized
- Personality comes from subtle character work, not big poses

## Timing characteristics

- **Walks:** 12-16 frames per step (standard human pace)
- **Runs:** 6-8 frames per step
- **Head turns:** 16-24 frames (natural pace)
- **Reactions:** subtle, building over real-time durations
- **Holds:** brief, with always-on subtle motion (breathing, blinks)

## What to avoid in realistic style

- **Extreme squash/stretch** (immediately reads as cartoon)
- **Huge takes** (looks like a sitcom mug)
- **Twinning** (read as puppet-like)
- **Lack of micro-motion** (no breathing, no blinks)
- **Linear motion** (no ease)
- **Cartoon physics** (impossible motion, weightless leaps)

## Realistic mouth animation

For dialogue:
- Mouth shapes match phonemes more carefully than cartoon
- Jaw movement is mostly vertical (real human anatomy)
- Lips are precise but small in movement
- Tongue visible but not exaggerated
- Cheeks barely move

The opposite of Looney Tunes mouth animation, which is wide and exaggerated.

## Realistic facial expressions

For emotion:
- Subtle eyebrow shifts (millimeters in real measurement)
- Eye narrowing or widening (small changes)
- Mouth corners (small upward / downward)
- Jaw position (relaxed vs. clenched)
- Nostril flare (subtle)

These are all SMALL movements. The audience reads them because the rest of the animation respects realism.

## Realistic body language

- Standing pose: subtle S-curve (contrapposto), one foot slightly weight-bearing
- Breathing visible: chest rises and falls
- Constant micro-motion: head bob, blink, weight shift
- No held statue poses (8+ frames of stillness reads as unconscious)
- Realistic proportions and joint constraints

## What to specify in prompts

```
Style: realistic / naturalistic animation. Motion matches real-world physics. Timing matches real human pace. Animated on ones for smooth motion. Subtle acting — no big expressions. Constant subtle motion (breathing, blinks, weight shifts) prevents statue-like stillness. Mouth animation follows real anatomy. Avoid: extreme squash/stretch, cartoonish takes, linear motion.
```

## Reference works

- **cartoon-in-live-action hybrid films** — cartoon-in-realistic-world
- **Ralph Bakshi's rotoscoped films** — Lord of the Rings (1978), Cool World
- **modern animation directors's Pixar films** — character realism with cartoon style
- **Polar Express, Beowulf** — early mocap films
- **The Adventures of Tintin (2011)** — mocap-driven realism
- **Disney's live-action remakes** — photoreal CGI animals

## Subset: Disney Live-Action Style

For modern photoreal CGI (Lion King 2019, Jungle Book 2016):
- Animal motion captures real animal behavior
- Subtle expressions
- Real-world physics
- Hair, fur, water all simulated
- Faces are photoreal but slightly stylized

## Prompt examples

> "Realistic naturalistic 2D animation. Character sits in a chair and looks out the window thoughtfully. Sitting motion takes 2 seconds (48 frames at 24fps). Body lowers smoothly, weight transfers to chair. Head turns toward window over 24 frames. Hand rises to touch chin in subtle thinking gesture. Eyes settle on something outside. Breathing visible — chest rises and falls. Subtle blink every 3 seconds. Mouth in resting position, occasionally tightens with thought. NO extreme expressions or movements. cartoon/live-action hybrid / rotoscoped feel."

> "Realistic CGI photoreal animation. Character delivers an emotional confession. Mouth animation follows real phoneme shapes precisely. Eyes well up with tears (subtle change in light reflection). Eyebrows knit together by 2-3 mm. Head tilts down 5 degrees. Hand brushes hair behind ear. Speech is delivered at natural pace, with natural pauses. No big expressions — emotion conveyed through subtle constant motion. Disney photoreal remake style."

## Code translation

```javascript
const REALISTIC_STYLE = {
 walkStep: 13 / 24, // natural pace
 runStep: 7 / 24,
 headTurn: 18 / 24, // slower than cartoon
 gesture: 18 / 24,
 
 // Easing — natural physics
 defaultEase: 'power2.inOut',
 fallEase: 'power2.in', // gravity
 settleEase: 'power2.out', // no elastic
 
 // Squash/stretch — minimal
 squashAmount: 0.05, // 5% maximum
 stretchAmount: 0.05,
 
 // Always-on motion (prevents statue look)
 breathingCycle: 4, // 4 seconds per breath
 blinkFrequency: 3, // every 3 seconds avg
 microMotionAmount: 0.5, // very subtle drift
};

// Realistic idle = always slightly moving
function realisticIdle(character) {
 // Breathing
 gsap.to(character.chest, { 
 y: '+=2', 
 duration: 2, 
 yoyo: true, 
 repeat: -1, 
 ease: 'sine.inOut' 
 });
 
 // Periodic blinks
 setInterval(() => {
 if (Math.random() < 0.4) {
 gsap.to(character.eyelids, { 
 closed: 1, 
 duration: 0.08, 
 yoyo: true, 
 repeat: 1 
 });
 }
 }, 2500); // every 2.5 seconds
 
 // Subtle weight shifts
 setInterval(() => {
 gsap.to(character.body, { 
 x: (Math.random() - 0.5) * 3, 
 duration: 3,
 ease: 'sine.inOut'
 });
 }, 5000);
}
```
