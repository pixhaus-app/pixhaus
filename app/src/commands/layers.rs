//! Layer CRUD and property commands.

use pixhaus_core::project::{BlendMode, Layer, LayerId, LayerKind, SpriteId, UserData};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Arguments for adding a new layer.
#[derive(Debug, Deserialize)]
pub struct LayerAddArgs {
    /// Sprite to add the layer to.
    pub sprite_id: SpriteId,
    /// Display name for the new layer.
    pub name: String,
    /// Variant-specific payload (raster, group, tilemap, reference).
    pub kind: LayerKind,
}

/// Adds a new layer to a sprite. The layer is appended above all existing layers.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_add(args: LayerAddArgs, state: State<'_, AppState>) -> CommandResult<Layer> {
    let mut doc = state.doc.write().await;
    let id = LayerId::new(doc.next_id);
    doc.next_id += 1;
    let layer = {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprites
            .iter_mut()
            .find(|s| s.id == args.sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(args.sprite_id.get()),
            })?;
        let layer = Layer {
            id,
            name: args.name,
            kind: args.kind,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            user_data: UserData::default(),
        };
        sprite.layers.push(layer.clone());
        layer
    };
    doc.dirty = true;
    Ok(layer)
}

/// Removes a layer from a sprite by ID. Also removes all cels on that layer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_delete(
    sprite_id: SpriteId,
    layer_id: LayerId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprites
            .iter_mut()
            .find(|s| s.id == sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        let before = sprite.layers.len();
        sprite.layers.retain(|l| l.id != layer_id);
        if sprite.layers.len() == before {
            return Err(AppCommandError::NotFound {
                entity: "layer".into(),
                id: u64::from(layer_id.get()),
            });
        }
        // Remove cels that belong to the deleted layer.
        sprite.cels.retain(|c| c.layer_id != layer_id);
    }
    doc.dirty = true;
    Ok(())
}

/// Moves a layer to a new position in the layer stack.
///
/// `new_index` is clamped to `[0, layers.len() - 1]`.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_reorder(
    sprite_id: SpriteId,
    layer_id: LayerId,
    new_index: u32,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprites
            .iter_mut()
            .find(|s| s.id == sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        let pos = sprite.layers.iter().position(|l| l.id == layer_id).ok_or(
            AppCommandError::NotFound {
                entity: "layer".into(),
                id: u64::from(layer_id.get()),
            },
        )?;
        let target = (new_index as usize).min(sprite.layers.len().saturating_sub(1));
        let layer = sprite.layers.remove(pos);
        sprite.layers.insert(target, layer);
    }
    doc.dirty = true;
    Ok(())
}

/// Sets the blend mode for a layer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_set_blend_mode(
    sprite_id: SpriteId,
    layer_id: LayerId,
    blend_mode: BlendMode,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let layer = find_layer_mut(&mut doc, sprite_id, layer_id)?;
        layer.blend_mode = blend_mode;
    }
    doc.dirty = true;
    Ok(())
}

/// Sets the opacity for a layer (`0` = fully transparent, `255` = fully opaque).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_set_opacity(
    sprite_id: SpriteId,
    layer_id: LayerId,
    opacity: u8,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let layer = find_layer_mut(&mut doc, sprite_id, layer_id)?;
        layer.opacity = opacity;
    }
    doc.dirty = true;
    Ok(())
}

/// Sets the visibility of a layer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_set_visibility(
    sprite_id: SpriteId,
    layer_id: LayerId,
    visible: bool,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let layer = find_layer_mut(&mut doc, sprite_id, layer_id)?;
        layer.visible = visible;
    }
    doc.dirty = true;
    Ok(())
}

/// Sets the locked state of a layer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_set_locked(
    sprite_id: SpriteId,
    layer_id: LayerId,
    locked: bool,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let layer = find_layer_mut(&mut doc, sprite_id, layer_id)?;
        layer.locked = locked;
    }
    doc.dirty = true;
    Ok(())
}

/// Returns all layers in a sprite, bottom to top (index 0 is the bottom layer).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_list(
    sprite_id: SpriteId,
    state: State<'_, AppState>,
) -> CommandResult<Vec<Layer>> {
    let doc = state.doc.write().await;
    let sprite = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?
        .sprites
        .iter()
        .find(|s| s.id == sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    Ok(sprite.layers.clone())
}

/// Serializable summary of a layer rename operation.
#[derive(Debug, Serialize)]
pub struct LayerRenamed {
    /// The layer that was renamed.
    pub layer_id: LayerId,
    /// The new display name.
    pub name: String,
}

/// Renames a layer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_rename(
    sprite_id: SpriteId,
    layer_id: LayerId,
    name: String,
    state: State<'_, AppState>,
) -> CommandResult<LayerRenamed> {
    let mut doc = state.doc.write().await;
    {
        let layer = find_layer_mut(&mut doc, sprite_id, layer_id)?;
        layer.name.clone_from(&name);
    }
    doc.dirty = true;
    Ok(LayerRenamed { layer_id, name })
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn find_layer_mut(
    doc: &mut crate::state::DocumentStore,
    sprite_id: SpriteId,
    layer_id: LayerId,
) -> CommandResult<&mut Layer> {
    doc.project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?
        .sprites
        .iter_mut()
        .find(|s| s.id == sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?
        .layers
        .iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or(AppCommandError::NotFound {
            entity: "layer".into(),
            id: u64::from(layer_id.get()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::LayerKind;

    #[test]
    fn layer_constructed_with_defaults() {
        let id = LayerId::new(1);
        let layer = Layer {
            id,
            name: "bg".into(),
            kind: LayerKind::Raster,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            user_data: UserData::default(),
        };
        assert_eq!(layer.opacity, 255);
        assert!(layer.visible);
        assert!(!layer.locked);
    }
}
