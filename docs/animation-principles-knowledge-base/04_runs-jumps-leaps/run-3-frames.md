# The 3-Drawing Run — The Sprint Speed

Three frames per step at 24fps = 8 steps per second. This is the standard for very fast cartoon sprints. Classical animators favored 6 and 8 frame runs for "real" runs, but 3-frame runs for explosive bursts.

## The three poses

Each step is a single drawing:

| Frame | Pose |
|-------|------|
| 1 | **Down/Contact** — landing |
| 2 | **Passing (airborne)** — both feet off ground, body peak |
| 3 | **Up/Push-off** — leg extending, ready for next contact |

Then the cycle repeats with the other leg.

## Why 3 frames feel different from 4

A 4-frame run gives time for the body to articulate through each phase. A 3-frame run **skips intermediate moments** — the eye fills them in.

This creates a sense of frantic urgency. The character is moving faster than the eye can fully track. Used for:
- Last-second escape
- Maximum-effort sprint
- Pursued by something terrifying
- Comedic over-running

## An a classical animation pioneer note

> a classical animation pioneer felt that frame six really was a better run than a drawing of a run on four or five, and a master action animator, the great exponent of fast action at Warner, always preferred to do runs on 6's and 8's.
So 3-frame runs are the EXTREME case — useful but not the default. The defaults are 6 or 8.

## When to use 3-frame runs

- Sprint moments in an otherwise normal sequence
- Comedy where character escalates running speed
- Last-shot-before-impact moments
- Stylized "everything is moving fast" sequences

## When NOT to use 3-frame runs

- Realistic human motion (impossible speed)
- Long shots where audience needs to read character
- Any time you need clear acting through the body

## Prompt-ready language

### Video model
> "Maximum-speed sprint, 3 frames per step at 24fps (8 steps per second). Body leans dramatically forward, almost horizontal. Legs blur with speed. Arms pumping rapidly. Hair and clothes streaming behind. Used for moment of peak desperation/escape."

### Code
```javascript
const FPS = 24;
const FRAMES_PER_STEP = 3;
const STEP_DURATION = FRAMES_PER_STEP / FPS; // 0.125s per step

const sprintCycle = gsap.timeline({ repeat: -1 });
sprintCycle
 .to(body, { y: 4, duration: STEP_DURATION * 0.33, ease: "none" }) // contact
 .to(body, { y: -8, duration: STEP_DURATION * 0.33, ease: "none" }) // airborne
 .to(body, { y: -2, duration: STEP_DURATION * 0.33, ease: "none" }); // push-off
```

## Linked concepts

- [[run-4-frames]]
- [[run-2-frames]]
- [[run-cycle-fundamentals]]
