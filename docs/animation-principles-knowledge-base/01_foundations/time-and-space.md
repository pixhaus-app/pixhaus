# Time and Space — The Two Elements of Animation

> Animation is all about time and space. Strange that the Americans were the ones who figured it out.
## The two variables

Every motion in animation reduces to two questions:

- **TIME** — *when* does the impact happen? The boink, the hit, the rhythm.
- **SPACE** — *how far apart* are the positions between extremes?

You can hold time constant and change feel entirely by changing spacing. You can hold spacing constant and change feel entirely by changing time.

## The coin experiment (no drawing required)

take a coin, film one frame at a time, move the coin between frames. You are now animating with zero drawing skill — pure time + space.

**Experiment 1 — same time, even spacing:** Move the coin across the page in 24 frames (1 second), equal intervals. Looks mechanical, robotic, unalive.

**Experiment 2 — same time (24 frames), uneven spacing:** Slow at the start, fast in the middle, slow to a stop. Same duration. Now it feels *alive*. It has weight, it has intention.

The lesson: **time alone is not enough. Spacing carries the personality.**

## What hides inside "time and space"

In just a coin moving across a page, the audience reads:
- **Weight** (heavy or light)
- **Material** (rigid, soft, sticky)
- **Size** (big, small)
- **Speed** (fast, slow, hesitant)
- **Personality** (jaunty, erratic, cautious, optimistic, pessimistic)

All of that, before a single drawing exists. This is why timing and spacing matter more than draftsmanship.

## The bouncing ball — every concept in one example

A ball bouncing across the screen shows:

| Concept | What the ball reveals |
|---------|----------------------|
| Timing | When the "boink" hits — the impact frames |
| Spacing | Closely-packed positions at the slow top of the arc, far-apart positions at the bottom (fast) |
| Squash and stretch | Optional — works on soft balls, not on golf balls |
| Arcs | The whole path is curves, never straight lines |
| Slow-in / slow-out | Ball decelerates approaching the top of arc, accelerates approaching the bottom |
| Weight | Heavy balls bounce low and fast; light balls float high and slow |

**Key You do not always need squash and stretch — overuse makes everything feel rubbery.

## How to use this in an AI prompt

**For video models (Sora, Veo, Kling, Runway):**
> "A red ball bounces across the floor from left to right. Each bounce lasts about 12 frames at 24fps. The ball moves slowly at the peak of each arc and accelerates as it falls. On impact, the ball compresses slightly and springs back. Three bounces total, each progressively lower."

**For keyframe image models (Midjourney, Flux):**
> Generate the three extremes: ball at peak of arc 1, ball at impact (squashed), ball at peak of arc 2 (smaller). These become tween targets.

**For code (CSS, GSAP):**
> `animation-timing-function: cubic-bezier(0.4, 0, 0.6, 1)` for slow-in/slow-out at the top of arc; transform-origin bottom for squash on impact.

## The takeaway formula

> **Animation feel = (time chosen) × (spacing distribution)**

Master these two before worrying about anything else. Every other principle in this book is a refinement of these two ideas.

## Linked concepts

- [[the-bouncing-ball]] — Full breakdown of the bouncing ball as a teaching tool
- [[extremes-breakdowns-inbetweens]] — How drawings are organized along the timeline
- [[slow-in-slow-out]] — The spacing pattern that makes things feel natural
