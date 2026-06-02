# Weight in a Jump — Selling Impact

A jump can feel weightless or heavy depending entirely on how you handle the contact frames. This is where the "contact-before-squash" trick is most useful.

## The a master action animator contact-before-squash trick (applied to a jump)

When a heavy character lands from a jump:

### Without the trick (weightless)
```
Frame 1: airborne, falling
Frame 2: SQUASH on contact — knees deeply bent, body compressed
Frame 3: recovery starts
```

The character squashes immediately on contact. Reads as light.

### With the trick (heavy)
```
Frame 1: airborne, falling
Frame 2: CONTACT — feet planted, body still extended (one frame of contact, no squash)
Frame 3: SQUASH — knees bend, body compresses
Frame 4: recovery starts
```

The character touches down for ONE FRAME before squashing. Adds visible weight.

> Place at least one foot in contact with the ground before squashing it. And keep at least one leg still in contact when it leaves the surface. That gives more change to the action.
## When to use the trick

- **Heavy characters** — definitely use
- **Big jumps from height** — definitely use
- **Cartoon weight** — exaggerated use (hold contact 2-3 frames)
- **Standard jump** — optional
- **Light/quick jump** — skip the trick

## The "leaving the ground" version

For a character about to push off into a jump:

### Without the trick
```
Frame 1: standing
Frame 2: crouching
Frame 3: airborne (immediately)
```

### With the trick (more weight on push-off)
```
Frame 1: standing
Frame 2: crouching (deep anticipation)
Frame 3: extending (legs stretching, but ONE FOOT STILL ON GROUND)
Frame 4: airborne (both feet leave)
```

The "extending but still in contact" frame adds weight to the push-off.

## What about the airborne phase?

In the air, weight reads through:

### Trajectory shape
- **Heavy character** — low parabolic arc, doesn't get far
- **Light character** — high arc, floats
- **Athletic character** — efficient arc, controlled

### Body posture in flight
- **Heavy** — body stays compressed, arms close, doesn't extend
- **Light** — body extends fully, arms reach, floats
- **Athletic** — controlled extension or tuck

### Time at apex
- **Heavy** — barely pauses at top
- **Light** — held at apex (feels like hovering)
- **Athletic** — brief apex, then drops

## The impact frame

When the character lands, the IMPACT FRAME is everything.

For a heavy character:
- **Dust puff** at feet on impact
- **Held contact** for 2-3 frames
- **Visible compression** through whole body
- **Ground may indicate cracks or deformation** (cartoon style)
- **Camera may shake** (in video)

For a light character:
- **Clean foot placement**
- **Quick smooth absorption**
- **No held compression**
- **Immediate recovery**

## The recovery / "gathering" phase

After landing, what happens next sells weight:

### Heavy recovery
- **Stays in low position** for several frames
- **Hands may push on ground** to stand
- **Pushes up effortfully**
- **Holds standing pose briefly** to "recover breath"
- **Maybe wipes brow**

### Light recovery
- **Pops back up** with energy
- **Body uses recovery as transition to next action**
- **No held standing pose**

## Specific weight calibrations

### Feather-weight character
- No anticipation crouch (or tiny)
- Quick launch
- High floating arc
- Soft landing, no compression
- Continues motion smoothly

### Standard human jump
- Medium anticipation crouch (0.25s)
- Quick launch (0.1s)
- Medium arc, brief apex
- Standard landing with some absorption
- Smooth recovery

### Heavy character / loaded jump
- Deep prolonged anticipation crouch (0.5s)
- Slow launch effort (0.3s with strain)
- Low arc, doesn't get high
- Heavy landing with deep compression
- Slow effortful recovery

### Stone / boulder / anvil
- No anticipation (can't crouch)
- No active launch (only falls)
- Tight low arc (gravity only)
- Massive impact with deep ground deformation
- Held still after landing (settles into ground)

## Prompt-ready language

### Video model — heavy jump
> "Heavy character jumps. Deep anticipation crouch over 0.5 seconds — visible strain. Pushes off with effort, body extends slowly. Airborne phase is low and brief — only manages a small height. Lands HARD on both feet with one frame of contact before knees deeply absorb the impact. Body compresses dramatically. Dust puff on impact. Stays in low crouch for 1 full second before slowly rising. Body language conveys exhaustion from the effort."

### Video model — light jump
> "Light character springs upward. No visible anticipation, just immediate light push-off. Body extends fully in the air, arms up. Hovers at apex for a moment. Drops gracefully, landing softly with knees barely bending. Immediately flows into next motion without held landing pose."

### Code (heavy jump with weight trick)
```javascript
const heavyJump = gsap.timeline();
heavyJump
 // Deep anticipation
 .to(body, { y: 40, scaleY: 0.6, scaleX: 1.2, duration: 0.5, ease: "power2.out" })
 // Strained push-off (one foot still on ground)
 .to(body, { y: -10, scaleY: 1.1, duration: 0.3, ease: "power2.in" })
 // Airborne (low arc)
 .to(body, { y: -40, duration: 0.2, ease: "power2.out" })
 .to(body, { y: -10, duration: 0.2, ease: "power2.in" })
 // CONTACT frame (no squash yet) — KEY TO WEIGHT
 .to(body, { y: 0, scaleY: 1, duration: 0.04, ease: "none" })
 // THEN squash
 .to(body, { y: 30, scaleY: 0.5, scaleX: 1.4, duration: 0.1, ease: "power4.in" })
 // Held compression
 .to(body, { y: 30, duration: 0.4, ease: "none" })
 // Slow recovery
 .to(body, { y: 0, scaleY: 1, scaleX: 1, duration: 0.6, ease: "power2.inOut" });
```

## Linked concepts

- [[jumps-and-leaps]]
- [[mass-and-motion]]
- [[showing-weight]]
- [[the-bouncing-ball]]
