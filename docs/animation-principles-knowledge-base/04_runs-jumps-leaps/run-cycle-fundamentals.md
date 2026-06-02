# Run Cycle Fundamentals — The Airborne Difference

A walk and a run look similar, but they differ on one critical point:

> **In a walk, one foot is always touching the ground. In a run, there is at least one frame where BOTH feet are off the ground.**

That's the technical definition. Cross that threshold and you have a run.

## The four poses of a run (modified from the walk)

The run cycle uses the same four-pose vocabulary as a walk:

1. **CONTACT** — front foot lands
2. **DOWN (Recoil)** — knee bends to absorb impact
3. **PASSING** — but in a run, **both feet are airborne at this moment**
4. **UP (Push-off)** — back leg pushes off, body launches into the air

The key difference: at the PASSING pose, neither foot is touching the ground. The body is briefly flying.

## Run on different frame counts

### Run on 6 (4 steps per second — fast walk or slow run)
Standard "fast run" timing. Six frames per step gives just enough room for:
- 1 contact
- 1-2 down/recoil frames
- 1 passing (airborne)
- 1-2 up frames

Both feet airborne at the passing — this is what makes it a run, not a fast walk.

### Run on 4 (6 steps per second — sprint)
the standard sprint timing. Four frames per step. Each frame is:
- 1: Contact (or near-contact)
- 2: Down / recoil
- 3: Passing (airborne)
- 4: Push-off / up

At this speed, **always animate on ones** (every frame is unique). Twos would strobe badly.

### Run on 3 (the very fast cartoon run)
Three frames per step. Almost no room for inbetweens. The pose changes are radical between each frame. Used for:
- Road Runner's "meep meep" sprint
- Bugs Bunny's "skedaddle"
- Last-second escape moments

### Run on 2 (the impossible cartoon run — )
Two frames per step. The audience should see this as impossible — but And the eye jumps to the space where the passing pose would normally be."*

This creates the visual impression of 12 steps per second. It's not anatomically possible but reads correctly because the audience's eye fills in the missing passing pose.

## s for runs

### Rule 1: Runs are ALWAYS on ones
(Except the 2-frame "trick run" where you're deliberately using twos.)

Reason: the spacing between frames is so wide that twos cause strobing — the brain sees two distinct positions instead of motion.

### Rule 2: Head moves less than in a walk
> When we lift the body, the head position increases by only half a head or a third of a head
A common error: making the head bob too much in a run. In reality, a runner keeps their head relatively stable to maintain visual focus on where they're going.

### Rule 3: Body leans forward dramatically
The faster the run, the more lean. A sprinter is leaning at maybe 30 degrees forward. A jogger maybe 15.

The lean is what makes the run feel fast even at slower tempos. A vertical body running fast feels wrong.

### Rule 4: Arms are tighter than in a walk
> In a normal run, the arms swing in opposition like a walk
Arms in a fast run:
- Bent at the elbows (~90 degrees, not straight)
- Hands held in fists or relaxed claws
- Swing is shorter than walk arms (forearms drive forward, not full arms)
- Stay close to the body for aerodynamic efficiency

### Rule 5: Different runs for different bodies
Running TOWARD something (eager) looks different from running FROM something (terrified). *"What are they running for? With what purpose? It will have a dramatic effect on the run."*

## What's in the air

When both feet are off the ground (the PASSING pose in a run), several things happen:

- **Body is at its highest point** (briefly weightless)
- **Front leg extending forward** for next contact
- **Back leg trailing behind**, pushed up by previous step
- **Arms in mid-swing** (one forward, one back)
- **Hair, clothing fly** with momentum
- **Eyes typically focused forward** (no time to look elsewhere)

## How to encode a run cycle in AI prompts

### Video model — standard run
> "Character runs at 4 steps per second (6 frames per step at 24fps), animated on ones. Each cycle: contact (front foot lands), down (knee absorbs), passing (BOTH FEET OFF GROUND, body airborne), up (back leg pushes off). Body leans forward 15-20 degrees. Arms swing in opposition, bent at elbows. Head bobs minimally. Hair and clothing flow back."

### Video model — sprint
> "Sprinting run at 6 steps per second (4 frames per step at 24fps). Body leans 30 degrees forward. Both feet leave the ground for 1-2 frames between each contact. Arms pump short and fast at chest height. Knees drive high. Eyes fixed forward. Animated on ones."

### Video model — cartoon run
> "Cartoon-style fast run, 2 frames per step (12 steps per second). Legs blur into a circular motion. Arms wheel rapidly. Body leans dramatically forward. Hair and clothing trail behind dramatically. Speed lines on background."

### Code (GSAP)
```javascript
// 6-frame run cycle (4 steps/sec) at 24fps
const runCycle = gsap.timeline({ repeat: -1 });
const stepDuration = 6 / 24; // 0.25s per step

runCycle
 // Right foot contact
 .set(body, { y: 0 })
 .to(body, { y: 5, duration: stepDuration * 0.3, ease: "power2.in" }) // down
 .to(body, { y: -8, duration: stepDuration * 0.3, ease: "power2.out" }) // passing (airborne!)
 .to(body, { y: -3, duration: stepDuration * 0.4, ease: "power2.in" }) // up to next contact

 // Left foot contact (mirror)
 .to(body, { y: 5, duration: stepDuration * 0.3, ease: "power2.in" })
 .to(body, { y: -8, duration: stepDuration * 0.3, ease: "power2.out" })
 .to(body, { y: -3, duration: stepDuration * 0.4, ease: "power2.in" });
```

### Image keyframes
For a 6-frame run, generate 3 keyframes (the rest can interpolate):
1. **CONTACT** — front foot landing, back leg trailing
2. **PASSING (airborne)** — both feet off ground, body at peak
3. **PUSH-OFF** — back leg extending, body launching into next stride

## Common run mistakes

1. **Both feet always on ground** — that's a fast walk, not a run
2. **Head bobbing as much as walk** — overdone, looks like jumping not running
3. **Arms swinging full-length** — locks elbows, looks unnatural at speed
4. **Body upright** — kills the speed perception
5. **Animating on twos** — causes strobing, motion looks broken

## Linked concepts

- [[run-4-frames]]
- [[run-3-frames]]
- [[run-2-frames]]
- [[the-standard-walk]]
- [[ones-vs-twos]]
