//! Palette CRUD and color management commands.

use pixhaus_core::project::{Palette, PaletteEntry, PaletteId, Rgba, SpriteId, UserData};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Arguments for adding a color to a palette.
#[derive(Debug, Deserialize)]
pub struct PaletteAddColorArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target palette.
    pub palette_id: PaletteId,
    /// Color to add.
    pub color: Rgba,
    /// Optional human-readable name for the swatch.
    pub name: Option<String>,
}

/// Arguments for setting a color at a specific palette index.
#[derive(Debug, Deserialize)]
pub struct PaletteSetColorArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target palette.
    pub palette_id: PaletteId,
    /// Index of the swatch to update.
    pub index: u32,
    /// New color value.
    pub color: Rgba,
    /// Optional rename for the swatch.
    pub name: Option<String>,
}

/// Result returned when two palettes are swapped.
#[derive(Debug, Serialize)]
pub struct PaletteSwapResult {
    /// ID of the first palette.
    pub from_id: PaletteId,
    /// ID of the second palette.
    pub to_id: PaletteId,
}

/// Adds a new empty palette to a sprite.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn palette_add(
    sprite_id: SpriteId,
    name: String,
    state: State<'_, AppState>,
) -> CommandResult<Palette> {
    let mut doc = state.doc.write().await;
    let id = PaletteId::new(doc.next_id);
    doc.next_id += 1;
    let palette = {
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
        let palette = Palette {
            id,
            name,
            colors: Vec::new(),
            user_data: UserData::default(),
        };
        sprite.palettes.push(palette.clone());
        palette
    };
    doc.dirty = true;
    Ok(palette)
}

/// Removes a palette from a sprite by ID.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn palette_delete(
    sprite_id: SpriteId,
    palette_id: PaletteId,
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
        let before = sprite.palettes.len();
        sprite.palettes.retain(|p| p.id != palette_id);
        if sprite.palettes.len() == before {
            return Err(AppCommandError::NotFound {
                entity: "palette".into(),
                id: u64::from(palette_id.get()),
            });
        }
    }
    doc.dirty = true;
    Ok(())
}

/// Appends a color to a palette. Returns the index of the new swatch.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn palette_add_color(
    args: PaletteAddColorArgs,
    state: State<'_, AppState>,
) -> CommandResult<u32> {
    let mut doc = state.doc.write().await;
    let index = {
        let palette = find_palette_mut(&mut doc, args.sprite_id, args.palette_id)?;
        let entry = PaletteEntry {
            color: args.color,
            name: args.name,
        };
        palette.colors.push(entry);
        u32::try_from(palette.colors.len() - 1).map_err(|_| AppCommandError::Validation {
            detail: "palette has too many colors".into(),
        })?
    };
    doc.dirty = true;
    Ok(index)
}

/// Removes the swatch at `index` from a palette.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn palette_remove_color(
    sprite_id: SpriteId,
    palette_id: PaletteId,
    index: u32,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let palette = find_palette_mut(&mut doc, sprite_id, palette_id)?;
        let idx = index as usize;
        if idx >= palette.colors.len() {
            return Err(AppCommandError::OutOfRange {
                detail: format!(
                    "palette index {} out of range (palette has {} colors)",
                    idx,
                    palette.colors.len()
                ),
            });
        }
        palette.colors.remove(idx);
    }
    doc.dirty = true;
    Ok(())
}

/// Replaces the color (and optionally the name) at a specific palette index.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn palette_set_color(
    args: PaletteSetColorArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let palette = find_palette_mut(&mut doc, args.sprite_id, args.palette_id)?;
        let idx = args.index as usize;
        let len = palette.colors.len();
        let entry = palette
            .colors
            .get_mut(idx)
            .ok_or_else(|| AppCommandError::OutOfRange {
                detail: format!("palette index {idx} out of range (palette has {len} colors)"),
            })?;
        entry.color = args.color;
        if let Some(name) = args.name {
            entry.name = Some(name);
        }
    }
    doc.dirty = true;
    Ok(())
}

/// Swaps the positions of two palettes in a sprite's palette list.
///
/// This is an ordering operation only; the palette IDs and colors are unchanged.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn palette_swap(
    sprite_id: SpriteId,
    from_id: PaletteId,
    to_id: PaletteId,
    state: State<'_, AppState>,
) -> CommandResult<PaletteSwapResult> {
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
        let from_pos = sprite.palettes.iter().position(|p| p.id == from_id).ok_or(
            AppCommandError::NotFound {
                entity: "palette".into(),
                id: u64::from(from_id.get()),
            },
        )?;
        let to_pos = sprite.palettes.iter().position(|p| p.id == to_id).ok_or(
            AppCommandError::NotFound {
                entity: "palette".into(),
                id: u64::from(to_id.get()),
            },
        )?;
        sprite.palettes.swap(from_pos, to_pos);
    }
    doc.dirty = true;
    Ok(PaletteSwapResult { from_id, to_id })
}

/// Returns all palettes in a sprite.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn palette_list(
    sprite_id: SpriteId,
    state: State<'_, AppState>,
) -> CommandResult<Vec<Palette>> {
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
    Ok(sprite.palettes.clone())
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn find_palette_mut(
    doc: &mut crate::state::DocumentStore,
    sprite_id: SpriteId,
    palette_id: PaletteId,
) -> CommandResult<&mut Palette> {
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
        .palettes
        .iter_mut()
        .find(|p| p.id == palette_id)
        .ok_or(AppCommandError::NotFound {
            entity: "palette".into(),
            id: u64::from(palette_id.get()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_swap_result_preserves_ids() {
        let result = PaletteSwapResult {
            from_id: PaletteId::new(1),
            to_id: PaletteId::new(2),
        };
        assert_eq!(result.from_id.get(), 1);
        assert_eq!(result.to_id.get(), 2);
    }
}
