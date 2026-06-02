# The Standard Walk ' Four-Pose Foundation

> Walking is the first thing to learn. Learn walks of every kind, because walking is almost the hardest thing to do well.
A walk is a series of controlled falls. The body leans forward, a leg extends to catch the fall, lands, takes the weight, pushes off, and the next leg catches. Step, catch, step, catch.

## The four key poses of a single step

Master these and you can build any variation.

### 1. CONTACT
The moment a foot first hits the ground. Front leg extended, back leg extended behind. Body level: slightly higher than the midpoint of the up/down cycle. Arms in opposition to legs (right arm forward when left leg is forward).

### 2. DOWN (Recoil)
Just after contact. Body weight transfers onto the front leg, knee bends, body drops to its lowest point. This is where weight reads — the knee absorbing impact.

### 3. PASSING (Middle / Breakdown)
The free leg passes the standing leg. Standing leg is straight, body at its highest point. This is the "breakdown" of the walk — the pose that makes the walk personality-specific.

### 4. UP
The standing leg straightens fully and pushes the body up to its highest point. The free leg swings forward into the next contact. Body and head reach their peak.

## The pose sequence

```
CONTACT → DOWN → PASSING → UP → CONTACT (next step)
 1 2 3 4 5
```

In a 12-frame step (the default natural walk):
- Frame 1: Contact
- Frame 4: Down
- Frame 7: Passing
- Frame 10: Up
- Frame 13: Contact (next step)

## What rises and falls

The body and head describe a wave going up and down. In a standard walk:

- **DOWN** is the lowest point (knee bent)
- **PASSING** is roughly the middle (or slightly above)
- **UP** is the highest point (pushed up by straightened leg)
- **CONTACT** is between UP and DOWN (descending again)

**This up-and-down of the head/body is what reads as weight.** No up-and-down = no weight (see the "sliding tightrope" walk ## Arms in opposition

In a normal walk, arms swing opposite to legs:
- Right leg forward → left arm forward
- Left leg forward → right arm forward

The peak of the arm swing happens slightly **after** the contact — at the DOWN pose, not at CONTACT. This is one of those "the body does what it does, not what we'd prefer" rules.

## The two methods of planning a walk

### Method 1 — Contact Method (the default)
Start by drawing the two CONTACT poses (one per step). Then add PASSING poses. Then DOWN and UP. Then inbetweens.

**Why use it:** the contacts are dynamic, moving poses. They give you the silhouette and intent. Easy to plan around.

It's the recommended starting method.

### Method 2 — Down Pose Method (a classical animation pioneer)
Start with the two DOWN poses (knees bent, body low). Then PASSING. Then UP. Then CONTACT.

**Why use it:** the down poses already contain the weight. You've solved the up/down before you've planned anything else. Better for inventive, non-realistic walks (Goofy walks, exaggerated comedy walks).

### use both
Use Contact Method for naturalistic walks. Use Down Pose Method when inventing weird, character-specific walks.

## The lean — controlled falling

Walking is falling forward and catching yourself. The body leans into the direction of travel.

- **Slow walk:** small lean, body stays nearly upright
- **Fast walk:** large lean, body angled significantly forward
- **Run:** very large lean, body angled dramatically forward

The lean matches the speed. A character moving fast without leaning forward looks wrong (sliding, not walking).

## Why you can't just rotoscope a walk

The audience reads them as wrong. Reason: real walks have less up/down than feels right on screen. **You must exaggerate the up/down** to communicate weight.

> "It's the up-and-down of the mass that conveys the sensation of weight."

## Up/down amount by walk type

- **Mr. Macho swagger:** large up/down, legs wide apart
- **Feminine glide:** minimal up/down, legs close together, foot crosses over
- **Heavy character:** medium up/down but slow timing, deep down pose
- **Child:** large up/down, exaggerated leg lift
- **Sneak:** held passing, almost no down
- **Drunk:** unpredictable up/down, uneven step length

## Standard walk in different timings

A character walking at different speeds is still "the standard walk" — just timed differently.

| Frames per step | Steps per second | Feel |
|-----------------|------------------|------|
| 8 | 3 | Cartoon walk, brisk |
| 12 | 2 | Natural walk (the default) |
| 16 | 1.5 | Leisurely stroll |
| 20 | 1.2 | Older or tired |
| 24 | 1 | Slow / heavy |
| 32 | 0.75 | "Show me the way home" |

## Prompt-ready language for a standard walk

### Video model
> "Character performs a standard walk cycle, 12 frames per step at 24fps. Each step follows the sequence: contact (front leg extended, weight just landing), down (knee bends to absorb), passing (free leg swings past, body at highest), up (push-off, body fully extended), then next contact. Arms swing in opposition to legs. Head and body bob up and down with each step."

### Image keyframe
Generate 4 poses per step:
1. CONTACT pose
2. DOWN pose
3. PASSING pose
4. UP pose

These are the extremes. Interpolation fills the rest.

### Code (GSAP)
```javascript
const walkStep = gsap.timeline({ repeat: -1 });
walkStep
 .to(body, { y: 0, duration: 0.13, ease: "power2.in" }) // contact to down
 .to(body, { y: -5, duration: 0.13, ease: "power2.out" }) // down to passing (start rising)
 .to(body, { y: -10, duration: 0.13, ease: "power2.out" }) // passing to up (peak)
 .to(body, { y: -2, duration: 0.13, ease: "power2.in" }) // up to next contact
;
```

## Linked concepts

- [[walk-timing-chart]] — Full frame-count reference
- [[weight-shift-and-belt-line]] — The body sells the walk
- [[arm-swing]] — Detailed arm motion
- [[counter-reaction]] — Upper body opposing lower body
