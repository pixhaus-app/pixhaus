# Dialogue Accents — Head Accents That Sell the Line

The most important Head accents, body shifts, and gestures are what make a line read as alive.

## The fundamental rule

> The old masters sharpened physical actions and moved the head 3 or 4 frames AHEAD of the modulation, and put the mouth action at the extreme.
The pattern:
1. Head moves to its accent position **3-4 frames BEFORE** the audio peak
2. Mouth opens to its position at the audio peak
3. Mouth stays open through the held syllable

This forward-offset of head accents is what makes professional dialogue look "tight" — the head lands at the punch position right before the sound hits.

## Visualizing the offset

```
Frame: 1 2 3 4 5 6 7 8
Sound: "POW!"
Head: ↑ ↑ (landed here)
 |start move up
Mouth: open

The HEAD moves up at frames 3-4.
The MOUTH opens at frame 6.
The audio peak is at frame 6.
```

The head leads. The mouth follows.

## Why this works

In real speech, the body precedes the voice. We breathe in to speak, we move into emphasis positions, we **prepare our body** for what we're about to say. Then the words come.

When animation matches mouth-to-audio perfectly but the head is static, it looks like a puppet. When the head leads the mouth (and the audio), it looks like the character is *thinking* the words.

## The dominant pattern

> Most of the time the head accent goes UP.
The standard pattern for any emphasized syllable:

1. **Anticipation down** — head dips slightly (1-2 frames)
2. **Head accent up** — head moves up sharply (2-3 frames)
3. **Mouth opens on vowel** — mouth animation peaks (held)

This UP-direction accent is the workhorse. It conveys energy, emphasis, life.

**Reversal:** sometimes the accent goes DOWN. This is more rare but valid for declarative beats:
- "I told you SO." (down accent on "SO")
- "Then we WAIT." (down accent on "WAIT")

Both directions work. UP is the default.

## Counting accents in a sentence

For each sentence of dialogue, identify the major accents.

> 'Well, at last you're home!'
Mark the accented words. Each accented word needs a head accent. The rest of the sentence flows through transitions.

For longer sentences:
> It was a dark and stormy night.
- DARK (head accent up)
- STOR-my (head accent down on STOR)
- NIGHT (head accent up)

Three accents. Three head movements. Everything else is smoothed.

## Big accents vs. small accents

You don't have to hit every accent equally. Pattern A is more energetic, talky. Pattern B is more emphatic, contained.

## Hard vs. soft dialogue accents

Apply the hard/soft accent rules from `08_takes-and-accents/`:

### Hard accent
- "NO!" — head down, then bounces back up sharply, holds
- "OF COURSE!" — sharp head snap, recoil, held

### Soft accent
- "Yeesss, that's what I thought..." — head gently rising and lowering
- "Hello there..." — gentle flowing motion

Mix them in a single line:
> OH! [hard] Well, you see, [soft] I never KNEW [hard] about that...
## Body accent example (from the classical tradition)

' Note the accent on 'oo' (in 'cooler'). She does it with her shoulder. The shoulder rises when she turns toward us, drops to anticipate, then rises rapidly for the main accent on 'cooler'."*

Layered accents:
- Shoulder rises (body action)
- Head dips (head anticipation)
- Head rises sharply (head accent)
- Mouth opens wide on "OO" (mouth animation)

All these happen in coordination, but with the head and shoulder offset 3-4 frames ahead of the mouth/sound.

## The 'attitude' rule

*"Get the body attitude right and you can almost skip the mouth. Mouth action should be the LAST thing you work on."*

Order of priority for dialogue animation:
1. **Body language / attitude** (most important)
2. **Head accents and movement**
3. **Eye darts and blinks**
4. **Facial expression (eyebrows, cheeks)**
5. **Mouth shapes** (least important)

If 1-4 are right, the mouth could be a vague flap and the line still reads.

This is why It forced him to mark the head accents correctly to be convincing."*

A character without a mouth can deliver a believable line if the head accents are right.

## The "ONE pose per phrase" rule

*"We can only do one thing at a time. Like we can only say one word at a time, we can only project one gesture at a time. The whole pose works toward that one thing."*

For each phrase or beat of dialogue:
- One main accent
- One body posture
- One emotional state

Don't try to combine "happy and surprised and angry" in one beat. The body picks ONE.

For a long line, the character can transition between different attitudes — but each phrase gets one clear posture.

## Prompt-ready language

### Video model — dialogue with accents
> "Character says 'Well, at LAST you're HOME!' Three head accents — small dip then up on WELL (1-2 frames offset before audio), small dip then up on LAST (3-4 frames before audio), bigger dip then up on HOME (3-4 frames before audio). Mouth animation hits each vowel cleanly. Body angled in welcoming posture throughout. Hands gesture open."

### Video model — dialogue with body accent
> "Character says 'I'll slip into something cooler' with seductive intent. The line accent is on 'cooler.' Body language carries the line: shoulders rise as the character turns toward camera, then shoulder drops to anticipate, then rises sharply with the 'oo' of cooler. Head moves down to anticipate, then up for the accent (3 frames before the 'oo' sound). Mouth opens wide on 'oo'."

### Video model — angry emphasis
> "Character emphatically says 'I told you NO!' Three hard accents. On 'told' — head dips down, then snaps up (4 frames before 'told' audio). On 'you' — smaller accent. On 'NO' — head dips deep, then snaps up with whole body involvement, eyes wide, mouth opens on the O. Held shocked pose."

### Code (head accent with audio offset)
```javascript
function speakWithAccent(audioStart, accentWords) {
 accentWords.forEach(({ word, audioTime, intensity }) => {
 // Head accent happens BEFORE audio
 const headOffset = -4 / 24; // 4 frames at 24fps
 const headTime = audioStart + audioTime + headOffset;
 
 gsap.to(head, {
 y: -10 * intensity, // up for accent
 duration: 0.1,
 ease: "back.out(2)",
 delay: headTime
 });
 
 // Mouth opens on the audio peak
 gsap.to(mouth, {
 morphTo: 'open_vowel',
 duration: 0.05,
 delay: audioStart + audioTime
 });
 });
}
```

## Linked concepts

- [[phrasing]]
- [[mouth-shapes-phonemes]]
- [[hard-accent-bounces]]
- [[soft-accent-continues]]
- [[the-secret]]
