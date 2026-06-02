# Balance and Counterbalance — The Lines of Force in a Pose

Every character pose has weight distribution. Balance points must be respected or the character looks like they'll fall over. Counterbalance is when the body opposes one weight with another to maintain that balance.

## The center of gravity

Imagine a vertical line from the top of the character's head straight down. **This line must pass over the supporting feet** for the character to stand without falling.

```
GOOD: balanced character
 O ← head
 | ← center line
 /|\
 |
 / \
 / \ ← feet, with center line passing between them

BAD: character is about to fall
 O ← head leaning out past feet
 \
 \ ← center line falls outside foot base
 \
 |
 / \
 / \ ← feet here, but center line is off to the side
```

Audiences read center-of-gravity intuitively. A pose that violates this looks wrong even if they can't say why.

## When you can violate balance

### Falling (intentionally off-balance)
The body is past its support — gravity is now in control. Use to show:
- Tripping
- Diving
- Stumbling
- Wind blowing character
- Surprise

### Walking (controlled falling)
A walk IS controlled falling. The body briefly tips forward past balance, then the next foot lands and catches it. *"Walking is a process of falling, catching yourself just in time."*

### Leaning into pull/push
Character is pulled or pushed by external force. The lean represents the force vector.

### Counterbalance with an object/limb
The body extends a limb or holds an object to shift the effective center of gravity. This is what makes heavy lifting balanced.

## The counterbalance principle

When a character carries weight in one hand, the body must counterbalance:

```
RIGHT WAY (counterbalanced):
 O ← head leans LEFT
 \
 | 
 /|\___[heavy] ← weight in RIGHT hand
 /
 / \
 / \

The body forms a tilted shape, head and torso lean OPPOSITE to the weight.
Center of gravity stays over the feet.

WRONG WAY (no counterbalance):
 O ← head still upright
 |
 |____[heavy]
 |
 / \

Center of gravity now shifted past the feet on the side of the weight.
Character would topple over.
```

## Counterbalance shapes in common poses

### Carrying a suitcase
- Suitcase in right hand
- Right shoulder pulled down by weight
- Left shoulder rises (counterbalance)
- Left arm swings out for balance
- Head tilts slightly left
- Body's vertical axis tilts to the LEFT slightly (away from suitcase)

### Holding a baby on left hip
- Hip pushed out left to support baby
- Right shoulder lower (relative to baby)
- Right hip rises higher
- Body forms an S-curve
- Center of gravity stays over feet

### Lifting heavy box off ground
- Knees bent
- Back arched BACKWARD (counterbalancing forward weight of box)
- Head tilted back
- Arms below body
- Stance wide

### Standing on one foot
- Other leg extends out behind for balance
- Arms out to sides
- Body angles slightly forward over standing foot
- Eyes fix on horizon (helps balance)

### Pushing something heavy forward
- One leg extends back (brace leg)
- One leg bent forward (drive leg)
- Body leans FORWARD over front foot
- Arms extended forward, pressed against object
- Head down between arms
- Hip drives forward

## The line of action

The whole pose flows along this line.

Good poses have ONE clear line of action. Bad poses have multiple competing lines.

### Examples
- A character reaching up — line of action runs from grounded foot through hip, spine, arm, to fingertips
- A character recoiling — line of action curves backward through body
- A character winding up — line of action twists like a spring

## The S-curve (the most useful pose shape)

A vertical S through the body is the most natural and dynamic standing pose:

```
 O
 |
 /
 \
 |
 /
 / \
```

- Head tilted slightly (top of S)
- Shoulders one direction
- Hips opposite direction
- Knees can be slightly opposite hips
- Weight on one foot, other foot slightly forward

The S-curve is naturally dynamic and shows the body's natural counter-rotation between shoulders and hips. It's the default for any "interesting" standing pose.

## The C-curve (used for emotion)

A character bent forward into a C:

```
 O ← head down
 /
 /
/
|
/ \
```

Shows: sadness, despair, exhaustion, contemplation, prayer.

Or backward into a reverse C:

Shows: surprise, awe, defiance, joy.

## Twin poses (the cardinal sin)

A character with both arms doing the same thing at the same angle is "twinning":

```
 \O/ ← BOTH arms same angle, looks staged
 |
 / \
```

*"Symmetry is the enemy of animation."* Avoid:
- Both arms at same angle
- Both legs perfectly parallel
- Both eyes doing exactly the same thing
- Both sides of the face symmetric

Solution: stagger and offset.

```
 \O/
 | ← left arm slightly forward, right arm back
 / \ ← weight on one leg, other relaxed
```

(See `14_staging-silhouette/avoiding-twins.md` for full breakdown.)

## Prompt-ready language

### Video model — balanced static pose
> "Character stands in a balanced, dynamic pose. Weight on right foot, left foot slightly forward and relaxed. Hips tilted left, shoulders tilted right (counter-rotation through spine — S-curve). Head tilted slightly. One hand on hip, other hanging. No twinning between left and right sides."

### Video model — counterbalance
> "Character carries heavy bucket in right hand. Body counterbalances — head and torso lean LEFT to keep center of gravity over the feet. Right shoulder pulled down by weight. Left arm extends out and back for additional balance. Knees slightly bent for stability."

### Video model — controlled fall
> "Character is pulled forward by external force. Body leans forward dramatically, center of gravity now past the feet. One leg extends back for last-moment balance attempt. Arms windmill forward. About to fall."

### Code (S-curve standing pose)
```javascript
// Establishing an S-curve idle pose
gsap.set(character, {
 // Head slight tilt left
 headRotation: -3,
 // Shoulders tilted right (down on right)
 shoulderRotation: 4,
 // Hips tilted left (opposite to shoulders)
 hipRotation: -5,
 // Weight on right foot
 weightDistribution: { rightFoot: 0.7, leftFoot: 0.3 }
});
```

## Linked concepts

- [[showing-weight]]
- [[pressure-and-force]]
- [[line-of-action]]
- [[avoiding-twins]]
