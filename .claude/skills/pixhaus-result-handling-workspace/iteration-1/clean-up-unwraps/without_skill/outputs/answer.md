# Rewritten `layer_ops.rs`

The workspace forbids `unwrap()`/`expect()`/`panic!()` outside tests (clippy `-D warnings`). Each call site falls into one of two buckets: it can surface a real error (so return `Result`), or it has a defensible non-failing fallback (so use `unwrap_or`-style helpers). I picked per site below.

The one signature I changed is `active_opacity` — it returned a bare `f32` but was relying on `unwrap()` for two separate "might not exist" cases. The honest fix is a fallback, not a new `Result`, so I kept it `f32` but documented the default. `active_layer_name` similarly can't fail meaningfully if we accept that "no active layer" yields an empty name, but the active index is a document invariant, so I made it return `Result` to avoid silently masking corruption — see note. (If you'd rather not touch that signature, the fallback variant is in the notes.)

```rust
// pixhaus_core::layer — layer operations.

use crate::{Error, Result};

pub struct Document {
    layers: Vec<Layer>,
    active: usize,
}

pub struct Layer {
    pub name: String,
    pub opacity: Option<f32>,
}

/// Opacity to assume when a layer carries no explicit value.
const DEFAULT_OPACITY: f32 = 1.0;

impl Document {
    pub fn active_layer_name(&self) -> Result<String> {
        let layer = self
            .layers
            .get(self.active)
            .ok_or(Error::NoActiveLayer)?;
        Ok(layer.name.clone())
    }

    pub fn active_opacity(&self) -> f32 {
        self.layers
            .get(self.active)
            .and_then(|layer| layer.opacity)
            .unwrap_or(DEFAULT_OPACITY)
    }

    pub fn rename_active(&mut self, raw: &str) -> Result<()> {
        let trimmed = parse_layer_name(raw)?;
        self.layers
            .get_mut(self.active)
            .ok_or(Error::NoActiveLayer)?
            .name = trimmed;
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

## Per-change notes

- **`active_layer_name` line 17 `self.layers.get(self.active).unwrap()`** → `ok_or(Error::NoActiveLayer)?`, signature now `-> Result<String>`. A bad active index means a corrupt document, not a normal miss; returning an error surfaces it instead of panicking the editor. Fallback alternative if you must keep `-> String`: `.map(|l| l.name.clone()).unwrap_or_default()`.

- **`active_opacity` line 22 `.expect("active index valid")`** → folded into `.get(...).and_then(...).unwrap_or(DEFAULT_OPACITY)`. Opacity has a sensible default (fully opaque), and a missing layer here just collapses to that default rather than crashing a render/UI read path that can't propagate errors.

- **`active_opacity` line 23 `layer.opacity.unwrap()`** → handled by the same `and_then(|layer| layer.opacity)` chain feeding `unwrap_or(DEFAULT_OPACITY)`. `opacity` is an `Option<f32>` by design, so `None` is expected, not exceptional — give it the default instead of asserting it's set.

- **`rename_active` line 27 `parse_layer_name(raw).unwrap()`** → `?`. The function already returns `Result` and `parse_layer_name` returns the exact `Error::EmptyLayerName` we want to propagate; `?` forwards it for free.

- **`rename_active` line 28 `self.layers.get_mut(self.active).unwrap()`** → `ok_or(Error::NoActiveLayer)?`. Same invariant as `active_layer_name`, but here the caller already gets a `Result`, so propagating the error is the natural choice and changes no signature.

Note: this assumes the crate's `Error` enum has a `NoActiveLayer` variant (alongside the existing `EmptyLayerName`). If it doesn't yet, add it to the `thiserror` enum in the error module — that's the one extra edit needed for this to compile.
