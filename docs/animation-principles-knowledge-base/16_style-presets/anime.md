# Anime Style — Japanese Limited Animation

Anime developed its own animation grammar — based on Disney foundations but constrained by lower budgets, leading to ingenious solutions: held poses, smear frames, impact moments, and stylized motion.

## Key characteristics

- **Mixed frame rates** (ones, twos, threes, fours — used deliberately)
- **Long held poses** with subtle motion underneath
- **Impact frames** (single dramatic poses with extreme effect)
- **Speed lines** and background streaks instead of detailed motion
- **Camera moves** instead of character animation (panning over still poses)
- **Smear frames** — impossible elongated drawings during fast motion
- **Specific eye styles** (large, expressive, distinct shapes)
- **Hair physics** dominates animation budget (hair always moves)
- **Distinct mouth shapes** (simplified, often just 3-4 mouth positions)

## The anime efficiency trick

Anime budgets meant 12-15 frames per second typical (often less). To make this work, animators:

1. **Held poses for several frames** (3-6 frames per pose)
2. **Camera moves over still images** (zoom, pan, push-in)
3. **Background scrolls past static character** (instead of animating walk)
4. **Single impact frame** for big moments (one wild distorted drawing)
5. **Smear frames** during fast motion (impossible elongated shapes)
6. **Selective full animation** for emotional peaks

This creates a distinctive rhythm — long pauses punctuated by sudden bursts.

## Timing characteristics

- **Walks:** Often 16-24 frames per step, simplified
- **Runs:** 4-6 frames per step with held poses
- **Action moments:** 2-3 frame impact poses
- **Emotional reactions:** 24-48 frame holds with subtle motion
- **Camera moves:** common — pan, zoom, dolly over still frames

## Anime-specific moves

### The 3-stage transformation
Anime's signature moment: character undergoes magical transformation in 3 distinct phases:

```
Phase 1 (1-2 seconds): held setup pose with environmental effects
Phase 2 (1-2 seconds): rapid morphing/transformation (animated on ones)
Phase 3 (2-3 seconds): held final transformed pose with held effects
```

### The impact frame
A single dramatic distorted drawing held for 1-2 frames, often white or color-shifted:

- Punch lands: 1 frame of distorted impact face, often white silhouette
- Sword strikes: 1 frame of impossible angle, blur lines
- Explosion: 1 frame of pure white, then second pose of shocked character

### The speed line scene
Character is "running" but they're not actually animated — they're drawn once, and the BACKGROUND scrolls past with speed lines. The character pose is static but the camera and background create motion.

### The dramatic held pose
Character poses dramatically. Held for 4-6 seconds. Hair flows in wind. Eyes glint. The pose IS the moment.

## on limited animation

The anime approach exemplifies what he says: *"It's a style choice. Each frame can be on threes or fours. Holds and slow drift are perfectly fine on threes."*

The key is committing to the style. Anime that tries to be full Disney looks broken. Anime that commits to its own grammar looks distinctive.

## What to specify in prompts

```
Style: anime / Japanese animation. Mixed frame rates — long held poses (3-6 frames) punctuated by 1-frame impact moments and selective full animation. Camera moves common — panning and zooming over still frames. Hair animation prominent. Speed lines for fast action. Distinct simplified mouth shapes (3-4 positions). Large expressive eyes. Background often more detailed than character animation.
```

## Reference anime

- **Studio Ghibli (Miyazaki)** — most fluid, closest to Disney
- **Cowboy Bebop** — slick action choreography
- **Akira (1988)** — landmark, full animation moments
- **Evangelion** — held poses with internal monologue
- **Demon Slayer** — modern blend of 2D and 3D
- **Attack on Titan** — impact frames and camera moves
- **Naruto / One Piece** — long-running with efficient animation
- **Spirited Away** — Miyazaki at peak

## Specific anime styles within the broader category

### Shōnen action (fighting anime)
- Quick action with impact frames
- Speed lines / motion blur
- Energy aura effects
- Powered-up transformation sequences
- Yelling with distinct mouth shapes

### Shōjo (girl-target)
- Held romantic poses
- Sparkle effects
- Hair animation prominent
- Soft lighting / pink/purple palettes
- Camera holds on emotional faces

### Mecha
- Detailed mechanical designs
- Multi-stage transformation
- Held cockpit views
- Camera moves over static mechs
- Explosion / impact frames

### Cute / Chibi
- Simplified animation
- Big eyes, small bodies
- Bouncy timing
- Quick emotion changes
- Simple mouth shapes

## Prompt examples

> "Anime action scene. Two characters fight. Character A leaps forward — full animation on ones for 8 frames showing the leap. Held impact frame: 1-frame of clash with white background and motion lines, both characters distorted with impact. Then held aftermath pose 3 seconds — character A panting, hair blowing, sword raised. Camera slowly pushes in during the hold. Speed lines fade. Studio Ghibli / Cowboy Bebop style."

> "Anime dramatic transformation. Character begins transformation in a held pose for 2 seconds, hair starting to lift. Then rapid morphing animation on ones for 1.5 seconds — clothing changes, energy aura forms, body silhouette shifts. Held final transformed pose for 3 seconds with hair flowing, energy effects pulsing, eyes glowing. Camera slowly zooms in. Magical girl anime / Shōjo style."

## Code translation

```javascript
const ANIME_STYLE = {
 // Most poses held longer than other styles
 defaultPoseHold: 8 / 24, // 8 frames typical
 emotionalHold: 48 / 24, // 2 seconds held emotional pose
 impactFrame: 2 / 24, // very short distorted impact
 
 // Action moments
 fastAction: 4 / 24, // brief fast moment
 
 // Camera typically does the work
 cameraMoveDuration: 60 / 24, // 2.5 seconds for camera move
 
 // Easing
 defaultEase: 'power2.inOut',
 impactEase: 'power4.in',
 cameraEase: 'sine.inOut',
};

// Limited animation pattern — most things stay still
function animePose(character, duration) {
 // Static pose with only subtle movement (hair, breathing)
 gsap.to(character.hair, { 
 y: '+=3', 
 duration: 1, 
 yoyo: true, 
 repeat: -1,
 ease: 'sine.inOut'
 });
 // Character body otherwise still
}
```
