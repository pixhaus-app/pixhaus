# Filling the Overlay and catch-all arms

Two arms, two different problems — don't reach for the same macro on both.

```rust
fn blend(mode: BlendMode, src: Rgba8, dst: Rgba8) -> Rgba8 {
    match mode {
        BlendMode::Normal => blend_normal(src, dst),
        BlendMode::Multiply => blend_multiply(src, dst),

        // Not implemented yet — coming back to this. `unimplemented!` is a
        // deliberate "decided not to build this branch yet" marker, which
        // reads truer than `todo!` here. Either way the Stop gate's
        // `-D warnings` will reject the session while this is present, so it
        // can't quietly ship: it forces you back before the session goes green.
        BlendMode::Overlay => unimplemented!("Overlay blend mode"),

        // The upstream dispatcher guarantees this never reaches here. The
        // invariant is enforced *outside* this function, so the type can't
        // express it — hence `unreachable!` with the invariant stated. This
        // slips clippy and survives `-D warnings`, so the gate stays green.
        other => unreachable!(
            "blend() received {other:?}, which the dispatcher filters out before this call"
        ),
    }
}
```

## Why each arm is what it is

These two arms look similar — both are "I'm not handling this normally" — but they are opposite cases, and the workspace lint config treats them very differently. The repo sets `panic`/`unwrap_used`/`expect_used` to **deny**, `todo`/`unimplemented` to **warn**, and the Stop gate runs `cargo clippy --workspace --all-targets -- -D warnings`, which promotes every warning to a hard error.

### The Overlay arm: `unimplemented!` (or `todo!`)

This is genuine "come back to it" scaffolding. You have two defensible choices:

- `todo!("Overlay blend mode")` — "I will write this."
- `unimplemented!("Overlay blend mode")` — "a branch I've decided not to build yet."

Both are clippy `warn`, not `deny`, so they compile during your edit loop. Pick `unimplemented!` if you want it to read as a deliberate not-yet-built branch; `todo!` if it reads more as active-edit scaffolding. The wording difference is the only thing at stake — mechanically they behave identically here.

The key thing to be honest about: **neither survives the Stop gate.** Because `-D warnings` promotes the `warn`-level lint to an error, a session that still contains `Overlay => unimplemented!(...)` will fail `cargo clippy --workspace --all-targets -- -D warnings`. That's the desired behavior — it's a safety net, not a finish line. It means:

- You can keep working with this arm in place; the per-edit clippy (`--tests -- -D warnings` via the post-edit hook on the touched crate) and the Stop gate will both flag it, so you get the reminder loud and early.
- You will **not** accidentally ship an unimplemented blend mode to production. The gate physically won't let the session close clean until Overlay is either implemented or turned into a real `Err`.

So if you want the Stop gate green *right now*, Overlay isn't done — you'd have to implement it. If you're fine with the gate flagging the one known-unfinished branch while you iterate, `unimplemented!`/`todo!` is exactly the right placeholder and won't cause a surprise panic in production, because production code only ships after a clean gate.

Do **not** use `panic!`, `unwrap()`, or `expect()` to stub this — they're `deny`, so they won't even compile in non-test code.

### The catch-all arm: `unreachable!`

This is the opposite case. You're not deferring work — you're asserting a branch can never execute because the dispatcher upstream filters that variant before `blend()` is ever called.

`unreachable!` is the one panic-family macro that is **not linted at all**: it isn't `deny`, it isn't `warn`, so it compiles and it passes the `-D warnings` gate. That's exactly why it fits the catch-all and not the Overlay arm — it lets the session go green, which is what you asked for.

The honest caveat: `unreachable!` is still a live `panic!` wearing a permission slip. It slips both clippy and the gate, which makes it the dangerous one. So two rules apply:

1. **Only use it on a branch you've genuinely proven impossible.** Here the proof lives upstream in the dispatcher, so it's legitimate — but the proof is outside this function, which is why you can't simply rely on the compiler.
2. **State the invariant in the message.** The comment and the `unreachable!("... which the dispatcher filters out ...")` string record *why* it can't happen, so a future reader (or a future refactor that changes the dispatcher) knows what assumption is being leaned on. The `{other:?}` also gives you the offending variant in the panic message if the invariant is ever violated.

### The better long-term move (optional)

`unreachable!` is a runtime gamble — it's only sound as long as the dispatcher keeps its promise. If you can, make the branch impossible *in the type system* instead of asserting it at runtime: have the dispatcher hand `blend()` a narrower type that simply can't represent the filtered-out variant (a separate enum of "blendable" modes, or a newtype the dispatcher constructs only after filtering). Then there's no catch-all arm to write, the match is exhaustive over real cases, and "this can't happen" becomes a compile error rather than a deferred panic. That's the type-state direction (`pixhaus-type-state`); it's more work than this task needs today, but it's the move that retires the `unreachable!` entirely.

## Summary

- **Overlay** → `unimplemented!("Overlay blend mode")` (or `todo!`). Compiles now, no production panic risk, but the Stop gate will flag it via `-D warnings` until you finish it. That's intended.
- **Catch-all** → `unreachable!("... dispatcher filters this out ...")`. Passes the gate (not linted), no surprise in practice as long as the dispatcher invariant holds. Comment the invariant; ideally encode it in the type later so the arm disappears.
- Never stub either arm with `panic!`/`unwrap()`/`expect()` — they're `deny` and won't compile outside tests.
