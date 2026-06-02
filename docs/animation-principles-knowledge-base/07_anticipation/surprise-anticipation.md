# Surprise Anticipation — The Bait and Switch

The audience watches a character wind up for an action. Their brain predicts what's coming. Then the character does something completely different. This is the surprise anticipation — a tool for comedy, shock, and unpredictable character moments.

## The structure

```
1. ANTICIPATION (suggesting Action A)
2. SURPRISE — character does Action B instead
3. REACTION (either to Action B, or character realizes they did the wrong thing)
```

The setup primes the audience for one outcome. The payoff delivers another.

## ```
[Character winds up to punch] → audience expects: punch lands
↓
[Character pulls a flower out of their pocket instead] → laugh / surprise
```

Or:

```
[Character winds up to jump] → audience expects: leap
↓
[Character just walks away normally] → comic letdown
```

Or (dramatic):

```
[Villain raises arm slowly to strike] → audience tenses
↓
[Villain instead places a gentle hand on character's shoulder] → unease/twist
```

## Why this works

The audience's brain has already "rehearsed" the predicted action during the antic. When the actual action differs, two things happen simultaneously:

1. **Surprise** — their prediction was wrong
2. **Recognition of the trick** — they get it, often as a laugh

This is the foundation of cartoon comedy. Looney Tunes, Tom & Jerry, cartoon/live-action hybrid — all heavily reliant on the surprise antic.

## Variations of surprise antic

### The "fake punch / kiss"
Wind up like a punch. Deliver a kiss instead. (Or a slap, a poke, anything unexpected.)

### The "fake jump / sit"
Crouch like a jump. Just sit down instead.

### The "fake throw / drop"
Wind up to throw an object. Drop it instead.

### The "Wile E. Coyote setup"
Elaborate antic prepares for action A. The antic itself triggers Action B (a trap, gravity, etc.) and Action A never happens.

### The "double take" (special case)

```
Character glances at something normally
↓
Character looks away
↓
Brain catches up: "Wait, what was that?"
↓
HUGE TAKE — eyes pop, body recoils, mouth opens
```

The first glance was an "anticipation" for the take, but the audience didn't know it. The delayed reaction is the actual antic that releases into the take.

### The "no action"
Character winds up dramatically. Audience expects something big. Character does NOTHING. The held antic IS the joke.

## Setup and payoff timing

The surprise antic relies on careful timing:

| Phase | Frames (24fps) | Purpose |
|-------|----------------|---------|
| Setup (antic) | 12-30 | Build expectation |
| Held pose | 4-12 | Audience locks in prediction |
| Delivery (the surprise) | 4-12 | Reveal the actual action |
| Reaction beat | 8-24 | Audience processes |

The held pose is essential. Without it, the audience doesn't have time to form the wrong prediction.

## Examples from the descriptions

### The pickpocket
A character with elaborate prep looks like they're about to do something theatrical. Audience watches in suspense. Suddenly — they were just picking the other character's pocket the whole time.

### The fake-out throw
Pitcher winds up huge. Doesn't throw. Pretends to throw. Throws on the next windup which is much smaller.

### The held expectation
Character points at the door dramatically as if about to leave. Holds the pose. Holds longer. Audience laughs at the over-extended hold. THEN the character finally leaves.

## Combining surprise with regular anticipation

You can layer:

```
Normal antic + Action A → Normal antic + Action B (surprise) → Normal antic + Action C
```

The audience learns the pattern, then the pattern breaks. Used for escalating gags.

## Prompt-ready language

### Video model — fake punch
> "Character winds up dramatically as if to throw a punch. Body coiled, fist drawn back, mean expression. Holds the windup for 1 second. Then, instead of punching, gently lifts a piece of dust off the other character's shoulder. Other character did not flinch. Beat. Subtle smile."

### Video model — fake jump
> "Character does an elaborate crouch as if preparing for a huge jump. Body fully loaded, arms swinging back, deep knee bend. Holds the prep pose for 1.5 seconds. Then simply stands back up and walks away normally. No jump occurred."

### Video model — comedic held antic
> "Character lifts hand dramatically, eyes wide, mouth open, clearly about to make a profound announcement. Holds this 'mid-pronouncement' pose for 3 full seconds with no sound. Beat. Beat. Beat. Lowers hand. Walks off."

### Code — surprise antic
```javascript
const surpriseAntic = gsap.timeline();
surpriseAntic
 // BIG ANTIC — sets up expectation of huge action
 .to(character, { x: -50, rotation: -15, duration: 0.7, ease: "power2.out" })
 // HELD — audience locks in prediction
 .to(character, { duration: 0.4 })
 // SURPRISE — small unexpected action
 .to(character, { x: -10, rotation: 5, duration: 0.5, ease: "back.out(1.5)" })
 // BEAT — audience processes
 .to(character, { duration: 0.5 });
```

## When to use surprise antic vs. straight antic

| Use straight antic when... | Use surprise antic when... |
|---------------------------|----------------------------|
| Action is the point | Anticipation is the joke |
| Maintaining tension | Wanting to relieve tension with a laugh |
| Heroic / dramatic moment | Character work / comic moment |
| Realistic style | Stylized / cartoon style |
| Audience needs to predict | Audience expectation should be subverted |

## Linked concepts

- [[basic-anticipation]]
- [[the-AAR-formula]]
- [[double-takes]]
- [[hard-accent-bounces]]
