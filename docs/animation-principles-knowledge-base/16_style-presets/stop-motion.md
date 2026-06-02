# Stop-Motion Style — Aardman, Coraline, Wallace & Gromit

Stop-motion has a distinct rhythm imposed by its production method: physical models photographed one frame at a time. Even when imitated in 2D or CGI, the style requires specific timing.

## Key characteristics

- **Drawn on twos by necessity** — but also for stylistic feel
- **Slight pop between frames** (inherent to single-frame photography)
- **Tangible weight** (real puppets respect physics)
- **Subtle motion blur** (or none — depends on camera)
- **Distinct texture** (clay, fabric, fur, paper visible)
- **Limited frame rate feel** (12fps perceived, even if presented at 24fps)
- **Held expressive poses** common
- **Slight imperfections** in motion (real-world handling)

## ## The "stop-motion feel" in other media

To simulate stop-motion in:

### 2D animation
- Animate on threes (8fps perceived)
- Or on twos with slight pose variations between
- Slight pop / shift between frames
- Held poses common
- Distinct hand-drawn feel

### CGI
- Render at 12-15 fps
- Or render at 24fps but with frame-doubling
- Add slight noise/grain
- Limit motion blur (or apply selectively)
- Subtle imperfections in pose between frames

### Code animation
- Use `steps()` easing in CSS
- Snap to 12fps update rate
- Add slight random position variation
- Held poses with no interpolation

## Stop-motion specific moves

### The "settling pose" hold
After any motion, the puppet is allowed to settle (rest with gravity for a moment) before next action. This creates a natural pause.

### The "replacement smile"
For mouth shapes, stop-motion often uses replacement parts (different faces / mouths swapped frame to frame). Each replacement is held for 2+ frames.

### The "puppet weight"
Puppets feel grounded because they ARE grounded. Their weight pulls them in real ways. Translate this to:
- Always-on slight downward pressure
- Visible armature joint mechanics
- Realistic cloth physics
- Hair that DOESN'T defy gravity

### The "held shape"
A stop-motion pose is often more "held" than a 2D pose — partly because moving the puppet is expensive, partly because the audience expects it.

## Reference works

- **Wallace and Gromit (Aardman)** — classic clay animation
- **Chicken Run (2000)** — Aardman feature
- **Coraline (2009)** — Laika peak craftsmanship
- **Kubo and the Two Strings (2016)** — Laika beauty
- **Fantastic Mr. Fox (2009)** — Wes Anderson + stop-motion
- **Isle of Dogs (2018)** — Wes Anderson continued
- **The Nightmare Before Christmas (1993)** — Tim Burton classic
- **Anomalisa (2015)** — adult stop-motion

## Aardman-specific characteristics

The Aardman house style (Wallace & Gromit, etc.) has:
- Big teeth, big eyes (claymation features)
- Expressive faces with replacement parts
- British humor in poses (raised eyebrow, subtle takes)
- Dry comic timing — held beats
- "Cheeky" character expressions

## Laika-specific characteristics

Laika films (Coraline, Kubo, etc.) have:
- High-detail puppets (3D-printed replacement faces)
- Smooth animation on twos
- Cinematic lighting
- Strong emotional acting
- Dark / fantasy aesthetics

## What to specify in prompts

```
Style: stop-motion animation. Animated on twos at 24fps (12 unique frames per second). Slight pop between frames typical of physical puppet handling. Held poses common. Tangible weight — characters respect gravity and physics. Visible textures (clay/fabric/fur). Limited motion blur. Slight imperfections in pose-to-pose continuity add character. Aardman or Laika style.
```

## Prompt examples

> "Aardman / Wallace and Gromit stop-motion style. Character makes tea slowly with deliberate movements. Each gesture: reaches for kettle (held 8 frames in mid-reach), grasps handle (1 frame snap), lifts kettle (24 frames slow rise), pours tea (held 12 frames steady), sets down kettle (16 frames careful placement). Animated on twos throughout. Slight pop between frames. Visible clay texture on character's hands. British kitchen setting. Dry comedic timing — beats between actions feel slightly too long, building humor."

> "Laika / Coraline stop-motion style. Young girl walks slowly down dark hallway, animated on twos at 24fps (12 unique frames per second). Each step takes 20 frames. Body sways subtly with each footfall. Hair has visible texture and drags behind head motion (overlap on twos). Held breath during fearful moments. Slight pop between frames. Cinematic moody lighting. Tangible weight in every step."

## Code translation

```javascript
const STOP_MOTION_STYLE = {
 // Effective frame rate
 effectiveFPS: 12, // even though rendering at 24
 
 // Walking
 walkStep: 20 / 24, // slower than drawn
 
 // Holds
 defaultHoldDuration: 8 / 24,
 
 // Motion characteristics
 // Use steps() in CSS or quantized frame updates in JS
};

// CSS for "stop-motion" feel
const stopMotionCSS = `
 .stop-motion-element {
 animation-timing-function: steps(12, end);
 /* 12 discrete steps per second */
 }
`;

// JS for stop-motion render loop
function stopMotionTick(deltaTime) {
 // Only update every N frames
 if (frameCounter++ % 2 === 0) {
 updateAnimation(deltaTime);
 // Add slight random position offset for "pop"
 character.x += (Math.random() - 0.5) * 0.5;
 }
}
```

## Linked concepts

- [[classic-disney]]
- [[looney-tunes-cartoon]]
- [[realistic]]
- [[../01_foundations/ones-vs-twos]]
