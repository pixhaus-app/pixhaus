# Eyes — Where the Soul Lives

The eyes carry the most communicative content of any part of the body. A single eye dart can convey suspicion. A held gaze can convey love or threat. A blink can be a punctuation mark.

## The fundamental rule

**The eyes lead the head.**

In any directional motion, the eyes shift FIRST, then the head follows. By 1-3 frames.

```
Frame 1: eyes start to dart right (1 frame)
Frame 2: eyes locked on target right
Frame 3: head begins turning right
Frame 5: head fully turned right
```

This is how human heads actually work. The brain decides where to look, the eyes go, the heavy head catches up.

## Eye darts

The eye is the **fastest part of the body**. A real eye dart happens in 1-2 frames at 24fps — too fast to track consciously, but the audience perceives them.

| Action | Frames | Notes |
|--------|--------|-------|
| Reflexive eye dart | 1-2 | Just a single drawing change |
| Considered glance | 3-4 | Brief inbetween |
| Slow scan | 8-12 | Eye actually arcs across (rare) |

## Blinks

Blinks aren't just biological — they're punctuation marks for the audience.

### Blink types

| Type | Frames at 24fps | Use |
|------|----------------|-----|
| Quick blink | 4 (2 close, 2 open) | Casual, default |
| Normal blink | 6 (2 close, 2 hold, 2 open) | Most common, contemplative |
| Slow blink | 12-16 | Tired, suspicious, sexy, dramatic |
| Wide blink | 8 with held wide-open finish | Shock, surprise |

### When characters blink (real behavior)

- **At the fastest part of a head turn** — covers the visual "snap"
- **At transitions in thought** — closing eyes to think
- **When changing expressions** — covers the morph
- **About every 2-3 seconds** in calm states
- **More often when stressed** (5-10 per minute when nervous)
- **Less often when intensely focused** (hardly at all when staring at something important)

### The strategic blink

A blink during a transition makes the transition invisible. Use this:

```
Frame 1-8: Looking at object A, content expression
Frame 9-10: BLINK CLOSE (eyes shut)
Frame 11-12: BLINK OPEN — now looking at object B, alert expression
Frame 13+: Held new state
```

The blink completely hid the eye movement AND the expression change. Audience reads "decision made."

## Eye darts during head turns

> During the head turn, something interesting happens
So during a head turn:
- **Normal head turn:** eyes blink halfway through
- **Terrified head turn:** eyes stay open, locked on
- **Casual head turn:** eyes drift with head, may blink

This is why animated characters look natural when they blink at the fastest part of a head turn.

## Eye contact and gaze

Where the eyes look defines what the character is thinking about.

### Direct gaze (at another character)
- **Sustained** → connection, threat, love, attention
- **Brief** → acknowledgment, polite
- **Avoided** → shame, lying, distraction

### Looking at object
- **At hands** → working, focused
- **At floor** → sad, thinking, defeated
- **At ceiling** → exasperation, thinking, prayer
- **Side glance** → suspicion, judgment, sneaky thought

### Eyes "into the distance"
- **Unfocused, looking through people** → daydreaming, thinking deeply, troubled
- **Beyond the listener** → not listening, planning, plotting

## Eyebrows — emotion modifiers

Eyebrows are the most expressive single feature of a character. ## The "eyes do their own arc" rule

eyes move on their own arcs, independent of the head.

In a turning head:
- Head sweeps in arc A
- Eyes follow arc B (often different)
- Pupils inside the eye move on arc C

This independence creates life. A character whose eyes ALWAYS look exactly where their head points reads as a doll. A character whose eyes track independently reads as alive.

```
Character turns head right.
Frame 1-3: Eyes lock on a NEW target before head finishes turn
Frame 4-6: Head completes the turn
Frame 7+: Eyes already settled on new target, head catches up
```

## The "looking left/right" gaze chart

When a character is in 3/4 view, where they look says a lot:

- **Up-left** → remembering visual (looking at past images)
- **Up-right** → constructing visual (imagining)
- **Left-side** → remembering audio
- **Right-side** → constructing audio
- **Down-left** → internal dialog (talking to self)
- **Down-right** → internal feeling (sensing emotion)

This is somewhat controversial science but widely used in animation as a shortcut. Use it for thinking shots.

## Pupil size as emotion

The pupil can be drawn at different sizes:

- **Large pupil (dilated)** → arousal, fear, drug effect, low light
- **Small pupil (constricted)** → anger, focus, bright light
- **Normal pupil** → resting state

A character whose pupils dilate when they see their love interest is a classic cartoon trick.

## Pupil shape as character

- **Round** → normal, human
- **Cat-slit** → cat-like, dangerous, feral
- **Wide circle with small pupil** → cute, kawaii, anime
- **Dot eyes** → simplified, comedic, cute
- **No pupil (just eyes)** → mysterious, distant, alien

## Prompt-ready language

### Video model — eyes leading head turn
> "Character notices something off to the right. Eyes dart right first (1-2 frames). Then a beat (3-4 frames hold). Then head begins to turn right to follow eyes (8 frames). At the fastest point of the head turn, a quick blink (4 frames). Head settles facing right, eyes already locked on target."

### Video model — strategic blink during transition
> "Character changes from confused expression to determined expression. Mid-transition (frame 4-5 of the change), character blinks — eyes close for 3 frames, then open showing the new determined expression. The blink hides the morph."

### Video model — eye contact dynamics
> "Two characters in dialogue. Character A makes brief eye contact (held for 8 frames), then eyes dart down to floor (1 frame), held looking down for 12 frames during a difficult confession. Eyes finally rise back up to meet listener (3 frames) for the resolution of the line."

### Code (eye and head with offset)
```javascript
const lookAtTarget = gsap.timeline();

// 1. Eyes lead — dart to target
lookAtTarget.to(eyes, {
 lookX: target.x,
 lookY: target.y,
 duration: 0.08, // ~2 frames
 ease: "power3.out"
});

// 2. Brief hold of eyes-locked-but-head-still
lookAtTarget.to({}, { duration: 0.1 });

// 3. Blink during head turn
lookAtTarget.to(eyelids, { closed: 1, duration: 0.04 });

// 4. Head turns (in parallel with mid-blink)
lookAtTarget.to(head, {
 rotation: targetAngle,
 duration: 0.3,
 ease: "power2.inOut"
}, "-=0.04");

// 5. Eyes open at end of head turn
lookAtTarget.to(eyelids, { closed: 0, duration: 0.04 }, "-=0.1");
```

## Linked concepts

- [[expression-changes]]
- [[invisible-anticipation]]
- [[body-language]]
- [[counter-reaction]] (eyes leading is a form of this)
