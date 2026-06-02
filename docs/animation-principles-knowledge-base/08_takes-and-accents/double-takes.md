# Takes and Double Takes — Classic Cartoon Reaction

A "take" is a sudden surprised reaction. The character sees something, then *takes* — their body and face explode into a reaction. This is one of the defining moves of classic Hollywood animation (Disney 30s-40s, Warner Bros. Looney Tunes).

## What is a take?

A take is an **anticipation of an accent that completes** — the character builds tension and then releases it explosively.

```
[See something] → [Down anticipation] → [Up accent] → [Normalize]
```

the classical tradition defines it: *"A take is an anticipation of an accent that concludes."*

## The standard take structure

| Frame range | What happens |
|-------------|--------------|
| 1-2 | Character sees the surprising thing |
| 3-6 | DOWN anticipation — body crouches, eyes squint, getting ready |
| 7-8 | Accent UP — body explodes upward, eyes pop, mouth opens, arms fly out |
| 9-16 | Held accent / normalize back to baseline |

The down-then-up pattern is universal. The body must compress before it explodes.

## The classic Disney take (timing breakdown)

for a Disney-style take, approximately 2/3 of a second:

```
DOWN ON TWOS, UP ON ONES, DOWN ON TWOS
[Drawing 1] — drawing
[Drawing 3, 5, 7] — going down on twos (3 drawings, 6 frames total going down)
[Drawing 8] — accent UP (1 frame!)
[Drawing 16] — settle position
```

The KEY insight: **the accent itself is only 2 frames**. By frame 9, the body is already heading back down. **Drawing #16 is where the audience actually reads the pose** — that's the "captured" frame.

```
 Frames: 1 3 5 7 8 ... 16
 Action: see -- -- -- POP ↓ settle
 going down coming back
```

## The classic Warner Bros. take (faster timing)

Warner Bros. cartoons used a tighter timing — about half a second:

```
DOWN ON TWOS, JUMP FROM 7 TO 9, DOWN ON ONES
```

The "jump from 7 to 9" omits drawing 8 entirely. The action goes directly from down to up with no transition drawing — pure snap.

This creates the explosive "Bugs Bunny eye pop" feel.

## The double take (delayed reaction)

A double take is when the character doesn't react immediately. The reaction is *delayed*, and that delay is itself the joke.

```
Phase 1: Character glances at surprising thing → normal expression (no reaction)
Phase 2: Character looks away → brain catches up
Phase 3: HUGE TAKE — eyes pop, body recoils, full reaction
Phase 4: Character snaps head back to look at the thing again
```

The timing of a double take:
- Initial glance: 6-8 frames
- Pause (the "wait, what?" moment): 8-16 frames
- Take explosion: standard take timing (12-16 frames)
- Snap back to look: 4-6 frames

The longer the pause, the funnier the double take. Hollywood masters held the pause until just before the audience thought "is the character going to react?"

## The triple take

Same as double take but with one more bounce of "looking back." Each "look back" reacts a little differently. Used in extreme comedy.

## the "extra collapse" variation

A standard take has one accent. The first peek up loads the audience for the bigger reaction. *Anticipating the anticipation* doubles the impact.

## The Tex Avery wild take

Tex Avery (Warner Bros. then MGM) is famous for extreme takes. His method:

> He extended takes into a series of compound actions
A Tex Avery take might have:
- Eyes pop out 1 frame
- Eyes return 2 frames
- Eyes pop out further 1 frame
- Mouth drops 2 frames
- Tongue rolls out 1 frame
- Body recoils 2 frames
- Body launches up 1 frame
- Etc.

Each part has its own timing. The result is a chaotic, cumulative explosion that's visually overwhelming.

## The a classic film actor subtle take

Lots of action happening around him. Yet his take dominates the screen — by being **incredibly subtle**.

```
- Head moves to extreme (the accent) in 3 frames!
- Then settles in a virtual hold over 8 frames.
- Total: 3-frame take with a settle.
```

The lesson: a take doesn't have to be huge. A 3-frame head movement with a held settle can dominate a busy frame.

This is the **realistic-style take** — the same structure as a cartoon take but compressed to its essence.

## Volume preservation

A master animator's rule:

*"I keep the same amount of flesh in a take."*

Even when the character is squashing down or stretching up wildly, the total volume of the character stays the same. Don't grow them larger or smaller — just redistribute the mass.

If the head squashes flat, the cheeks bulge sideways.
If the body stretches tall, it gets thinner.
If the body crouches deep, parts widen.

This is the basic principle of squash and stretch (see `13_twelve-principles/01-squash-and-stretch.md`).

## Distortion in motion

*"Don't be afraid of distortion within the action. Our drawings can look strange, but we really only see the start and end positions. We feel the distortion internally, and that's what counts."*

The middle frames of a take can be wildly distorted. The audience doesn't consciously see them — they only see the start (rest pose) and the end (settle pose). But the distorted middle frames *transmit energy*.

This is why fast actions in cartoons can have impossibly stretched drawings — they're not meant to be read individually, only felt as a rush of motion.

## a classical animation pioneer's hand flourish

For ornate takes, Babbitt would add hand circles at the end:

```
After the take + settle:
- Arms wheel around in tight circles, on ones (very fast)
- Left arm offset 2-3 frames from right arm
- May knock the character's hat off, then put it back on
- Feet may pedal in air
```

This creates a complex finishing flourish. Adds character work to what would otherwise be a simple take.

## Prompt-ready language

### Video model — standard cartoon take
> "Character sees something shocking. Standard cartoon take: anticipation (4 frames) — body crouches down, eyes squinting. Accent (1 frame) — body explodes upward, eyes pop wide, mouth drops open. Settle (10 frames) — body returns to a held shocked pose. Total ~16 frames at 24fps."

### Video model — double take
> "Character glances at the door, sees something odd but reacts neutrally. Walks past. After 1 full second, brain catches up — character SNAPS head back, eyes wide, jaw dropped, body recoiled in shock. Classic delayed reaction. Held shocked pose."

### Video model — wild Tex Avery take
> "Character does a wild cartoon take. Body crouches dramatically. Then explodes: eyes pop OUT of the head 6 inches, eyes return, eyes pop again EVEN BIGGER, tongue rolls out like a red carpet, body stretches 3x normal height, arms flail wildly. Each element has its own timing — chaotic but coherent. Returns to a stunned held pose."

### Video model — subtle take
> "Character does a subtle but powerful take. Eyes were focused on a book. Notice something. Head moves to look up in only 3 frames — sharp and quick. Then holds the new position dead still for 1 full second. Body otherwise frozen. The minimal movement reads as a strong reaction because of the held intensity."

### Code — cartoon take
```javascript
const cartoonTake = gsap.timeline();

// SEE
cartoonTake.to({}, { duration: 0.1 });

// ANTIC: crouch down
cartoonTake.to(body, { 
 y: 20, scaleY: 0.7, scaleX: 1.3,
 duration: 0.25, 
 ease: "power2.out" 
});

// ACCENT: explosive pop up
cartoonTake.to(body, { 
 y: -40, scaleY: 1.5, scaleX: 0.7, // tall and thin
 duration: 0.08, // 2 frames
 ease: "power4.in" 
});
cartoonTake.to(eyes, {
 scale: 2.5,
 duration: 0.04,
 ease: "back.out(3)"
}, "-=0.08");

// SETTLE
cartoonTake.to(body, { 
 y: 0, scaleY: 1, scaleX: 1,
 duration: 0.4, 
 ease: "elastic.out(1, 0.5)" 
});
cartoonTake.to(eyes, { scale: 1.2, duration: 0.3 }, "-=0.3");
```

## Linked concepts

- [[hard-accent-bounces]]
- [[soft-accent-continues]]
- [[surprise-anticipation]]
- [[the-AAR-formula]]
