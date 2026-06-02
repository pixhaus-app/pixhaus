# Whip Action — The Crack at the End

Whip action is like wave action but with a **sharp acceleration at the tip** — the "crack" of a whip. The motion peaks suddenly at the end rather than maintaining smooth flow throughout.

## The principle

A whip has:
1. Slow setup at the handle (root)
2. Wave traveling along the length
3. **Sudden acceleration** at the tip
4. Sharp peak (the crack)
5. Quick relaxation back

The acceleration concentrates energy at the tip, which is why a real whip's tip can break the sound barrier.

## In animation

For a whip-action sequence:

```
Frame 1: whip root starts forward
Frame 5: wave propagating along whip
Frame 9: tip catches up to motion
Frame 10: TIP CRACK — 1 frame of impossible extension
Frame 11: tip recoils
Frame 12-20: whip relaxing back to neutral
```

The 1-frame "crack" at frame 10 is the visual equivalent of the sound. It's the moment that registers as "snap!" to the viewer.

## > *"You need 1 frame here, before the impact, to lead the eye. The crack lasts only 1 frame. The sound goes 1 frame after."*

So the sequence is:
- **1 frame before impact**: whip at near-extension (lead the eye to the crack point)
- **1 frame of crack**: whip at impossibly-extended position (the snap)
- **1 frame after**: whip beginning to retract
- **Sound effect**: synced to the frame AFTER the visual crack

The sound being 1 frame after the visual is critical — it matches how real whip cracks read.

## Whip action applied to other things

The same principle works for anything where you want a sudden "crack" or "snap":

### Tail snapping
A cat or whip-fast tail can snap. Apply whip pattern:
- Setup (slow tail movement)
- Wave along tail length
- 1-frame crack at tip
- Quick recoil

### Hair flipping
A character dramatically flipping long hair:
- Setup as head turns
- Wave through hair
- 1-frame "flip" at hair tip (impossibly extended)
- Hair settles back

### Sleeve / fabric flick
A character flicks sleeves with attitude. Whip pattern through the fabric.

### Cartoon laughs (the Tiny Tim example)
Like this..."*
> the classical tradition admits he didn't fully get it at first. But once he saw it, he realized:
> The shoulders rise and fall in the laugh and you can see the whip pattern operating among the lines in the action.
A laugh has whip action because each laugh-burst has a quick build, peak, and recoil — like a whip crack.

## the eye-flutter whip example

Without it, the flutter is just blinking.

## Whip vs. wave vs. snap

These three concepts overlap but are distinct:

### Wave
Smooth oscillation throughout. No sudden peak. Like a flag in steady wind.

### Whip
Slow buildup → sudden 1-frame extreme → quick recoil. Like cracking a whip.

### Snap
Instant transition without buildup. Used for sharp accents (a head snap to look at something). No "wave-through" phase.

## When to use whip action

- **Cartoon laughs**
- **Whip cracks (obvious)**
- **Tail snaps**
- **Hair flips for drama**
- **Sleeve/fabric flicks**
- **Tongue rolling out (for cartoon characters)**
- **Tongue stuck out**
- **Surprise eye pops (when extreme)**

## Prompt-ready language

### Video model — whip crack
> "Character cracks a whip dramatically. Arm pulls back (anticipation 8 frames). Arm swings forward, whip body follows in a wave (6 frames). At the peak of the swing, the whip TIP cracks — 1 single frame where the tip is impossibly extended past its natural length. Then the whip recoils back over 8 frames. The crack is a sharp visible moment, not a smooth motion."

### Video model — whip action laugh
> "Character laughs with whip-action body. Each laugh burst has the pattern: shoulders quickly rise (4 frames), peak with mouth wide (1 frame of maximum stretch), shoulders fall back quickly (3 frames). Multiple laugh bursts in sequence. The momentary peak in each burst is what gives the laugh its 'crack' quality."

### Video model — eye flutter with whip
> "Character flutters eyelids dramatically. Eyelids close slowly (6 frames), held closed briefly (2 frames), then SPRING open. At the moment of opening, eyes pop unnaturally wide for ONE single frame, then settle to natural wide. The 1-frame impossible extension is what makes the flutter read as dramatic and flirty."

### Code (whip action with crack)
```javascript
const whipCrack = gsap.timeline();

// Setup (slow)
whipCrack.to(whipTip, { rotation: -90, duration: 0.4, ease: "power2.in" });

// Wave traveling
whipCrack.to(whipMid, { rotation: -45, duration: 0.2, ease: "power2.in" }, 0.2);
whipCrack.to(whipRoot, { rotation: 30, duration: 0.15, ease: "power2.in" }, 0.3);

// Lead-in to crack (1 frame at near-extension)
whipCrack.to(whipTip, { rotation: 80, duration: 1/24, ease: "none" });

// THE CRACK (1 frame, impossibly extended)
whipCrack.to(whipTip, { rotation: 120, duration: 1/24, ease: "none" }); // impossible angle

// Recoil
whipCrack.to(whipTip, { rotation: 90, duration: 0.08, ease: "power2.out" });

// Relax back
whipCrack.to([whipTip, whipMid, whipRoot], { 
 rotation: 0, 
 duration: 0.4, 
 ease: "elastic.out(1, 0.4)" 
});

// Sound effect synced to AFTER the visual crack
whipCrack.call(() => playSound('whip_crack'), null, "+=0.04"); // 1 frame after
```

## Linked concepts

- [[wave-action]]
- [[side-to-side-wiggle-formula]]
- [[breaking-joints]]
- [[hard-accent-bounces]]
