//! The snapshot of editor state that a script operates on.
//!
//! Scripts receive a `ScriptContext` at call time. The context is a
//! clone of the relevant document state — not a live reference. Reads
//! are cheap because the data is already in-process; writes are
//! expressed as `ScriptMutation` values collected in `ScriptOutput`.

use std::sync::Arc;

use pixhaus_core::project::{FrameIndex, LayerId, Palette, PaletteId, Project, Rgba, SpriteId};

/// Editor state snapshot passed to a script at invocation time.
///
/// All fields are cloned from the live document so scripts see a
/// consistent view even if the document mutates concurrently.
#[derive(Clone, Debug)]
pub struct ScriptContext {
    /// The full project, or `None` when no project is open.
    pub project: Option<Arc<Project>>,
    /// Identifies the focused sprite by id. The host resolves it
    /// against the project library via [`Project::sprite`].
    ///
    /// Replaces the legacy `active_sprite_index: Option<usize>` field
    /// the B9.1 cleanup removed alongside `Project.sprites`. Hosts
    /// upgrading from the index form: set this to the id of whichever
    /// `NamedSprite` you previously addressed positionally.
    pub active_sprite_id: Option<SpriteId>,
    /// Index into the active sprite's `layers` for the focused layer.
    pub active_layer_index: Option<usize>,
    /// Index of the focused frame on the active sprite's timeline.
    pub active_frame_index: Option<FrameIndex>,
    /// Current foreground color in the editor.
    pub fg_color: Rgba,
    /// Current background color in the editor.
    pub bg_color: Rgba,
}

impl ScriptContext {
    /// Constructs a context with no open project and default black/white colors.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            project: None,
            active_sprite_id: None,
            active_layer_index: None,
            active_frame_index: None,
            fg_color: Rgba::opaque(0, 0, 0),
            bg_color: Rgba::opaque(255, 255, 255),
        }
    }

    /// Constructs a context wrapping a project snapshot.
    #[must_use]
    pub fn with_project(project: Project) -> Self {
        Self {
            project: Some(Arc::new(project)),
            active_sprite_id: None,
            active_layer_index: None,
            active_frame_index: None,
            fg_color: Rgba::opaque(0, 0, 0),
            bg_color: Rgba::opaque(255, 255, 255),
        }
    }
}

/// Resolves the active sprite via [`Project::sprite`], falling back to
/// the first sprite the library yields when `active_sprite_id` is
/// unset (matches the prior "implicit index 0" behaviour callers
/// relied on).
pub(crate) fn resolve_active_sprite(ctx: &ScriptContext) -> Option<&pixhaus_core::project::Sprite> {
    let project = ctx.project.as_deref()?;
    match ctx.active_sprite_id {
        Some(id) => project.sprite(id),
        None => project
            .sprites_iter()
            .next()
            .map(|(named, _)| &named.sprite),
    }
}

/// Reference to the active palette, if one can be resolved.
///
/// Returns the first palette on the active sprite, or `None` when
/// there is no active sprite or the sprite has no palettes.
pub fn active_palette(ctx: &ScriptContext) -> Option<&Palette> {
    resolve_active_sprite(ctx).and_then(|sprite| sprite.palettes.first())
}

/// Resolves an optional `LayerId` to the layer's index in the active sprite.
pub fn layer_index_by_id(ctx: &ScriptContext, id: LayerId) -> Option<usize> {
    let sprite = resolve_active_sprite(ctx)?;
    sprite.layers.iter().position(|l| l.id == id)
}

/// Resolves an optional `PaletteId` to the palette's index in the active sprite.
pub fn palette_index_by_id(ctx: &ScriptContext, id: PaletteId) -> Option<usize> {
    let sprite = resolve_active_sprite(ctx)?;
    sprite.palettes.iter().position(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_context_has_no_project() {
        let ctx = ScriptContext::empty();
        assert!(ctx.project.is_none());
        assert!(ctx.active_sprite_id.is_none());
    }

    #[test]
    fn with_project_wraps_in_arc() {
        let project = Project::new("test");
        let ctx = ScriptContext::with_project(project);
        assert!(ctx.project.is_some());
        assert!(ctx.active_sprite_id.is_none());
    }
}
