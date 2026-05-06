using Pixhaus.Runtime;
using UnityEngine;

namespace PixhausSample
{
    // Simple back-and-forth patrol for the sample slime enemy.
    //
    // The enemy walks horizontally between two waypoints separated by
    // patrolDistance units. When it reaches either endpoint it reverses
    // direction and flips the sprite.
    //
    // Requires a PixhausAnimator on the same GameObject. If the animator is
    // missing, patrol movement still works and a one-time warning is logged.

    [RequireComponent(typeof(Rigidbody2D))]
    [RequireComponent(typeof(SpriteRenderer))]
    public class EnemyPatrol : MonoBehaviour
    {
        [SerializeField] public float patrolDistance = 3f;
        [SerializeField] public float speed          = 1.5f;

        private Rigidbody2D     rb;
        private SpriteRenderer  sr;
        private PixhausAnimator anim;

        private Vector2 origin;
        private float   direction = 1f; // +1 = right, -1 = left

        private void Awake()
        {
            rb   = GetComponent<Rigidbody2D>();
            sr   = GetComponent<SpriteRenderer>();
            anim = GetComponent<PixhausAnimator>();

            if (anim == null)
                Debug.LogWarning("[EnemyPatrol] PixhausAnimator not found. Add it and assign tags.", this);
        }

        private void Start()
        {
            origin = rb.position;
            anim?.Play("hop");
        }

        private void FixedUpdate()
        {
            var pos = rb.position;
            var offset = pos.x - origin.x;

            // Reverse at patrol endpoints.
            if (offset >= patrolDistance)
            {
                direction = -1f;
                sr.flipX  = true;
            }
            else if (offset <= 0f)
            {
                direction = 1f;
                sr.flipX  = false;
            }

            rb.linearVelocity = new Vector2(direction * speed, rb.linearVelocity.y);
        }

        // Called by other scripts (e.g., a collision handler) to play the hit animation.
        public void TakeHit()
        {
            if (anim == null) return;

            anim.Play("hit");
            anim.OnAnimationComplete += OnHitComplete;
        }

        private void OnHitComplete(string tag)
        {
            if (anim == null) return;

            anim.OnAnimationComplete -= OnHitComplete;
            anim.Play("hop");
        }
    }
}
