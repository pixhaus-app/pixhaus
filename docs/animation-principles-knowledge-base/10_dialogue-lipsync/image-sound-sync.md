# Image and Sound Sync — When to Hit the Frame

The technical question: should the mouth shape land on the exact audio frame, 1 frame ahead, 2 frames ahead, or what?

## the answer

> There is real sync that is at the level, directly on the modulation
So the rule:
- **Default: level sync** (mouth shape on the exact audio frame)
- **Alternative: 1 frame ahead**
- **Sometimes: 2 frames ahead**
- **Rarely: 3 frames ahead**
- **Never: after the audio**

## Why never after?

The brain reads animation as the cause of the sound. When the mouth opens AFTER the sound, the audio feels disconnected — like the character is a dubbed bad foreign film.

When the mouth opens slightly BEFORE the sound, the brain still reads them as connected (the audio "catches up" to the mouth).

## The "always 2 ahead" myth

"

> This is false. It's not just a rule. Sometimes the sync works better at level, sometimes it's better with the picture advancing one frame ahead or even 2 frames (here's the disease). And even sometimes it goes better with 3 frames ahead of the sound.
Don't blanket-apply a rule. Each line of dialogue should be evaluated independently.

## How to test

1. Animate the mouth and head accents at level sync (on the audio frame)
2. Run the playback
3. If it looks late, shift the animation 1 frame earlier
4. If it still looks late, shift 1 more frame
5. Stop when it reads correctly

The "correct" amount varies by:
- **Voice type** (slow voice = level OK; fast voice = ahead)
- **Frame rate** (24fps = small offsets; 60fps = larger offsets)
- **Action context** (busy scene = ahead; quiet scene = level)

## Head accent vs. mouth sync

These are different concepts:

### Head accent timing
**3-4 frames AHEAD of the audio.** The head lands at its accent position before the audio peak.

### Mouth shape timing
**Level or 0-1 frames ahead of the audio.** The mouth shape lands at or just before the audio peak.

So in a single accented word:
- Head moves up at frame N
- Mouth opens at frame N+3 (or N+4)
- Audio peak at frame N+3 (or N+4)

The head is well ahead. The mouth is on (or close to on) the audio.

## Mouth on ones vs. twos and sync

If you're animating on ones (a new drawing every frame):
- Sync the mouth at level (drawing N = audio frame N)

If you're animating on twos:
- Each drawing covers 2 frames
- For best sync, put the drawing 1 frame ahead of the audio
- So drawing N is shown for frames N and N+1, with the audio peak landing in the middle of that hold

```
On ones, level sync:
Audio frame: 5 6 7 8
Drawing: 4 5 6 7 (mouth shape changes every frame)
 ↑ "M" sound peaks here, drawing 6 is the M

On twos, 1 ahead:
Audio frame: 5 6 7 8
Drawing: [4] [5] [6] (each drawing shown 2 frames)
 ↑ M sound peaks at frame 7
 ↑ Drawing 5 is the M shape — held over frames 6 and 7
```

This puts the audio peak in the middle of the held drawing — which reads best to the audience.

## The exhaustion case

For long, dramatic dialogue (slow articulated speech):
- Sync at level works perfectly
- Each phoneme has time to be drawn separately
- 0 offset is fine

For fast cartoon dialogue (Bugs Bunny rapid-fire):
- 1-2 frames ahead works better
- Mouth shapes blur together quickly
- The "ahead" offset compensates for speed

## When to be more than 2 frames ahead

This is unusual but happens when:
- The character is mid-walk (motion eats sync attention)
- The dialogue has an unusual accent pattern
- The voice has a unique attack quality

When in doubt, try 1-2 frames ahead. Adjust as needed by previewing.

## Prompt-ready language

### Video model
> "Animate mouth shapes at level sync with the audio — mouth shape changes on the exact frame the new phoneme begins. Head accents should occur 3-4 frames BEFORE the corresponding audio emphasis. Never delay any animation behind the audio."

### Code (sync offsets)
```javascript
const SYNC_OFFSETS = {
 // For head accents (relative to audio peak)
 headAccent: -4 / 24, // 4 frames ahead
 
 // For body accents (relative to audio peak)
 bodyAccent: -3 / 24, // 3 frames ahead
 
 // For mouth shape changes (relative to phoneme onset)
 mouthShape: 0, // level by default
 // Adjust to -1/24 (1 frame ahead) for fast dialogue
 // or -2/24 (2 frames ahead) for hectic scenes
};

function syncToAudio(action, audioTimestamp, offsetType) {
 const triggerTime = audioTimestamp + SYNC_OFFSETS[offsetType];
 setTimeout(action, triggerTime * 1000);
}
```

### For AI video generation
Most video models handle sync automatically when audio is provided. The thing you can control is:
- Describe head accents BEFORE words ("head dips down 4 frames before the word 'NO'")
- Describe mouth shapes ON words ("mouth opens on the O sound")

This guides the model toward the right offset pattern.

## Linked concepts

- [[phrasing]]
- [[dialogue-accents]]
- [[mouth-shapes-phonemes]]
- [[ones-vs-twos]]
