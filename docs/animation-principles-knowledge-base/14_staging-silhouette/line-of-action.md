# Line of Action — The Energy Curve Through the Body

Every strong pose has a single, clear curve running through it — from head through body to extremities. This is the line of action. It's the energy backbone of the pose.

## The principle

A pose with a clear line of action reads as dynamic and alive. A pose without it reads as static and posed.

The line of action is usually:
- A simple curve (C-curve or S-curve)
- A straight diagonal line
- A spiral

Multiple competing lines = bad pose. One clear line = good pose.

## How to find the line of action

For any pose, trace an imaginary line from the top of the head, through the spine, through the legs (or arms, depending on direction of action), to the extremity.

```
Strong pose with clear line of action:

 O ← head
 / 
 / ← line curves through body
 O 
 | ← spine flows along the curve
 /\ 
 / \ ← line continues into legs
 / \ 
```

The character's whole body flows along one curve. The eye reads this curve as the "shape of the energy."

## The C-curve

A C-shaped line of action:

```
 O ← head
 / 
| ← body bent forward
 \ 
 \ ← knees back
 \
 \
```

C-curves convey:
- Vulnerability (forward C)
- Despair (forward C)
- Aggression (forward C with weight forward)
- Defiance (backward C)
- Awe / surprise (backward C with arms back)

## The reverse C-curve

```
 O ← head back
 / 
 / ← chest open
 / 
 / ← body leaning back
 |
 / \
```

Backward C-curves convey:
- Power, pride
- Surprise, awe
- Defiance
- Comedy (cartoon "tied to a tree" pose)

## The S-curve

A more complex curve with two reversals:

```
 O ← head tilted one direction
 | 
 / ← shoulders one way
 | 
 / ← hips opposite way
 / \ 
/ \ ← legs angled
```

S-curves convey:
- Elegance, grace
- Realism (standard human posing)
- Subtle character (dance, fashion)
- Counter-rotation through the spine

The S-curve is the DEFAULT for any natural standing or walking pose. It's how the human body actually distributes weight.

## The straight line (diagonal)

```
\
 \
 \
 O
 \
 \
 \
```

A straight diagonal line of action is the OPPOSITE of a curve. It conveys:
- Power, force
- Decisive action
- Aggression
- Falling, flying, leaping
- Mechanical / robotic

> Straight lines give power.
For powerful action poses (a leap, a strike, a charge), use a straight diagonal rather than a curve. The curve softens; the straight line accelerates.

## > *"Most of the time, the guide of the action is an arc in the form of a wave or in the form of a figure-eight."*

So natural motion follows curves. But for impactful single poses, you may use straight lines.

The general principle:
- **Curves for natural motion** (walking, gesturing, life)
- **Straight lines for force** (striking, pushing, breaking)

## When the line of action breaks

A pose with no clear line of action looks "stuck." Common causes:

1. **Twinning** — both arms doing the same thing, no through-line
2. **Square body** — head, shoulders, hips, legs all on the same vertical axis
3. **Conflicting curves** — chest curves one way, legs curve the opposite way, with no unifying line
4. **Static stance** — feet wide, body upright, no diagonal energy

## How to design with line of action first

The the classical tradition workflow for any pose:

1. Draw the line of action first (literally a single curved line)
2. Place the major body parts ALONG that line
3. Build out details only after the line is locked

This is how master animators get poses that read instantly. They don't start with the head and add body — they start with the line and add body along it.

## Line of action through motion

When animating, the line of action **changes through the motion**. Track how:

```
Frame 1: forward C-curve (anticipation crouch)
Frame 8: straight diagonal (mid-strike)
Frame 16: backward C-curve (follow-through)
Frame 24: S-curve (settled)
```

The character's "line shape" evolves through the action. This evolution IS the action's drama.

## Prompt-ready language

### Video model — strong line of action
> "Character in a powerful action pose with a single clear line of action. The line runs from the top of the head, through the spine, into the back leg, ending at the rear foot. The whole body flows along this diagonal curve. Arms extend away from the line to emphasize it, not break it."

### Video model — C-curve for despair
> "Character collapsed in despair. The whole body forms a deep forward C-curve — head down, shoulders curled forward, chest collapsed in on itself, knees bent inward. One unified curve from head to floor."

### Video model — S-curve for natural standing
> "Character stands in a relaxed natural pose. S-curve through the body: head tilted slightly left, shoulders tilted right, hips tilted left, weight on right leg. Body has the contrapposto stance of a classical sculpture."

### Video model — straight diagonal for action
> "Character leaping forward with maximum commitment. Line of action is a straight diagonal — from back foot, through the body, to the forward arm. Arrow-like form. No curves, just straight power. Aggressive forward motion."

### Code (line of action as design principle)
For 3D / rigged characters, the line of action principle translates to spine bone alignment:

```javascript
// Configure rig to follow a line of action curve
function setLineOfAction(rig, curveShape) {
 switch(curveShape) {
 case 'forward_C':
 rig.spineBones.forEach((bone, i) => {
 bone.rotation = -10 - i * 5; // Each bone tilts further forward
 });
 break;
 case 'S_curve':
 rig.spineBones.forEach((bone, i) => {
 bone.rotation = Math.sin(i / rig.spineBones.length * Math.PI) * 10;
 });
 break;
 case 'straight_diagonal':
 rig.spineBones.forEach((bone, i) => {
 bone.rotation = 30; // Each bone same angle = straight diagonal
 });
 break;
 }
}
```

## Linked concepts

- [[silhouette-readability]]
- [[C-and-S-curves]]
- [[balance-and-counterbalance]]
- [[avoiding-twins]]
