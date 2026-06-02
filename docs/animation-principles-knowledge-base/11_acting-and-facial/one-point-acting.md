# One-Point Acting — Don't Do Two Things at Once

a character can only do ONE thing at a time. One gesture, one expression, one intent per beat.

> We can only do one thing at a time. Like we can only say one word at a time, we can only project one gesture at a time. The whole pose works toward that one thing.
This is the rule against "over-animating" — the most common amateur mistake.

## The mistake: pointing while saying it

A bad actor points at the door and simultaneously says "There he goes!"

The pointing happens at the exact same moment as the speech. This looks staged, robotic, "gum chewing" (the term).

## The fix: separate the actions

Point FIRST. Then say it. (Or say it first, then point.)

### Sequence A: point, then speak
```
Frame 1-12: Character points at door (gesture established)
Frame 13: Held pose with finger pointed
Frame 14-30: Character says "There he goes!"
```

### Sequence B: speak, then point
```
Frame 1-15: Character says "There he goes!"
Frame 16-28: Character points at door (gesture lands)
Frame 29: Held pose, completed
```

Both work. **Combining them does not work** — looks fake.

## Why this is true

In real life, gestures and speech are coordinated but not simultaneous. The body prepares to point BEFORE the word arrives, or follows up AFTER. The eye reads each as a separate beat.

When animation combines them, the audience subconsciously feels "this character is doing two things at once for the camera" — they read it as performance, not life.

## "Pointing is stronger than words"

> Gestures are stronger than words.
If you only have one tool — gesture or speech — pick gesture. The audience will read the message either way.

This is why silent films work. The body says everything. Audio is icing.

## The corollary — pick ONE pose per phrase

For each phrase of dialogue or each beat of action, decide on ONE body pose. The whole body works toward that one pose.

For a 4-second scene:
- Beat 1 (1 sec): Pose A
- Beat 2 (1 sec): Transition to Pose B
- Beat 3 (1 sec): Pose B
- Beat 4 (1 sec): Transition to Pose C

Not: 47 poses crammed into 4 seconds. The audience can't read fast enough.

## How many poses per second?

5-1 | Important emotional beats |
| Normal acting | 1-2 | Standard dialogue scene |
| Fast / comedy | 2-3 | Rapid-fire jokes |
| Maximum | 4 | Tex Avery wild takes (and only briefly) |

For most acting, 1-2 poses per second is the sweet spot. The audience has time to read each one.

## The "wrist first" rule for pointing

And let the hand and fingers follow afterward."*

The wrist leads the gesture. The hand follows. The fingers extend last. This creates a wave through the limb — natural and alive.

```
Frame 1: wrist starts to rise
Frame 3: hand starts to rotate to point
Frame 5: fingers begin to extend
Frame 7: fingertip locks into target position
```

Each part has its own timing, lagging by 1-2 frames. The result reads as fluid.

## Combining gestures and dialogue (the right way)

You CAN have gesture and dialogue together — but they should be offset:

```
Frame 1-8: GESTURE begins (hand starts rising)
Frame 9-12: Gesture holds
Frame 13-30: DIALOGUE happens, gesture stays held
Frame 31: Gesture begins to release
```

The gesture is the **frame** for the dialogue. Both happen, but the gesture is established BEFORE the line and held THROUGH the line.

## What "the whole pose works toward one thing" means

For each beat, every part of the body should reinforce the same intent:

If the character is excited:
- Body leans forward (excitement)
- Head tilted up (excitement)
- Eyes wide (excitement)
- Arms gesturing open (excitement)
- Weight forward on toes (excitement)

NOT:
- Body excited (forward lean) + arms casual (hanging) + face neutral
- Body neutral + arms excited + face sad

Pick ONE emotion. Build the WHOLE pose around it. The audience reads it instantly.

## He always pushed the classical tradition toward simpler, clearer acting:

> The lesson: spectacular animation is fun but doesn't move audiences. **Clear emotional acting** does. And clear means simple — ONE thing at a time.

## The "hypnotize the audience" framing

An acting principle:

> Some actors hypnotize themselves to play the part, but there's a small group of actors who actually hypnotize the audience.
> So the idea is to hypnotize the audience.
Another way to think about it:

**captivate**.

> You're trying to capture the audience's attention and hold it. Grab them with something REAL they can identify with.
This works only if the acting is simple and clear. The audience can't be hypnotized by complexity — only by clarity. Pick ONE emotion per beat. Sell it completely.

## Prompt-ready language

### Video model — separate gesture and speech
> "Character delivers the line 'There he goes!' Sequence: first, points emphatically at the door (gesture takes 8 frames, building from wrist outward). Hold pointing pose for 4 frames. THEN says the line while still holding the pose. The gesture and the speech are SEPARATE actions, not simultaneous."

### Video model — single pose per phrase
> "Character speaks a 4-second monologue. Use only 4 distinct body poses, one per second. Each pose communicates ONE emotional state (curious → suspicious → angry → resolved). The whole body — head, shoulders, arms, hips, knees — works toward that single emotion per pose. Transitions between poses are smooth but distinct."

### Video model — wrist-first pointing
> "Character points to the sky. Wrist initiates motion first — rotates upward 2 frames before anything else. Hand and forearm follow 2 frames after wrist. Fingers extend last, fingertip arriving at full pointed position 6 frames after wrist initiated. The pointing motion has a wave-like quality through the arm."

### Code (wrist-first pointing)
```javascript
const pointAtTarget = gsap.timeline();

// Wrist leads
pointAtTarget.to(wrist, { rotation: 90, duration: 0.16, ease: "power2.in" });

// Hand follows (offset by 2 frames)
pointAtTarget.to(hand, { rotation: 90, duration: 0.16, ease: "power2.in" }, 0.08);

// Fingers extend last (offset by 4 frames total)
pointAtTarget.to(fingers, { extension: 1.0, duration: 0.12, ease: "back.out(2)" }, 0.16);

// Hold the point
pointAtTarget.to({}, { duration: 0.5 });

// Then dialogue begins
pointAtTarget.call(() => playDialogue("There he goes!"));
```

## Linked concepts

- [[expression-changes]]
- [[looking-for-contrasts]]
- [[body-language]]
- [[the-secret]]
