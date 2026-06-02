# Classic Disney Style — Snow White through Lion King

The gold standard of 2D animation. Smooth, full animation. Strong character acting. Subtle but committed exaggeration.

## Key characteristics

- **Drawn on ones for hero shots, twos for most acting**
- **Slow-in / slow-out on most motion**
- **Full body involvement** in every gesture
- **Strong silhouettes**
- **Solid construction** (volumes feel 3D even in 2D)
- **Subtle squash and stretch** (not rubber-hose)
- **Naturalistic timing** with selective exaggeration
- **Eye contact and emotional depth**

## Timing characteristics

- **Walks:** 12-16 frames per step (natural pace)
- **Runs:** 6-8 frames per step
- **Head turns:** 12-16 frames (smooth)
- **Reactions:** building takes over 12-24 frames
- **Settling:** generous follow-through, 16-30 frames

## Their work is the standard.

## What to specify in prompts

```
Style: classic Disney 2D animation, smooth and naturalistic. Animated on twos for most action with ones for hero moments. Strong attention to slow-in/slow-out timing, solid drawing, and emotional acting. Subtle squash and stretch — never rubbery. Characters feel like real beings with weight and volume.
```

## Reference films

For prompting consistency, reference specific films:

- **Snow White (1937)** — the foundational feature
- **Pinocchio (1940)** — peak craftsmanship
- **Bambi (1942)** — naturalism
- **Cinderella (1950)** — character acting
- **Sleeping Beauty (1959)** — a Disney character designer villains
- **101 Dalmatians (1961)** — pen-and-ink style
- **The Jungle Book (1967)** — a master character animator's animals
- **The Little Mermaid (1989)** — Disney Renaissance
- **Beauty and the Beast (1991)** — emotional acting
- **The Lion King (1994)** — sweeping action

## Prompt examples

> "Classic Disney 2D animation style. Female character walks across a Victorian parlor with grace and elegance. Walk has natural 14-frame-per-step timing. Body has a clear S-curve in each step. Hair flows with overlap. Animated on twos with ones during emphasis moments. Soft cinematic lighting like Beauty and the Beast."

> "Classic Disney animation style. Old male character expresses confusion. Slow head turn (16 frames), held look (12 frames), eyebrows knit together (4 frames), then a deliberate hand gesture (12 frames). Full body involvement — shoulder shift, slight stance change. Naturalistic timing throughout. Solid drawing with strong silhouette."

## Code translation

```javascript
const DISNEY_STYLE_TIMING = {
 walkStep: 14 / 24, // ~14 frames
 runStep: 7 / 24, // 7 frames
 headTurn: 14 / 24,
 gesture: 16 / 24,
 reactionPeak: 6 / 24,
 settleTime: 24 / 24, // 1 full second to settle
 
 // Easing defaults — never linear
 defaultEase: 'power2.inOut',
 anticEase: 'power2.out',
 actionEase: 'power3.in',
 settleEase: 'elastic.out(1, 0.3)', // subtle elastic, not bouncy
};
```
