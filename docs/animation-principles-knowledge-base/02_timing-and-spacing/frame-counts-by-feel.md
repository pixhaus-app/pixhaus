# Frame Counts by Feel — How Many Frames for Each Action

These are not arbitrary — they're decades of distilled craft. Use them as starting points, then adjust by feel.

All counts assume **24 frames per second** (standard film/animation). For 30fps multiply by 1.25; for 60fps multiply by 2.5.

## Universal action duration reference

### Head turns
| Action | Frames | Seconds | Notes |
|--------|--------|---------|-------|
| Snappy head turn (alert) | 4 | 0.17s | Almost no inbetweens |
| Quick head turn (curious) | 6-8 | 0.25-0.33s | Standard fast turn |
| Normal head turn | 12 | 0.5s | Default for dialogue scene |
| Slow head turn (thinking, sad) | 18-24 | 0.75-1.0s | With ease in/out |
| Very slow turn (heavy, old, suspicious) | 36-48 | 1.5-2.0s | Drag the eyes ahead |

**Key the classical tradition note:** during a head turn, the character almost always **blinks** at the fastest point of the rotation. Eyes can't track during fast rotation — they snap to the new target. This blink hides the snap.

### Eye darts (looking from one thing to another)
| Action | Frames | Notes |
|--------|--------|-------|
| Reflexive eye dart | 1-2 | Just a single drawing change |
| Considered glance | 3-4 | Brief inbetween |
| Slow scan | 8-12 | Eye actually arcs across |

The eye is **the fastest part of the body**. Eye darts always happen in 1-2 frames in real life.

### Blinks
| Action | Frames | Notes |
|--------|--------|-------|
| Quick reflexive blink | 4 (2 close, 2 open) | Standard |
| Normal blink | 6 (2 close, 2 hold, 2 open) | Most common |
| Slow blink (tired, suspicious, sexy) | 12-16 | Hold the closed for emphasis |
| Wide blink (shock, surprise) | 8 with held wide-open at end | Eyes pop after |

Blink frequency in real life: roughly **one every 2-3 seconds** when calm, more when stressed or talking.

### Mouth shapes (phonemes)
| Sound | Hold duration | Notes |
|-------|---------------|-------|
| Consonant (p, b, m) | 1-2 frames | Sharp closure |
| Vowel (a, e, i, o, u) | 3-6 frames | Held longer, shape readable |
| Sibilant (s, sh, f) | 2-4 frames | Quick |
| Long vowel (laughter, surprise) | 8+ frames | Held for expression |

### Gestures
| Action | Frames | Notes |
|--------|--------|-------|
| Snap of the fingers | 4 | 2 prep, 1 snap, 1 recoil |
| Pointing | 8-12 | Quick reach + held |
| Waving hello | 24-36 | 2-3 waves of 8-12 frames each |
| Reaching for an object | 18-24 | Anticipation + reach + grasp |
| Picking up a heavy object | 36-48 | Anticipation + lift + held strain |
| Throwing a ball | 24-36 | Wind-up + release + follow-through |

### Walks
| Walk type | Frames per step | Notes |
|-----------|-----------------|-------|
| Run (sprint) | 4 | Both feet airborne in middle |
| Run (fast) | 6 | One foot down at a time |
| Brisk walk | 8 | Determined, military-ish |
| Standard walk | 12 | The default |
| Casual walk | 16 | Relaxed |
| Slow / heavy / sad walk | 24 | Plodding |
| Very slow walk (old, sneak) | 32-48 | Held positions, drag |

(See `03_walks/walk-timing-chart.md` for full breakdown.)

### Body movements
| Action | Frames | Notes |
|--------|--------|-------|
| Quick reaction (surprise) | 2 | Snap |
| Take (cartoon double take) | 12-16 | See `08_takes-and-accents` |
| Sit down | 24-36 | Slow descent, settle |
| Stand up | 24-36 | Push off, balance |
| Jump anticipation | 8-12 | Squash down before launch |
| Jump airborne | 12-18 | Depends on jump height |
| Land + recover | 8-12 | Squash + settle |

### Camera moves (when prompting video models)
| Action | Frames | Notes |
|--------|--------|-------|
| Whip pan (action) | 6-8 | Snap from A to B |
| Quick pan | 24 | 1 second |
| Standard pan | 48 | 2 seconds |
| Slow dolly | 72-120 | 3-5 seconds |
| Slow push-in | 120+ | 5+ seconds, almost imperceptible |

### Holds (when nothing moves)
| Type | Frames | Notes |
|------|--------|-------|
| Moving hold (subtle drift) | 12-24 | Character isn't statue-still |
| Static hold (frozen) | 8-12 max | Longer = dead/disturbing |
| Comedic hold (stare/reaction) | 24-48 | Used for laughs |
| Dramatic hold | 24-72 | With moving hold underneath |

**the key rule:** *static holds longer than 8 frames look dead.* Always add a moving hold — tiny drift, blinks, breathing — under any held pose.

## Action-emotion frame chart (a the classical tradition favorite)

The same action with different frame counts reads as different emotions:

### "Character lifts hand to scratch head"
- **8 frames** (fast) = "Oh, I just remembered something"
- **16 frames** (normal) = "Hmm, thinking"
- **32 frames** (slow) = "I'm deeply confused"
- **48 frames** (very slow) = "I have no idea what just happened" or "I'm thinking very hard"

### "Character looks at object on table"
- **2 frames** to turn = "I just noticed it"
- **8 frames** = "I'm looking at it"
- **24 frames** = "I'm carefully studying it"
- **48 frames** = "I'm suspicious of it" or "I'm afraid of it"

## the "double the time" beginner rule

Most beginners (and most AI prompters) cram too much action into too little time. *"Go twice as slow as you think."*

If your prompt says "throws the ball in 1 second," try 2 seconds. The eye needs time to read the wind-up and the follow-through.

## How to use these counts in prompts

### Video models
Translate frame counts into seconds with explicit frame rates:

> "Character does a quick head turn over 8 frames (0.33 seconds at 24fps). Eyes blink at the midpoint. Head settles into the new direction."

> "Heavy lift over 1.5 seconds (36 frames): anticipation crouch over 0.4s, lift over 0.7s with visible strain, settle into upright held pose over 0.4s."

### Image keyframes
Generate keyframes at the rate implied by the count:

> For an 8-frame head turn at 24fps animation density: generate 4 keyframes (extremes + 2 breakdowns), interpolate the rest.

### Code
```javascript
// 12-frame head turn at 24fps = 0.5s
gsap.to(head, { rotation: 45, duration: 0.5, ease: "power2.inOut" });

// 36-frame heavy lift = 1.5s
const lift = gsap.timeline();
lift.to(character, { y: 20, scaleY: 0.9, duration: 0.4, ease: "power2.out" }) // antic crouch
 .to(character, { y: -100, scaleY: 1.1, duration: 0.7, ease: "power2.in" }) // lift with strain
 .to(character, { y: -90, scaleY: 1.0, duration: 0.4, ease: "back.out(1.5)" }); // settle
```

## When to break these counts

These are *defaults*. **time is the most powerful expressive variable**. If you want a character to feel young, energetic, sharp — go faster than the defaults. Heavy, old, contemplative — go slower. The frame counts above are starting points, not laws.

## Linked concepts

- [[time-and-space]]
- [[slow-in-slow-out]]
- [[walk-timing-chart]]
- [[run-cycle-fundamentals]]
