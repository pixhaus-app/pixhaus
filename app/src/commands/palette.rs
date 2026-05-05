//! Palette CRUD and color management commands.

use pixhaus_core::project::{Palette, PaletteEntry, PaletteId, Project, Rgba, SpriteId, UserData};
use pixhaus_core::undo::{
    Command, CommandError, CommandResult as UndoCommandResult, Error as UndoError,
};
use serde::{Deserialize, Deserializer, Serialize};
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Maps a `core::undo::Error` raised by `History::push` of a palette
/// command into the typed IPC error contract.
///
/// Mirrors `commands::undo::map_undo_error` but is duplicated here to
/// keep `commands::undo` private. `CommandFailed` and `Poisoned` both
/// indicate the project state is suspect; collapse them onto
/// `HistoryCorrupted` so the UI can show a recovery prompt.
fn map_palette_undo_error(err: UndoError) -> AppCommandError {
    match err {
        UndoError::CommandFailed { .. } | UndoError::Poisoned { .. } => {
            AppCommandError::HistoryCorrupted {
                detail: err.to_string(),
            }
        }
        other => AppCommandError::Validation {
            detail: other.to_string(),
        },
    }
}

// ── command impl: PaletteAddColorCommand ─────────────────────────────────────

/// A reversible "append color to palette" command.
///
/// Proves the `Command` pattern for IPC mutations: `apply` records the
/// index it inserted at so `undo` can remove exactly that entry.
struct PaletteAddColorCommand {
    sprite_id: SpriteId,
    palette_id: PaletteId,
    color: Rgba,
    name: Option<String>,
    /// Index of the appended entry; populated by the first `apply` call.
    added_index: u32,
}

impl Command for PaletteAddColorCommand {
    fn label(&self) -> &'static str {
        "add palette color"
    }

    fn apply(&mut self, project: &mut Project) -> UndoCommandResult {
        let palette = find_palette_in_project_mut(project, self.sprite_id, self.palette_id)
            .map_err(CommandError::from_message)?;
        let entry = PaletteEntry {
            color: self.color,
            name: self.name.clone(),
        };
        palette.colors.push(entry);
        self.added_index = u32::try_from(palette.colors.len() - 1)
            .map_err(|_| CommandError::Other("palette has too many colors".into()))?;
        Ok(())
    }

    fn undo(&mut self, project: &mut Project) -> UndoCommandResult {
        let palette = find_palette_in_project_mut(project, self.sprite_id, self.palette_id)
            .map_err(CommandError::from_message)?;
        let idx = self.added_index as usize;
        if idx < palette.colors.len() {
            palette.colors.remove(idx);
        }
        Ok(())
    }
}

// ── wire format ──────────────────────────────────────────────────────────────

/// Deserialises an optional field with three states: missing (None),
/// explicit `null` (Some(None)), or a value (Some(Some(value))).
///
/// Used by `palette_set_color` to distinguish "leave name unchanged"
/// (omit the key) from "clear the name" (`name: null`) — `Option<String>`
/// alone collapses both into `None` and gives no way to clear.
#[allow(
    clippy::option_option,
    reason = "three-state wire contract — see fn doc"
)]
fn double_option<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(d).map(Some)
}

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
    ///
    /// Wire format is three-state: omit the key to keep the existing
    /// name, send `null` to clear it, send a string to set it. The
    /// double-`Option` lets serde distinguish missing-key from
    /// `null`-value, which a flat `Option<String>` collapses.
    #[serde(default, deserialize_with = "double_option")]
    #[allow(
        clippy::option_option,
        reason = "three-state wire contract — see field doc"
    )]
    pub name: Option<Option<String>>,
}

/// Result returned when two palettes are swapped.
#[derive(Debug, Serialize)]
pub struct PaletteSwapResult {
    /// ID of the first palette.
    pub from_id: PaletteId,
    /// ID of the second palette.
    pub to_id: PaletteId,
}

// ── IPC commands ─────────────────────────────────────────────────────────────

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
///
/// This mutation is routed through the undo history so `undo` can remove
/// the color that was appended.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn palette_add_color(
    args: PaletteAddColorArgs,
    state: State<'_, AppState>,
) -> CommandResult<u32> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    // Pre-calculate the insertion index while holding an immutable project borrow.
    let next_index = {
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let palette = find_palette_in_project(project, args.sprite_id, args.palette_id)?;
        u32::try_from(palette.colors.len()).map_err(|_| AppCommandError::Validation {
            detail: "palette has too many colors".into(),
        })?
    };

    let cmd = PaletteAddColorCommand {
        sprite_id: args.sprite_id,
        palette_id: args.palette_id,
        color: args.color,
        name: args.name,
        added_index: 0,
    };

    // Disjoint field borrows: doc.project and doc.history are separate fields.
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    doc.history
        .push(Box::new(cmd), project)
        .map_err(map_palette_undo_error)?;

    doc.dirty = true;
    Ok(next_index)
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
        // Three-state name update: outer None = key omitted (keep),
        // Some(None) = explicit null (clear), Some(Some(s)) = set.
        if let Some(new_name) = args.name {
            entry.name = new_name;
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
        let mut from_pos: Option<usize> = None;
        let mut to_pos: Option<usize> = None;
        for (i, p) in sprite.palettes.iter().enumerate() {
            if p.id == from_id {
                from_pos = Some(i);
            }
            if p.id == to_id {
                to_pos = Some(i);
            }
            if from_pos.is_some() && to_pos.is_some() {
                break;
            }
        }
        let from_pos = from_pos.ok_or(AppCommandError::NotFound {
            entity: "palette".into(),
            id: u64::from(from_id.get()),
        })?;
        let to_pos = to_pos.ok_or(AppCommandError::NotFound {
            entity: "palette".into(),
            id: u64::from(to_id.get()),
        })?;
        sprite.palettes.swap(from_pos, to_pos);
    }
    doc.dirty = true;
    Ok(PaletteSwapResult { from_id, to_id })
}

/// Moves a colour from `from_index` to `to_index` within a palette,
/// shifting every entry between by one. Used by the palette panel's
/// drag-to-reorder UI; the previous behaviour was a `console.warn`
/// stub that left the on-disk order untouched.
///
/// Both indices are bounds-checked against the palette's current
/// length. `from_index == to_index` is a no-op.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn palette_reorder_colors(
    sprite_id: SpriteId,
    palette_id: PaletteId,
    from_index: u32,
    to_index: u32,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let palette = find_palette_mut(&mut doc, sprite_id, palette_id)?;
        reorder_palette_colors_in_place(palette, from_index, to_index)?;
    }
    doc.dirty = true;
    Ok(())
}

/// Pure reorder-in-place helper. Lifted out of `palette_reorder_colors`
/// so the bounds and shift behaviour are testable without standing up
/// a tauri `State`.
fn reorder_palette_colors_in_place(
    palette: &mut Palette,
    from_index: u32,
    to_index: u32,
) -> CommandResult<()> {
    let len = palette.colors.len();
    let from = from_index as usize;
    let to = to_index as usize;
    if from >= len {
        return Err(AppCommandError::OutOfRange {
            detail: format!("from_index {from} out of range (palette has {len} colors)"),
        });
    }
    if to >= len {
        return Err(AppCommandError::OutOfRange {
            detail: format!("to_index {to} out of range (palette has {len} colors)"),
        });
    }
    if from == to {
        return Ok(());
    }
    let entry = palette.colors.remove(from);
    palette.colors.insert(to, entry);
    Ok(())
}

/// Returns all palettes in a sprite.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn palette_list(
    sprite_id: SpriteId,
    state: State<'_, AppState>,
) -> CommandResult<Vec<Palette>> {
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

fn find_palette_in_project(
    project: &Project,
    sprite_id: SpriteId,
    palette_id: PaletteId,
) -> CommandResult<&Palette> {
    project
        .sprites
        .iter()
        .find(|s| s.id == sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?
        .palettes
        .iter()
        .find(|p| p.id == palette_id)
        .ok_or(AppCommandError::NotFound {
            entity: "palette".into(),
            id: u64::from(palette_id.get()),
        })
}

fn find_palette_in_project_mut(
    project: &mut Project,
    sprite_id: SpriteId,
    palette_id: PaletteId,
) -> CommandResult<&mut Palette> {
    project
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

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::{PaletteId, Project, Rgba, SpriteId};
    use pixhaus_core::undo::History;

    #[test]
    fn palette_swap_result_preserves_ids() {
        let result = PaletteSwapResult {
            from_id: PaletteId::new(1),
            to_id: PaletteId::new(2),
        };
        assert_eq!(result.from_id.get(), 1);
        assert_eq!(result.to_id.get(), 2);
    }

    fn args_from_json(json: &str) -> PaletteSetColorArgs {
        serde_json::from_str(json).expect("valid PaletteSetColorArgs json")
    }

    const BASE: &str =
        r#""sprite_id":1,"palette_id":2,"index":0,"color":{"r":0,"g":0,"b":0,"a":255}"#;

    #[test]
    fn name_omitted_means_keep() {
        let args = args_from_json(&format!("{{{BASE}}}"));
        assert!(args.name.is_none(), "outer None signals keep");
    }

    #[test]
    fn name_null_means_clear() {
        let args = args_from_json(&format!("{{{BASE},\"name\":null}}"));
        assert_eq!(args.name, Some(None), "Some(None) signals clear");
    }

    #[test]
    fn name_string_means_set() {
        let args = args_from_json(&format!("{{{BASE},\"name\":\"red\"}}"));
        assert_eq!(
            args.name,
            Some(Some("red".into())),
            "Some(Some) sets the value"
        );
    }

    // ── PaletteAddColorCommand round-trip ─────────────────────────────────

    fn make_project_with_palette() -> (Project, SpriteId, PaletteId) {
        use pixhaus_core::project::{Size, Sprite};

        let mut project = Project::new("test");
        let sprite_id = SpriteId::new(1);
        let palette_id = PaletteId::new(1);
        let mut sprite = Sprite::empty(
            sprite_id,
            "sprite",
            Size {
                width: 8,
                height: 8,
            },
        );
        sprite.palettes.push(Palette {
            id: palette_id,
            name: "main".into(),
            colors: Vec::new(),
            user_data: UserData::default(),
        });
        project.sprites.push(sprite);
        (project, sprite_id, palette_id)
    }

    #[test]
    fn palette_add_color_command_apply_adds_color() {
        let (mut project, sprite_id, palette_id) = make_project_with_palette();
        let mut history = History::new();

        let cmd = PaletteAddColorCommand {
            sprite_id,
            palette_id,
            color: Rgba {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
            name: Some("red".into()),
            added_index: 0,
        };
        history.push(Box::new(cmd), &mut project).unwrap();

        let palette = project
            .sprites
            .iter()
            .find(|s| s.id == sprite_id)
            .unwrap()
            .palettes
            .iter()
            .find(|p| p.id == palette_id)
            .unwrap();
        assert_eq!(palette.colors.len(), 1);
        assert_eq!(palette.colors[0].color.r, 255);
    }

    #[test]
    fn undo_removes_added_color() {
        let (mut project, sprite_id, palette_id) = make_project_with_palette();
        let mut history = History::new();

        let cmd = PaletteAddColorCommand {
            sprite_id,
            palette_id,
            color: Rgba {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
            name: None,
            added_index: 0,
        };
        history.push(Box::new(cmd), &mut project).unwrap();
        assert_eq!(
            project.sprites[0]
                .palettes
                .iter()
                .find(|p| p.id == palette_id)
                .unwrap()
                .colors
                .len(),
            1,
            "color was added"
        );

        history.undo(&mut project).unwrap();

        let palette = project
            .sprites
            .iter()
            .find(|s| s.id == sprite_id)
            .unwrap()
            .palettes
            .iter()
            .find(|p| p.id == palette_id)
            .unwrap();
        assert_eq!(
            palette.colors.len(),
            0,
            "undo must remove the appended color"
        );
    }

    // ── reorder_palette_colors_in_place ───────────────────────────────────

    fn palette_with_named_colors(names: &[&str]) -> Palette {
        let colors = names
            .iter()
            .map(|n| PaletteEntry {
                color: Rgba {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                name: Some((*n).into()),
            })
            .collect();
        Palette {
            id: PaletteId::new(1),
            name: "main".into(),
            colors,
            user_data: UserData::default(),
        }
    }

    fn names_of(palette: &Palette) -> Vec<&str> {
        palette
            .colors
            .iter()
            .map(|c| c.name.as_deref().unwrap_or(""))
            .collect()
    }

    #[test]
    fn reorder_forward_shifts_intermediate_entries_left() {
        let mut palette = palette_with_named_colors(&["a", "b", "c", "d"]);
        reorder_palette_colors_in_place(&mut palette, 0, 2).unwrap();
        assert_eq!(names_of(&palette), vec!["b", "c", "a", "d"]);
    }

    #[test]
    fn reorder_backward_shifts_intermediate_entries_right() {
        let mut palette = palette_with_named_colors(&["a", "b", "c", "d"]);
        reorder_palette_colors_in_place(&mut palette, 3, 1).unwrap();
        assert_eq!(names_of(&palette), vec!["a", "d", "b", "c"]);
    }

    #[test]
    fn reorder_same_index_is_a_noop() {
        let mut palette = palette_with_named_colors(&["a", "b", "c"]);
        reorder_palette_colors_in_place(&mut palette, 1, 1).unwrap();
        assert_eq!(names_of(&palette), vec!["a", "b", "c"]);
    }

    #[test]
    fn reorder_from_index_out_of_range_returns_out_of_range() {
        let mut palette = palette_with_named_colors(&["a", "b"]);
        let err = reorder_palette_colors_in_place(&mut palette, 5, 0).unwrap_err();
        assert!(
            matches!(err, AppCommandError::OutOfRange { .. }),
            "expected OutOfRange, got {err:?}"
        );
        // Palette is untouched on error.
        assert_eq!(names_of(&palette), vec!["a", "b"]);
    }

    #[test]
    fn reorder_to_index_out_of_range_returns_out_of_range() {
        let mut palette = palette_with_named_colors(&["a", "b"]);
        let err = reorder_palette_colors_in_place(&mut palette, 0, 9).unwrap_err();
        assert!(
            matches!(err, AppCommandError::OutOfRange { .. }),
            "expected OutOfRange, got {err:?}"
        );
        assert_eq!(names_of(&palette), vec!["a", "b"]);
    }

    #[test]
    fn redo_reapplies_add_color() {
        let (mut project, sprite_id, palette_id) = make_project_with_palette();
        let mut history = History::new();

        let cmd = PaletteAddColorCommand {
            sprite_id,
            palette_id,
            color: Rgba {
                r: 0,
                g: 128,
                b: 255,
                a: 255,
            },
            name: None,
            added_index: 0,
        };
        history.push(Box::new(cmd), &mut project).unwrap();
        history.undo(&mut project).unwrap();
        history.redo(&mut project).unwrap();

        let palette = project
            .sprites
            .iter()
            .find(|s| s.id == sprite_id)
            .unwrap()
            .palettes
            .iter()
            .find(|p| p.id == palette_id)
            .unwrap();
        assert_eq!(palette.colors.len(), 1);
        assert_eq!(palette.colors[0].color.g, 128);
    }
}
