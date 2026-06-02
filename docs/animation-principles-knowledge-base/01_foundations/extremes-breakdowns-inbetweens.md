# Extremes, Breakdowns, and Inbetweens

Every piece of animation is built from three drawing types. Understanding the hierarchy is essential for both traditional animation and prompting AI.

## The three drawing types

### 1. EXTREMES (also called Keys)
The poses at the *limits* of the motion — the start, the end, and the major positions in between.

Example: a character throwing a ball.
- Extreme 1: starting pose, ball held back
- Extreme 2: peak of windup
- Extreme 3: moment of release
- Extreme 4: end of follow-through

### 2. BREAKDOWNS (also called Passing Positions)
The drawings *between* extremes that define the *path* of the motion. Crucially, the breakdown is **not** simply the average of the two extremes. The breakdown is where the animator's choice lives.

Example: between "ball held back" and "moment of release", the breakdown determines whether the arc is a smooth curve, a wild swing, a tense controlled push, or a snap.

**** *The breakdown is where the personality goes.* A weak animator puts the breakdown in the middle. A strong animator offsets it deliberately.

### 3. INBETWEENS
Mechanical drawings that fill the gaps between extremes and breakdowns. These can be done by assistants (or AI). They are interpolations along the path defined by the breakdown.

## The hierarchy

```
EXTREMES → defines WHAT happens
 └─ BREAKDOWNS → defines HOW it happens (the path)
 └─ INBETWEENS → fills in WHERE the drawings sit on that path
```

## the notation

In a typical the classical tradition chart with 13 drawings between two extremes:

```
1 ----- 5 ----- 9 ----- 13
EXT BD BD EXT
```

Drawings 1 and 13 are extremes. Drawings 5 and 9 might be breakdowns. Everything else (2, 3, 4, 6, 7, 8, 10, 11, 12) are mechanical inbetweens.

## Why this matters for AI prompting

### Image-keyframe AI (Midjourney, Flux, GPT Image)
**You should generate the EXTREMES and BREAKDOWNS only.** Let an interpolation tool (Runway gen-3 interp, RIFE, AnimateDiff with controlnet) handle inbetweens. AI image generators are bad at temporal consistency but excellent at producing distinct strong poses — which is exactly what extremes need.

**Prompt structure:**
> "Pose 1 (extreme): character winding up, weight on back foot, arm pulled back, head turned."
> "Pose 2 (breakdown): character at peak windup, full body tension, chest puffed."
> "Pose 3 (extreme): release moment, arm forward, weight on front foot, ball gone."

### Video models (Sora, Veo, Kling, Runway)
You describe the **path** and the **rhythm**. The model handles the inbetweens. So your prompt should sound like a the classical tradition breakdown description, not a frame-by-frame list:

> "Character winds up by pulling arm back and shifting weight to back foot, holds the wind-up briefly, then throws forward in a smooth arc. The wind-up takes about 12 frames; the throw takes 4 frames; the follow-through takes 8 frames."

### Code (GSAP, After Effects)
Code-based animation maps directly:
- Extremes = `0%` and `100%` of a keyframe sequence
- Breakdown = intermediate `%` waypoint
- Inbetweens = the easing curve between them

```js
gsap.timeline()
 .to(arm, { rotation: -90, duration: 0.5, ease: "power2.in" }) // to extreme
 .to(arm, { rotation: -110, duration: 0.1, ease: "none" }) // to breakdown (over-windup)
 .to(arm, { rotation: 90, duration: 0.2, ease: "power3.out" }) // to extreme (release)
 .to(arm, { rotation: 70, duration: 0.4, ease: "power1.out" }); // follow-through
```

## The most common beginner mistake

Putting the breakdown exactly in the middle (the linear interpolation default). This produces flat, lifeless motion. **Offset your breakdown — that is where the life lives.**

In code terms: avoid `ease: "linear"`. Even `ease: "power2.inOut"` is better. Custom `cubic-bezier()` curves are how the breakdown gets offset.

## Linked concepts

- [[the-spacing-chart]] — Visual notation for spacing
- [[straight-ahead-vs-pose-to-pose]] — When to plan extremes vs. discover them
- [[slow-in-slow-out]] — What inbetween spacing should look like
