using System;
using UnityEngine;

namespace Pixhaus.Runtime
{
    // One named animation tag imported from a Pixhaus sprite sheet.
    // Consumed by PixhausAnimator for scripted playback.
    // The importer populates these; do not construct manually at runtime.

    [Serializable]
    public class PixhausAnimationTag
    {
        public string name;
        public Sprite[] sprites;   // frames in playback order (already reversed for "reverse" direction)
        public float[] durations;  // per-frame duration in seconds, parallel to sprites
        public WrapMode wrapMode;  // Loop, Once, or PingPong

        // Total clip length in seconds.
        public float TotalDuration
        {
            get
            {
                if (durations == null) return 0f;
                float total = 0f;
                foreach (var d in durations) total += d;
                return total;
            }
        }

        // Returns the sprite that should be displayed at the given elapsed time.
        // Clamps at the last frame for WrapMode.Once; wraps for Loop/PingPong.
        public Sprite GetSpriteAt(float elapsed)
        {
            if (sprites == null || sprites.Length == 0)
                return null;

            float total = TotalDuration;
            if (total <= 0f)
                return sprites[0];

            float t = WrapTime(elapsed, total);

            float acc = 0f;
            for (int i = 0; i < sprites.Length; i++)
            {
                acc += durations[i];
                if (t < acc)
                    return sprites[i];
            }
            return sprites[sprites.Length - 1];
        }

        private float WrapTime(float t, float total)
        {
            if (t <= 0f) return 0f;

            switch (wrapMode)
            {
                case WrapMode.Loop:
                case WrapMode.Default:
                    return t % total;

                case WrapMode.PingPong:
                    float cycle = total * 2f;
                    float mod = t % cycle;
                    return mod < total ? mod : cycle - mod;

                case WrapMode.Once:
                case WrapMode.ClampForever:
                default:
                    return Mathf.Min(t, total - Mathf.Epsilon);
            }
        }
    }
}
