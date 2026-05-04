# Sprite sheet import sample

This sample shows what a Pixhaus sprite sheet export looks like and how to wire it into
a scene with the PixhausAnimator component.

## Files

- `hero.pixhaussprite` — Pixhaus sprite sheet JSON (rename your export from `.json` to
  `.pixhaussprite` so Unity uses the Pixhaus importer automatically).
- `hero.png` — The packed sprite sheet PNG (not included; see below).

## Setup

1. Copy both this folder's contents into your Unity project's Assets folder.
2. Create a 64x16 RGBA PNG named `hero.png` in the same folder as `hero.pixhaussprite`.
   It should contain four 16x16 frames arranged left to right. Any four distinct visuals
   work for testing.
3. Unity imports `hero.pixhaussprite` and produces:
   - `hero` Texture2D (main asset)
   - `hero 0`, `hero 1`, `hero 2`, `hero 3` Sprite sub-assets
   - `idle` AnimationClip (frame 0, looping)
   - `walk` AnimationClip (frames 1-3, looping)

## Using PixhausAnimator

```csharp
using Pixhaus.Runtime;
using UnityEngine;

public class HeroController : MonoBehaviour
{
    private PixhausAnimator anim;

    private void Awake()
    {
        anim = GetComponent<PixhausAnimator>();
    }

    private void Update()
    {
        if (Input.GetAxisRaw("Horizontal") != 0)
            anim.Play("walk");
        else
            anim.Play("idle");
    }
}
```

Add a GameObject to the scene with:
- `SpriteRenderer` component
- `PixhausAnimator` component with the `tags` array populated from the import
- The `HeroController` script above

## Using Unity's Animator

Drag the `idle` or `walk` AnimationClip sub-assets into an AnimatorController as
you would any other clip. This path integrates with blend trees, parameters, and
transitions using the full Unity animation system.
