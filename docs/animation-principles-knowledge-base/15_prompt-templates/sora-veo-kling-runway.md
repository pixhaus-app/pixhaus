# Video Model Prompts — Sora, Veo, Kling, Runway

Templates for text-to-video generation. These models interpret natural-language descriptions of motion and produce video clips. the principles translate into specific prompt language.

## What video models understand

These models are trained on millions of hours of video — including animated content. They respond to:

- **Motion verbs** (throws, swings, leaps, falls)
- **Easing language** (smoothly, sharply, gradually, snaps)
- **Timing references** (over 2 seconds, in a moment, slowly)
- **Camera language** (close-up, wide shot, pan, dolly, push-in)
- **Style references** (Disney style, anime, claymation, rotoscope, photoreal)
- **Frame rate hints** (animated on twos, smooth 24fps, choppy stop-motion)

## Universal video model template

```
[STYLE STATEMENT — what kind of animation]
[CHARACTER DESCRIPTION — who or what]
[SETTING — where]
[ACTION SEQUENCE — what happens, in temporal order]
[TIMING DETAILS — pace and duration]
[CAMERA — angle, movement]
[ADDITIONAL DETAILS — lighting, mood, secondary motion]
```

## Template 1: Character Action

```
In [STYLE] animation style, [CHARACTER] performs [ACTION].

Phase 1 — Anticipation: Over [N] seconds, [character] [opposite-direction prep movement]. 
[Body language description]. [Where they're looking].

Phase 2 — Action: Over [N] seconds, [character] [main action]. 
[Quick body language description]. [Arms/body description].

Phase 3 — Reaction: Over [N] seconds, [character] [settle/follow-through]. 
[Final pose description]. [Secondary motion: hair, clothes].

Camera: [angle, any movement].
Lighting: [mood].
```

### Example: Character punches the air

> "In Disney 2D animation style, a young cartoon hero punches the air in determined celebration.
> Phase 1 — Anticipation: Over 0.5 seconds, the character pulls back, weight shifting to back foot, fist drawing back behind shoulder, body rotating away from target. Held briefly with confident expression. Eyes locked forward.
> Phase 2 — Action: Over 0.2 seconds, the character explosively rotates forward, weight transfers to front foot, fist drives forward through the air. Body forms a straight diagonal line of action.
> Phase 3 — Reaction: Over 0.5 seconds, the fist holds in extended position briefly, then settles back as character recovers. Hair lags behind motion. Camera holds steady at 3/4 view.
> Lighting: warm cinematic, slight backlight."

## Template 2: Dialogue Scene

```
[CHARACTER] says: "[LINE]"

Style: [animation style], with attention to head accents 3-4 frames before audio peaks. Mouth animation hits major vowels but doesn't articulate every letter (phrasing approach).

Body language: [what the body is doing during the line — must be in motion]
Head accents: [which words get emphasis]
Hard vs soft accents: [which are sharp/declarative vs flowing]
Eye direction: [where they're looking, when they look elsewhere]
Setting: [where they are, what's around them]
```

### Example: Character delivers a serious line

> "Character says: 'I told you not to come here.'
> Style: realistic 2D animation, 24fps. Head accents 3-4 frames before each emphasized word. Mouth shapes hit clearly on the important vowels: TOLD, NOT, HERE. Smear/skip between accents.
> Body language: Character slowly walks toward the listener throughout the line. Slow deliberate steps, 16 frames per step. Body language is tense — leaning slightly forward, hands clenched. Always in motion (not standing still while speaking).
> Head accents: Three hard accents on TOLD, NOT, HERE. Head dips down then snaps up just before each word.
> Eye direction: Locked on listener throughout. Single brief blink during transition between sentence halves.
> Setting: Dimly-lit hallway, character backlit. Cool color palette."

## Template 3: Movement / Locomotion

```
[CHARACTER] [walks/runs/leaps/etc.] across the scene.

Gait: [N frames per step at 24fps] = [pace description]
Body posture: [lean, weight shift, head position]
Arm motion: [opposition pattern, swing amount]
Head bob: [how much vertical motion]
Secondary motion: [hair, clothes, fabric]
Camera: [tracking shot / static / pan]
Background: [environment, what passes]
```

### Example: Character walks with purpose

> "A confident woman walks across a busy office floor.
> Gait: 12 frames per step at 24fps — natural brisk pace. About 2 steps per second.
> Body posture: Upright, slight forward lean, weight bouncing naturally with each step. Belt line tilts with each footfall.
> Arm motion: Arms swing in opposition to legs — right arm forward with left leg, etc. Peak swing happens 1 frame after foot contact. Moderate arm swing amplitude.
> Head bob: Visible up-and-down with each step, about half a head's height in motion. Head bobs adds weight feel.
> Secondary motion: Hair drags behind body motion by 3-4 frames. Coat trails by 5-6 frames. Subtle wave action through hair.
> Camera: Tracking shot from 3/4 rear angle, moving with character.
> Background: Modern office, blurred figures passing in opposite direction."

## Template 4: Take / Reaction

```
[CHARACTER] sees [STIMULUS] and reacts with a take.

Initial state (0-N frames): [character in pre-state, opposite of reaction]
Stimulus moment: [the trigger]
Anticipation (N frames): [body crouches / sinks down]
Accent (N frames): [explosive pop / reaction peak]
Settle (N frames): [final held reaction pose]

Style: [animation style — affects how exaggerated the take is]
```

### Example: Cartoon double-take

> "Cartoon character is reading a newspaper. They glance at something off-screen, look back at the paper, then suddenly do a double-take.
> Initial state (24 frames): Calm reading pose, slight smile, eyes scanning newspaper.
> Glance: Eyes flicker up briefly, head doesn't move. (8 frames)
> Pause: Brain catches up. Eyes still on paper, content expression. (12 frames)
> Anticipation: Body suddenly crouches, eyes squint, prep for reaction. (4 frames)
> Take accent: Character EXPLODES into reaction — eyes pop wide, mouth drops open, body recoils, newspaper crumples in fists, hair stands up. (3 frames)
> Settle: Held shocked pose, body recoiled, eyes wide, jaw dropped. (16 frames)
> Style: Classic Looney Tunes / Tex Avery exaggeration. Animated on twos for normal action, ones during the take explosion. Maximum cartoon distortion at the peak."

## Template 5: Object Animation

```
[OBJECT] [moves/falls/breaks/etc.] in [STYLE].

Material: [what it's made of, affects squash/stretch]
Forces: [gravity / wind / spring / impact]
Sequence:
 [0-N frames]: [setup state]
 [N-N frames]: [main motion]
 [N-N frames]: [impact / interaction]
 [N-N frames]: [follow-through / settle]

Camera: [framing]
```

### Example: Bouncing ball

> "A rubber ball bounces across a wooden floor.
> Material: Rubber, slight squash on impact, springs back to original shape.
> Forces: Gravity pulls down. Forward momentum carries it across the floor.
> Sequence:
> 0-6 frames: Ball at peak of arc, slow drift forward.
> 6-12 frames: Ball accelerating downward (gravity), stretching slightly vertical.
> 12-13 frames: Ball at impact frame — contacts floor, beginning to compress.
> 14-15 frames: Ball at maximum squash — wider than tall.
> 16-22 frames: Ball springs back upward, stretching slightly, arc rises.
> 23-30 frames: Ball at peak (lower than previous peak — energy lost).
> Repeat with diminishing height.
> Camera: Wide shot, side angle, tracking ball horizontally.
> Style: Realistic but cartoon-influenced — visible squash and stretch."

## Pro tips for video model prompts

### 1. Reference real animation traditions
"Disney 2D" / "Looney Tunes" / "Studio Ghibli" / "Pixar 3D" — these phrases carry strong style information.

### 2. Specify the frame rate feel
- "Animated on twos" = classic limited animation feel
- "Smooth 24fps" = full Disney animation feel
- "Anime-style mixed exposures" = anime feel
- "Smooth 60fps" = modern 3D feel
- "Stop-motion style" = textured handmade feel

### 3. Use motion verbs strategically
- "Smoothly transitions" (slow-in/out)
- "Explosively launches" (snap action)
- "Gradually settles" (slow-out)
- "Bounces" (multiple oscillations)
- "Whips" (whip action with crack)
- "Drifts" (slow continuous motion)

### 4. Specify duration in seconds (with frame conversion)
> "Animation lasts 2 seconds (48 frames at 24fps)"

The redundant frame count helps the model understand the desired pace.

### 5. Layer multiple principles
Don't just ask for "anticipation" — describe the anticipation. Don't just say "follow-through" — describe what continues moving.

### 6. Mention what should NOT happen
- "No linear motion"
- "No symmetric poses"
- "No static holds longer than 8 frames"
- "No mouth flapping on every letter"

These negatives help the model avoid common errors.

## Linked concepts

- [[universal-prompt-skeleton]]
- [[midjourney-flux-gpt-image]]
- [[code-css-gsap-after-effects]]
- [[../16_style-presets/]]
