# Jumps, Leaps, and Hops

the classical tradition breaks airborne motion into distinct types. Each has a specific shape and rhythm.

## Definitions

- **Jump** — both feet leave the ground, both feet land. Vertical or horizontal.
- **Leap** — like a jump but one foot leads, one trails. Bigger horizontal distance.
- **Hop** — one-footed jump. Land on same foot or alternate.
- **Skip** — step-hop-step-hop rhythm. Light, playful.

## The five phases of every jump

Every jump, leap, or hop has five distinct phases. **You need at least 5 unique drawings to make a jump read.**

| Phase | Pose | What's happening |
|-------|------|------------------|
| 1 | **Anticipation crouch** | Knees bend, body drops, weight loads |
| 2 | **Push-off / launch** | Legs extend explosively, body launches |
| 3 | **Airborne / apex** | Both feet off ground, body extended or tucked |
| 4 | **Landing contact** | Feet hit ground, knees absorb |
| 5 | **Recovery / settle** | Body returns to standing, follow-through |

Without all five, the jump doesn't feel complete. Skip the anticipation and the jump feels weightless. Skip the landing recovery and the jump feels glued.

## 5 drawings minimum

> We need five drawings to take a figure off the screen, on screen, or across the screen.
This applies broadly: any complete action needs at least 5 distinct stages. Anything fewer feels truncated.

## Timing for different jump heights

### Short hop (in place)
- Total: 12-18 frames
- Anticipation: 4 frames
- Airborne: 4 frames 
- Landing: 4 frames
- Recovery: 4 frames

### Medium jump
- Total: 24-36 frames
- Anticipation: 6-8 frames
- Push-off: 2 frames
- Airborne: 8-12 frames
- Landing: 4 frames
- Recovery: 6-10 frames

### Big leap (across a chasm)
- Total: 36-60 frames
- Anticipation: 8-12 frames (deep crouch)
- Push-off: 2-3 frames (explosive)
- Airborne: 12-24 frames (long flight)
- Landing: 4-6 frames
- Recovery: 10-15 frames (gather balance)

### Cartoon "jump up in surprise"
- Total: 12 frames
- Anticipation: 1 frame (barely)
- Launch: 1 frame
- Airborne (high): 4-6 frames
- Landing: 1 frame
- Recovery: 3 frames

## The anticipation crouch is the soul of the jump

This is the most important phase. The deeper and longer the crouch, the bigger the jump reads.

- **Tiny crouch** → small jump (it would be silly to crouch deep for a small hop)
- **Medium crouch** → standard jump
- **Deep crouch** → huge jump (you're loading a lot of energy)
- **Held deep crouch** → comedy super-jump

The anticipation tells the audience how big the jump will be **before it happens**.

## What happens in the air

### Body shape at apex
- **Extended (full stretch)** — fearless, leaping, athletic
- **Tucked** — graceful, controlled, gymnastic
- **Splayed (limbs flailing)** — surprise jump, scared, comedic
- **Spinning** — flip, gymnastic, action

### Forward motion
A standing jump goes straight up and down. A leap has forward momentum throughout. **The body in the air follows a parabolic arc** — symmetric in time about the apex.

### What stays the same
**The hands lead the motion in the air.** If reaching forward, hands lead the body. If preparing to land, hands come down and forward.

## Landing physics

The landing absorbs impact:
- **Knees bend** to lower the body
- **Body squashes** (cartoon: extreme; realistic: subtle)
- **Arms come forward** for balance
- **Head bobs down** with impact
- **Then recovers up to standing**

For heavy impact (big jump):
- **Held landing pose** — knees bent for several frames
- **Dust puff** on impact
- **Brief stunned moment** before standing up
- **Visible weight** through bent posture

For light landing (small hop):
- **Quick absorption** then immediate stand
- **No held pose**
- **Smooth continuous motion**

## Hops and skips

A hop is a single-foot jump. A series of hops, alternating feet, with a slight gap between them, is a skip.

```
Step-hop-step-hop-step-hop
left-left-right-right-left-left-right-right
```

the skip formula (from his "old lady running and skipping" example):
- 16-frame skip cycle on ones
- Each step plant takes about 8 frames
- Each hop airborne phase takes about 6 frames
- Arms swing wide for balance
- Head moves in a smooth circle through the cycle

## Specific the classical tradition examples

### The boxer's jump rope
> A boxer jumping rope barely lifts off the ground, almost no motion. Two small bounces per foot, very fluid.
This is a *low* jump cycle — almost a controlled bob. The lift is minimal but the timing is precise.

### The kid skipping rope
> A girl skipping rope uses a double bounce, with very defined accents.
The double-bounce: instead of one push per turn of the rope, two small bobs. This creates a playful rhythm.

## Prompt-ready language

### Video model — standing jump
> "Character jumps straight up. Anticipation: deep crouch with arms back over 6 frames (0.25s). Push-off: explosive extension over 2 frames. Airborne: body extends with arms reaching up, 8 frames at apex (0.33s). Landing: feet plant, knees absorb, body compresses for 3 frames. Recovery: stand up smoothly over 5 frames."

### Video model — running leap
> "Character runs and leaps across gap. Three steps of running, then dramatic launch off back leg. Airborne phase 12 frames at 24fps, body fully extended like a Superman pose, arms forward. Lands on lead foot, rolls into running motion."

### Code
```javascript
const jumpUp = gsap.timeline();
jumpUp
 // Anticipation crouch
 .to(body, { y: 30, scaleY: 0.7, duration: 0.25, ease: "power2.out" })
 // Push-off (explosive)
 .to(body, { y: -50, scaleY: 1.3, duration: 0.08, ease: "power4.out" })
 // Airborne (apex hold)
 .to(body, { y: -80, scaleY: 1, duration: 0.20, ease: "power2.out" })
 .to(body, { y: -50, duration: 0.20, ease: "power2.in" }) // fall start
 // Landing
 .to(body, { y: 30, scaleY: 0.6, scaleX: 1.4, duration: 0.10, ease: "power4.in" }) // squash
 // Recovery
 .to(body, { y: 0, scaleY: 1, scaleX: 1, duration: 0.25, ease: "elastic.out(1, 0.4)" });
```

### Image keyframes — generate these 5 poses

1. **Anticipation crouch** — knees deeply bent, arms drawn back, head down
2. **Mid-launch** — legs extending, body stretching upward, arms swinging up
3. **Apex** — body fully extended in air, arms up, body weightless
4. **Impact landing** — knees deeply bent, body squashed down, arms forward for balance
5. **Recovery** — standing back up, slight follow-through

## Linked concepts

- [[run-cycle-fundamentals]]
- [[weight-in-a-jump]]
- [[anticipation]]
- [[the-AAR-formula]]
