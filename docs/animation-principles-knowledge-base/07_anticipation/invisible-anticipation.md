# Invisible Anticipation — The 1-2 Frame Trick

Sometimes you don't want the audience to consciously see the anticipation. You want them to feel the "snap" of an action without consciously identifying the windup. This is invisible anticipation — a 1-3 frame opposite-direction motion that's too fast to register but adds enormous life to the snap.

## The principle

the classical tradition explains: even a quick action that *seems* to have no anticipation actually benefits from a hidden one.

```
Character looks left → eye darts left (instant snap)
```

vs.

```
Character looks left → 
 Frame 1-2: eye drifts SLIGHTLY RIGHT (invisible antic)
 Frame 3-4: eye snaps left, much further
```

The audience reads both as "eye snap left." But the second version feels alive. The first feels mechanical.

## When to use invisible antic

- **Eye darts** — head turns, glances
- **Sudden head movements** — startle, alert
- **Small precise gestures** — finger snaps, hand opens
- **Cartoon "snaps"** — any quick pose change
- **Acting tics** — character makes a small move that feels "natural"

## How long is invisible?

| Frames at 24fps | Visible? | Use |
|-----------------|----------|-----|
| 1 frame | Invisible — but felt | Eye dart, finger snap |
| 2 frames | Almost invisible | Most invisible antics |
| 3 frames | Subtly visible | Detailed character work |
| 4+ frames | Visible | Now it's a regular antic |

The threshold of visibility is around 2 frames. At 1-2 frames the brain doesn't consciously register the opposite-direction motion but DOES process it as "this motion has weight and intention."

## s

### The eye dart
A character about to look at something on the right:

```
Frame 1-2: pupil shifts SLIGHTLY LEFT (1-2 frames only)
Frame 3-4: pupil zips right to target
Frame 5+: held looking right
```

The 1-2 frame leftward drift is the invisible antic. Audience doesn't see it consciously but feels the eye "load" before snapping.

### The football juggle
A footballer pretending to kick a ball uses an exaggerated circular motion of the foot — a "florid" antic.

```
Frame 1-4: leg circles dramatically (visible antic — distracts)
Frame 5: foot strikes ball
Frame 6+: ball flies opposite direction
```

The circular motion is itself the anticipation of the strike. Without it, the kick looks robotic.

### The baseball catcher's throw

```
Catch the ball (settled position)
Frames 1-2: tiny pullback in opposite direction (invisible antic)
Frames 3+: throw action
```

The invisible antic adds 10x snap to the throw motion.

## Why this is different from a regular antic

A regular antic is **visible** — the audience sees it and predicts the action.

An invisible antic is **felt** — the audience doesn't consciously see it but it gives the action weight and snap.

You use invisible antic when:
- The character should *appear* to react instantly
- You want a "snap" feel without slowing the motion
- The action is so quick you can't afford 8+ frames of visible antic
- You want the *feeling* of weight without committing screen time

## The "snap" feel

*"This is what gives it the snap."*

A snap is when an action goes from rest to full speed in a tiny number of frames. Without invisible antic, snaps look mechanical (linear). With invisible antic, snaps look alive (charged).

The difference:
```
MECHANICAL SNAP (linear):
Frame 1: at rest position
Frame 2: 25% of way
Frame 3: 50%
Frame 4: 100%, held

ALIVE SNAP (with invisible antic):
Frame 1: at rest position
Frame 2: slight movement OPPOSITE direction (1 frame antic)
Frame 3: 75% of way to target
Frame 4: 100%, held with tiny overshoot
```

## Combining with regular antics

For maximum effect, layer both:

```
1. VISIBLE ANTIC (8-12 frames, opposite direction)
2. HELD POSE (2-4 frames)
3. INVISIBLE ANTIC (1-2 frames, opposite to action direction again — even further opposite)
4. ACTION (4-8 frames, fast)
5. SETTLE
```

This double-antic structure is what masters use for hero shots — punches, throws, dramatic moments. The micro-prep at the last instant before action adds extra "load."

## Prompt-ready language

### Video model — invisible antic in eye dart
> "Character's eyes dart to look at the suspicious object. The pupil briefly drifts a tiny amount in the OPPOSITE direction (1-2 frames, almost imperceptible) before snapping to the target. This subtle pre-motion gives the eye dart a sense of intention and weight."

### Video model — invisible antic in head turn
> "Character snaps head to look at the door. Just before the snap, the head tilts a fraction OPPOSITE the direction it will turn — only 1-2 frames of pre-motion. Then the head turns fast and decisively. The pre-motion is invisible to viewers but makes the turn feel snappy and intentional."

### Code (invisible antic in code)
```javascript
const headSnap = gsap.timeline();
headSnap
 // INVISIBLE ANTIC: 1 frame opposite direction (1/24 second)
 .to(head, { 
 rotation: -3, // tiny opposite movement
 duration: 0.04, // 1 frame at 24fps
 ease: "none"
 })
 // SNAP TO TARGET
 .to(head, { 
 rotation: 45, // main direction
 duration: 0.10, // ~2-3 frames
 ease: "power3.out"
 })
 // SETTLE
 .to(head, { 
 rotation: 42, 
 duration: 0.15, 
 ease: "back.out(2)" 
 });
```

### Image keyframes
For a snap action with invisible antic, generate:
1. Rest pose
2. **Invisible antic pose** (1-2 frames worth, just a tiny shift opposite) — this is YOUR HIDDEN KEYFRAME
3. Action pose

If only generating 2 keyframes, the invisible antic disappears into interpolation. Generating it explicitly preserves the snap.

## A test you can use

Animate a head snap two ways:
1. Direct: from rest to look-target
2. With 1-2 frames of invisible antic in opposite direction first

Watch both at full speed. The second feels alive. The first feels stiff. You won't consciously see the antic but you'll FEEL the difference.

## Linked concepts

- [[basic-anticipation]]
- [[surprise-anticipation]]
- [[hard-accent-bounces]]
- [[the-AAR-formula]]
