# Looney Tunes / Cartoon Style — Warner Bros, Hanna-Barbera, Tex Avery

The opposite of Disney's restraint. Extreme exaggeration. Snap action. Big takes. Cartoon physics. Squash and stretch pushed to its limits.

## Key characteristics

- **Drawn on twos for most action; ones for fast moments**
- **Snap actions — minimal slow-in, lots of held poses**
- **Extreme squash and stretch** (especially on takes)
- **Held impossible poses** for comedic effect
- **Broken joints** at extension peaks
- **Big takes** with eye pops, body recoils, mouth drops
- **Cartoon physics** (characters can fly, stretch, squash without injury)
- **Strong silhouettes** even in extreme poses

## the Warner Bros notes

Classical animation history involves a master action animator (Warner Bros. master) and refers to Tex Avery, Chuck Jones, and Bob Clampett. Key insights:

- **Tex Avery** pushed exaggeration further than anyone — wild takes, impossible distortion
- **Chuck Jones** added musical timing (his cartoons have rhythmic structure built in)
- **a master action animator** mastered fast action — Bugs Bunny's signature movements
- **Bob McKimson** — fluid, weighty animation

The Warner style is more aggressive in timing than Disney. Snaps are sharper. Takes are bigger. Holds are longer.

## Timing characteristics

- **Walks:** 8 frames per step (brisk)
- **Runs:** 4 frames per step (very fast)
- **Snap reactions:** 2-3 frames
- **Takes:** explosive 3-frame accent with 16-frame held shock
- **Holds:** dramatic, can be 24+ frames
- **Whole-body actions:** committed extremes

## The "Bugs Bunny" formula

- Confident strut
- Lean back into space
- Defiance / smug expressions
- Quick exits with motion blur
- Big takes when surprised
- Always one step ahead

## The "Daffy Duck" formula

- Frenetic energy
- Wild takes
- Self-pity poses
- Spit takes
- Extreme contortion
- Theatrical gestures

## What to specify in prompts

```
Style: Looney Tunes / classic Warner Bros cartoon animation. Animated on twos with snap actions on ones. Extreme squash and stretch on takes and impacts. Big committed cartoon poses. Held expressions for comedic timing. Cartoon physics (gravity, weight, materials all flexible). Bright bold colors. Strong silhouettes even in wild poses.
```

## Reference cartoons

- **Bugs Bunny shorts (various)** — confident cartoon hero
- **Road Runner / Wile E. Coyote** — pure action cartoon
- **Tom and Jerry (MGM)** — pure chase comedy
- **Tex Avery shorts** — wildest takes ever
- **Daffy Duck** — frenetic acting
- **The Fairly OddParents, Cartoon Network shows** — modern Looney heirs

## Prompt examples

> "Classic Looney Tunes style. A character sees a piano falling toward them. Cartoon take: looks up casually (frame 1-4), then suddenly realizes (frame 5-6, eyes pop, mouth drops). Body crouches deeply in anticipation (frames 7-10). EXPLOSIVE jump straight up — body stretches impossibly tall (frame 11-12). Held mid-air with cartoon physics for 8 frames as piano passes underneath. Then character drops back down with squash on landing. Bright bold colors, extreme distortion."

> "Tex Avery wild take. Character sees a beautiful person enter the room. Anticipation: body coils down with eyes squinting in disbelief. Then explosive multi-stage take: eyes shoot OUT of head on stalks, jaw drops to floor, tongue rolls out, body stretches up tall and thin, ears (or hair) shoot upward, heart pulses out of chest in cartoon shape. All happening in cascade with 2-frame offsets between elements. Held shocked pose."

## Code translation

```javascript
const LOONEY_TUNES_STYLE = {
 walkStep: 8 / 24,
 runStep: 4 / 24,
 snapAction: 2 / 24,
 takeAccent: 3 / 24,
 comedyHold: 32 / 24,
 
 // Easing — much snappier than Disney
 defaultEase: 'power3.inOut',
 anticEase: 'power2.out',
 actionEase: 'power4.in', // sharper acceleration
 settleEase: 'back.out(2.5)', // dramatic overshoot
 takeEase: 'back.out(4)', // extreme overshoot
 
 // Squash/stretch ratios (more extreme)
 squashRatio: { y: 0.5, x: 1.5 },
 stretchRatio: { y: 1.5, x: 0.5 },
};
```
