# Counter-Reaction — How the Upper Body Resists the Lower Body

the classical tradition calls this "the secret of weight." When the lower body moves one way, the upper body resists for a few frames before catching up. This counter-reaction is what makes a walk look alive and weighted.

## The principle

Mass has inertia. When the lower body (legs/hips) accelerates in a direction, the upper body (chest/head/arms) **lags behind for a moment** before being dragged along.

```
Frame 1: hips start to move forward
Frame 2: chest still stationary (inertia)
Frame 3: chest finally responds, starts following
Frame 4: head still lagging
Frame 5: everything catches up
```

This is most visible in:
- Starting from rest (hips lead, chest follows)
- Stopping suddenly (chest continues, hips stopped)
- Changing direction
- Bumpy walks (head bobs from inertia)

## In a walk specifically

Each step is a small "start and stop" cycle for the body. With every footfall:
- **Hip drops** (weight on the bent leg)
- **Chest still falling** (mass continuing down)
- **Head bobs last** (heaviest, slowest to react)

This creates the head-bob signature of a heavy walk.

## The "drag" in fat / weight / fabric

Anything with mass attached to a moving body shows counter-reaction:

- **Belly** — lags behind the chest when the chest moves forward
- **Buttocks** — bounce on the down phase of each step
- **Breasts** — counter-react with each step
- **Cheeks / jowls** — wobble during sudden movements
- **Loose clothing** — drags behind the body
- **Hair** — follows the head with a delay
- **Tail** (animal) — drags behind the body

## The general formula

> **More mass = more counter-reaction = more delay frames**

| Body part | Lag (frames at 24fps) |
|-----------|----------------------|
| Solid head (small movement) | 1-2 |
| Hair / loose scalp | 3-5 |
| Cheeks / jowls (jiggly) | 4-8 |
| Belly / loose abdominal | 4-6 |
| Buttocks during step bounce | 2-4 |
| Loose clothing (shirt, dress) | 3-6 |
| Heavy coat | 4-8 |
| Long hair (ponytail) | 6-10 |
| Tail | 4-8 |

## How to prompt counter-reaction

### Video models
> "Heavy walk with visible inertia — character's chest and head lag slightly behind the hip motion. Belly and clothing have small follow-through wobble on each step. Long hair drags behind head motion with a 4-frame delay."

> "Sudden stop — hips and legs come to a halt, but the upper body continues forward momentum for a beat before snapping back. Hair and clothing overshoot, then settle."

### Code

```javascript
// Body and head walk timeline
const walkBody = gsap.timeline({ repeat: -1 });
const walkHead = gsap.timeline({ repeat: -1 });

walkBody.to(body, { y: 10, duration: 0.25, ease: "power2.in" }) // down
 .to(body, { y: 0, duration: 0.25, ease: "power2.out" }); // up

// Head lags by 2 frames (about 0.08s)
walkHead.to(head, { y: 10, duration: 0.25, ease: "power2.in", delay: 0.08 })
 .to(head, { y: 0, duration: 0.25, ease: "power2.out" });

// Hair lags further, with overshoot
walkHair.to(hair, { y: 14, duration: 0.30, ease: "power2.in", delay: 0.16 }) // overshoot
 .to(hair, { y: 0, duration: 0.40, ease: "elastic.out(1, 0.5)" });
```

### Image keyframes
For a single dramatic moment (e.g., sudden stop):

1. **Frame 1:** body stopped, but arms/hair still mid-swing forward
2. **Frame 2:** arms/hair at maximum forward overshoot
3. **Frame 3:** arms/hair settling back

These are extreme poses showing the overshoot moment.

## Counter-reaction on direction change

A character running, suddenly stopping and turning around:

```
Frame 1: body still moving forward at full speed
Frame 2: feet plant, body decelerating
Frame 3: feet stopped, chest still leaning forward
Frame 4: chest catches up, hips begin to twist
Frame 5: upper body rotates to new direction
Frame 6: head finally turns to face new direction
Frame 7: arms still swinging through from old direction
Frame 8: full new pose established
```

This is why fast cartoon turns look chaotic and alive — many parts are simultaneously catching up, each with its own timing.

## s

### The drunk walk
The drunk's lack of central control means each body part moves independently. The hips might lurch right while the chest is still going left. The head wobbles separately from the chest. The hands are doing their own thing entirely.

This is **maximum counter-reaction** — no part is in sync with the rest.

### The marching walk
A trained soldier marching has **minimum counter-reaction** — every part moves in sync. This is what makes military marches look mechanical and rigid. It's deliberately suppressing the body's natural counter-reaction.

### The dejected walk
Slow timing with extra counter-reaction in the head and shoulders. The body moves forward listlessly, head dragging behind. Each step has visible "weight" because the upper body is always lagging.

## How to spot a walk with bad counter-reaction

A walk without counter-reaction looks:
- Robotic
- Stiff
- Like a paper cutout
- Like a doll

The fix: introduce delay in the secondary parts. Head lags behind body by 1-2 frames. Hair lags 3-5 frames. Clothing lags 4-6 frames.

## The "broken into parts" approach

*"Delay parts. Don't do all the work at once."*

When animating a character, treat each major body section as having its own timeline:

```
Hips: [moves]
Chest: [moves, 1-2 frames after hips]
Head: [moves, 2-3 frames after chest]
Hair: [moves, 3-5 frames after head]
Hands: [move, 1-2 frames after arms]
Clothes: [move, 3-6 frames after body]
```

This staggered timing IS counter-reaction in practice. Every animation system (GSAP, After Effects, traditional X-sheets) supports it through delay or stagger functions.

## Linked concepts

- [[arm-swing]]
- [[simple-overlap]]
- [[breaking-joints]]
- [[mass-and-motion]]
