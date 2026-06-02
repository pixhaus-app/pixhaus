# Expression Changes — Pause, Then Change. Never Crossfade.

The single most important principle for facial acting: **expressions don't blend, they switch.**

When a character changes from one emotion to another, they don't smoothly morph between them. They pause. The new expression appears. Then they hold.

This is what reveals a character is THINKING. It is the cornerstone of all acting in animation.

## an early Disney mentor's principle (via a Disney master and another Disney master)

From the foundational Disney animation text, the foundational text on Disney animation:

> If you're looking at a portrait and the subject gradually lowers their eyebrows into a frown, pauses, then arches one eyebrow and looks sideways
> Through the change of expression, the thought process was shown.
## The structure

```
EXPRESSION 1 → CHANGE → PAUSE → EXPRESSION 2 → HELD
 (2-4 frames) (4-8 frames) (held)
```

Two distinct expressions. A brief transition. A held pause. The new expression. NOT a slow morph from one to the other.

## Why this works

Real human faces don't blend expressions either. We make one face, hold it briefly, then make another. The micro-pauses between expressions are when the brain is making the new decision.

When animation slowly morphs faces, it looks creepy — the audience reads "uncanny valley" or "this character is on drugs." Distinct expression changes read as "this character is making decisions."

## the progressive example

"

### Version 2 (add anticipation)
```
[Neutral] → [Eyebrows UP first (antic)] → [Lowering brows / frowning] → [Pause] → [One brow up, looking sideways]
```

The slight upward antic before the frown makes the frown READ more strongly. The eyebrows momentarily rise (opposite direction) before settling into the frown.

### Version 3 (anticipate the second expression too)
```
[Neutral] → [Eyebrows UP] → [Lowering / frowning] → [Pause] → [Eyebrows DOWN first (antic)] → [Single brow up, looking sideways]
```

Anticipate the brow-raise by first lowering the brows. Now both transitions have antic.

### Version 4 (add a blink during the transition)
```
[Neutral] → [Antic up] → [Frown] → [Pause WITH BLINK] → [Antic down] → [Brow up, sideways glance]
```

A slow blink during the pause covers the transition and adds depth. The character is "closing their eyes to think."

## Always anticipate the change

> More change, more pop.
1. Anticipate in the opposite direction first
2. Change to new expression
3. Hold and let the audience read

Never just snap from one to another.

## Where to put the change

A master animator's rule:

> Don't change the expression DURING a big movement. Change it at the end of the movement, where the audience can see it.
If a character is turning their head from one direction to another:
- **Wrong:** expression morphs during the turn (audience can't read it)
- **Right:** old expression held during turn, NEW expression appears at the end

a character is reading a book. They hear a noise. They look up startled.

```
Frame 1-8: Reading, content expression
Frame 9-16: Look UP at noise — expression UNCHANGED yet
Frame 17: Settled in new direction, NOW expression changes to startled
Frame 18+: Hold the startled expression
```

The audience reads:
- "Character was happy reading"
- "Character looked up at something"
- "Character became startled by what they saw"

Three distinct beats. Each one readable.

## The contrast technique

To strengthen an expression change, **start with the OPPOSITE of where you're going.**

A character is going to be startled. Don't start them neutral. Start them DEEPLY relaxed, engrossed in something pleasant.

```
Start state: Deeply engaged, slight smile, eyebrows relaxed UP
 ↓
 Hears noise, looks up
 ↓
End state: Startled, eyes wide, eyebrows DOWN/UP in alarm
```

The contrast between the two states amplifies the change. Audience reads more drama because they have more visual difference to perceive.

> The greater the contrast between the two expressions, the more powerful the change.
## The blink as transition cover

A blink (2-4 frames closed) is a great way to transition between expressions:

```
[Expression A held]
[Eyes close — blink]
[Eyes open showing Expression B]
[Held]
```

The audience reads it as "character processed something." It's also natural — people blink when thinking.

Use blinks especially when:
- Two very different expressions
- A "deciding" moment
- Recovering from surprise
- Settling after intense emotion

## Prompt-ready language

### Video model — basic expression change
> "Character changes expression from suspicion to surprise. Holds suspicious face (frowning brows, narrowed eyes) for 12 frames. Then ANTICIPATION: brows momentarily lift up further (2 frames). Then SNAP to surprise (eyes wide, brows up, mouth open) in 3 frames. Held surprised expression for 24 frames. No gradual morphing — sharp transition with brief antic."

### Video model — change at end of motion
> "Character is reading book contentedly. Hears noise off-screen. Looks UP at source of sound — head turn takes 10 frames, expression UNCHANGED during the turn. At end of turn (frame 11), expression suddenly changes to startled — eyes wide, brows up, mouth slightly open. The expression change happens AFTER the head turn completes, not during it."

### Video model — contrasting expressions
> "Character expression changes: starts in deep contemplation (eyebrows down, slight frown), holds 1 second. Then ANTIC: brows go further down (2 frames). Then SNAP to delighted surprise (eyes wide open, brows arched high, big smile) over 3 frames. Held delighted for 1 second. The contrast between the two expressions makes the change feel huge."

### Code (expression change with antic)
```javascript
const expressionChange = gsap.timeline();

// HOLD: current expression
expressionChange.set(face, { ...expressionSerious });
expressionChange.to({}, { duration: 0.5 });

// ANTIC: brief opposite direction
expressionChange.to(face, { 
 ...invertExpression(expressionSerious, 0.3), 
 duration: 0.08, 
 ease: "power2.out" 
});

// CHANGE: snap to new expression
expressionChange.to(face, { 
 ...expressionSurprised, 
 duration: 0.12, 
 ease: "back.out(2)" 
});

// HOLD: new expression
expressionChange.to({}, { duration: 0.7 });
```

## The pause is mandatory

The most critical element: **pause between the change and the next action.**

Without a pause:
- Expression changes are unclear
- Audience doesn't have time to read
- Acting looks rushed

With a pause:
- Audience reads "decision moment"
- Character feels alive and thinking
- Subsequent actions land properly

Minimum pause: 6 frames at 24fps. Longer for emotional moments — up to 48 frames for dramatic shifts.

## Linked concepts

- [[looking-for-contrasts]]
- [[basic-anticipation]]
- [[the-AAR-formula]]
- [[eyes]]
