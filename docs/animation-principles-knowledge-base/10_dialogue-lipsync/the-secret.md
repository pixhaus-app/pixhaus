# The Secret of Lip Sync — Move the Character

a master character animator, as recounted in animation lore about "the secret" of lip sync:

> You know famous puppeteers. He's a genius. He understood something puppeteers never got before. He just puts a sock on his hand and even though he never marks the sound exactly, he does it much better than many animators with all their technical resources. Study what he does. He is developing the action. He's always going somewhere with the frog when he talks.
## The secret

**Move the character. Always be going somewhere when they speak.**

This is the most important insight in the entire dialogue chapter:

> Go somewhere, anywhere, while talking.
A character standing still while delivering a line — even with perfect lip sync — looks dead. A character walking, turning, gesturing, leaning while talking — even with sloppy lip sync — looks alive.

## Why this is the secret

Speech is part of action. Real people don't pause to deliver dialogue. They speak WHILE doing other things — walking, looking, gesturing, thinking. The speech is embedded in motion.

When animation puts the character on pause to deliver a line, the audience subconsciously reads it as "puppet show" — the dialogue is happening to a marionette.

When animation has the character in motion *while* speaking, the audience reads "alive person."

## "

> He hardly moved his mouth at all. He was talking through his teeth while advancing toward the rabbit. He was PROGRESSING TOWARD the rabbit!
The Fox was moving the entire time. The mouth animation was minimal. But the line landed perfectly because of the forward motion.

## Apply this universally

For any dialogue animation, ask:

1. **Where is the character moving from?**
2. **Where is the character moving to?**
3. **What is happening in their body during the line?**

If the answers are "nowhere" / "nowhere" / "nothing" — the dialogue is dead.

Some good options:
- **Walking** while talking (most basic)
- **Approaching** the listener
- **Backing away** from the listener
- **Turning** their body
- **Sitting down** or **standing up**
- **Reaching** for something
- **Picking something up**
- **Putting something down**
- **Looking around** (eyes moving)
- **Adjusting clothing/hair**
- **Gesturing** (hands in motion)

## The "going somewhere" can be subtle

It doesn't have to be a full walk. Subtle progressions:

- Shifting weight from one foot to the other
- Slow head turn during the line
- Eyes traveling to a new point of focus
- Tiny shift toward or away from listener
- Slow exhale that lowers the body
- Hand rising gradually
- Body leaning into a position

The key is **continuous progression** through the line — not start, talk, stop.

## How this interacts with head accents

Head accents (see `dialogue-accents.md`) work IN ADDITION to the progression. The progression is the underlying motion; the accents are the punctuation on top.

```
PROGRESSION: [smooth continuous motion across the line]
ACCENTS: [|] [|] [|]
WORDS: "Well, at last you're home!"
```

The character progresses across the room while delivering the line. On each accent word, the head punctuates with an accent move — but the underlying body motion never stops.

## The mouth becomes secondary

When the character is in motion, the audience reads the body. The mouth becomes secondary. As The mouth action should be the LAST thing you work on."*

A character actively moving across the screen with vague mouth animation reads better than a static character with perfect mouth animation.

## The "advancing" technique

A specific application: when delivering an emphatic line, have the character advance TOWARD the listener (or camera).

- "I told you NO!" — advance toward listener
- "Get OUT!" — advance toward listener, pointing
- "I AM the law." — slight forward lean toward camera
- "I love you." — advance slowly toward listener

The forward progression amplifies the line. The character is literally invading the listener's space.

For passive lines:
- "I don't know..." — slight retreat or shrug back
- "Maybe..." — head and body shift away from question source
- "I'm tired." — body sinking down

The body's direction of motion echoes the emotional direction of the line.

## When to ignore "the secret"

A character can pause and be still ONLY when:

1. **They've finished their line and they're holding for response** (waiting for the other character to react)
2. **Dramatic still moment** (loaded pause for effect)
3. **Death scene / unconscious / paralyzed**
4. **Frozen with fear**

In these cases, the stillness IS the action. The audience reads "this character is currently doing the action of holding still."

But during the actual delivery of dialogue, the character should be in motion.

## Prompt-ready language

### Video model — dialogue with motion
> "Character delivers the line 'I told you not to come here' while slowly walking toward the listener. Each step is 12 frames. Body language is tense — leaning forward, hands tight. Head moves slightly with each step. Three head accents on TOLD, NOT, HERE (each 3-4 frames before the audio). Mouth shapes are clear but not over-articulated. The continuous forward motion carries the line."

### Video model — gestural dialogue
> "Character delivers 'Let me think about this for a moment' while reaching up to scratch their head. Hand rises slowly throughout the line. Fingers touch the side of the head on 'think.' Hand pauses there during 'about this' and slowly lowers during 'for a moment.' The hand motion is the visual thread — the speech is laid over it."

### Video model — sitting down dialogue
> "Character delivers 'Now sit down and listen to me' while themselves slowly sitting down in their chair. The sitting motion takes the full length of the line — 2 seconds. Each word lands as the character is in a different sitting phase. Their hand pats the chair next to them on 'sit.' Their voice drops in volume as they settle into the chair."

### Code (continuous body motion under dialogue)
```javascript
// While the dialogue plays, body keeps moving
const dialogueWithMotion = gsap.timeline();

// Continuous forward walk underneath the dialogue
dialogueWithMotion.to(character, { 
 x: '+=200', 
 duration: 3, 
 ease: "none" // constant forward motion
});

// Head accents punctuate on top
const accentTimes = [0.5, 1.6, 2.7]; // in seconds
accentTimes.forEach(t => {
 dialogueWithMotion.to(head, { 
 y: -8, 
 duration: 0.08, 
 ease: "back.out(2)"
 }, t - 4/24); // 4 frames ahead of beat
 dialogueWithMotion.to(head, { 
 y: 0, 
 duration: 0.15, 
 ease: "power2.out"
 }, t);
});
```

## Linked concepts

- [[phrasing]]
- [[dialogue-accents]]
- [[body-language]]
- [[one-point-acting]]
