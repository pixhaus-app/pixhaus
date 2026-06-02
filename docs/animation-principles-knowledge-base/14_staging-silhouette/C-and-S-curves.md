# C-Curves and S-Curves — The Two Most Useful Pose Shapes

Every good standing pose can be reduced to one of two underlying curves: a C-curve or an S-curve. Mastering these gives you a vocabulary for body design that reads instantly.

## The C-curve

A pose where the body forms a single arc.

### Forward C-curve

```
 O ← head forward and down
 /
 /
|
 \
 \
 \
```

Body bent forward. Head down. Sometimes called "cave man" pose.

**Reads as:**
- Sadness, despair
- Vulnerability
- Old age
- Defeat
- Tired
- Hiding / shame
- Thinking deeply
- Pain

### Backward C-curve

```
 O ← head back
 /
 /
 /
 /
 |
 \
 \
```

Body arched back. Head up. Chest open.

**Reads as:**
- Pride, arrogance
- Surprise, awe
- Worship / spiritual
- Defiance
- Looking up at something tall
- Cartoon "frozen in surprise" pose

## The S-curve

A pose with two reversing arcs through the body.

```
 O ← head one way
 |
 / ← shoulders the other
 |
 / ← hips the first way again
 / \
/ \
```

This is the **classical contrapposto** stance — discovered by ancient Greek sculptors as the most natural human pose.

**Reads as:**
- Elegance, grace
- Realism, natural
- Relaxed standing
- Refined character
- Dance, fashion model
- Sexy / sensual (when emphasized)
- Smart, calculated

## Why the S-curve is the default

The human body in real life has natural counter-rotation between shoulders and hips when standing relaxed. The head adds a third axis of tilt. This creates a natural S-curve in any non-frozen standing pose.

If you draw a character standing perfectly straight (head, shoulders, hips all aligned vertically), they look stiff and posed. Add S-curve and they immediately feel alive.

## When to choose each shape

### Use forward C-curve for:
- Sad characters
- Old characters
- Tired/sick characters
- Defensive postures
- Hiding from danger
- Carrying heavy weight (forward lean)
- Thinking pose

### Use backward C-curve for:
- Proud/arrogant characters
- Surprised reactions
- Defiance
- Comedic "frozen in shock"
- Looking at something tall
- Sneeze/laugh recoil

### Use S-curve for:
- Default standing pose
- Walking (S-curve through the stride)
- Casual conversation
- Elegant characters
- Realistic naturalism
- Most "normal" beats

## The S-curve and walking

A walk is built on continuously alternating S-curves. As each foot lands:
- Hips tilt one way
- Shoulders counter-tilt the other way
- Head adds tilt the first direction

The character's spine traces an S through every step. The S reverses direction with each step.

This is what makes walks look organic. Robots walk without S-curves (everything moves as a single block). Humans walk with S-curves (counter-rotation through the spine).

## Combining curves with line of action

The C-curve or S-curve is the "line of action" for the pose. (See `line-of-action.md`.)

Strong poses have one clear curve. Weak poses have competing curves or no curve at all.

When designing a pose:
1. Decide the emotion / intent
2. Choose the appropriate curve (C-forward, C-back, S, or straight diagonal)
3. Build the body parts along that curve
4. Adjust details

The curve comes first. Everything else follows.

## Working with the straight diagonal

Sometimes you want NO curve — a straight diagonal line of action. This is rarer but powerful.

```
\
 \
 \
 O
 \
 \
 \
```

A character whose whole body forms a straight diagonal:

**Reads as:**
- Power, decisive force
- Maximum commitment to an action
- Aggressive striking
- Falling / flying
- Drilling forward

*"Straight lines give power."*

Compare:
- A character leaping with a curved body (S-curve through the leap) → graceful
- A character leaping with a straight diagonal body (rigid arrow shape) → powerful

Use straight diagonals sparingly for hero moments.

## Curves in motion

The shape of the line of action **changes through any motion**:

### Throwing a ball
```
Frame 1: backward C-curve (windup — body coiled back)
Frame 6: straight diagonal (mid-throw — body launching forward)
Frame 12: forward C-curve (follow-through — body collapsing forward)
Frame 18: S-curve (settled — natural standing)
```

The throw is a JOURNEY through curve shapes. Each phase has a different shape.

### Jumping
```
Frame 1: forward C-curve (crouch antic)
Frame 4: straight diagonal (push-off — leg extension)
Frame 8: backward C-curve (apex — body arched, arms up)
Frame 12: straight diagonal (descending)
Frame 16: forward C-curve (landing crouch)
Frame 22: S-curve (settled standing)
```

## Prompt-ready language

### Video model — character in forward C-curve
> "Character in a deep forward C-curve pose. The whole body forms a single curve — head down, shoulders hunched forward, chest collapsed inward, knees slightly bent, weight forward. The body silhouette is a clean C-shape opening backward. Reads as defeated / contemplative."

### Video model — character in backward C-curve
> "Character in a backward C-curve pose. Body arched back, head tilted up, chest pushed forward and out, shoulders pulled back, body forming a single curve. Arms may be back at sides. Reads as proud / defiant."

### Video model — character in S-curve
> "Character standing in a natural S-curve. Head tilted slightly left, shoulders tilted right (lower right shoulder), hips tilted left (higher left hip), weight on right leg. Body has the classic contrapposto stance. Reads as relaxed and elegant."

### Video model — straight diagonal for power
> "Character in a powerful action pose with a straight diagonal line of action. Body forms an arrow shape from back foot through head and into raised arm. No curves. Rigid maximum-commitment pose. Reads as aggressive force."

### Code (curve-based pose presets)
```javascript
const POSE_CURVES = {
 forwardC: {
 headRotation: 25, // tilted forward and down
 chestRotation: -15, // shoulders forward
 spineCurl: -20, // spine curved forward
 hipRotation: 10,
 weight: 'centered',
 },
 backwardC: {
 headRotation: -15,
 chestRotation: 15,
 spineCurl: 20, // spine arched back
 hipRotation: -5,
 weight: 'centered_back',
 },
 sCurve: {
 headRotation: -5,
 chestRotation: -8,
 spineCurl: 0, // no overall curve
 hipRotation: 10, // counter to chest
 weight: 'right_leg', // contrapposto
 },
 diagonal: {
 headRotation: -20,
 chestRotation: -20,
 spineCurl: 0, // straight
 hipRotation: -20,
 weight: 'forward_aggressive',
 },
};

// Apply preset to character
function setPoseCurve(character, curveType) {
 gsap.to(character, { 
 ...POSE_CURVES[curveType], 
 duration: 0.5, 
 ease: "power2.inOut" 
 });
}
```

## Linked concepts

- [[line-of-action]]
- [[silhouette-readability]]
- [[balance-and-counterbalance]]
- [[weight-shift-and-belt-line]]
