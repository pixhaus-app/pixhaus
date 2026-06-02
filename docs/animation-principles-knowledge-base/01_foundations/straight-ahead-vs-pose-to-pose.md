# Three Ways to Animate — Straight Ahead, Pose to Pose, and Hybrid

Each has a use case. Choosing the wrong one is the most common cause of failed animation.

## Method 1 — Straight Ahead

Start at drawing 1, animate drawing 2, then 3, then 4, and so on, discovering the motion as you go. No planning. Pure flow.

**Strengths:**
- Spontaneous, alive, surprising
- Captures motion that is fluid and chaotic (fire, water, smoke, hair, cloth, chaotic action)
- Good for one-shot bursts where pre-planning would kill the energy

**Weaknesses:**
- Drift — the character looks different by the end than at the start
- Hard to fit into a fixed time/length
- Hard to direct multiple animators on the same shot
- Hard to register against a soundtrack (dialogue, music)

**When to use:** explosions, splashes, smoke trails, flame, hair caught in wind, anything elemental.

## Method 2 — Pose to Pose

Plan all the extreme poses first, then the breakdowns, then the inbetweens. Top-down.

**Strengths:**
- Predictable, controlled, repeatable
- Easy to hit specific timing (e.g., land a punch on beat 4)
- Easy to direct and review
- Works well with dialogue sync

**Weaknesses:**
- Can feel stiff if the extremes are weak
- Less spontaneous
- Risk of "twinning" (symmetrical, lifeless poses) if extremes are not strongly designed

**When to use:** dialogue scenes, character acting, choreographed action, anything that needs to land on a music cue or audio sync.

## Method 3 — Hybrid (the classical method)

Plan the extremes pose-to-pose. Animate the transitions straight ahead. Best of both worlds.

**Strengths:**
- Strong silhouettes at the extremes
- Living, spontaneous transitions between them
- The standard professional approach for most character animation

**Weaknesses:**
- Requires discipline
- Easy to slip back into pure straight-ahead between extremes and lose the planned timing

**When to use:** most character animation. This is the default.

## How this maps to AI tools

### Image keyframe AI = pose to pose
You are forced into pose to pose. AI image generators produce strong individual frames but can't track between them. So you generate extremes, hand off to an interpolator.

**Prompt strategy:** Lock the character description (seed, character LoRA, identity reference) and vary only the pose description per extreme. Use the same camera angle and lighting unless intentionally changing.

### Video models = hybrid
Video models like Sora and Veo are doing hybrid behind the scenes. You give them a description of extremes (what should be true at key moments) and they interpolate. Your job is to phrase prompts in a way that encodes both the extremes and the rhythm.

**Prompt strategy:**
> "First the character stands relaxed [extreme 1], then pulls arm back into a deep wind-up over half a second [path], holds the wind-up briefly [held extreme 2], then releases the throw in a fast forward arc [path], ending with arm extended and body twisted [extreme 3]."

### Code = pure pose to pose
Every keyframe in CSS, GSAP, or After Effects is an extreme. The easing curve handles the inbetweens.

```js
gsap.timeline()
 .set(char, { x: 0, rotation: 0 }) // extreme 1
 .to(char, { x: 50, rotation: -10, duration: 0.5, ease: "power2.in" }) // extreme 2 (windup)
 .to(char, { x: 200, rotation: 20, duration: 0.2, ease: "power3.out" }) // extreme 3 (release)
```

### Straight-ahead in AI = procedural / physics
The only way to do real straight-ahead with AI is to use physics simulation, particle systems, or procedural generation (cloth sim, hair sim, fluid sim). These create motion frame by frame from rules, not from planned keyframes.

## of thumb

> If you can plan it, plan it. If you can't plan it, animate straight ahead. If you can plan the extremes but the transitions need life, do both.
## The most common mistake

Animators (and AI prompters) try to do pose-to-pose for chaotic actions (fire, water) and get stiff results. Or they try to straight-ahead for dialogue and lose the lip sync. **Match the method to the kind of motion.**

## Linked concepts

- [[extremes-breakdowns-inbetweens]]
- [[time-and-space]]
- [[the-AAR-formula]]
