# Image Keyframe Prompts — Midjourney, Flux, GPT Image

Image models don't animate — they generate stills. But you can use them to produce **keyframes** that get interpolated by other tools (Runway Gen-3 interp, AnimateDiff, RIFE, EBSynth).

For keyframe generation, **think like a pose-to-pose animator.** Generate the extremes; let the interpolator fill the rest.

## What to generate

For any animation, generate these keyframe types:

### 1. Extremes
The poses at the limits of the action (start, end, major intermediate positions).

### 2. Breakdowns
The poses that define the PATH between extremes. Offset breakdowns are where personality lives.

### 3. Held poses
Long-duration static poses (anticipation hold, accent hold, settle hold).

### 4. Optional inbetweens
Only generate these if your interpolator needs help with complex transitions.

## How many keyframes per second?

| Target animation | Keyframes per second | Notes |
|------------------|---------------------|-------|
| Smooth Disney style | 6-12 | Heavy keyframes, smooth interpolation |
| Anime with held poses | 3-6 | Held poses + impact frames |
| Stop-motion feel | 12+ | Each "drawn on twos" |
| Limited TV cartoon | 3-4 | Held poses with occasional accent |
| Cartoon take | Generate 5-7 for the take itself | Tight timing |

For a 2-second action at 24fps animation:
- Total frames: 48
- Keyframes needed for Disney smooth: 12-24
- Keyframes needed for anime: 6-12
- Keyframes needed for limited: 6-8

## Universal keyframe prompt template

```
[CHARACTER DESCRIPTION — exactly the same in every prompt]
[POSE DESCRIPTION — what makes THIS keyframe different]
[CAMERA — should usually be the same across keyframes]
[STYLE — should be exactly the same in every prompt]
```

The KEY rule: **everything stays the same except the pose**.

If your prompts vary in style, lighting, or character description between keyframes, the interpolator will fight your output. Lock the character.

## Locking the character

Use the same exact description in every prompt:

```
[LOCKED — repeated in every keyframe]:
"A 20-year-old woman with shoulder-length brown hair, blue eyes, wearing a red t-shirt and jeans, standing in a modern living room with afternoon light. Disney 2D animation style."

[VARIABLE — changes per keyframe]:
Keyframe 1: "[LOCKED] Standing in a relaxed neutral pose, looking forward."
Keyframe 2: "[LOCKED] Body crouched in anticipation, knees bent, arms swinging back."
Keyframe 3: "[LOCKED] Mid-leap, arms thrown up, body fully extended in the air."
Keyframe 4: "[LOCKED] Just landed, knees bent in absorption, arms forward for balance."
Keyframe 5: "[LOCKED] Standing upright again with a triumphant smile, fists raised."
```

The locked part ensures the character is consistent. The variable part defines the keyframe.

## Tools for character consistency

For better consistency across keyframes:

### Midjourney
- Use the `--cref` (character reference) parameter with a reference image
- Set `--seed` to the same value for all keyframes
- Use `--sref` (style reference) for consistent rendering

### Flux
- Use IP-Adapter with a reference image
- Or fine-tune a LoRA on your character
- Lock the seed across all keyframes

### GPT Image
- Use the image-edit endpoint with a reference image
- Provide the previous keyframe as context
- Be very explicit about what changes per pose

## Template 1: Action Sequence Keyframes

Generate 5 keyframes for a complete action (anticipation, peak, follow-through):

```
KEYFRAME 1 — Starting pose:
[Character], relaxed standing pose, neutral expression, hands at sides, weight evenly distributed.

KEYFRAME 2 — Anticipation:
[Character], crouched in preparation, knees bent, weight shifted back, arms drawn back, body coiled, eyes locked on target.

KEYFRAME 3 — Action peak (the extreme):
[Character], in mid-action, body fully extended in the direction of motion, arms reaching, weight transferred forward, dynamic line of action.

KEYFRAME 4 — Follow-through:
[Character], body continued past action position, slight overshoot, secondary parts (hair, clothes) trailing behind.

KEYFRAME 5 — Settle:
[Character], returned to balanced pose, slight forward weight residue from the action, recovered breath.
```

## Template 2: Dialogue Beat Keyframes

For an emotional dialogue beat:

```
KEYFRAME A — Pre-speech (held):
[Character], normal expression, looking at listener, mouth closed, slightly contemplative.

KEYFRAME B — Speech midpoint (mouth open on accent):
[Character], mouth open on important vowel ("A" shape), eyes wide for emphasis, head accented up, body language reinforcing emotion.

KEYFRAME C — Post-speech (settled):
[Character], mouth closed, expression revealing emotional aftermath (relief / sadness / anger), eyes slightly different position than before.
```

These 3 keyframes carry an entire 2-3 second dialogue beat with interpolation.

## Template 3: Walk Cycle Keyframes

For a single step of a walk cycle, generate the 4 standard poses:

```
KEYFRAME 1 — Contact (right foot lands):
[Character], walking, right foot just contacting ground in front, left foot pushing off behind, body slightly forward-leaning, arms in opposition.

KEYFRAME 2 — Down (recoil):
[Character], walking, right knee bent absorbing weight, body at lowest point, left foot just leaving ground.

KEYFRAME 3 — Passing (free leg passing):
[Character], walking, left leg swinging forward past right leg, right leg straight underneath body, body at highest point.

KEYFRAME 4 — Up (push-off):
[Character], walking, right leg fully extended pushing off, left foot reaching forward for next contact, body at upper position.
```

Mirror these for the next step (left foot leads).

## Template 4: Expression Change Keyframes

For an emotion transition:

```
KEYFRAME A — Initial expression (held):
[Character], [emotion 1] — describe eyes, eyebrows, mouth, head angle, body language.

KEYFRAME B — Anticipation of change:
[Character], beginning to feel the new emotion. Eyes start to shift. Slight body anticipation (in OPPOSITE direction of where they're going).

KEYFRAME C — Final expression (held):
[Character], [emotion 2] — describe eyes, eyebrows, mouth, head angle, body language.
```

The interpolator handles the transition; keyframe B prevents a flat morph by showing the antic.

## Pro tips for keyframe generation

### 1. Use consistent camera angle
The same view in every keyframe. If you switch from 3/4 to side, the interpolator gets confused.

### 2. Use consistent framing
Same crop, same composition. Character should be roughly the same size in frame across keyframes.

### 3. Describe poses precisely
Don't say "running" — say:
> "Body leaning 25 degrees forward, right knee high and forward, left leg trailing behind extended fully, both arms bent at 90 degrees, right arm forward, left arm back, eyes locked forward."

The more specific, the more consistent the result.

### 4. Lock lighting and time of day
"Afternoon golden light from upper left" in every keyframe. Don't let the AI shift lighting between frames.

### 5. Use negative prompts
- "No motion blur" (you want crisp keyframes)
- "No multiple poses in same image"
- "No background changes"

### 6. Generate more than you need
Generate 2-3 variations per keyframe. Pick the best one. The interpolator works better with consistent picks than with whatever first came out.

## Interpolating between keyframes

After generating keyframes, use:

- **Runway Gen-3 Image-to-Video** — best for stylized 2D
- **AnimateDiff** (with controlnet) — open-source option
- **RIFE** — frame interpolation
- **EBSynth** — for 2D consistency
- **Stable Video Diffusion** — for short clips

The interpolator fills in the inbetweens based on your keyframes.

## Common keyframe mistakes

### 1. Generating intermediate poses
Don't generate "the slight movement halfway through the throw." Generate only the EXTREMES. Let interpolation handle the middle.

### 2. Inconsistent character description
"A young woman" in one frame and "a young girl" in another will give you different characters. Lock the description.

### 3. Different rendering styles between frames
Don't switch between "watercolor style" and "anime style" across keyframes.

### 4. Too few keyframes
A 4-second action with only 2 keyframes will look slow and morph-y. Aim for 6-12 keyframes for important actions.

### 5. Too many keyframes
A 1-second action with 20 keyframes will look choppy and inconsistent. The interpolator can't reconcile so many variations.

## Linked concepts

- [[universal-prompt-skeleton]]
- [[sora-veo-kling-runway]]
- [[code-css-gsap-after-effects]]
- [[../01_foundations/extremes-breakdowns-inbetweens]]
