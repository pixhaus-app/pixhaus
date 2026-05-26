# Filling the Overlay and catch-all arms

Two arms, two different jobs. They look similar but the reasoning is opposite, so don't reach for the same macro on both.

```rust
fn blend(mode: BlendMode, src: Rgba8, dst: Rgba8) -> Rgba8 {
    match mode {
        BlendMode::Normal => blend_normal(src, dst),
        BlendMode::Multiply => blend_multiply(src, dst),

        // Not implemented yet — coming back to this.
        // `todo!()` documents intent and keeps the Stop gate green:
        // clippy's `todo` lint is set to `warn`, not `deny`, so
        // `-D warnings` does NOT promote it to an error here.
        BlendMode::Overlay => todo!("Overlay blend math"),

        // The upstream dispatcher guarantees this variant never reaches
        // `blend`. We encode that invariant rather than papering over it
        // with a silent fallback. `unreachable!()` is the right tool for
        // "the type system allows it but the program logic forbids it."
        other => unreachable!("blend() received {other:?}, which the dispatcher filters out"),
    }
}
```

## Why `todo!()` for the unfinished Overlay arm

You want three things from the placeholder: it satisfies the type checker (the arm must produce an `Rgba8`), it survives the Stop gate, and it screams "finish me" so it can't quietly ship.

`todo!()` gives you all three. It diverges (`-> !`), so it type-checks in an arm that's supposed to return a value without you having to invent a fake return. And critically for your gate: the workspace clippy config sets `clippy::todo` to **warn**, not deny. The Stop gate runs `cargo clippy --workspace --all-targets -- -D warnings`. `-D warnings` turns the *standard* `warn`-level lints into errors, but it does not change lint levels you've explicitly pinned in config — a configured `warn` stays a warning. So `todo!()` shows up as a warning in your local build, nags you, and does not fail the gate.

Don't use `unimplemented!()` here. It's also configured to warn, so the gate behaves the same, but the semantics differ: `todo!()` means "I intend to write this," `unimplemented!()` means "this case is intentionally not supported." Overlay is the former. Pick the macro that tells the truth about your intent.

Don't substitute a real-but-wrong value (e.g. falling through to `blend_normal`) just to make the arm compile silently. That's worse than a placeholder — it's a correctness bug that the gate can't catch and a reviewer might miss. The whole point of `todo!()` is that it's visible.

## Why `unreachable!()` for the catch-all — not `_ => {}` and not `panic!()`

The catch-all is a different problem. This variant *can't* reach the function per the dispatcher's contract, but the compiler doesn't know that — the `match` still has to be exhaustive over `BlendMode`. You're encoding an invariant the type system can't express.

`unreachable!()` is exactly that statement: "control flow cannot get here; if it does, my reasoning was wrong." It diverges so the arm type-checks, and the message documents the contract for the next reader. If the dispatcher ever regresses and lets the variant through, you get a loud, located panic in a debug/test build instead of silent corruption — which is what you want from a violated invariant.

On the gate: `clippy::unreachable` is **not** in the deny set you described (only `panic`, `unwrap_used`, `expect_used` are denied; `todo`/`unimplemented` warn). `unreachable!()` isn't flagged by those, so it passes clippy clean. It expands to a `panic!` under the hood, but the `clippy::panic` lint targets literal `panic!()` calls in your source, not the macro's internal expansion — so writing `unreachable!()` does not trip the `panic` deny.

That's also why you must **not** write `panic!("unreachable")` by hand here: that literal `panic!` *is* what `clippy::panic = deny` catches, and it would fail the Stop gate. `unreachable!()` is the sanctioned way to say the same thing.

Two more options to reject:

- **`_ => {}` / a silent default value.** A catch-all that returns some default (or an empty block, if this returned `()`) hides the bug. If the dispatcher contract breaks, you'd ship wrong pixels with no signal. The invariant deserves an assertion, not a swallow.
- **`unreachable_unchecked()`** (the `core::hint` one). That's `unsafe`, and `unsafe` is forbidden workspace-wide. Not an option, and you don't want UB guarding a cross-module contract anyway.

## The "surprise panic in production" concern

Both macros panic if hit. The difference is whether a hit is *possible under correct operation*:

- The `unreachable!()` arm only fires if an upstream invariant is already violated — i.e. there's a bug. A panic there is the correct, loud failure, not a surprise. The alternative (silent fallback) would be the actual landmine.
- The `todo!()` arm *will* panic if Overlay is ever selected before you finish it. That's the one to be deliberate about. It's fine as a checked-in placeholder *because* it's loud and the warning reminds you, but don't let a release go out with a user-reachable `todo!()`. Before you ship Overlay as a selectable mode, the arm must hold real blend math. Treat the clippy warning as a release blocker, not noise.

If you'd rather the unfinished path degrade gracefully in production instead of panicking, the alternative is to not list Overlay as selectable yet (gate it out in the dispatcher / UI), so the `todo!()` stays genuinely unreachable until implemented. But as long as Overlay can't be picked, `todo!()` is the honest placeholder and the gate stays green.
