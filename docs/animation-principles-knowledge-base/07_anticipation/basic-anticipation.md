# Basic Anticipation — The Prep Move Before Every Action

> There are only three things in animation: Anticipation, Action, Reaction. Everything else flows from these.
Every meaningful action in animation has three parts. Anticipation is the preparation — the windup, the prep, the breath before the action. Skip it and the action looks dead.

## What anticipation does

Anticipation **tells the audience what's about to happen**. It gives them time to read the intention. They anticipate WITH the character — they're not surprised when the action lands; they're satisfied.

> Tell them what you're going to do. Do it. Tell them what you did.
This three-step structure is the core rhythm of animation:
1. Anticipation (windup)
2. Action (the move)
3. Reaction / settle (the response, follow-through)

## The fundamental rule

**Anticipation always goes in the OPPOSITE direction of the main action.**

| Action direction | Anticipation direction |
|------------------|------------------------|
| Throw forward | Pull back |
| Jump up | Crouch down |
| Run left | Sway right |
| Sit down | Slight rise |
| Stand up | Slight dip |
| Look right | Slight glance left |
| Punch forward | Pull arm back |

- "We go back before we go forward"
- "We go forward before we go back"
- "We go down before we go up"
- "We go up before we go down"

> **"Before going somewhere, first go the other way."**

## Why does this work physiologically?

It's how real bodies move. Throwing a ball, you pull your arm back to load energy. Jumping, you crouch to load the legs. Even subtle actions — sitting down — you tilt back slightly before lowering yourself. The body is always pre-loading the opposing muscle group.

When animation skips anticipation, the action looks "rotoscope wrong" — like an alien suddenly snapping a limb into a new position. No life.

## Anticipation timing

Anticipation is usually **slower than the action itself**. The pattern is:

```
SLOW ANTICIPATION → ZIP! FAST ACTION → SETTLE
```

Frame counts (at 24fps):
- **Fast cartoon antic:** 4-8 frames
- **Standard antic:** 8-16 frames 
- **Heavy / loaded antic:** 16-30 frames
- **Comic exaggerated antic:** 24-60 frames (held for laughs)

The action itself is typically faster:
- **Fast action:** 2-4 frames
- **Standard action:** 4-8 frames
- **Big swung action:** 8-16 frames

## Examples by action type

### Throwing a ball
1. **Antic:** weight shifts to back foot, arm pulls back, body rotates back
2. **Action:** weight shifts forward, body rotates, arm swings forward in arc
3. **Reaction:** arm follows through, body settles, head looks at thrown object

### Punching
1. **Antic:** weight shifts back, fist drawn back, shoulder cocked
2. **Action:** body rotates, weight transfers, fist drives forward (note: actual contact is just 1-2 frames)
3. **Reaction:** body continues forward rotation slightly, fist recoils, settle

### Note (the a classical animation pioneer tip on punches):
> Never show the hand contacting the cheek. Show the hand AFTER it has passed the cheek and the cheek is moving.
Skip the contact frame, show only the aftermath. It reads as 10x more impactful.

### Jumping
1. **Antic:** body crouches down, weight loads in legs
2. **Action:** legs extend explosively, body launches up
3. **Reaction:** at apex, slight pause, then fall with squash on landing, settle

### Sitting down
1. **Antic:** small upward shift, eye contact with chair
2. **Action:** body lowers backward toward chair
3. **Reaction:** settle into chair, slight bounce/adjust

### Standing up
1. **Antic:** slight forward lean, weight shifts to one leg
2. **Action:** push up with legs, body extends
3. **Reaction:** find balance, settle into standing pose

### Walking from rest
the specific case: how do you start walking?

The naïve approach: lead with the foot in the direction of travel.

**WRONG** (going left, leading with left foot): looks artificial.

The correct approach: lead with the foot opposite to the direction of travel.

**RIGHT** (going left, leading with right foot): natural.

Or even better: **anticipate by retracting the foot first.**

```
ANTIC: right foot pulls back slightly
ACTION: right foot swings forward into first step (going left)
```

This tiny pull-back is invisible to conscious viewers but reads as natural starting motion.

## Marcel Marceau's rule

> Use big anticipation.
Mimes communicate everything through anticipation. They can't speak; the body must tell the story. Their anticipations are enormous — exaggerated to absurdity — and the audience reads them perfectly.

In animation, you can adjust the size of the anticipation based on:
- **Style** (cartoon = big antic; realistic = subtle antic)
- **Character** (theatrical = big; reserved = small)
- **Stakes** (big action = big antic; small action = small antic)

## The danger of over-using anticipation

*"The only problem with anticipation can be its wear: 'I already know, you see, now he's going to do this... how boring.'"*

Too much obvious anticipation telegraphs every action. Audiences get bored.

The solution: **the surprise anticipation** — pretend to wind up for one action, then do something else entirely. The audience expects action A based on the antic; they get action B. Funny or shocking.

## Anticipation in different parts of the body

The body has multiple antic locations:

- **Body antic:** whole body shifts before main action
- **Limb antic:** arm or leg prepares
- **Head antic:** head shifts before turning
- **Eye antic:** eyes look toward target before head turns
- **Breath antic:** chest inhales before action (especially in dialogue)

You can stack antics: eyes lead, then head, then body. Each is a few frames offset from the last. This creates layered, alive motion.

## the "punching with intent" example

The body antic for a punch:

| Frame | Body | Arm | Eyes |
|-------|------|-----|------|
| 1-6 | Slowly cocking back, weight shifting | Pulling back | Looking at target |
| 7-8 | Held at full antic position | Held cocked | Locked on target |
| 9-11 | Body rotating forward | Arm shooting forward | Tracking |
| 12 | Body in extended pose | Arm extended past target | (Contact frame — skip!) |
| 13-14 | Body settling | Arm recoiling | (post-impact) |

Notice the antic takes 8 frames, the action takes 4. **Antic is roughly 2x the action time** for most strikes.

## Prompt-ready language

### Video model — anticipation in a throw
> "Character throws a ball. Anticipation phase (12 frames, 0.5s): weight shifts to back foot, body rotates back, arm pulls fully back behind shoulder. Brief held pose at peak windup. Action phase (4 frames): explosive forward rotation, weight transfers, arm swings forward in fast arc. Reaction phase (8 frames): arm follows through to fully extended, body settles into balance, head tracks the ball."

### Video model — anticipation in a jump
> "Character anticipates a jump. Slowly crouches down over 0.5 seconds — knees bend deeply, body lowers, arms swing back. Brief hold at deep crouch. Then explosive launch: legs extend, body shoots upward, arms swing forward. The crouch is twice as long as the launch — anticipation should be deliberate and slow."

### Code — anticipation pattern
```javascript
const actionWithAntic = gsap.timeline();
actionWithAntic
 // ANTIC: slow opposite-direction prep
 .to(character, { 
 x: -30, // opposite direction
 duration: 0.5, 
 ease: "power2.out" 
 })
 // HELD ANTIC POSE
 .to(character, { duration: 0.1 }) // dead hold
 // FAST ACTION
 .to(character, { 
 x: 100, // main direction, much further
 duration: 0.15, 
 ease: "power4.in" 
 })
 // REACTION / SETTLE
 .to(character, { 
 x: 90, // small recoil
 duration: 0.3, 
 ease: "elastic.out(1, 0.4)" 
 });
```

## Linked concepts

- [[the-AAR-formula]]
- [[surprise-anticipation]]
- [[invisible-anticipation]]
- [[takes-and-accents]]
