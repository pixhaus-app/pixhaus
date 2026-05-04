using System;
using UnityEngine;

namespace Pixhaus.Runtime
{
    // Drives sprite animation without a Unity Animator or AnimatorController.
    //
    // Populate the tags array (via the Inspector or from code) with PixhausAnimationTag
    // instances sourced from a .pixhaussprite import. Call Play(tagName) to start an
    // animation. The component updates the attached SpriteRenderer each frame.
    //
    // For blend trees, transitions, or complex state machines, use the AnimationClip
    // sub-assets from the import with a standard Unity Animator instead.

    [RequireComponent(typeof(SpriteRenderer))]
    [AddComponentMenu("Pixhaus/Pixhaus Animator")]
    public class PixhausAnimator : MonoBehaviour
    {
        [SerializeField] public PixhausAnimationTag[] tags;
        [SerializeField] public string defaultTag;
        [SerializeField] public float speed = 1f;

        // Fired when a non-looping animation plays to completion.
        public event Action<string> OnAnimationComplete;

        private SpriteRenderer spriteRenderer;
        private PixhausAnimationTag current;
        private float elapsed;
        private bool isPlaying;
        private bool completeFired;

        public string CurrentTag => current?.name;
        public bool IsPlaying => isPlaying;

        private void Awake()
        {
            spriteRenderer = GetComponent<SpriteRenderer>();
        }

        private void Start()
        {
            if (!string.IsNullOrEmpty(defaultTag))
                Play(defaultTag);
        }

        private void Update()
        {
            if (!isPlaying || current == null)
                return;

            elapsed += Time.deltaTime * speed;

            var sprite = current.GetSpriteAt(elapsed);
            if (sprite != null)
                spriteRenderer.sprite = sprite;

            if (current.wrapMode == WrapMode.Once && !completeFired)
            {
                float total = current.TotalDuration;
                if (elapsed >= total)
                {
                    completeFired = true;
                    isPlaying = false;
                    OnAnimationComplete?.Invoke(current.name);
                }
            }
        }

        // Start playing a named tag from the beginning.
        // No-ops if the tag name is not found; logs a warning.
        public void Play(string tagName)
        {
            var tag = FindTag(tagName);
            if (tag == null)
            {
                Debug.LogWarning($"[PixhausAnimator] Tag '{tagName}' not found on {name}.", this);
                return;
            }

            current = tag;
            elapsed = 0f;
            isPlaying = true;
            completeFired = false;
        }

        // Resume the current animation from the given normalized position [0, 1].
        public void PlayFrom(string tagName, float normalizedTime)
        {
            Play(tagName);
            if (current != null)
                elapsed = normalizedTime * current.TotalDuration;
        }

        public void Stop()
        {
            isPlaying = false;
            elapsed = 0f;
        }

        public void Pause()
        {
            isPlaying = false;
        }

        public void Resume()
        {
            if (current != null)
                isPlaying = true;
        }

        public bool IsPlayingTag(string tagName) =>
            isPlaying && current != null && current.name == tagName;

        private PixhausAnimationTag FindTag(string tagName)
        {
            if (tags == null) return null;
            foreach (var tag in tags)
            {
                if (tag != null && tag.name == tagName)
                    return tag;
            }
            return null;
        }
    }
}
