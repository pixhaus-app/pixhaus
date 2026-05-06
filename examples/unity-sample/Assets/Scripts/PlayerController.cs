using Pixhaus.Runtime;
using UnityEngine;

namespace PixhausSample
{
    // Moves the player and drives PixhausAnimator tags.
    //
    // Requires a PixhausAnimator component on the same GameObject. The animator
    // is found in Awake; if it is missing, movement still works but no animation
    // plays and a warning is logged once.
    //
    // Input: Unity's legacy Input Manager axes ("Horizontal", "Vertical", "Fire1").
    // Run:   hold Left Shift while moving.
    // Attack: press "Fire1" (Left Ctrl or left mouse button by default).

    [RequireComponent(typeof(Rigidbody2D))]
    [RequireComponent(typeof(SpriteRenderer))]
    public class PlayerController : MonoBehaviour
    {
        [SerializeField] public float moveSpeed = 3f;
        [SerializeField] public float runSpeed  = 6f;

        private Rigidbody2D     rb;
        private SpriteRenderer  sr;
        private PixhausAnimator anim;

        private bool attackPending;

        private void Awake()
        {
            rb   = GetComponent<Rigidbody2D>();
            sr   = GetComponent<SpriteRenderer>();
            anim = GetComponent<PixhausAnimator>();

            if (anim == null)
                Debug.LogWarning("[PlayerController] PixhausAnimator not found. Add it and assign tags.", this);
        }

        private void Start()
        {
            Play("idle");
        }

        private void Update()
        {
            attackPending = Input.GetButtonDown("Fire1");
        }

        private void FixedUpdate()
        {
            var h = Input.GetAxisRaw("Horizontal");
            var v = Input.GetAxisRaw("Vertical");

            bool moving  = h != 0f || v != 0f;
            bool running = moving && Input.GetKey(KeyCode.LeftShift);

            var dir = new Vector2(h, v).normalized;
            var speed = running ? runSpeed : moveSpeed;
            rb.linearVelocity = dir * speed;

            // Flip sprite to face movement direction.
            if (h < 0f) sr.flipX = true;
            else if (h > 0f) sr.flipX = false;

            // Animation priority: attack > run > walk > idle.
            if (attackPending)
            {
                attackPending = false;
                PlayOnce("attack");
                return;
            }

            if (anim != null && anim.IsPlayingTag("attack") && anim.IsPlaying)
                return; // let attack finish

            if (running)      Play("run");
            else if (moving)  Play("walk");
            else              Play("idle");
        }

        private void Play(string tag)     => anim?.Play(tag);

        private void PlayOnce(string tag)
        {
            anim?.Play(tag);
            if (anim != null)
                anim.OnAnimationComplete += OnAttackComplete;
        }

        private void OnAttackComplete(string tag)
        {
            if (anim != null)
                anim.OnAnimationComplete -= OnAttackComplete;
            Play("idle");
        }
    }
}
