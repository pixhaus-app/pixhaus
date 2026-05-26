# Rewritten `layer_ops.rs`

Every `unwrap`/`expect` is gone. The two infallible getters keep their plain
return types and recover with a sensible default; the one `Result`-returning
method propagates with `?` and turns a bad index into a typed error.

One assumption worth flagging: `rename_active` needs an `Error` variant for an
out-of-range active index. The skill's own example uses
`Error::LayerIndexOutOfRange { index }`, so I used that. If your `Error` enum
names it differently, swap the variant — the call-site shape (`ok_or_else(..)?`)
stays the same.

```rust
// pixhaus_core::layer — clippy-clean: no unwrap/expect outside tests.

use crate::{Error, Result};

pub struct Document {
    layers: Vec<Layer>,
    active: usize,
}

pub struct Layer {
    pub name: String,
    pub opacity: Option<f32>,
}

impl Document {
    pub fn active_layer_name(&self) -> String {
        self.layers
            .get(self.active)
            .map(|layer| layer.name.clone())
            .unwrap_or_default()
    }

    pub fn active_opacity(&self) -> f32 {
        self.layers
            .get(self.active)
            .and_then(|layer| layer.opacity)
            .unwrap_or(1.0)
    }

    pub fn rename_active(&mut self, raw: &str) -> Result<()> {
        let trimmed = parse_layer_name(raw)?;
        let layer = self
            .layers
            .get_mut(self.active)
            .ok_or_else(|| Error::LayerIndexOutOfRange { index: self.active })?;
        layer.name = trimmed;
        Ok(())
    }
}

fn parse_layer_name(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(Error::EmptyLayerName);
    }
    Ok(trimmed.to_string())
}
```

## Per-change rationale

- **`active_layer_name` — `.get(..).unwrap()` → `.map(..).unwrap_or_default()`**: the fn returns a bare `String`, not a `Result`, so there's no caller to bubble to; a missing active layer yields an empty name (a harmless display string) instead of crashing the editor. Signature kept.
- **`active_opacity` — `.get(..).expect(..)` and `opacity.unwrap()` → one `.and_then(..).unwrap_or(1.0)`**: both the missing-layer and the `None`-opacity cases collapse to the same sensible fallback — fully opaque (`1.0`). `unwrap_or` (not `unwrap_or_else`) because `1.0` is a trivial constant, so eager eval is free. Signature (`-> f32`) kept.
- **`rename_active` — `parse_layer_name(raw).unwrap()` → `parse_layer_name(raw)?`**: this fn already returns `Result<()>`, so an empty-name parse failure should travel to the caller; `?` propagates `Error::EmptyLayerName` and runs any `From` conversion for free.
- **`rename_active` — `.get_mut(self.active).unwrap()` → `.ok_or_else(|| Error::LayerIndexOutOfRange { index: self.active })?`**: a bad active index is a real error this `Result` fn can report, so turn the `None` into a typed variant and `?` it; `ok_or_else` (lazy) because constructing the variant is non-trivial, matching the skill's `Option`-to-`Result` rule. (No order/borrow issue: the lookup is bound before the assignment, so the `&mut` borrow ends cleanly.)
```
```

## Notes

- No `todo!`/`unimplemented!`/`unreachable!` introduced, so nothing trips the Stop gate's `-D warnings`.
- `parse_layer_name` was already clean and is unchanged.
- Both getters stay infallible by design — they're cheap accessors used in hot UI paths, and forcing every caller to handle a `Result` for "which layer is active" would be churn for no safety gain. The fallbacks (empty name, opaque `1.0`) are the right empty values, which is exactly when `unwrap_or*` is the correct tool rather than a typed error.
