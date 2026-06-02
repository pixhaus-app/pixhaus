# Pressure and Force — Communicating Effort Even When Still

Static poses can communicate enormous effort. A character pushing hard on an immovable wall doesn't move — but the *pose* and the *details* tell us they're pushing with everything.

## The vocabulary of pressure

### Touching (no force)
- Hand contacts surface
- Fingers in natural curl
- No body involvement
- No deformation of either object or hand

### Pressing (moderate force)
- Fingers curl into the surface
- Wrist may bend back slightly
- Forearm slightly tense
- No body weight transferred yet
- Some deformation of soft objects

### Pushing (significant force)
- Whole arm extended and tense
- Shoulder pushed forward
- Body weight leaning into push
- One foot back for bracing
- Surface may deform if soft

### Pushing with full effort
- Both arms extended
- Body weight fully behind push
- One foot far back, bent
- Head down
- Visible strain (grimace, gritted teeth)
- If force exceeds friction, character slides

### Maximum effort against immovable object
- Whole body crouched into push
- One leg straight (driving)
- One leg bent (recovery)
- Hands flat or fists
- Face grimacing
- Visible held strain
- Maybe sweat drops, motion lines, vein detail (cartoon)
- Hands or feet might slip momentarily

## How force shows in the hand and surface

### Hand on a balloon (soft, light pressure)
- Hand maintains shape
- Balloon deforms around hand
- Slight indentation
- No force needed

### Hand pressing on a bowling ball (hard, light pressure)
- Hand barely contacts surface
- Hand might curl around the ball slightly
- Shoulders push up gently (hand is being held up against ball weight)
- No deformation of either

### Hand pressing on water (downward force)
- Hand displaces some water
- Water has minimal "give"
- Hand maintains shape
- Ripples or splash effects on contact
- Hand sinks below surface if force continues

### Squeezing the end (compressing for grip)
- Fingers curl into grip
- Whole hand shape changes (squeezed)
- Object deforms where pressure is applied
- Used for sneaking up grip, throttling, applying force

## The force source map

Where does the effort come from in the body? This is **trace the force to its origin.**

| Force origin | Type of motion | Examples |
|--------------|---------------|----------|
| Wrist | Tiny gestures | Picking up a coin |
| Elbow | Small precise motion | Writing, drinking |
| Shoulder | Arm gestures | Reaching, throwing |
| Hip | Whole body involvement | Heavy lifting, pushing |
| Whole body | Maximum effort | Pulling, pushing against immovable |

A pencil pickup originates at the **elbow** — the rest of the body stays still.
A brick pickup originates at the **shoulder** — the arm + slight body bend.
A heavy box pickup originates at the **hip** — the whole body squats and rises.
A maximum push originates at the **whole body** — everything is involved.

The deeper the origin in the body, the heavier the implied object/force.

## Stopping is part of weight

A common **

When a character is in motion and needs to stop:
- The faster they're moving, the more visible the stop
- The heavier they are, the harder it is to stop
- Each body part has its own inertia and stops at its own time

> Anything that's in motion tends to continue in its direction. Arms, hair, clothes, hands.
When a character abruptly stops:
1. Feet plant (frame 1)
2. Body decelerates but is still moving forward (frame 2)
3. Body finally stops (frame 3)
4. Arms continue past body (frame 3-4)
5. Head still pitching forward (frame 4-5)
6. Hair flies forward (frame 4-6)
7. Clothes overshoot (frame 5-7)
8. Everything settles back into final pose (frame 7-12)

The heavier each part, the more it overshoots. The harder the stop, the more dramatic the overshoot.

## a Disney master's quote (from the classical tradition)

> We have to do something to make convincingly the stopping of a weighted thing.
Classical animators emphasized:

a stop is an event, not just the absence of motion. It needs:
- A decision moment (the character commits to stopping)
- A deceleration phase
- An overshoot phase (parts continue)
- A settle phase (everything finds its rest position)

## a master character animator on stops

> Stopping things convincingly is one of the hardest things in animation. When you go to stop, choose a good place. How are you stopping, what kind of stop is it, alert stop or lazy stop? Choosing where is an important choice. I hate seeing a foot following and landing without anything happening.
The takeaway: every stop is a choice. Don't just let motion fade out. **Decide** where and how the stop happens. Make it intentional.

## Prompt-ready language

### Video model — character pushing against wall
> "Character pushes hard against an immovable wall. Whole body crouched into the push, one leg extended back as a brace, one leg bent forward. Both hands flat against wall. Shoulders pressed up. Head down between arms. Face grimacing with effort. Wall does not move — character's feet slip back slightly when effort peaks. Held strain pose. Then character collapses backward, exhausted."

### Video model — character putting effort into a stop
> "Character running at full speed, then commits to stopping. Feet plant — front foot forward, back foot bracing. Body continues forward over planted feet for 1-2 frames before halting. Arms swing forward past the stopped body. Hair flies forward. Body finally settles back upright. Overshoot and recovery is visible."

### Code (heavy stop with follow-through)
```javascript
const heavyStop = gsap.timeline();

// Main body decelerates
heavyStop.to(body, {
 x: 200,
 duration: 0.3,
 ease: "power3.in" // decelerates hard
});

// Arms overshoot — start later, continue past
heavyStop.to(arms, {
 x: 230, // overshoot by 30
 duration: 0.4,
 ease: "power2.out"
}, "-=0.2");

// Hair overshoot — even more
heavyStop.to(hair, {
 x: 250, // overshoot by 50
 duration: 0.5,
 ease: "power2.out"
}, "-=0.3");

// Settle: everything recovers to final pose
heavyStop.to([arms, hair], {
 x: 200,
 duration: 0.6,
 ease: "elastic.out(1, 0.4)"
});
```

## Linked concepts

- [[showing-weight]]
- [[balance-and-counterbalance]]
- [[counter-reaction]]
- [[mass-and-motion]]
