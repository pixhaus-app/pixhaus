# Quadruped Walk Pattern — The Four-Leg Cycle

Animals on four legs follow specific patterns. the classical tradition covers the basic quadruped walk in detail.

## The standard four-leg sequence

For a horse, dog, cat, or any quadruped walking normally, the legs move in a specific sequence:

```
1. Front-left foot lifts and steps forward
2. Back-right foot lifts and steps forward (1/4 cycle later)
3. Front-right foot lifts and steps forward (1/2 cycle from start)
4. Back-left foot lifts and steps forward (3/4 cycle from start)
```

This creates a continuous wave of motion through the legs. **At any moment, 3 legs are on the ground.** This is the slow, stable quadruped walk.

## The sequence as a chart

```
Time (in 8ths of cycle): 1 2 3 4 5 6 7 8
Front-Left: UP - - - - - - -
Back-Right: - - UP - - - - -
Front-Right: - - - - UP - - -
Back-Left: - - - - - - UP -
```

Each foot lifts 1/4 of the cycle apart. The opposite-side foot lifts next.

## The diagonal pair rule

Legs move in **diagonal pairs** — front-left with back-right (one diagonal), front-right with back-left (the other diagonal). This is the natural quadruped gait.

This is why a quadruped looks balanced even at any frame — they always have at least 3 legs supporting (typically the legs forming an X across the body).

## The trot (faster gait)

In a trot, diagonal pairs move TOGETHER:
- Front-left and back-right move together
- Then both rest while front-right and back-left move
- 2 legs in air at a time briefly

This is more bouncy and faster than a walk.

## The gallop (fastest gait)

In a gallop, the legs gather underneath the body, then extend out:
- All 4 legs gathered close
- All 4 legs extended out (longest stride)
- Brief airborne phase (all 4 off ground)

Both front legs land roughly together (one slightly before the other). Then both back legs land together. This creates the rocking-horse rhythm of a gallop.

## The body's motion above the legs

While the legs cycle, the body also moves:

### Up and down
- Body rises when supporting legs are extended (the "up" pose)
- Body falls when a new leg lands (the "down" pose)
- For a quadruped: 4 up/down cycles per full leg cycle (one per footfall)

### Side-to-side
- Body rocks slightly side-to-side as weight transfers
- Less than in a human walk
- Spine flexes laterally

### Forward
- Body progresses forward continuously
- Acceleration during diagonal-pair pushes
- Slight deceleration between pushes

## The head and neck

The head and neck add additional motion:

### Walking quadruped
- Head bobs forward and back slightly with each step
- Neck flexes up and down
- For some animals (horses), head bobs significantly forward
- For others (cats), head stays more level

### Running quadruped
- Head extends forward at full gallop
- Neck stretches out for balance and aerodynamics
- Head and tail in opposition (tail down when head up, tail up when head down)

## The tail

The tail provides counterbalance and adds expression:

### Resting quadruped
- Tail follows body motion with delay (overlap)
- Sways gently side-to-side
- Wave action through tail length

### Active quadruped
- Tail extended for balance
- Counter-rotates with body
- Whips with sudden direction changes

## Foot landing details

When each foot lands, it does so in a specific pattern:

### Heel-first (digitigrade animals — dogs, cats)
- Toe touches first
- Pad rolls down
- Heel doesn't touch (most digitigrades)

### Hoof-first (ungulates — horses, deer)
- Toe of hoof contacts first
- Rolls through to full weight on hoof
- Then heel contact

### Foot-flat (humans, bears, pandas)
- Heel first
- Roll through to toes

Match the foot pattern to the animal.

## Different animals, different cycles

### Dog walking
- Standard 4-leg pattern
- Tail wagging or held depending on mood
- Head bobs slightly
- Walk timing: ~16 frames per stride at 24fps

### Cat walking
- Standard 4-leg pattern but more graceful
- Tail held high with wave motion
- Head stays nearly level (predator stalking)
- Walk timing: ~20-24 frames per stride

### Horse walking
- Standard 4-leg pattern
- Significant head bob (forward and down)
- Tail swishes for flies
- Walk timing: ~24 frames per stride at 24fps

### Bird walking (bipedal)
- Standard biped pattern
- Head bobs forward with each step (chicken-like)
- Tail/wings adjust for balance
- Walk timing varies enormously by species

## on observation

> a classical animation pioneer always emphasized: study live-action reference for animals.
Animals don't follow theoretical patterns — they have nuanced individual gaits. Reference footage is essential for accurate animal animation.

But for stylized cartoon animals, the theoretical pattern works as a starting point.

## Prompt-ready language

### Video model — dog walking
> "Dog walking forward at standard pace, 16 frames per stride. Four-leg cycle: front-left lifts and steps, then 4 frames later back-right, then 4 frames later front-right, then 4 frames later back-left. Diagonal pairs work together. Body bobs up and down with each footfall. Tail follows body with slight delay. Head stays mostly level."

### Video model — horse trotting
> "Horse trots. Diagonal pairs move together — front-left and back-right swing forward together, then plant. Then front-right and back-left swing forward together. Rhythmic 1-2-1-2 pattern. Body bounces up and down with each diagonal pair landing. Head bobs noticeably forward and back. Tail held out behind, swishing."

### Video model — cat sneaking
> "Cat sneaking forward in stalking pose. Body held LOW to the ground. Each leg moves slowly and deliberately — long held passing poses. Tail twitches at the tip with focused attention. Head stays level — eyes locked on prey. About 32 frames per stride. Body shows muscle tension throughout."

### Code (quadruped gait timing)
```javascript
const quadrupedWalk = {
 frontLeft: { liftStart: 0, cycleDuration: 0.67 },
 backRight: { liftStart: 0.25, cycleDuration: 0.67 },
 frontRight: { liftStart: 0.5, cycleDuration: 0.67 },
 backLeft: { liftStart: 0.75, cycleDuration: 0.67 },
};

// Each leg cycles 1/4 phase apart
Object.entries(quadrupedWalk).forEach(([leg, params]) => {
 gsap.to(legs[leg], {
 rotation: 30,
 duration: params.cycleDuration / 4, // lift duration
 yoyo: true,
 repeat: -1,
 delay: params.liftStart,
 ease: "power2.inOut"
 });
});
```

## Linked concepts

- [[the-standard-walk]]
- [[counter-reaction]]
- [[wave-action]]
- [[walk-timing-chart]]
