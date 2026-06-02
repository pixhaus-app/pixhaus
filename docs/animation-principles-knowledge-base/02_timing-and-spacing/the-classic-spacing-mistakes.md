# The Classic Spacing Mistakes (and How to Avoid Them)

the classical tradition lists the recurring errors that beginners — and AI prompters — make. Each one is a violation of how spacing actually works in the real world.

## Mistake 1 — The Linear Inbetween (Dead-Center Trap)

**The error:** putting the inbetween exactly halfway between two extremes.

**Why it's wrong:** real motion is almost never at constant speed. The midpoint inbetween produces flat, lifeless, "tweenable" motion.

**The example:** a hammer swings down and a nail bends.

```
Wrong:
[hammer raised] → [hammer halfway down, nail half bent] → [hammer at nail, fully bent]
```

The nail doesn't bend gradually — it bends at the moment of impact. The correct inbetween is:

```
Right:
[hammer raised] → [hammer almost at nail, nail still straight] → [hammer at nail, fully bent]
```

**The rule:** ask *when does the change actually happen?* and put the inbetween there.

**AI prompt fix:**
> "the hammer descends rapidly with the nail straight, then bends sharply at the moment of impact in the final frame"

## Mistake 2 — Drop of Water Tween

**The error:** showing a water drop "halfway falling" between two positions.

**Why it's wrong:** water at moderate speed doesn't have intermediate visible positions. Either it's at A or at B, with motion blur between.

```
Wrong:
[drop forming] → [drop half-fallen] → [drop at floor + splash]

Right:
[drop forming] → [drop streak/blur] → [drop at floor + splash]
 OR omit entirely 
```

**AI prompt fix:**
> "water drop forms, then a single streak shows its fall path, ending with the splash on the ground"

## Mistake 3 — Telephone Pole / Perspective Midpoint

**The error:** when an object zooms toward camera, putting the inbetween at the visual midpoint.

**Why it's wrong:** the object covers most of its distance in the first half of the time (it appears slower at distance, faster up close). The inbetween must be biased toward the *near* extreme.

**** draw diagonals from corner to corner of the bounding rectangle of the two extremes. The intersection is the *technical* midpoint in perspective. Then bias even *further* toward the near extreme for natural motion.

**AI prompt fix:**
> "object accelerates toward camera with increasing apparent speed — small at start, then rapid scale-up near the end"

## Mistake 4 — Rubber Hose Everything

**The error:** applying squash and stretch to every object regardless of its material.

**Why it's wrong:** a steel bowling ball doesn't deform. A glass doesn't bend. Over-squashing makes everything feel like cheap rubber.

**(from a master action animator):** *"Use squash and stretch sparingly. The right spacing on a rigid object beats the wrong spacing on a rubbery one."*

**The exception:** stylized cartoon worlds (Looney Tunes, cartoon/live-action hybrid) where everything is intentionally rubbery. Even then, the *amount* of squash matches the material narratively — Bugs Bunny squashes less than Yosemite Sam, who squashes less than the ground he hits.

**AI prompt fix:**
> "preserve rigid object shapes — squash and stretch only on naturally elastic materials (rubber ball, character body in cartoon style)"

## Mistake 5 — Constant-Speed Everything

**The error:** linear easing on every motion (the default for naive code-based animation and many AI tools).

**Why it's wrong:** nothing in nature moves at constant speed. Even a car at "constant" speed has road bumps, steering corrections, engine vibration.

**AI prompt fix:**
> "no linear motion — every action has natural easing, deceleration, or follow-through"

For code: replace `ease: "none"` or `linear` with at least `ease-in-out` everywhere. Custom `cubic-bezier()` curves for character feels.

## Mistake 6 — Twinning (Symmetry Across the Body)

**The error:** both arms doing the same thing at the same time. Both legs at the same angle. Both eyes blinking at exactly the same moment.

**Why it's wrong:** real bodies are asymmetric. Even when you "wave with both hands," one is always slightly leading.

**** *"Always offset the timing of paired body parts."* If left arm reaches up, right arm reaches up 1-2 frames later or to a slightly different position.

**AI prompt fix:**
> "asymmetric body movement — left side leads right by a small offset, never perfectly mirrored"

(See `14_staging-silhouette/avoiding-twins.md` for full breakdown.)

## Mistake 7 — Arc-Less Motion (Straight-Line Paths)

**The error:** moving a hand, head, or limb in a straight line between two extremes.

**Why it's wrong:** every joint in the body is a hinge. A reaching hand moves on an arc around the elbow/shoulder. A turning head sweeps on an arc.

**** trace the path of a single point (the hand, the nose, the eye) over time. If it's a straight line, the motion is wrong.

**The "trace one point" trick:** a classic supervisor's fix — when a horse animator can't get the motion right, mark only the position of the horse's eye on a separate sheet across all frames. The path of just the eye reveals bad arcs. Fix the arcs and the animation snaps to life.

**AI prompt fix:**
> "all limb and body motion follows natural arcs around joints — wrists arc around elbows, hands arc around shoulders, heads sweep through arcs not straight lines"

## Mistake 8 — Trying Too Much in Too Little Time

**The error:** cramming multiple actions into a tiny number of frames.

**Why it's wrong:** the audience needs time to *read* each action. A run cycle with a character also yelling and waving and turning their head in 8 frames is visually unintelligible.

**** a beginner animator did "Twelve-Frame Benny's Yawn" — twelve elaborate drawings of a yawn animation. He ran them on ones (12 frames = 0.5s). Too fast — invisible. Ran on twos (12 frames stretched to 1s). Still too fast. Inbetweens to 24 drawings on twos (2 seconds). *Almost right.* Then added easing buffers at start and end. *Now alive.*

**** *"Go twice as slow as you think."*

**AI prompt fix:**
> "one major action per beat — anticipation, action, settle — each beat needs 8-16 frames minimum to read"

## Mistake 9 — No Anticipation

**The error:** action starts from a static rest pose with no wind-up.

**Why it's wrong:** real actions almost always have anticipation. A character about to jump dips down first. About to throw, pulls back first. Even subtle actions (head turning to look) have tiny preparatory shifts.

**AI prompt fix:**
> "every major action begins with anticipation in the opposite direction — wind-up before throw, crouch before jump, weight shift before walk"

(See `07_anticipation/` for full breakdown.)

## Mistake 10 — No Settle / Follow-Through

**The error:** action stops abruptly at the end pose, like a freeze frame.

**Why it's wrong:** mass keeps moving. Hair, clothes, jowls, fat, ears all continue past the body's stop. Even rigid actions have a tiny settle/overshoot.

**AI prompt fix:**
> "actions end with follow-through — secondary parts (hair, cloth) continue, body settles with a tiny overshoot and recovery"

For code:
```javascript
ease: "back.out(1.2)" // overshoots target by 20%, then settles
// or
ease: "elastic.out(1, 0.4)" // overshoots and oscillates
```

## Mistake 11 — Static Held Poses Beyond 8 Frames

**The error:** holding a pose perfectly still for many frames.

**Why it's wrong:** real life never holds. Even a "statue" has breathing, blinks, micro-tremor. A character locked in place for more than 8 frames looks dead or paralyzed.

**** *"Always have a moving hold under any held pose."* Tiny drift, breathing, blink — something must keep moving.

**AI prompt fix:**
> "no static frozen holds — even paused poses have subtle breathing, blinking, or small drift movement underneath"

## Mistake 12 — Forgetting the Eyes Move Independently

**The error:** the eyes always look where the head is pointing.

**Why it's wrong:** in real human movement, **the eyes lead the head** by 1-3 frames. You look first, then your head follows. This is true for almost every directional motion.

**AI prompt fix:**
> "eyes shift to new target 1-3 frames before head turns to follow"

## Quick reference card

```
WHEN ANIMATING ANY MOTION, CHECK:

[ ] Are extremes positioned correctly (not just at end of path)?
[ ] Is the breakdown offset (not at the midpoint)?
[ ] Does the inbetween follow an arc (not a straight line)?
[ ] Is there anticipation before the action?
[ ] Is there follow-through after the action?
[ ] Are paired parts (arms, legs) asymmetric in timing?
[ ] Do secondary parts (hair, cloth) drag and settle?
[ ] Does the eye lead the head?
[ ] Is the held pose moving (subtle drift)?
[ ] Does the spacing match the weight?
```

## Linked concepts

- [[the-spacing-chart]]
- [[slow-in-slow-out]]
- [[mass-and-motion]]
- [[avoiding-twins]]
