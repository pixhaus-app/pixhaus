//! The snapshot of editor state that a script operates on.
//!
//! Scripts receive a `ScriptContext` at call time. The context is a
//! clone of the relevant document state — not a live reference. Reads
//! are cheap because the data is already in-process; writes are
//! expressed as `ScriptMutation` values collected in `ScriptOutput`.

use std::sync::Arc;

use pixhaus_core::project::{FrameIndex, LayerId, Palette, PaletteId, Project, Rgba};

/// Editor state snapshot passed to a script at invocation time.
///
/// All fields are cloned from the live document so scripts see a
/// consistent view even if the document mutates concurrently.
#[derive(Clone, Debug)]
pub struct ScriptContext {
    /// The full project, or `None` when no project is open.
    pub project: Option<Arc<Project>>,
    /// Index into `project.sprites` identifying the focused sprite.
    pub active_sprite_index: Option<usize>,
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
            active_sprite_index: None,
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
            active_sprite_index: None,
            active_layer_index: None,
            active_frame_index: None,
            fg_color: Rgba::opaque(0, 0, 0),
            bg_color: Rgba::opaque(255, 255, 255),
        }
    }
}

/// Reference to the active palette, if one can be resolved.
///
/// Returns the first palette on the active sprite, or `None` when
/// there is no active sprite or the sprite has no palettes.
pub fn active_palette(ctx: &ScriptContext) -> Option<&Palette> {
    let project = ctx.project.as_deref()?;
    let sprite = project.sprites.get(ctx.active_sprite_index.unwrap_or(0))?;
    sprite.palettes.first()
}

/// Resolves an optional `LayerId` to the layer's index in the active sprite.
pub fn layer_index_by_id(ctx: &ScriptContext, id: LayerId) -> Option<usize> {
    let project = ctx.project.as_deref()?;
    let sprite = project.sprites.get(ctx.active_sprite_index.unwrap_or(0))?;
    sprite.layers.iter().position(|l| l.id == id)
}

/// Resolves an optional `PaletteId` to the palette's index in the active sprite.
pub fn palette_index_by_id(ctx: &ScriptContext, id: PaletteId) -> Option<usize> {
    let project = ctx.project.as_deref()?;
    let sprite = project.sprites.get(ctx.active_sprite_index.unwrap_or(0))?;
    sprite.palettes.iter().position(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_context_has_no_project() {
        let ctx = ScriptContext::empty();
        assert!(ctx.project.is_none());
        assert!(ctx.active_sprite_index.is_none());
    }

    #[test]
    fn with_project_wraps_in_arc() {
        let project = Project::new("test");
        let ctx = ScriptContext::with_project(project);
        assert!(ctx.project.is_some());
        assert!(ctx.active_sprite_index.is_none());
    }
}
