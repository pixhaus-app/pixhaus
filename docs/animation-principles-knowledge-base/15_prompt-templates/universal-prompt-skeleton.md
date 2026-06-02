# Universal Prompt Skeleton — Tool-Agnostic Animation Prompt

A fill-in-the-blank template that works across video models, image keyframe generators, and code-based animation. Adapt the relevant sections to your tool.

## The master template

```
[CHARACTER / SUBJECT DESCRIPTION]
- Who or what is in the scene
- Style (cartoon / realistic / anime / etc.)
- Distinctive features

[ACTION / MOTION]
- What they're doing
- The MAIN action
- The sequence of events (anticipation → action → reaction)

[TIMING]
- Frame rate (24fps standard)
- Frames per action
- Pacing notes (slow / fast / variable)

[BODY LANGUAGE / POSE]
- Posture, stance
- Where the weight is
- Line of action (C-curve / S-curve / diagonal)
- Asymmetry notes (avoid twins)

[FACIAL / EYE / HEAD]
- Expression at start, middle, end
- Eye direction and movement
- Blinks at transitions
- Head accents (3-4 frames before dialogue/emphasis)

[SECONDARY MOTION]
- Hair, cloth, ears, tail movement
- Overlap timing (4-8 frames lag)
- Follow-through after main action stops

[STAGING / CAMERA]
- Camera angle
- Silhouette readability
- Background context

[STYLE / RENDER]
- Animation style (Disney / Anime / Realistic / Stop-motion)
- Frame rate feel (on ones / on twos / on threes)
- Color palette
```

## Worked example: throwing a ball

Using the template for "character throws a ball at a wall":

```
[CHARACTER]
A young cartoon character, casual outfit, athletic build.

[ACTION]
Throws a baseball at a wall in 3 phases:
1. Anticipation: pulls back with weight on back foot (12 frames)
2. Action: swings forward, releases ball (6 frames)
3. Reaction: arm follow-through, balance recovery, tracks ball (12 frames)

[TIMING]
24fps. Total sequence ~30 frames (1.25 seconds).

[BODY LANGUAGE]
S-curve through body during windup (head one way, hips other).
Straight diagonal line of action during the throw.
Slight C-curve forward during follow-through.
Weight shifts: back foot in antic, front foot in throw, balanced in settle.

[FACIAL / EYE]
Eyes locked on target throughout.
Brief 1-frame squint at moment of release.
Eyes track ball after release.

[SECONDARY MOTION]
Hair lags behind head by 3 frames.
Shirt drags behind body by 4 frames during throw.
Follow-through arm continues past target — visible overshoot.

[STAGING]
3/4 view of character.
Silhouette clearly shows arm extending past body.
Wall visible in background as target.

[STYLE]
Disney-style cartoon. Animated on twos for most action, ones during the fast throw.
Bright colors. Slight squash on body during throw.
```

## Three-line minimum version

For quick prompts, use the simplified template:

```
[Character] + [Action with timing] + [Style notes]
```

Example:
> "Cartoon character throws baseball at wall over 1.5 seconds. Standard anticipation-action-reaction with arm windup, fast throw, and follow-through. Animated on twos, Disney style."

## Five-line standard version

```
1. CHARACTER + SETTING
2. MAIN ACTION
3. TIMING / RHYTHM
4. SECONDARY DETAILS (hair, cloth, eyes)
5. STYLE
```

Example:
> "1. A confident young woman, casual clothing, in a modern apartment.
> 2. She walks across the room and sits down on a couch, picking up a book.
> 3. Total motion 4 seconds. Walk takes 2 seconds at 16-frame-per-step pace. Sit takes 1.5 seconds (slow ease-in to seat). Picking up book takes 0.5 seconds (anticipation reach, grasp, lift).
> 4. Hair flows with each step (4-frame overlap). Eyes track to the book before her hand reaches it. Brief blink as she settles into couch.
> 5. Modern realistic 2D style. Smooth animation on ones for walks, twos for sit. Soft warm lighting."

## Per-Tool Adaptations

### For Sora / Veo / Kling / Runway (video models)

Emphasize:
- Verb-based action descriptions
- Frame timing in seconds (not frames)
- Camera movements
- Style references

Example:
> "A young cartoon character throws a baseball at a wall. They start in a relaxed standing pose for 1 second, then over the next 0.5 seconds they wind up — pulling their arm back, shifting weight to back foot, body rotating away from target. Then over 0.3 seconds, they explosively rotate forward and release the ball. Arm follows through past the body for 0.5 seconds. Camera holds steady throughout. Disney 2D animation style."

### For Midjourney / Flux / GPT Image (keyframes)

Emphasize:
- Static pose descriptions
- Composition / framing
- Each frame as a distinct image

Example (for 4 keyframes):
> "Pose 1 (relaxed standing): Cartoon character standing relaxed, holding baseball loosely in right hand, looking forward casually.
> Pose 2 (wind-up): Same character, body rotated back, weight on back foot, ball pulled back behind head, eyes locked on target.
> Pose 3 (release): Body rotated fully forward, ball just released from fingertips, arm at full extension, weight on front foot.
> Pose 4 (follow-through): Arm continued past target position, body twisted forward, eyes tracking ball, mouth slightly open in exertion."

### For Code (CSS / GSAP / After Effects)

Emphasize:
- Exact frame counts / durations
- Easing functions
- Transform values
- Timeline structure

Example:
```javascript
const throwBall = gsap.timeline();
throwBall
 // Anticipation (0.5s)
 .to(body, { rotation: -30, x: -10, duration: 0.5, ease: "power2.out" })
 // Hold antic (0.1s)
 .to({}, { duration: 0.1 })
 // Throw action (0.3s)
 .to(body, { rotation: 25, x: 15, duration: 0.3, ease: "power3.in" })
 // Release moment
 .call(() => releaseBall())
 // Follow-through (0.4s)
 .to(body, { rotation: 5, x: 0, duration: 0.4, ease: "elastic.out(1, 0.5)" });
```

## Quick reference cheat sheet

### For ANY action, ask:

1. **Where is the anticipation?** (Opposite direction, 30-50% of total time)
2. **What's the action itself?** (Fast, 10-30% of total time)
3. **What's the reaction/settle?** (Follow-through, 30-50% of total time)
4. **Where's the line of action?** (C-curve / S-curve / diagonal)
5. **What's the easing?** (Slow-in / slow-out / linear / snap)
6. **What lags?** (Hair, clothes, secondary parts)
7. **What's the style?** (How exaggerated, what frame rate feel)

If you can answer these 7, you have a complete prompt.

## Linked concepts

- [[sora-veo-kling-runway]]
- [[midjourney-flux-gpt-image]]
- [[code-css-gsap-after-effects]]
- [[../16_style-presets/]]
