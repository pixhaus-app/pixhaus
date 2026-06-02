# The AAR Formula — Anticipation, Action, Reaction

The most important rhythm in animation. Master animator An old animation principle:

there are only three things in animation — **Anticipation, Action, Reaction.** Everything else flows from these.

This is the universal pattern for any meaningful action. Memorize it. Apply it everywhere.

## The formula

```
1. ANTICIPATION → 2. ACTION → 3. REACTION
 (windup, prep) (the move) (settle, follow-through)
```

Every action in animation follows this structure:

- A character about to speak: breath in (antic) → words come out (action) → settle / blink (reaction)
- A character about to jump: crouch (antic) → leap (action) → land + recover (reaction)
- A character about to look at something: drift opposite direction briefly (antic) → snap to target (action) → blink / hold (reaction)
- A character about to throw: pull back (antic) → throw forward (action) → arm follow-through + settle (reaction)

## Why all three are needed

### Without anticipation
The action looks instantaneous, like a glitch. Like the body teleported. No physics, no intent.

### Without action
You have a windup that goes nowhere. Looks like the character changed their mind, or like the animation is broken.

### Without reaction
The action stops abruptly. Like a freeze-frame at the end. Body doesn't settle, doesn't react to what just happened. Feels stapled together.

Each piece is structurally necessary. Skip any one and the audience reads it as wrong.

## The timing pattern

The three phases are typically **NOT equal in duration**. | Phase | Typical proportion | Why |
|-------|-------------------|-----|
| Anticipation | 30-50% | Slow buildup, audience reads intent |
| Action | 10-30% | Fast — the move itself is the briefest part |
| Reaction | 30-50% | Settle takes time, follow-through needs space |

So for a 24-frame action:
- 8-12 frames antic
- 2-6 frames action
- 8-12 frames reaction

Notice: the action itself is the SHORTEST. The buildup and aftermath are longer than the strike.

This matches the "Chaplin three-step" rule: tell them what you're going to do (antic), do it (action), tell them what you did (reaction).

## Variations by action type

### Slow-build dramatic action
```
LONG ANTIC | SHORT ACTION | LONG REACTION
[--------] [--] [--------]
50% 10% 40%
```
Examples: a samurai drawing a sword and striking. A baseball pitcher winding up and throwing.

### Quick reflexive action
```
SHORT ANTIC | SHORT ACTION | MEDIUM REACTION
[-] [-] [---]
20% 20% 60%
```
Examples: catching a falling object. Reacting to a hot stove.

### Held / suspenseful action
```
HELD ANTIC | SHORT ACTION | SHORT REACTION
[------------------] [-] [--]
80% 10% 10%
```
Examples: cartoon character about to launch into a chase. Dramatic comedy hold.

### Continuous flowing action
```
ANTIC | ACTION | (REACTION = ANTIC OF NEXT)
[-] [-] [-]
33% 33% 33%
```
Examples: chained dance moves. Combat combos. Conversational gestures.

## > *"There are only three things in animation:*
> *1. Anticipation*
> *2. Action*
> *3. Reaction*
> *Everything else implicates all of the rest."*
> Learn these things well and you will animate well.
This is the simplest summary of the entire craft.

## Chaplin's version

the silent film masters (master of silent comedy timing):

> 1. Tell them what you're going to do.
> 2. Do it.
> 3. Tell them what you did.
Same formula, different vocabulary. The "telling" is the body language — the antic shows intent, the reaction shows result.

## Applying it to dialogue

Dialogue acting follows AAR rhythm at multiple scales:

### Per word
- Antic: tongue/lip prepare for sound
- Action: phoneme formed
- Reaction: mouth relaxes / transitions to next antic

### Per sentence
- Antic: breath in, head positions, eyes shift
- Action: words come out, body gesture, head accents
- Reaction: settle, blink, breath out

### Per emotional beat
- Antic: build up of feeling (eyes narrow, shoulders tense)
- Action: emotional release (laughs, cries, shouts)
- Reaction: deflation, comedown, transition

## Applying it to camera moves

Even camera moves benefit from AAR rhythm:

- **Antic** — camera holds, audience anticipates
- **Action** — camera moves (pan, dolly, push)
- **Reaction** — camera holds at new position, audience settles

A camera that's always moving never gives the audience time to feel the move. Hold-move-hold.

## When to break the AAR formula

rules are made to be broken — once you understand them.

Break AAR when:

### Pure surprise
Skip the antic entirely. Audience reads it as shock.

### Comic timing
Hold the antic absurdly long. Audience laughs at the hold itself.

### Dream sequences / surreal moments
No antic, no reaction, just floating actions. Reads as dreamlike.

### Continuous motion
Stream of actions where the reaction of one IS the antic of the next.

## Prompt-ready language

### Video model — explicit AAR structure
> "Action sequence with full anticipation, action, reaction structure. Anticipation phase (12 frames at 24fps): character pulls back, weight shifts away from target, body coils. Held pose at peak of windup for 4 frames. Action phase (6 frames): rapid forward motion, fluid arc to target. Reaction phase (10 frames): body settles past target slightly, then recoils back to balanced standing pose."

### Video model — using AAR vocabulary
> "Apply Anticipation-Action-Reaction rhythm: 40% of the time should be windup (opposite-direction prep), 15% should be the main action (fast), 45% should be the settle and follow-through."

### Code template for any AAR sequence
```javascript
function createAARSequence(target, mainMove, totalDuration = 1) {
 const antic = totalDuration * 0.4;
 const action = totalDuration * 0.15;
 const reaction = totalDuration * 0.45;
 
 const tl = gsap.timeline();
 
 // ANTIC: opposite direction, slow ease
 tl.to(target, { 
 ...invertProperties(mainMove, 0.3),
 duration: antic, 
 ease: "power2.out" 
 });
 
 // HELD ANTIC
 tl.to(target, { duration: totalDuration * 0.05 });
 
 // ACTION: main move, fast ease-in
 tl.to(target, { 
 ...mainMove,
 duration: action, 
 ease: "power3.in" 
 });
 
 // REACTION: small overshoot then settle
 tl.to(target, { 
 ...overshootProperties(mainMove, 0.1),
 duration: reaction * 0.4, 
 ease: "power2.out" 
 });
 tl.to(target, { 
 ...mainMove,
 duration: reaction * 0.6, 
 ease: "elastic.out(1, 0.4)" 
 });
 
 return tl;
}
```

## A quick check

For any animation you're creating or prompting, ask:

1. **Where's the anticipation?** (If there isn't one, the action will feel mechanical)
2. **Is the action fast enough?** (Slow actions read as deliberate; fast actions read as energetic)
3. **Is there a settle/reaction?** (Without it, the action ends in a freeze-frame)

If you can answer all three, your animation has rhythm. If any are missing, the audience will feel something is off.

## Linked concepts

- [[basic-anticipation]]
- [[surprise-anticipation]]
- [[invisible-anticipation]]
- [[hard-accent-bounces]]
- [[soft-accent-continues]]
