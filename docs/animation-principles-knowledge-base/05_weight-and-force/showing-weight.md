# Showing Weight — How Weight Reads on Screen

> How did you make the tiger in The Jungle Book feel so heavy?
> Well, on every drawing I knew where the weight was. I knew where the weight was at every given moment in the character. I knew where it was and where it was coming from, knew where it was moving through and where it was going.
Weight isn't drawn on the character. It's *placed* — the animator decides where the weight lives in each pose. The drawing then reflects that decision.

## The fundamental insight

In every pose, ask: **where is the weight?**

- On the front leg? On the back leg? Split?
- In the chest? In the hips?
- On the held object? On the body?

A character's silhouette can be identical between two animators — but if one of them knew where the weight was in every frame, the animation feels alive. The other looks like a puppet.

## How to make a character look like they're lifting something heavy

### The anticipation/preparation
Before lifting, a character must **prepare** for the weight. This anticipation is what sells the weight.

1. **Visual inspection** — character looks at the object, considers it
2. **Repositions feet** — wider stance, planted base
3. **Bends knees** — gets low to the object
4. **Hands approach the object carefully**
5. **Grips** — visible effort in the hands
6. **Spine arches backward** to counterbalance the weight

Skip the anticipation and the lift looks like the object is made of styrofoam.

### The lift itself
1. **Legs straighten** to push up
2. **Back arches further** initially (more counterbalance)
3. **Then the spine reverses** — body straightens up to vertical
4. **Knees may shake** with effort
5. **Head positions strategically** to maintain balance
6. **Brief held strain** at the top — the character is fighting the weight

### What changes about the body
- **Shoulders rise** under heavy weight (involuntary tension)
- **Neck shortens** (head pulled down by weight on shoulders)
- **Knees stay bent** until secure
- **Feet may shuffle** in small uneven steps to find balance
- **Facial expression** — strain, grimace, gritted teeth

## Comparison table — the same character lifting different objects

| Object | Anticipation | Body during lift | Recovery |
|--------|-------------|------------------|----------|
| Feather | None | No body change | None |
| Coffee mug | Brief glance | Arm only | None |
| Cinder block | Wider stance, brief crouch | Arm + slight body bend | Brief pause |
| Heavy rock | Deep crouch, visible inspection | Whole body involved | Held strain |
| Anvil | Long preparation, big stance | Maximum body strain, knees shaking | Long held strain or collapse |

**The drawing of the object doesn't change.** What changes is the character's body language. Same hand, different objects, totally different weight reads.

## Specific weight examples

### Hand picking up a silk scarf (no weight)
- No body involvement at all
- Arm reaches casually
- Fingers pinch lightly
- No change in posture
- Continues smoothly

### Hand picking up a brick (moderate weight)
- Slight body shift toward the brick
- Arm extends, knees bend slightly
- Grip closes deliberately
- Arm pulls up — the other arm may counterbalance
- Head angle may oppose the brick (counterweight)
- Body settles into a new balanced pose

### Picking up a 50-pound rock
- Stance widens before reaching
- Deep knee bend, body lowered to rock
- Both hands grip
- Back arches dramatically backward (counterbalance)
- Legs push to straighten
- Body rises slowly with the rock
- Held standing pose with visible strain
- Knees still slightly bent
- Body shaking

### Carrying a sack of potatoes
- Body bent forward, sack on back
- Knees stay bent throughout walk
- Feet apart (tripod stance for stability)
- Feet drag rather than lift
- Slow walk timing
- Shoulders drop
- Head and neck angle down
- Step length is short and uneven (the "pause, step, step, pause" rhythm)

## Pressing and pushing weight

The same principles for pushing as lifting:

### Pressing on something soft (cloth, water)
- Hand makes contact
- Fingers may curl into the soft surface
- No body weight needed
- Minimal whole-body involvement

### Pressing on something hard and resistant (a stuck door)
- Whole body involves
- Lean into the door with full body weight
- One foot back as brace
- Arms extend then arms bend (pushing harder)
- Head pushes forward
- Feet may slide if force exceeds friction

### Pressing on something heavy (a boulder)
- Lower body squat
- Whole body acts as a wedge
- Pushing through the legs, not arms
- Slow incremental motion
- Visible effort in face

## Source of the action by weight

the key technique: **find where the action comes from** in the body.

For a pencil pick-up: action comes from the **elbow** (forearm only).
For a brick pick-up: action comes from the **shoulder** (whole arm).
For a heavy box: action comes from the **hip** (whole body involved).

This sounds technical but it's about thinking *which joint is the prime mover.* The heavier the object, the deeper into the body the action originates.

## Prompt-ready language

### Video model — light pickup
> "Character casually picks up the feather with one hand. No anticipation, just a smooth reach and grasp. Body stays in normal standing pose throughout. No visible effort."

### Video model — heavy pickup
> "Character prepares to lift the heavy crate. First, examines it from multiple angles for 2 seconds. Then widens stance, bends knees deeply. Grips the crate firmly. Back arches backward in counterbalance. Slowly straightens legs while keeping spine arched. As the crate reaches knee height, spine begins to reverse arch — character now standing more upright. Knees tremble visibly. Held strain pose for 1 second at top. Body language conveys serious weight."

### Video model — carrying heavy load
> "Character walks while carrying heavy sack on shoulder. Body bent forward to counterbalance. Knees stay bent through entire walk. Feet wider than normal. Step timing erratic — small step, pause, step, step, pause. Shoulders dropped. Slight wobble suggests barely-managed weight."

### Code — heavy lift
```javascript
const heavyLift = gsap.timeline();
heavyLift
 // Anticipation: stance widening + knee bend
 .to(character, { scaleX: 1.1, y: 30, duration: 0.8, ease: "power2.inOut" })
 // Reaching for object
 .to(arms, { y: 50, duration: 0.4, ease: "power2.out" })
 // Grip moment (held)
 .to({}, { duration: 0.3 })
 // Initial lift — body strain
 .to(character, { y: 0, duration: 1.0, ease: "power4.out" })
 // Held strain at top
 .to({}, { duration: 0.5 })
 // Slight knee tremor (small oscillation)
 .to(character, { y: 5, duration: 0.1, repeat: 3, yoyo: true, ease: "sine.inOut" });
```

### Image keyframes — heavy lift (5 poses)
1. **Preparation** — character standing, looking at object, wider stance forming
2. **Deep crouch** — knees fully bent, body lowered over object, hands gripping
3. **Mid-lift strain** — back arched backward, knees bending to push up, face grimacing
4. **Top of lift** — body straightening up, object at chest level, knees still slightly bent
5. **Held strain** — fully upright with object, visible exhaustion, body trembling

## on weight

> The pressure is part of the weight.
A character pressing on something with effort visually transmits weight even when nothing is moving. Apply this thinking: every static pose should communicate where the weight is concentrated. A character standing has weight on their feet. A character leaning on a wall has weight transferred to that arm. A character holding something has weight pulling them.

When the audience can *see* where the weight is, the animation reads as alive. When the weight is ambiguous, the animation feels floaty.

## Linked concepts

- [[pressure-and-force]]
- [[balance-and-counterbalance]]
- [[mass-and-motion]]
- [[weight-in-a-jump]]
