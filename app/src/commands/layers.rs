//! Layer CRUD and property commands.

use pixhaus_core::project::{
    BlendMode, Layer, LayerId, LayerKind, Sprite, SpriteId, TilesetId, UserData,
};
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
    let doc = state.doc.read().await;
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

/// Sets the parent of a layer. Pass `parent_id = None` to make it top-level.
///
/// Rejects if `parent_id` points to a non-existent layer, to a layer that
/// is not a group, to the layer itself (self-parent), or to one of the
/// layer's own descendants (which would close a cycle).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_set_parent(
    sprite_id: SpriteId,
    layer_id: LayerId,
    parent_id: Option<LayerId>,
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
        set_layer_parent_in_sprite(sprite, layer_id, parent_id)?;
    }
    doc.dirty = true;
    Ok(())
}

/// Pure parent-assignment helper. Lifted out of `layer_set_parent` so the
/// cycle and self-parent rejection paths are testable without standing
/// up a tauri `State`.
///
/// Validation order:
/// 1. Reject self-parent (`parent_id == Some(layer_id)`).
/// 2. Verify the candidate parent exists and is a group.
/// 3. Walk the candidate parent's ancestor chain; reject if `layer_id`
///    appears anywhere — that assignment would close a cycle and break
///    every tree-walking consumer (flattening, hit-testing, render
///    ordering).
/// 4. Apply the parent assignment.
fn set_layer_parent_in_sprite(
    sprite: &mut Sprite,
    layer_id: LayerId,
    parent_id: Option<LayerId>,
) -> CommandResult<()> {
    if Some(layer_id) == parent_id {
        return Err(AppCommandError::Validation {
            detail: "cannot reparent a layer onto itself".into(),
        });
    }
    if let Some(pid) = parent_id {
        let parent =
            sprite
                .layers
                .iter()
                .find(|l| l.id == pid)
                .ok_or(AppCommandError::NotFound {
                    entity: "layer".into(),
                    id: u64::from(pid.get()),
                })?;
        if !matches!(parent.kind, LayerKind::Group { .. }) {
            return Err(AppCommandError::Validation {
                detail: "target layer is not a group".into(),
            });
        }
        // Walk the candidate parent's ancestor chain. If layer_id shows
        // up as one of those ancestors, the new parent is a descendant
        // of the layer being moved — assigning it would close a cycle.
        // Bound the walk by the layer count so a pre-existing cycle in
        // the input data can't hang the validator.
        let mut current = parent.parent;
        let max_steps = sprite.layers.len();
        let mut steps = 0;
        while let Some(ancestor_id) = current {
            if ancestor_id == layer_id {
                return Err(AppCommandError::Validation {
                    detail: "reparent would create a cycle".into(),
                });
            }
            steps += 1;
            if steps > max_steps {
                return Err(AppCommandError::Validation {
                    detail: "layer parent chain has a pre-existing cycle".into(),
                });
            }
            current = sprite
                .layers
                .iter()
                .find(|l| l.id == ancestor_id)
                .and_then(|l| l.parent);
        }
    }
    let layer =
        sprite
            .layers
            .iter_mut()
            .find(|l| l.id == layer_id)
            .ok_or(AppCommandError::NotFound {
                entity: "layer".into(),
                id: u64::from(layer_id.get()),
            })?;
    layer.parent = parent_id;
    Ok(())
}

/// Converts a layer to a group. Removes any cels on the layer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_convert_to_group(
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
        {
            let layer = sprite.layers.iter_mut().find(|l| l.id == layer_id).ok_or(
                AppCommandError::NotFound {
                    entity: "layer".into(),
                    id: u64::from(layer_id.get()),
                },
            )?;
            layer.kind = LayerKind::Group { collapsed: false };
        }
        // Groups have no cels; drop any that were on this layer.
        sprite.cels.retain(|c| c.layer_id != layer_id);
    }
    doc.dirty = true;
    Ok(())
}

/// Converts a layer to a tilemap layer using the given tileset. Drops
/// every cel on the layer — tilemap cels are a different variant and
/// will be re-populated when the user paints tiles.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_convert_to_tilemap(
    sprite_id: SpriteId,
    layer_id: LayerId,
    tileset_id: TilesetId,
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
        // Validate tileset exists.
        if !sprite.tilesets.iter().any(|ts| ts.id == tileset_id) {
            return Err(AppCommandError::NotFound {
                entity: "tileset".into(),
                id: u64::from(tileset_id.get()),
            });
        }
        {
            let layer = sprite.layers.iter_mut().find(|l| l.id == layer_id).ok_or(
                AppCommandError::NotFound {
                    entity: "layer".into(),
                    id: u64::from(layer_id.get()),
                },
            )?;
            layer.kind = LayerKind::Tilemap {
                tileset: tileset_id,
            };
        }
        // Drop every cel on the layer (see fn doc — tilemap cels are a
        // different variant and will be re-populated when the user paints).
        sprite.cels.retain(|c| c.layer_id != layer_id);
    }
    doc.dirty = true;
    Ok(())
}

/// Merges a layer with the layer immediately below it.
///
/// Requires S01 (pixel-buffer registry). Returns `Unimplemented` until
/// that stream lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_merge_down(
    _sprite_id: SpriteId,
    _layer_id: LayerId,
    _state: State<'_, AppState>,
) -> CommandResult<()> {
    Err(AppCommandError::Unimplemented {
        stream: "S01".into(),
    })
}

/// Merges the given set of layers into a single raster layer.
///
/// Requires S01 (pixel-buffer registry). Returns `Unimplemented` until
/// that stream lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_merge_selected(
    _sprite_id: SpriteId,
    _layer_ids: Vec<LayerId>,
    _state: State<'_, AppState>,
) -> CommandResult<()> {
    Err(AppCommandError::Unimplemented {
        stream: "S01".into(),
    })
}

/// Flattens all visible layers into a single raster layer.
///
/// Requires S01 (pixel-buffer registry). Returns `Unimplemented` until
/// that stream lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_flatten_visible(
    _sprite_id: SpriteId,
    _state: State<'_, AppState>,
) -> CommandResult<()> {
    Err(AppCommandError::Unimplemented {
        stream: "S01".into(),
    })
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
    use pixhaus_core::project::{LayerKind, Size};

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

    // ── set_layer_parent_in_sprite cycle guards ──────────────────────────

    fn layer_with(id: u32, parent: Option<LayerId>, kind: LayerKind) -> Layer {
        Layer {
            id: LayerId::new(id),
            name: format!("L{id}"),
            kind,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent,
            user_data: UserData::default(),
        }
    }

    /// Sprite shaped as a chain of group layers: A (1) → B (2) → C (3),
    /// where A is the deepest ancestor and C is a leaf group. Used to
    /// exercise both the self-parent and indirect-cycle paths.
    fn sprite_with_group_chain() -> Sprite {
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        sprite
            .layers
            .push(layer_with(1, None, LayerKind::Group { collapsed: false }));
        sprite.layers.push(layer_with(
            2,
            Some(LayerId::new(1)),
            LayerKind::Group { collapsed: false },
        ));
        sprite.layers.push(layer_with(
            3,
            Some(LayerId::new(2)),
            LayerKind::Group { collapsed: false },
        ));
        sprite
    }

    #[test]
    fn set_parent_to_self_returns_validation() {
        let mut sprite = sprite_with_group_chain();
        let layer_id = LayerId::new(2);
        let err = set_layer_parent_in_sprite(&mut sprite, layer_id, Some(layer_id)).unwrap_err();
        assert!(
            matches!(err, AppCommandError::Validation { .. }),
            "self-parent must reject with Validation; got {err:?}"
        );
    }

    #[test]
    fn set_parent_creating_cycle_is_rejected() {
        // A → B → C is the existing chain; reparent A onto C would close
        // a cycle (C's ancestors include A through B).
        let mut sprite = sprite_with_group_chain();
        let err = set_layer_parent_in_sprite(&mut sprite, LayerId::new(1), Some(LayerId::new(3)))
            .unwrap_err();
        assert!(
            matches!(err, AppCommandError::Validation { .. }),
            "indirect cycle must reject with Validation; got {err:?}"
        );
        // Existing parent assignment must not have been mutated by the
        // failed call.
        assert_eq!(sprite.layers[0].parent, None, "A's parent stayed None");
    }

    #[test]
    fn set_parent_to_unrelated_group_succeeds() {
        // Sanity check that the cycle guard isn't over-eager: moving a
        // top-level layer under an unrelated group is allowed.
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        sprite
            .layers
            .push(layer_with(1, None, LayerKind::Group { collapsed: false }));
        sprite.layers.push(layer_with(2, None, LayerKind::Raster));
        set_layer_parent_in_sprite(&mut sprite, LayerId::new(2), Some(LayerId::new(1))).unwrap();
        assert_eq!(sprite.layers[1].parent, Some(LayerId::new(1)));
    }

    #[test]
    fn set_parent_to_non_group_returns_validation() {
        // A raster layer can't host children — the existing rejection
        // path must still fire.
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        sprite.layers.push(layer_with(1, None, LayerKind::Raster));
        sprite.layers.push(layer_with(2, None, LayerKind::Raster));
        let err = set_layer_parent_in_sprite(&mut sprite, LayerId::new(2), Some(LayerId::new(1)))
            .unwrap_err();
        assert!(
            matches!(err, AppCommandError::Validation { .. }),
            "non-group parent must reject with Validation; got {err:?}"
        );
    }
}
