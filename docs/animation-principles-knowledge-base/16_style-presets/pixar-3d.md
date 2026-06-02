# Pixar / Modern 3D Style — Toy Story through Soul

Modern 3D animation builds on Disney's foundations but uses computer-generated rigs. The visual style is photorealistic textures with stylized character design. The animation principles are unchanged.

## Key characteristics

- **Smooth animation** (computer interpolation between keyframes)
- **Strong acting** in the Disney tradition
- **Solid drawing** — characters always feel 3D
- **Subtle squash and stretch** (selectively applied)
- **Photoreal materials, stylized shapes**
- **Full body involvement** in every gesture
- **High frame rate feel** (smooth, but typically still on ones)
- **Cinematic camera work** (real-camera moves and lenses)

## A note on the principles in 3D

The 3D rig provides perfect interpolation between keys. But the keys themselves must follow the principles. The computer doesn't replace animation knowledge — it executes it.

In fact, the principles are MORE important in 3D because:
- The rig CAN do mechanical motion (computers don't have natural easing)
- Linear interpolation between keys is the default — must override
- Without animator intent, 3D looks robotic
- The same principles that worked in 2D Disney work in 3D Pixar

## Timing characteristics

- **Walks:** 12-16 frames per step (natural)
- **Runs:** 6-8 frames per step
- **Reactions:** built over 12-24 frames
- **Holds:** very brief — usually <8 frames with always-on subtle motion
- **Camera moves:** common, cinematic, often complex

## What makes Pixar's style distinctive

### Strong character acting
Pixar animators come from a Disney lineage. a Disney master, another Disney master, an early Disney mentor directly mentored early Pixar animators. The acting tradition continues.

### Solid construction
3D rigs maintain perfect volume across all poses. Squash and stretch is applied through rig controls (not natural drawing distortion).

### Cinematic cinematography
3D allows complex camera moves. Pixar uses real-camera language — lens distortion, depth of field, dolly shots, etc.

### Stylized character design
Even with photoreal rendering, character designs are stylized — big heads, simplified features. The "Pixar look" combines stylized characters with photoreal worlds.

## Frame rate considerations

Pixar renders at 24fps but the smoothness can feel like more. Reasons:
- Motion blur applied during render
- Smooth interpolation between keys
- Subtle micro-motion always present

For "Pixar feel" in other media:
- Don't go below 24fps perceived
- Add motion blur to fast motion
- Avoid the on-twos staccato

## What to specify in prompts

```
Style: Pixar / modern 3D animation. Smooth animated motion at 24fps. Strong character acting in the Disney tradition. Solid drawing — characters maintain volume in 3D throughout. Subtle squash and stretch only on appropriate moments. Photoreal materials with stylized character designs. Cinematic camera work. Always-on subtle motion (breathing, micro-movements) prevents statue-like stillness.
```

## Reference Pixar films

- **Toy Story (1995)** — landmark 3D animation
- **The Incredibles (2004)** — superhero action in 3D
- **Up (2009)** — emotional Disney heritage
- **Inside Out (2015)** — character acting peak
- **Coco (2017)** — beautiful color/light
- **Soul (2020)** — abstract concepts visualized
- **WALL-E (2008)** — silent character acting

## Other modern 3D
- **DreamWorks** (Shrek, How to Train Your Dragon) — similar tradition
- **Illumination** (Despicable Me, Minions) — more cartoon-leaning
- **Sony Pictures Animation** (Spider-Verse) — bold stylization
- **Disney CGI** (Frozen, Moana) — direct Disney lineage

## Spider-Verse style (special case)

A specific 3D substyle worth noting — *Into the Spider-Verse* style:
- Animated on twos sometimes (limited animation feel in 3D)
- Bold halftone shading
- Comic book aesthetic
- Mixed frame rates within shots
- Stylized motion blur
- Frame skips for stylized impact moments

This is "3D pretending to be 2D" — using limited animation grammar in a 3D pipeline.

## Prompt examples

> "Pixar / modern 3D animation style. Character walks across a sunlit park. Walk has natural 14-frame-per-step timing. Body has clear S-curve in motion. Full character acting — facial expressions read clearly. Subtle squash on each footfall. Hair has physics simulation, drags behind body. Cinematic camera tracks alongside character with shallow depth of field. Soft warm afternoon light. Toy Story / Inside Out smoothness."

> "Spider-Verse style 3D animation. Animated on twos at 24fps (12 unique frames per second). Bold comic-book aesthetic with visible halftone shading. Character punches forward — anticipation crouch on twos, then 1-frame impact pose, then settle on twos. Frame skips during fast action for stylized energy. Speed lines and comic ben-day dots. Frenetic energy."

## Code translation

```javascript
const PIXAR_STYLE = {
 walkStep: 13 / 24,
 runStep: 7 / 24,
 gesture: 16 / 24,
 
 // Smooth interpolation
 defaultEase: 'power2.inOut',
 
 // Subtle effects
 squashAmount: 0.1, // 10%
 stretchAmount: 0.1,
 
 // Camera language
 cameraEase: 'sine.inOut',
 
 // Always-on motion
 microMotionEnabled: true,
};

const SPIDER_VERSE_STYLE = {
 effectiveFPS: 12, // animated on twos
 
 // Frame-stepping
 useSteps: true,
 
 // Frame skips for impact
 impactSkip: true,
 
 // Visual style
 halftone: true,
 comicEffects: true,
};
```

## Linked concepts

- [[classic-disney]] (foundational style)
- [[looney-tunes-cartoon]]
- [[realistic]]
- [[anime]]
