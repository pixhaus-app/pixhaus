# Mass and Motion — How Spacing Communicates Weight

> Think masses, not lines.
You can communicate weight without drawing differently. Just change the spacing. This is the most underrated insight: *the same shape can be heavy or light depending entirely on the spacing chart between extremes.*

## The principle

When a heavy object accelerates, it accelerates **gradually**. The spacing chart for a heavy mass is:

```
HEAVY OBJECT FALLING:
||||---|--|---|----|----|------|-------|---------|
^ tight cluster, lots of frames at start
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
 ^ spacing grows large only late
```

When a light object accelerates, it accelerates **suddenly**.

```
LIGHT OBJECT FALLING:
|----------|---------|--------|-------|------|----|
^ wide spacing immediately
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
 ^ stays roughly even
```

## the bowling ball test

the classic example: a bowling ball cannot *decelerate* on the way down. Gravity is a constant accelerating force. Yet a famous beginner animation textbook published exactly that — a bowling ball with even spacing falling. *"Of course it accelerates — it's gravity!"*

This is why spacing matters more than drawing skill: *the wrong spacing makes the right drawing read as wrong physics.*

## Weight indicators by spacing

| Weight | Falling spacing | Landing impact | Bounce |
|--------|-----------------|-----------------|--------|
| Feather (light) | Slow, drifts side-to-side | None | None |
| Ping-pong ball | Quick to accelerate, light | Tiny squash | High bounce, decays slowly |
| Tennis ball | Standard gravity accel | Moderate squash | Medium bounce |
| Bowling ball | Heavy gravity accel | Deep dent | No bounce, dust puff |
| Anvil | Standard gravity, but BIG | Massive impact frame | Held still, ground cracks |

## How spacing communicates weight (without changing the drawing)

### Light character / object
- **Quick acceleration into motion** — wide spacing immediately
- **High arcs** — gravity affects them less visually
- **Minimal settle** — they don't need to recover from gravity
- **Continued drift** — they float and decay slowly
- **No squash on impact** — they don't have mass to compress

### Heavy character / object
- **Slow acceleration** — many close-spaced frames at start
- **Low arcs** — they fight gravity, never get high
- **Deep settle / recovery** — they hit hard and need time to recover
- **Squash and dust** — impact frames with deformation
- **Held impact** — they stop hard

## The a master action animator contact-before-squash trick

A specific the classical tradition discovery, attributed to a master action animator:

Before a bouncing ball squashes on impact, **insert one contact frame at undeformed size, then squash on the next frame**. This adds a frame of "change" and reads as more weight and more impact.

```
Without trick:
... falling ... → SQUASH → rising ...

With trick:
... falling ... → CONTACT (undeformed, touching ground) → SQUASH (deformed) → rising ...
```

For a jumping character: keep one foot on the ground for an extra frame before pushing off. This adds the same kind of weight cue.

**** "On the way *back up*, you don't need the contact frame — just on impact." The asymmetry is part of the trick.

## Mass × force = motion (the prompt formula)

The animation impression of weight is:

> **perceived weight = (number of slow frames at start of motion) × (depth of squash on impact) × (held duration after impact)**

A character can look heavy by:
1. Slow anticipation (many frames at start)
2. Heavy follow-through (large overshoot then settle)
3. Held finish (the character is "recovering")

A character can look light by:
1. No anticipation
2. Immediate fast movement
3. No follow-through, just a clean stop

## Prompt-ready weight modifiers

### For "heavy" weight in video model prompts:
- "movement has weight and inertia — heavy character resists acceleration"
- "anticipation crouch before lifting, body strains during action"
- "impact frames where character compresses on contact with ground"
- "follow-through lingers — heavy mass settles slowly after impact"
- "shoulders drop and back arches under the load"
- "small dust puffs on each footfall"
- "head lags behind body movement (overlap on heavy turns)"

### For "light" weight in video model prompts:
- "character moves with lightness and quick acceleration"
- "high arcs, floaty quality to movement"
- "no visible strain, immediate response to intention"
- "minimal follow-through, clean stops"
- "feathers, fabric, hair drift slowly after main body moves"

### In code (mass-based easing):

```javascript
// Heavy mass — slow accel, hard stop
gsap.to(heavyObj, { y: 200, duration: 1.2, ease: "power4.in" });

// Light mass — quick accel, ease stop
gsap.to(lightObj, { y: 200, duration: 0.6, ease: "power2.inOut" });

// Heavy landing with squash
gsap.timeline()
 .to(heavyObj, { y: 200, duration: 0.8, ease: "power3.in" })
 .to(heavyObj, { scaleY: 0.6, scaleX: 1.3, duration: 0.1, ease: "power4.out" }) // squash
 .to(heavyObj, { scaleY: 1, scaleX: 1, duration: 0.5, ease: "elastic.out(1, 0.5)" }); // settle
```

### In image keyframes:

For a heavy lift, generate these poses:
1. **Antic crouch** — knees deeply bent, back rounded, hands gripping
2. **Mid-lift strain** — back arched, neck tense, mouth grimacing, eyes squinting
3. **Top of lift** — locked-out arms, body straining, sweat visible
4. **Held pose** — settling, recovering, breathing

For a light lift, just:
1. **Reach pose** — clean reach with no body involvement
2. **Lifted pose** — object held casually, body relaxed

## The exaggerated weight trick

For cartoon weight (Looney Tunes feel), exaggerate further:
- Make the antic crouch deeper than physically necessary
- Add visible squash on the character's whole body during the lift
- Make the strain face cartoonishly stretched
- Add motion lines / sweat drops

For realistic weight:
- Slight knee bend (not deep)
- Subtle back arch
- Held breath visible in chest
- Recovery is just a slight settling

## Linked concepts

- [[showing-weight]]
- [[the-bouncing-ball]]
- [[squash-and-stretch]]
- [[slow-in-slow-out]]
