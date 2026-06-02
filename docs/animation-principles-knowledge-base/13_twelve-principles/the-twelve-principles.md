# The 12 Principles of Animation (the classical tradition-Flavored)

The 12 principles were articulated by Disney's "Nine Old Men" — a Disney master, another Disney master, and their peers — in the foundational Disney animation text (1981). Here is the canonical list, with the annotations and AI-prompt applications.

---

## 1. Squash and Stretch

The most identifiable Disney principle. Objects and characters deform under force — squashing on impact, stretching during fast motion — while maintaining their volume.

### - **Don't overuse.** A bowling ball doesn't squash. A glass doesn't stretch. Apply selectively.
- **Volume preservation** is the rule. A character can be 3x taller — but they must be 3x thinner.
- a master action animator's "rubber hose" warning: too much squash makes everything look gummy.

### When to use
- Soft objects (rubber ball, character bodies in cartoon style)
- Cartoon takes (eyes pop, mouth distorts)
- Impact moments (squash on landing)
- Fast motion (slight stretch during the move)

### When to avoid
- Rigid objects (metal, glass, wood)
- Realistic style
- Mechanical motion
- Anything stylistically "serious"

### Prompt language
> "Character compresses on impact — body squashes wider as it makes contact with ground, then springs back to original height as the character stands. Volume preserved — when squashed, body is wider; when stretched, body is thinner."

```javascript
// Squash on landing
gsap.to(character, { 
 scaleY: 0.6, scaleX: 1.3, // volume preserved
 duration: 0.08, 
 ease: "power4.in" 
});
```

---

## 2. Anticipation

The wind-up before the action. (See `07_anticipation/` for full breakdown.)

### s
- Anticipation goes in the OPPOSITE direction of the main action
- Anticipation is usually slower than the action
- Even a 1-frame "invisible antic" adds snap
- Without anticipation, actions look mechanical

### Prompt language
> "Action has clear anticipation — before the main move, character pulls in the opposite direction. Anticipation phase is slow and deliberate (10-12 frames), action phase is fast (3-4 frames)."

---

## 3. Staging

Composing the shot so the action reads clearly. The audience should know where to look.

### - **Silhouette readability** (see `14_staging-silhouette/silhouette-readability.md`)
- **One pose per phrase / one idea per beat**
- **Camera position matters** — pick the angle that best shows the action

### Prompt language
> "Stage the action clearly. The character's silhouette should be unambiguous against the background. One main visual focus per moment — don't have competing action in the frame."

---

## 4. Straight Ahead vs. Pose to Pose

(See `01_foundations/straight-ahead-vs-pose-to-pose.md` for full breakdown.)

### Quick summary
- **Straight Ahead** — animate frame by frame, discover the motion (best for fluids, fire, chaos)
- **Pose to Pose** — plan extremes first, fill in (best for character acting)
- **Hybrid** — pose to pose for extremes, straight ahead for transitions (the professional default)

### Prompt language
> "For dialogue/acting scenes: pose-to-pose approach — design strong key poses first.
For fluid/chaotic motion: straight-ahead approach — let the motion emerge frame by frame."

---

## 5. Follow Through and Overlapping Action

When the main body stops, secondary parts (hair, clothes, ears, tails) keep moving and settle separately.

### s
- The body stops, the head continues, the hair continues further, the cloth continues even further
- Each part has its own timing
- Drag, follow-through, settle, oscillate, rest

### s
- A character's hair continuing after they stop
- Clothing wobbling after a sudden movement
- Jowls jiggling on a heavy character
- Tail trailing behind a running animal

### Prompt language
> "Action has clear follow-through — when the body stops, hair continues moving for 4-6 frames before settling. Clothes continue another 2-3 frames. Secondary mass parts (belly, jowls, hair) have visible overlap motion."

```javascript
// Body stops, hair continues
gsap.to(body, { x: 100, duration: 0.5, ease: "power3.in" });
gsap.to(hair, { x: 110, duration: 0.7, ease: "elastic.out(1, 0.3)" });
```

---

## 6. Slow In and Slow Out

(See `02_timing-and-spacing/slow-in-slow-out.md` for full breakdown.)

### Quick summary
- Most natural motion starts slowly, accelerates, ends slowly
- Spacing is tight at the start and end, wide in the middle
- Equivalent to `ease-in-out` in code

### - Don't apply universally — some actions are intentionally linear or snap-then-decay
- Mechanical motion is linear
- Falling bodies have only slow-out (gravity is constant acceleration)
- Whip motion is only slow-in (then snap)

### Prompt language
> "Smooth ease-in-ease-out timing — motion starts slowly, accelerates through middle, decelerates as it approaches end position. No abrupt starts or stops."

---

## 7. Arcs

All natural motion follows curved paths, not straight lines.

### s
- A reaching hand arcs around the elbow joint
- A turning head sweeps in an arc
- A walking foot rises and falls in an arc
- The eye traces an arc when looking from one thing to another

### The "trace one point" trick
A classic supervisor's note: when a horse animator's poses looked stiff, the fix was to mark only the position of the horse's eye on a separate layer across all frames. The path of just the eye revealed bad arcs.

### Prompt language
> "All motion follows natural arcs around joint pivots. Reaching hands arc around elbows. Heads sweep through arcs, not straight lines. Trace the path of any moving point — it should be a curve, not a line."

---

## 8. Secondary Action

Additional movements that support the main action but don't compete with it.

### Examples
- A character running while wiping sweat from forehead
- A character walking while looking around nervously
- A character speaking while gesturing with one hand

### - Secondary action should **support** the main action, not distract from it
- One main action per beat, with secondary action layered subtly
- Too many simultaneous actions = unreadable

### Prompt language
> "Main action: character walks across room. Secondary action: nervously adjusts collar. The secondary action supports the nervousness theme without competing with the walk."

---

## 9. Timing

(See `02_timing-and-spacing/frame-counts-by-feel.md` for full breakdown.)

### s
- The most expressive variable in animation
- Same action with different timing = different emotion
- Match timing to character, age, mood, energy

### Frame count reference
- Fast action: 2-6 frames
- Normal acting: 8-16 frames
- Slow / heavy: 16-32 frames
- Very slow: 32-48+ frames

### Prompt language
> "Timing reflects character — energetic young character moves with 8-frame gestures (fast). Older or contemplative character moves with 20-frame gestures (slow). Mood shifts the timing without changing the geometry."

---

## 10. Exaggeration

Pushing reality further than real life for clarity and impact.

### - A character expressing surprise doesn't just open their eyes — eyes POP
- A heavy character doesn't just bend slightly — back ARCHES dramatically
- A walk isn't just stepping — it has visible UP/DOWN of head and body

### > *"We exaggerate the result of an emotion. We experience the emotion and exaggerate."*

The audience reads animation through exaggerated cues. Real-life-level subtlety doesn't translate to animation.

### When to push exaggeration further
- Cartoon style (push as far as you can)
- Comedy
- Acting reveals
- Hero moments

### When to dial back
- Realistic style
- Emotional realism
- Subtle character moments
- "Documentary" feel

### Prompt language
> "Exaggerate the emotion — character's surprise is BIG (eyes popping, body recoiling, mouth dropping). Real-life subtlety doesn't read on screen. Push the cartoon distortion further than reality."

---

## 11. Solid Drawing (Solid Modeling)

The character has weight, dimension, and consistent volume — even in stylized art.

### s
- A flat 2D character should still feel like a 3D object that's been drawn flat
- The character's volume should be consistent across all angles
- Foreshortening and perspective should respect the 3D form
- The character occupies real space in the scene

### For AI image generation
This translates to: the character should be consistent across keyframes. Same proportions. Same volume. Different poses, same character.

### Prompt language
> "Maintain solid drawing — character's volume and proportions stay consistent across all poses. Even when stretched or squashed, the underlying mass is preserved. Foreshortening respects the 3D form."

---

## 12. Appeal

Characters and movements that are pleasing to watch. The undefinable quality that draws audiences in.

### - Strong silhouettes
- Clear staging
- Consistent character design
- Emotional truth in performance
- "Captivating" (a Disney master) or "hypnotizing" (Ned Beatty)

### What makes a character appealing
- Distinct silhouette
- Memorable design
- Personality clear from a single pose
- Eyes that connect (where they look = where audience looks)
- Charisma — they want to be on screen

### (from the end of the book)
The "ingredients" The tools have been trained on the classical tradition-influenced literature and understand these terms.

### Universal prompt skeleton

> "Animate this scene with strong attention to the 12 principles of animation. Specifically:
> - **Squash and stretch:** [where it applies, what materials]
> - **Anticipation:** [what the main action is and what the antic looks like]
> - **Staging:** [how the silhouette should read]
> - **Follow-through:** [secondary parts that continue after main action]
> - **Slow in/out:** [easing pattern]
> - **Arcs:** [trace the curve of the main motion]
> - **Exaggeration:** [push beyond realism]
> - **Appeal:** [character should feel charismatic]"

### Selective application

For most prompts, you don't need all 12. Pick the 2-3 most relevant:

- **Action scenes** → anticipation, follow-through, arcs, exaggeration
- **Dialogue** → timing, slow-in/out, secondary action, appeal
- **Walks** → arcs, slow-in/out, follow-through, timing
- **Comedy takes** → anticipation, squash/stretch, exaggeration, timing
- **Heavy lifting** → anticipation, follow-through, exaggeration, solid drawing

## Linked concepts

- All other files in this knowledge base are applications of these 12 principles
- Foundation files: `01_foundations/`
- Timing and spacing: `02_timing-and-spacing/`
- Specific actions: `03_walks/`, `04_runs-jumps-leaps/`, `05_weight-and-force/`
- Acting: `10_dialogue-lipsync/`, `11_acting-and-facial/`
