# Animation Principles — AI Prompt Knowledge Base

A dense, tool-agnostic distillation of classical animation principles, restructured for use with modern AI animation tools — video generators (Sora, Veo, Kling, Runway), image keyframe generators (Midjourney, GPT Image, Flux), and code-based animation (CSS, GSAP, After Effects, Phaser).

These principles are the accumulated craft knowledge of generations of animators, tracing back through the Golden Age of hand-drawn animation. This knowledge base translates that lineage into a form a language model can actually use.

## How to use this

**Drop into a system prompt or context window.** Each topic is a self-contained markdown file. Load all of them, load a subset, or paste individual files into a chat depending on the job.

**Reference by file name in prompts.** Example: "Animate this character using the timing rules from `02_timing-and-spacing` and the walk variation from `03_walks/variations.md`."

**Combine principles + style preset + template.** A good AI animation prompt blends three layers:
1. **Principle** (timing, spacing, anticipation) — from sections 01–14
2. **Style preset** (cartoon, anime, realistic, stop-motion) — from `16_style-presets`
3. **Template** (Sora vs Midjourney vs code) — from `15_prompt-templates`

## What's inside

```
00_README/ You are here
01_foundations/ Why animation works — time and space
02_timing-and-spacing/ The math of motion (most transferable section)
03_walks/ Standard walk + 14 variations
04_runs-jumps-leaps/ Run cycles 2-12 frames, jumps, leaps
05_weight-and-force/ Selling weight, force, balance
06_flexibility-overlap-follow-through/ Breaking joints, drag, overlap
07_anticipation/ Anticipation, surprise antic, invisible antic
08_takes-and-accents/ Hard accents, soft accents, double takes
09_wiggles-waves-whips/ Side-to-side wiggle, wave action, whip
10_dialogue-lipsync/ Mouth shapes, phrasing, accents, the secret
11_acting-and-facial/ Expression changes, contrasts, eyes, body language
12_animal-action/ Quadruped walk patterns
13_twelve-principles/ The 12 classical principles
14_staging-silhouette/ Silhouette, line of action, twins, C/S curves
15_prompt-templates/ Fill-in-the-blank prompts per tool
16_style-presets/ Cartoon / anime / realistic / stop-motion modifiers
```

## The core idea — read this first

> **Animation is all about time and space.**

Every other principle in this knowledge base is downstream of two questions:

1. **TIMING** — *when* do things happen? (the boink, the impact, the rhythm)
2. **SPACING** — *how far apart* are the in-between drawings? (close = slow, far = fast)

The same number of frames with different spacing produces radically different motion. A character can take exactly 1 second to cross a screen and feel light, heavy, hesitant, optimistic, drunk, or determined — purely by changing the spacing of the in-betweens. This is the fundamental insight you can lean on hardest when working with AI animation tools.

## How AI tools map to the principles

| Principle | Sora/Veo/Kling/Runway | Midjourney/Flux (keyframes) | CSS/GSAP/code |
|-----------|----------------------|------------------------------|----------------|
| Timing (frame counts) | "duration: 24 frames @ 24fps" — phrase the rhythm in seconds | N/A | `duration: 1s` |
| Spacing (in-between distribution) | "starts slow, accelerates, then slow stop" | Render extremes only | `cubic-bezier()` / easing |
| Anticipation | "before lifting, character crouches and looks up at the box" | Render the antic pose as one keyframe | Pre-tween: 6 frames opposite direction |
| Squash & stretch | "ball stretches as it falls, squashes on impact, springs back" | Show stretch pose | `transform: scale()` keyframes |
| Follow-through | "hair and coat continue moving after body stops" | Render with trailing parts mid-air | Stagger child element animations |
| Weight | "heavy lift: shoulders rise first, then back arches, knees bend deep" | Render at peak strain | Slow-in/slow-out on heavy objects |

## Source

This knowledge base is an original synthesis of widely-known classical animation craft — the same principles taught in every animation school. Frame counts, spacing patterns, and acting concepts are drawn from common industry knowledge. No copyrighted text is reproduced.

## Final note

The principles are simple. Mastery is not. A language model can apply these principles consistently — that's its strength. But the *judgment* of when to bend a rule is what makes animation feel alive. Use this knowledge base as scaffolding, not as scripture.
