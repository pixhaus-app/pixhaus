# Slow-In and Slow-Out — The Single Most Important Spacing Concept

> If you only learn one principle of animation, learn this one.
Almost every natural motion in the universe starts slowly, accelerates, and ends slowly. A head turn. A hand reaching. A door opening. A ball at the top of an arc. A car braking. A dancer striking a pose.

Slow-in and slow-out is the spacing pattern that captures this physics. It is the difference between motion that feels alive and motion that feels mechanical.

## The default pattern

```
extreme 1 extreme 2
||||---|---|--------|--------|---|---||||
1 2 3 4 5 6 7 8 9 10 11 12 13
```

- Drawings 1-4: close together (slow start)
- Drawings 5-9: spread out (fast middle)
- Drawings 10-13: close together (slow stop)

This is the spacing of a normal head turn, a normal arm gesture, a normal walk step.

## What it looks like in code

```javascript
// CSS
animation-timing-function: ease-in-out;
// or more precisely:
animation-timing-function: cubic-bezier(0.42, 0, 0.58, 1);

// GSAP
ease: "power2.inOut"
ease: "power3.inOut" // more dramatic
ease: "sine.inOut" // gentler

// AfterEffects
Easy Ease (F9)
```

## What it looks like in a video prompt

- "head turn that begins slowly, accelerates through the middle of the rotation, and decelerates as it reaches the target gaze"
- "arm gesture eases out of rest, swings through smoothly, eases to a stop"
- "ball decelerates as it approaches the peak of its arc, then accelerates as it falls"

## When NOT to use slow-in slow-out

the classical tradition is emphatic: this is *not* a universal rule. Many actions break it deliberately.

### Linear motion (constant speed)
- Mechanical things (conveyor belts, gears, robotic motion)
- Sliding objects (a coin skidding on ice)
- Tracking shots (camera moving at constant speed)
- Anything with steady forward momentum

### Snap actions
- Sword swings (no slow-in — straight from rest to full speed)
- Punches at full extension
- Lightning bolts
- Cartoon takes (held, then snap)

### Decay motion (slow-out only, no slow-in)
- A ball dropped from rest (gravity = constant acceleration, no slow-in)
- A character collapsing from standing
- A door slamming shut

### Whip motion (slow-in only, no slow-out)
- A whip crack
- Wind-up of a baseball pitcher
- A tail snapping

## The "twin trap" — when slow-in slow-out goes wrong

When *every* motion has slow-in slow-out, everything starts to feel the same. Soft, indecisive, gummy. The cure is to **deliberately break the pattern** for actions that should feel sharp or surprising:

- The *single* sharp accent in a scene of soft motion is what makes it pop
- A character that always eases in and out feels weak; mixing snap actions and held beats creates character

## Spacing chart reference

## How this interacts with the breakdown

The breakdown drawing in a slow-in slow-out motion sits *at the fastest part* of the chart — usually drawing 7 in a 13-drawing sequence. This is the moment of maximum velocity. It's where you put your most expressive pose, because it's the drawing the eye will catch as it passes through.

## Prompt-ready phrases (verbatim)

For video models, drop these in:

- "natural easing — slow start, accelerates, slow finish"
- "movement begins from rest with deliberate buildup, peaks in speed mid-motion, settles smoothly at end"
- "no abrupt starts or stops — every motion has a wind-up and follow-through"
- "ease-in-out timing on all major character motion"
- "gestures begin gently, gain energy through the middle, decelerate into a held pose"

For code:

```javascript
// Universal "feels natural" easing
gsap.to(element, { x: 200, duration: 0.6, ease: "power2.inOut" });

// More dramatic, more cartoon
gsap.to(element, { x: 200, duration: 0.6, ease: "power3.inOut" });

// Gentle, lifelike
gsap.to(element, { x: 200, duration: 0.6, ease: "sine.inOut" });

// With a small overshoot (more alive)
gsap.to(element, { x: 200, duration: 0.6, ease: "back.inOut(1.2)" });
```

## A test you can use

Take any motion and ask:
1. Does it start from rest? → slow-in
2. Does it end at rest? → slow-out
3. Is it mechanical or constrained? → linear
4. Is there an impact or sudden release? → break the pattern

Then either confirm the easing matches, or change it.

## Linked concepts

- [[the-spacing-chart]]
- [[time-and-space]]
- [[the-bouncing-ball]]
- [[arcs]]
