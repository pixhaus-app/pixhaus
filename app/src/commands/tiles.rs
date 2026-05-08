//! Tilemap editing commands.

use pixhaus_core::project::{
    Cel, CelData, FrameIndex, IVec2, Layer, LayerId, LayerKind, PixelBufferId, Project, Size,
    SpriteId, TileCell, TileFlags, TileIndex, TileProperties, TilemapData, Tileset, TilesetId,
    TilesetSource, UserData,
};
use pixhaus_core::tilemap::{AutotileKind, resolve_autotile};
use pixhaus_core::undo::{Command, CommandError, CommandResult as UndoCommandResult};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::error::{AppCommandError, CommandResult};
use crate::state::{AppState, DocumentStore};

// ── Event payloads ───────────────────────────────────────────────────────────

/// Payload emitted when a single cell on a tilemap layer changes.
///
/// One event fires per `tile_place`, `tile_erase`, or per-cell write that the
/// renderer / panel must reflect. Coordinates are tile cells, not pixels.
#[derive(Debug, Clone, Serialize)]
pub struct TilemapCellChangedPayload {
    /// Sprite owning the layer.
    pub sprite_id: u32,
    /// Tilemap layer that changed.
    pub layer_id: u32,
    /// Frame whose tilemap cel changed.
    pub frame_index: u32,
    /// Column (x) of the changed cell.
    pub cell_x: u32,
    /// Row (y) of the changed cell.
    pub cell_y: u32,
}

/// Payload emitted when many cells on a tilemap layer change at once.
///
/// `tile_autotile_apply` ships one of these instead of N per-cell events so
/// the renderer can re-upload the cel's tile grid in a single pass.
#[derive(Debug, Clone, Serialize)]
pub struct TilemapBulkChangedPayload {
    /// Sprite owning the layer.
    pub sprite_id: u32,
    /// Tilemap layer that changed.
    pub layer_id: u32,
    /// Frame whose tilemap cel changed.
    pub frame_index: u32,
}

// ── Argument types ───────────────────────────────────────────────────────────

/// Arguments for placing a tile on a tilemap cel.
#[derive(Debug, Deserialize)]
pub struct TilePlaceArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target tilemap layer.
    pub layer_id: LayerId,
    /// Target frame.
    pub frame_index: FrameIndex,
    /// Column (x) of the cell in the tile grid.
    pub cell_x: u32,
    /// Row (y) of the cell in the tile grid.
    pub cell_y: u32,
    /// Tile to place.
    pub cell: TileCell,
}

/// Arguments for erasing a tile on a tilemap cel.
#[derive(Debug, Deserialize)]
pub struct TileEraseArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target tilemap layer.
    pub layer_id: LayerId,
    /// Target frame.
    pub frame_index: FrameIndex,
    /// Column (x) of the cell to erase.
    pub cell_x: u32,
    /// Row (y) of the cell to erase.
    pub cell_y: u32,
}

/// Arguments for applying autotile rules to a region of a tilemap cel.
#[derive(Debug, Deserialize)]
pub struct AutotileArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target tilemap layer.
    pub layer_id: LayerId,
    /// Target frame.
    pub frame_index: FrameIndex,
    /// Autotile algorithm to run. Standard variants (`Blob47`, `Corner16`,
    /// `Minimal4`) need no extra data; the `Custom` variant carries its
    /// own rules + default tile inline. The frontend usually mirrors the
    /// kind already persisted on the target tileset (`Tileset.autotile`).
    pub kind: AutotileKind,
    /// Tile that the resolver should treat as the "filled" base.
    /// All cells matching this tile (or any non-empty cell) participate
    /// in autotile resolution; the `result_tile` is offset from the
    /// tileset's autotile region using `source_tile` as the anchor.
    pub source_tile: TileIndex,
}

/// Arguments for setting per-tile metadata (collision, animation) on a tileset.
#[derive(Debug, Deserialize)]
pub struct TilesetSetTileMetadataArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target tileset.
    pub tileset_id: TilesetId,
    /// Index of the tile being annotated.
    pub tile_index: TileIndex,
    /// Replacement metadata for the tile.
    pub metadata: TileProperties,
}

// ── Undo commands ────────────────────────────────────────────────────────────

/// Reversible "set one tilemap cell" command.
struct TileSetCellCommand {
    sprite_id: SpriteId,
    layer_id: LayerId,
    frame_index: FrameIndex,
    cell_x: u32,
    cell_y: u32,
    new_cell: TileCell,
    /// Captured on first `apply`.
    prev_cell: TileCell,
    label: &'static str,
}

impl Command for TileSetCellCommand {
    fn label(&self) -> &str {
        self.label
    }

    fn apply(&mut self, project: &mut Project) -> UndoCommandResult {
        let data = find_tilemap_data_mut(project, self.sprite_id, self.layer_id, self.frame_index)
            .map_err(CommandError::from_message)?;
        let idx = cell_index(data, self.cell_x, self.cell_y)
            .ok_or_else(|| CommandError::from_message("tile cell out of range".to_owned()))?;
        self.prev_cell = data.cells[idx];
        data.cells[idx] = self.new_cell;
        Ok(())
    }

    fn undo(&mut self, project: &mut Project) -> UndoCommandResult {
        let data = find_tilemap_data_mut(project, self.sprite_id, self.layer_id, self.frame_index)
            .map_err(CommandError::from_message)?;
        let idx = cell_index(data, self.cell_x, self.cell_y)
            .ok_or_else(|| CommandError::from_message("tile cell out of range".to_owned()))?;
        data.cells[idx] = self.prev_cell;
        Ok(())
    }
}

/// Reversible "rewrite tilemap cells in bulk" command.
///
/// Used by autotile apply; carries before/after snapshots of every changed
/// cell so undo restores them exactly. The full grid isn't snapshotted —
/// only cells whose contents actually changed.
struct TileBulkCommand {
    sprite_id: SpriteId,
    layer_id: LayerId,
    frame_index: FrameIndex,
    /// `(cell_x, cell_y, before, after)` for each changed cell.
    changes: Vec<(u32, u32, TileCell, TileCell)>,
}

impl Command for TileBulkCommand {
    fn label(&self) -> &'static str {
        "autotile apply"
    }

    fn apply(&mut self, project: &mut Project) -> UndoCommandResult {
        let data = find_tilemap_data_mut(project, self.sprite_id, self.layer_id, self.frame_index)
            .map_err(CommandError::from_message)?;
        for (x, y, _before, after) in &self.changes {
            let idx = cell_index(data, *x, *y).ok_or_else(|| {
                CommandError::from_message(
                    "tile cell out of range during autotile apply".to_owned(),
                )
            })?;
            data.cells[idx] = *after;
        }
        Ok(())
    }

    fn undo(&mut self, project: &mut Project) -> UndoCommandResult {
        let data = find_tilemap_data_mut(project, self.sprite_id, self.layer_id, self.frame_index)
            .map_err(CommandError::from_message)?;
        for (x, y, before, _after) in &self.changes {
            let idx = cell_index(data, *x, *y).ok_or_else(|| {
                CommandError::from_message("tile cell out of range during autotile undo".to_owned())
            })?;
            data.cells[idx] = *before;
        }
        Ok(())
    }

    fn estimated_size_bytes(&self) -> usize {
        // 4 bytes x + 4 bytes y + 5 bytes before + 5 bytes after = 18; round to 24.
        self.changes.len() * 24
    }
}

/// Reversible "replace tile metadata at index" command.
struct TilesetSetTileMetadataCommand {
    sprite_id: SpriteId,
    tileset_id: TilesetId,
    tile_index: TileIndex,
    new_metadata: TileProperties,
    /// Snapshot captured on first `apply`. `None` records "the index was
    /// past the end of `properties` — undo should truncate back to that
    /// length".
    prev: Option<TileProperties>,
    /// Length of `properties` before apply, captured on first `apply`.
    /// Lets undo distinguish "extended the vec to fit" from "overwrote
    /// an existing entry".
    prev_len: usize,
}

impl Command for TilesetSetTileMetadataCommand {
    fn label(&self) -> &'static str {
        "set tile metadata"
    }

    fn apply(&mut self, project: &mut Project) -> UndoCommandResult {
        let tileset = find_tileset_mut(project, self.sprite_id, self.tileset_id)
            .map_err(CommandError::from_message)?;
        let idx = self.tile_index.get() as usize;
        let in_range = u32::try_from(idx)
            .ok()
            .is_some_and(|i| i < tileset.tile_count);
        if !in_range {
            return Err(CommandError::from_message(format!(
                "tile index {idx} out of range (tileset has {} tiles)",
                tileset.tile_count
            )));
        }
        self.prev_len = tileset.properties.len();
        if idx < tileset.properties.len() {
            self.prev = Some(tileset.properties[idx].clone());
            tileset.properties[idx] = self.new_metadata.clone();
        } else {
            self.prev = None;
            // Pad with defaults up to (but not including) idx, then push
            // the new metadata at idx.
            tileset.properties.resize(idx, TileProperties::default());
            tileset.properties.push(self.new_metadata.clone());
        }
        Ok(())
    }

    fn undo(&mut self, project: &mut Project) -> UndoCommandResult {
        let tileset = find_tileset_mut(project, self.sprite_id, self.tileset_id)
            .map_err(CommandError::from_message)?;
        let idx = self.tile_index.get() as usize;
        if let Some(prev) = self.prev.take() {
            if idx < tileset.properties.len() {
                tileset.properties[idx] = prev;
            }
        } else {
            // We extended the vec on apply — truncate back.
            tileset.properties.truncate(self.prev_len);
        }
        Ok(())
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Computes the linear index for `(cell_x, cell_y)` if both are in range.
fn cell_index(data: &TilemapData, x: u32, y: u32) -> Option<usize> {
    if x >= data.width || y >= data.height {
        return None;
    }
    Some((y as usize) * (data.width as usize) + (x as usize))
}

/// Finds the tilemap layer and returns its tileset id, or fails with a
/// typed error if the layer is missing or not a tilemap.
fn lookup_tilemap_layer(
    project: &Project,
    sprite_id: SpriteId,
    layer_id: LayerId,
) -> CommandResult<(&Layer, TilesetId)> {
    let sprite =
        project
            .sprites
            .iter()
            .find(|s| s.id == sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
    let layer =
        sprite
            .layers
            .iter()
            .find(|l| l.id == layer_id)
            .ok_or(AppCommandError::NotFound {
                entity: "layer".into(),
                id: u64::from(layer_id.get()),
            })?;
    let LayerKind::Tilemap { tileset } = layer.kind else {
        return Err(AppCommandError::Validation {
            detail: format!("layer {} is not a tilemap layer", layer_id.get()),
        });
    };
    Ok((layer, tileset))
}

/// Returns a mutable reference to the tilemap grid for `(sprite, layer, frame)`.
///
/// Used by undo command bodies, which receive only `&mut Project` and need to
/// re-find the cel after the lock has been re-acquired by the caller. Returns
/// a plain `String` so callers can wrap in their preferred error type
/// (`CommandError` for undo, `AppCommandError` at the IPC layer).
fn find_tilemap_data_mut(
    project: &mut Project,
    sprite_id: SpriteId,
    layer_id: LayerId,
    frame_index: FrameIndex,
) -> Result<&mut TilemapData, String> {
    let sprite = project
        .sprites
        .iter_mut()
        .find(|s| s.id == sprite_id)
        .ok_or_else(|| format!("sprite {} not found", sprite_id.get()))?;
    let cel = sprite
        .cels
        .iter_mut()
        .find(|c| c.layer_id == layer_id && c.frame_index == frame_index)
        .ok_or_else(|| {
            format!(
                "tilemap cel for layer {} frame {} not found",
                layer_id.get(),
                frame_index.get()
            )
        })?;
    let CelData::Tilemap { data } = &mut cel.data else {
        return Err(format!(
            "cel for layer {} frame {} is not a tilemap cel",
            layer_id.get(),
            frame_index.get()
        ));
    };
    Ok(data)
}

/// Looks up a tileset by id, returning a String error compatible with
/// `CommandError::from_message`.
fn find_tileset_mut(
    project: &mut Project,
    sprite_id: SpriteId,
    tileset_id: TilesetId,
) -> Result<&mut Tileset, String> {
    let sprite = project
        .sprites
        .iter_mut()
        .find(|s| s.id == sprite_id)
        .ok_or_else(|| format!("sprite {} not found", sprite_id.get()))?;
    sprite
        .tilesets
        .iter_mut()
        .find(|t| t.id == tileset_id)
        .ok_or_else(|| format!("tileset {} not found", tileset_id.get()))
}

/// Ensures a tilemap cel exists for `(sprite, layer, frame)`. Returns the
/// initial dimensions (cols × rows) of the grid; existing grids keep their
/// shape, freshly-created grids are sized from sprite canvas / tile size.
///
/// Mirrors the lazy-cel pattern in `canvas::ensure_raster_buffer` but for
/// tilemap cels: paint commands shouldn't require a separate "create cel"
/// IPC, so the first paint at a cell allocates the grid.
fn ensure_tilemap_cel(
    doc: &mut DocumentStore,
    sprite_id: SpriteId,
    layer_id: LayerId,
    frame_index: FrameIndex,
) -> CommandResult<(u32, u32)> {
    // Phase 1: gather what we need under a shared borrow; check if the
    // cel already exists.
    let (canvas_w, canvas_h, tile_size, exists) = {
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let (_layer, tileset_id) = lookup_tilemap_layer(project, sprite_id, layer_id)?;
        let sprite = project.sprites.iter().find(|s| s.id == sprite_id).ok_or(
            AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            },
        )?;
        let tileset = sprite.tilesets.iter().find(|t| t.id == tileset_id).ok_or(
            AppCommandError::NotFound {
                entity: "tileset".into(),
                id: u64::from(tileset_id.get()),
            },
        )?;
        let exists = sprite
            .cels
            .iter()
            .any(|c| c.layer_id == layer_id && c.frame_index == frame_index);
        (
            sprite.canvas.width,
            sprite.canvas.height,
            tileset.tile_size,
            exists,
        )
    };

    if exists {
        // Cel already there — confirm it's a tilemap cel and return its dims.
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let sprite = project.sprites.iter().find(|s| s.id == sprite_id).ok_or(
            AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            },
        )?;
        let cel = sprite
            .cels
            .iter()
            .find(|c| c.layer_id == layer_id && c.frame_index == frame_index)
            .ok_or(AppCommandError::NotFound {
                entity: "cel".into(),
                id: u64::from(frame_index.get()),
            })?;
        return match &cel.data {
            CelData::Tilemap { data } => Ok((data.width, data.height)),
            _ => Err(AppCommandError::Validation {
                detail: format!(
                    "cel for layer {} frame {} exists but is not a tilemap cel",
                    layer_id.get(),
                    frame_index.get()
                ),
            }),
        };
    }

    // Allocate the grid sized to cover the sprite canvas. Round up so the
    // grid always covers the visible area; cells past the sprite edge are
    // still addressable (the renderer may clip them).
    if tile_size.width == 0 || tile_size.height == 0 {
        return Err(AppCommandError::Validation {
            detail: "tileset tile size must be positive".into(),
        });
    }
    let cols = canvas_w.div_ceil(tile_size.width).max(1);
    let rows = canvas_h.div_ceil(tile_size.height).max(1);

    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let sprite = project
        .sprites
        .iter_mut()
        .find(|s| s.id == sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    sprite.cels.push(Cel {
        layer_id,
        frame_index,
        position: IVec2::zero(),
        opacity: 255,
        data: CelData::Tilemap {
            data: TilemapData::empty(cols, rows),
        },
        user_data: UserData::default(),
    });
    Ok((cols, rows))
}

/// Validates that a tile index references a tile in the layer's tileset.
fn validate_tile_index(
    project: &Project,
    sprite_id: SpriteId,
    tileset_id: TilesetId,
    cell: TileCell,
) -> CommandResult<()> {
    if cell.is_empty() {
        return Ok(());
    }
    let sprite =
        project
            .sprites
            .iter()
            .find(|s| s.id == sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
    let tileset =
        sprite
            .tilesets
            .iter()
            .find(|t| t.id == tileset_id)
            .ok_or(AppCommandError::NotFound {
                entity: "tileset".into(),
                id: u64::from(tileset_id.get()),
            })?;
    if cell.index.get() >= tileset.tile_count {
        return Err(AppCommandError::Validation {
            detail: format!(
                "tile index {} out of range (tileset has {} tiles)",
                cell.index.get(),
                tileset.tile_count
            ),
        });
    }
    Ok(())
}

// ── IPC commands ─────────────────────────────────────────────────────────────

/// Places a tile cell on a tilemap layer.
///
/// Lazily creates the tilemap cel for `(layer, frame)` if one doesn't
/// exist yet, then writes `args.cell` into the grid and pushes a single
/// undo entry. Emits `tilemap:cell-changed` so the renderer / panel can
/// refresh.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tile_place(
    app: AppHandle,
    args: TilePlaceArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    // Validate layer is a tilemap and tile_index is in range.
    let tileset_id = {
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let (_layer, tileset_id) = lookup_tilemap_layer(project, args.sprite_id, args.layer_id)?;
        validate_tile_index(project, args.sprite_id, tileset_id, args.cell)?;
        tileset_id
    };
    let _ = tileset_id;

    // Ensure the tilemap cel exists, then bounds-check the target cell.
    let (cols, rows) = ensure_tilemap_cel(doc, args.sprite_id, args.layer_id, args.frame_index)?;
    if args.cell_x >= cols || args.cell_y >= rows {
        return Err(AppCommandError::OutOfRange {
            detail: format!(
                "tile cell ({}, {}) out of range (grid is {}x{})",
                args.cell_x, args.cell_y, cols, rows
            ),
        });
    }

    let cmd = TileSetCellCommand {
        sprite_id: args.sprite_id,
        layer_id: args.layer_id,
        frame_index: args.frame_index,
        cell_x: args.cell_x,
        cell_y: args.cell_y,
        new_cell: args.cell,
        prev_cell: TileCell::empty(),
        label: "place tile",
    };
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    doc.history.push(Box::new(cmd), project)?;
    doc.dirty = true;

    emit_cell_changed(
        &app,
        args.sprite_id,
        args.layer_id,
        args.frame_index,
        args.cell_x,
        args.cell_y,
    );
    Ok(())
}

/// Erases a tile cell on a tilemap layer (writes `TileCell::empty()`).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tile_erase(
    app: AppHandle,
    args: TileEraseArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    {
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let (_layer, _tileset_id) = lookup_tilemap_layer(project, args.sprite_id, args.layer_id)?;
    }

    let (cols, rows) = ensure_tilemap_cel(doc, args.sprite_id, args.layer_id, args.frame_index)?;
    if args.cell_x >= cols || args.cell_y >= rows {
        return Err(AppCommandError::OutOfRange {
            detail: format!(
                "tile cell ({}, {}) out of range (grid is {}x{})",
                args.cell_x, args.cell_y, cols, rows
            ),
        });
    }

    let cmd = TileSetCellCommand {
        sprite_id: args.sprite_id,
        layer_id: args.layer_id,
        frame_index: args.frame_index,
        cell_x: args.cell_x,
        cell_y: args.cell_y,
        new_cell: TileCell::empty(),
        prev_cell: TileCell::empty(),
        label: "erase tile",
    };
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    doc.history.push(Box::new(cmd), project)?;
    doc.dirty = true;

    emit_cell_changed(
        &app,
        args.sprite_id,
        args.layer_id,
        args.frame_index,
        args.cell_x,
        args.cell_y,
    );
    Ok(())
}

/// Applies an autotile rule set to every empty / source-tile cell on the
/// layer's grid based on its 8-neighbor pattern.
///
/// Cells that already hold an unrelated tile (one that isn't the source
/// tile and isn't empty) are left alone. The result tile index is offset
/// from `args.source_tile` so the resolver's `0..N` output lands on the
/// caller-chosen autotile region in the tileset.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tile_autotile_apply(
    app: AppHandle,
    args: AutotileArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    {
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let (_layer, tileset_id) = lookup_tilemap_layer(project, args.sprite_id, args.layer_id)?;
        // source_tile must exist in the tileset.
        let probe_cell = TileCell {
            index: args.source_tile,
            flags: TileFlags::empty(),
        };
        validate_tile_index(project, args.sprite_id, tileset_id, probe_cell)?;
    }

    let (_cols, _rows) = ensure_tilemap_cel(doc, args.sprite_id, args.layer_id, args.frame_index)?;

    // Compute the changes against an immutable snapshot of the grid so the
    // resolver sees a consistent neighbor view, then apply them in one undo
    // entry.
    let changes = {
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let sprite = project
            .sprites
            .iter()
            .find(|s| s.id == args.sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(args.sprite_id.get()),
            })?;
        let cel = sprite
            .cels
            .iter()
            .find(|c| c.layer_id == args.layer_id && c.frame_index == args.frame_index)
            .ok_or(AppCommandError::NotFound {
                entity: "cel".into(),
                id: u64::from(args.frame_index.get()),
            })?;
        let CelData::Tilemap { data } = &cel.data else {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "cel for layer {} frame {} is not a tilemap cel",
                    args.layer_id.get(),
                    args.frame_index.get()
                ),
            });
        };
        compute_autotile_changes(data, &args.kind, args.source_tile)
    };

    if changes.is_empty() {
        return Ok(());
    }

    let cmd = TileBulkCommand {
        sprite_id: args.sprite_id,
        layer_id: args.layer_id,
        frame_index: args.frame_index,
        changes,
    };
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    doc.history.push(Box::new(cmd), project)?;
    doc.dirty = true;

    let payload = TilemapBulkChangedPayload {
        sprite_id: args.sprite_id.get(),
        layer_id: args.layer_id.get(),
        frame_index: args.frame_index.get(),
    };
    if let Err(err) = app.emit("tilemap:bulk-changed", payload) {
        tracing::warn!("failed to emit tilemap:bulk-changed: {err}");
    }
    Ok(())
}

/// Computes which cells the autotile resolver would change, given the
/// current grid snapshot. Returns `(x, y, before, after)` for each cell
/// whose contents would change.
fn compute_autotile_changes(
    data: &TilemapData,
    kind: &AutotileKind,
    source_tile: TileIndex,
) -> Vec<(u32, u32, TileCell, TileCell)> {
    let base = source_tile.get();
    let mut changes = Vec::new();
    for y in 0..data.height {
        for x in 0..data.width {
            // Skip cells that hold an unrelated tile (not empty, not source).
            // Cells holding the source tile (or any tile in the autotile
            // region near it) are eligible to be re-resolved as the
            // neighbor pattern changes.
            let current = data.cell(x, y).unwrap_or_else(TileCell::empty);
            if !current.is_empty() && !is_autotile_member(current.index.get(), base) {
                continue;
            }
            let resolved = resolve_autotile(data, x, y, kind, |c| c.is_some_and(|c| !c.is_empty()));
            // Apply the source-tile base offset. resolve_autotile returns
            // a 0..N tile index relative to the autotile region.
            let target_index = base.saturating_add(resolved.index.get());
            let target = TileCell {
                index: TileIndex::new(target_index),
                flags: resolved.flags,
            };
            // Empty source cells stay empty unless the resolver picked a
            // non-zero offset that the caller wants painted; we treat
            // empty in / empty out as a no-op.
            if current.is_empty() && resolved.index.get() == 0 {
                continue;
            }
            if current != target {
                changes.push((x, y, current, target));
            }
        }
    }
    changes
}

/// Returns true if `index` falls in the autotile region anchored at `base`.
///
/// Blob47 has 47 tiles, Corner16 has 16, Minimal4 has 4 — bounded above by
/// 47, so any tile within `[base, base + 47)` counts as "the user previously
/// painted via autotile" and is fair game for re-resolution.
fn is_autotile_member(index: u32, base: u32) -> bool {
    index >= base && index < base.saturating_add(47)
}

fn emit_cell_changed(
    app: &AppHandle,
    sprite_id: SpriteId,
    layer_id: LayerId,
    frame_index: FrameIndex,
    cell_x: u32,
    cell_y: u32,
) {
    let payload = TilemapCellChangedPayload {
        sprite_id: sprite_id.get(),
        layer_id: layer_id.get(),
        frame_index: frame_index.get(),
        cell_x,
        cell_y,
    };
    if let Err(err) = app.emit("tilemap:cell-changed", payload) {
        tracing::warn!("failed to emit tilemap:cell-changed: {err}");
    }
}

// ── Tileset management ────────────────────────────────────────────────────────

/// Arguments for adding a new tileset to a sprite.
#[derive(Debug, Deserialize)]
pub struct TilesetAddArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Display name for the new tileset.
    pub name: String,
    /// Width of each tile in pixels.
    pub tile_width: u32,
    /// Height of each tile in pixels.
    pub tile_height: u32,
}

/// Lists all tilesets in a sprite.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tileset_list(
    sprite_id: SpriteId,
    state: State<'_, AppState>,
) -> CommandResult<Vec<Tileset>> {
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
    Ok(sprite.tilesets.clone())
}

/// Adds a new empty tileset to a sprite.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tileset_add(
    args: TilesetAddArgs,
    state: State<'_, AppState>,
) -> CommandResult<Tileset> {
    if args.tile_width == 0 || args.tile_height == 0 {
        return Err(AppCommandError::Validation {
            detail: format!(
                "tileset tile size must be positive (got {}x{})",
                args.tile_width, args.tile_height
            ),
        });
    }
    let mut doc = state.doc.write().await;
    let id = TilesetId::new(doc.next_id);
    doc.next_id += 1;
    // PixelBufferId(0) is the project's null/sentinel slot. Mint a fresh
    // id from doc.next_id so the inline tileset's atlas buffer doesn't
    // alias the sentinel — the actual atlas pixels are written to this
    // buffer when S01 grows the tileset's tile count.
    let buffer_id = PixelBufferId::new(doc.next_id);
    doc.next_id += 1;
    let tileset = {
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
        let tileset = Tileset {
            id,
            name: args.name,
            tile_size: Size::new(args.tile_width, args.tile_height),
            // tile_count = 1 (just the implicit empty tile at index 0).
            // The pixel buffer subsystem (S01) grows this as tiles are added.
            tile_count: 1,
            base_index: 1,
            source: TilesetSource::Inline { buffer: buffer_id },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        };
        sprite.tilesets.push(tileset.clone());
        tileset
    };
    doc.dirty = true;
    Ok(tileset)
}

/// Renames a tileset.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tileset_rename(
    sprite_id: SpriteId,
    tileset_id: TilesetId,
    name: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
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
    let tileset = sprite
        .tilesets
        .iter_mut()
        .find(|t| t.id == tileset_id)
        .ok_or(AppCommandError::NotFound {
            entity: "tileset".into(),
            id: u64::from(tileset_id.get()),
        })?;
    tileset.name = name;
    doc.dirty = true;
    Ok(())
}

/// Sets per-tile metadata (collision, animation) on a tileset entry.
///
/// Pushes a project-level history entry so the change participates in
/// undo/redo. Returns the updated tileset so the UI can refresh without
/// a separate round-trip.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tileset_set_tile_metadata(
    args: TilesetSetTileMetadataArgs,
    state: State<'_, AppState>,
) -> CommandResult<Tileset> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    let cmd = TilesetSetTileMetadataCommand {
        sprite_id: args.sprite_id,
        tileset_id: args.tileset_id,
        tile_index: args.tile_index,
        new_metadata: args.metadata,
        prev: None,
        prev_len: 0,
    };
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    doc.history.push(Box::new(cmd), project)?;
    doc.dirty = true;

    // Return the updated tileset so the UI can refresh local state without
    // a follow-up tileset_list round-trip.
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let sprite = project
        .sprites
        .iter()
        .find(|s| s.id == args.sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(args.sprite_id.get()),
        })?;
    let tileset = sprite
        .tilesets
        .iter()
        .find(|t| t.id == args.tileset_id)
        .ok_or(AppCommandError::NotFound {
            entity: "tileset".into(),
            id: u64::from(args.tileset_id.get()),
        })?;
    Ok(tileset.clone())
}

/// Sets (or clears) the autotile rule set bound to a tileset.
///
/// Persists `Tileset.autotile`, marks the document dirty, and returns
/// the updated tileset so the caller can refresh local state in one
/// round-trip. Standard variants (`Blob47`, `Corner16`, `Minimal4`) need
/// no extra data; the `Custom` variant carries its own rules + default
/// tile inline.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tileset_set_autotile(
    sprite_id: SpriteId,
    tileset_id: TilesetId,
    autotile: Option<AutotileKind>,
    state: State<'_, AppState>,
) -> CommandResult<Tileset> {
    let mut doc = state.doc.write().await;
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
    let tileset = sprite
        .tilesets
        .iter_mut()
        .find(|t| t.id == tileset_id)
        .ok_or(AppCommandError::NotFound {
            entity: "tileset".into(),
            id: u64::from(tileset_id.get()),
        })?;
    tileset.autotile = autotile;
    let updated = tileset.clone();
    doc.dirty = true;
    Ok(updated)
}

/// Arguments for capturing a tile from a raster source layer into a tileset.
#[derive(Debug, Deserialize)]
pub struct TilesetAddTileArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Tileset that grows by one tile.
    pub tileset_id: TilesetId,
    /// Raster layer to read pixels from. Must be a raster layer; tilemap
    /// and group layers reject with a Validation error.
    pub source_layer_id: LayerId,
    /// Frame on the source layer to read from.
    pub frame_index: FrameIndex,
    /// Top-left X coordinate of the capture region in the source layer's
    /// pixel buffer.
    pub source_x: i32,
    /// Top-left Y coordinate of the capture region in the source layer's
    /// pixel buffer.
    pub source_y: i32,
}

/// Result of [`tileset_add_tile`]: the updated tileset and the index of
/// the newly captured tile.
#[derive(Debug, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TilesetAddTileResult {
    /// Tileset with `tile_count` incremented and (potentially) a
    /// freshly-allocated atlas buffer.
    pub tileset: Tileset,
    /// Index of the new tile (i.e. previous `tile_count`). Suitable for
    /// `selectedTileIndex` so the user can immediately paint with it.
    pub index: TileIndex,
}

/// Captures a `tile_size`-sized region from a raster layer and appends it
/// to the tileset's inline atlas as a new tile.
///
/// Atlas layout (per `core/src/project/tileset.rs`) is row-major with
/// tiles laid out left-to-right; growing the buffer means widening it by
/// one tile-width. Pixels outside the source layer's buffer read as
/// transparent — matches Aseprite's atlas-build behaviour and lets the
/// user capture from regions that overhang the canvas.
///
/// External tilesets reject with a Validation error: capture only writes
/// into Inline buffers. Locked source layers reject the same way.
#[tauri::command(async, rename_all = "snake_case")]
#[allow(clippy::too_many_lines)] // multi-phase: validate, read, grow, install, mutate
pub async fn tileset_add_tile(
    args: TilesetAddTileArgs,
    state: State<'_, AppState>,
) -> CommandResult<TilesetAddTileResult> {
    let mut doc = state.doc.write().await;

    // Phase 1 — resolve everything under a shared borrow.
    let (atlas_buffer_id, tile_w, tile_h, prev_tile_count, src_buffer_id) = {
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let sprite = project
            .sprites
            .iter()
            .find(|s| s.id == args.sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(args.sprite_id.get()),
            })?;
        let tileset = sprite
            .tilesets
            .iter()
            .find(|t| t.id == args.tileset_id)
            .ok_or(AppCommandError::NotFound {
                entity: "tileset".into(),
                id: u64::from(args.tileset_id.get()),
            })?;
        let TilesetSource::Inline { buffer } = tileset.source else {
            return Err(AppCommandError::Validation {
                detail: "capture supports only inline tilesets".into(),
            });
        };
        let layer = sprite
            .layers
            .iter()
            .find(|l| l.id == args.source_layer_id)
            .ok_or(AppCommandError::NotFound {
                entity: "layer".into(),
                id: u64::from(args.source_layer_id.get()),
            })?;
        if layer.locked {
            return Err(AppCommandError::LayerLocked {
                layer_id: args.source_layer_id.get(),
            });
        }
        if !matches!(layer.kind, LayerKind::Raster) {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "source layer {} is not a raster layer",
                    args.source_layer_id.get()
                ),
            });
        }
        let cel = sprite
            .cels
            .iter()
            .find(|c| c.layer_id == args.source_layer_id && c.frame_index == args.frame_index)
            .ok_or(AppCommandError::NotFound {
                entity: "cel".into(),
                id: u64::from(args.frame_index.get()),
            })?;
        let CelData::Raster {
            buffer: src_buffer, ..
        } = cel.data
        else {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "cel for layer {} frame {} is not a raster cel",
                    args.source_layer_id.get(),
                    args.frame_index.get()
                ),
            });
        };
        (
            buffer,
            tileset.tile_size.width,
            tileset.tile_size.height,
            tileset.tile_count,
            src_buffer,
        )
    };

    if tile_w == 0 || tile_h == 0 {
        return Err(AppCommandError::Validation {
            detail: "tileset tile size must be positive".into(),
        });
    }

    // Phase 2 — read tile pixels from the source buffer (out-of-bounds = transparent).
    let captured = read_tile_pixels(
        &doc.pixel_buffers,
        src_buffer_id,
        args.source_x,
        args.source_y,
        tile_w,
        tile_h,
    );

    // Phase 3 — grow (or create) the atlas and write the captured tile at
    // index `prev_tile_count`. Atlas dimensions are
    // `(tile_w * tile_count, tile_h)`; index 0 is the empty sentinel,
    // initialised to all-zero on first write.
    let new_tile_count = prev_tile_count + 1;
    let new_atlas_w = tile_w * new_tile_count;
    let bytes_per_pixel: u32 = 4;
    let new_stride = new_atlas_w * bytes_per_pixel;

    let mut new_pixels = vec![0u8; (new_atlas_w * tile_h * bytes_per_pixel) as usize];

    let atlas_idx = doc
        .pixel_buffers
        .iter()
        .position(|e| e.id == atlas_buffer_id.get());
    if let Some(idx) = atlas_idx {
        let entry = &doc.pixel_buffers[idx];
        // Copy old atlas rows into the wider buffer, preserving every
        // existing tile.
        let old_w = entry.width;
        let old_stride = entry.stride;
        let copy_w = old_w.min(new_atlas_w) as usize;
        let copy_bytes = copy_w * bytes_per_pixel as usize;
        for y in 0..tile_h.min(entry.height) as usize {
            let src_row_start = y * old_stride as usize;
            let dst_row_start = y * new_stride as usize;
            new_pixels[dst_row_start..dst_row_start + copy_bytes]
                .copy_from_slice(&entry.pixels[src_row_start..src_row_start + copy_bytes]);
        }
    }

    // Write the captured tile into the new slot.
    let dst_x_start = (prev_tile_count * tile_w) as usize;
    for y in 0..tile_h as usize {
        let src_row_start = y * (tile_w as usize) * bytes_per_pixel as usize;
        let dst_row_start = y * new_stride as usize + dst_x_start * bytes_per_pixel as usize;
        let row_bytes = tile_w as usize * bytes_per_pixel as usize;
        new_pixels[dst_row_start..dst_row_start + row_bytes]
            .copy_from_slice(&captured[src_row_start..src_row_start + row_bytes]);
    }

    // Phase 4 — install the buffer entry.
    if let Some(idx) = atlas_idx {
        let entry = &mut doc.pixel_buffers[idx];
        entry.width = new_atlas_w;
        entry.height = tile_h;
        entry.stride = new_stride;
        entry.pixels = new_pixels;
    } else {
        doc.pixel_buffers
            .push(pixhaus_io::pixhaus::PixelBufferEntry {
                id: atlas_buffer_id.get(),
                width: new_atlas_w,
                height: tile_h,
                stride: new_stride,
                pixels: new_pixels,
            });
    }

    // Phase 5 — bump tile_count and return the updated tileset.
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let sprite = project
        .sprites
        .iter_mut()
        .find(|s| s.id == args.sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(args.sprite_id.get()),
        })?;
    let tileset = sprite
        .tilesets
        .iter_mut()
        .find(|t| t.id == args.tileset_id)
        .ok_or(AppCommandError::NotFound {
            entity: "tileset".into(),
            id: u64::from(args.tileset_id.get()),
        })?;
    tileset.tile_count = new_tile_count;
    let updated = tileset.clone();
    doc.dirty = true;

    Ok(TilesetAddTileResult {
        tileset: updated,
        index: TileIndex::new(prev_tile_count),
    })
}

/// Reads a `tile_w × tile_h` region of RGBA pixels from a source buffer
/// at `(source_x, source_y)`. Pixels outside the buffer (including
/// negative coords or coords past the buffer width/height) read as
/// transparent (all zero).
fn read_tile_pixels(
    buffers: &[pixhaus_io::pixhaus::PixelBufferEntry],
    src_buffer_id: PixelBufferId,
    source_x: i32,
    source_y: i32,
    tile_w: u32,
    tile_h: u32,
) -> Vec<u8> {
    const BYTES_PER_PIXEL: usize = 4;
    let mut out = vec![0u8; (tile_w as usize) * (tile_h as usize) * BYTES_PER_PIXEL];
    let Some(entry) = buffers.iter().find(|e| e.id == src_buffer_id.get()) else {
        return out;
    };
    let src_w = entry.width;
    let src_h = entry.height;
    let src_stride = entry.stride as usize;
    // Convert the negative-tolerant source origin into a clipped
    // (src_x_start, dst_x_start, copy_w) triple so the inner loop only
    // touches valid u32 indices.
    let (src_col, dst_col, copy_w) = clip_axis(source_x, src_w, tile_w);
    let (src_row, dst_row, copy_h) = clip_axis(source_y, src_h, tile_h);
    if copy_w == 0 || copy_h == 0 {
        return out;
    }
    let row_bytes = copy_w as usize * BYTES_PER_PIXEL;
    for r in 0..copy_h {
        let s_row = src_row + r;
        let d_row = dst_row + r;
        let src_off = (s_row as usize) * src_stride + (src_col as usize) * BYTES_PER_PIXEL;
        let dst_off = (d_row as usize) * (tile_w as usize) * BYTES_PER_PIXEL
            + (dst_col as usize) * BYTES_PER_PIXEL;
        out[dst_off..dst_off + row_bytes]
            .copy_from_slice(&entry.pixels[src_off..src_off + row_bytes]);
    }
    out
}

/// Clips a one-dimensional capture range against a source extent.
///
/// Returns `(src_start, dst_start, copy_len)` so a negative `source`
/// origin shifts the destination right while clipping the source to its
/// valid range. `copy_len == 0` means the capture lies entirely outside
/// the source.
fn clip_axis(source: i32, src_extent: u32, tile_extent: u32) -> (u32, u32, u32) {
    if tile_extent == 0 || src_extent == 0 {
        return (0, 0, 0);
    }
    let (src_start, dst_start) = if source < 0 {
        let shift = source.unsigned_abs().min(tile_extent);
        (0, shift)
    } else {
        let s = source.unsigned_abs();
        if s >= src_extent {
            return (0, 0, 0);
        }
        (s, 0)
    };
    let remaining_src = src_extent.saturating_sub(src_start);
    let remaining_dst = tile_extent.saturating_sub(dst_start);
    (src_start, dst_start, remaining_src.min(remaining_dst))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::{BlendMode, CollisionShape, Layer, LayerKind, Project, Sprite};
    use pixhaus_core::undo::{Error as UndoError, History};

    fn project_with_tilemap_layer() -> (Project, SpriteId, LayerId, TilesetId) {
        let mut project = Project::new("test");
        let sprite_id = SpriteId::new(1);
        let layer_id = LayerId::new(2);
        let tileset_id = TilesetId::new(3);
        let mut sprite = Sprite::empty(sprite_id, "main", Size::new(64, 48));
        sprite.tilesets.push(Tileset {
            id: tileset_id,
            name: "dungeon".into(),
            tile_size: Size::new(16, 16),
            tile_count: 64,
            base_index: 1,
            source: TilesetSource::Inline {
                buffer: PixelBufferId::new(99),
            },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        });
        sprite.layers.push(Layer {
            id: layer_id,
            name: "ground".into(),
            kind: LayerKind::Tilemap {
                tileset: tileset_id,
            },
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            user_data: UserData::default(),
        });
        // Pre-populate a 4x3 tilemap cel at frame 0.
        sprite.cels.push(Cel {
            layer_id,
            frame_index: FrameIndex::new(0),
            position: IVec2::zero(),
            opacity: 255,
            data: CelData::Tilemap {
                data: TilemapData::empty(4, 3),
            },
            user_data: UserData::default(),
        });
        project.sprites.push(sprite);
        (project, sprite_id, layer_id, tileset_id)
    }

    #[test]
    fn tile_set_cell_command_writes_and_reverts() {
        let (mut project, sprite_id, layer_id, _tileset_id) = project_with_tilemap_layer();
        let mut history = History::new();
        let cmd = TileSetCellCommand {
            sprite_id,
            layer_id,
            frame_index: FrameIndex::new(0),
            cell_x: 1,
            cell_y: 1,
            new_cell: TileCell {
                index: TileIndex::new(5),
                flags: TileFlags::FLIP_X,
            },
            prev_cell: TileCell::empty(),
            label: "place tile",
        };
        history.push(Box::new(cmd), &mut project).unwrap();

        // Cell should now hold the placed tile.
        let data =
            find_tilemap_data_mut(&mut project, sprite_id, layer_id, FrameIndex::new(0)).unwrap();
        assert_eq!(
            data.cell(1, 1).unwrap(),
            TileCell {
                index: TileIndex::new(5),
                flags: TileFlags::FLIP_X,
            },
        );

        history.undo(&mut project).unwrap();
        let data =
            find_tilemap_data_mut(&mut project, sprite_id, layer_id, FrameIndex::new(0)).unwrap();
        assert_eq!(data.cell(1, 1).unwrap(), TileCell::empty());
    }

    #[test]
    fn tile_set_cell_command_rejects_out_of_bounds() {
        let (mut project, sprite_id, layer_id, _tileset_id) = project_with_tilemap_layer();
        let mut history = History::new();
        // Grid is 4x3; (5, 5) is out of range.
        let cmd = TileSetCellCommand {
            sprite_id,
            layer_id,
            frame_index: FrameIndex::new(0),
            cell_x: 5,
            cell_y: 5,
            new_cell: TileCell {
                index: TileIndex::new(1),
                flags: TileFlags::empty(),
            },
            prev_cell: TileCell::empty(),
            label: "place tile",
        };
        let err = history.push(Box::new(cmd), &mut project).unwrap_err();
        assert!(
            matches!(err, UndoError::CommandFailed { .. }),
            "expected CommandFailed for OOB cell, got {err:?}",
        );
    }

    #[test]
    fn lookup_tilemap_layer_rejects_non_tilemap_layer() {
        let (mut project, sprite_id, _, _) = project_with_tilemap_layer();
        // Add a raster layer alongside the tilemap layer.
        let raster_id = LayerId::new(99);
        project.sprites[0]
            .layers
            .push(Layer::raster(raster_id, "fx"));

        let err = lookup_tilemap_layer(&project, sprite_id, raster_id).unwrap_err();
        assert!(
            matches!(err, AppCommandError::Validation { .. }),
            "non-tilemap layer must reject as Validation, got {err:?}",
        );
    }

    #[test]
    fn lookup_tilemap_layer_returns_tileset_id() {
        let (project, sprite_id, layer_id, tileset_id) = project_with_tilemap_layer();
        let (_, ts) = lookup_tilemap_layer(&project, sprite_id, layer_id).unwrap();
        assert_eq!(ts, tileset_id);
    }

    #[test]
    fn validate_tile_index_rejects_out_of_range() {
        let (project, sprite_id, _, tileset_id) = project_with_tilemap_layer();
        // Tileset has tile_count=64; index 100 is out of range.
        let cell = TileCell {
            index: TileIndex::new(100),
            flags: TileFlags::empty(),
        };
        let err = validate_tile_index(&project, sprite_id, tileset_id, cell).unwrap_err();
        assert!(matches!(err, AppCommandError::Validation { .. }));
    }

    #[test]
    fn validate_tile_index_passes_for_empty_cell() {
        let (project, sprite_id, _, tileset_id) = project_with_tilemap_layer();
        validate_tile_index(&project, sprite_id, tileset_id, TileCell::empty()).unwrap();
    }

    #[test]
    fn compute_autotile_changes_paints_minimal4_neighbors() {
        // Build a 3x3 grid with the center cell holding the source tile;
        // empty neighbors that touch the filled center get filled with
        // base+1 (one cardinal connection), the center itself stays at
        // base (zero cardinals).
        let mut data = TilemapData::empty(3, 3);
        data.cells[4] = TileCell {
            index: TileIndex::new(10),
            flags: TileFlags::empty(),
        };
        let changes = compute_autotile_changes(&data, &AutotileKind::Minimal4, TileIndex::new(10));
        // The 4 cardinally-adjacent empty cells (N, E, S, W of center)
        // each have one cardinal neighbor (the center), so they resolve
        // to index 1 → target 11. The 4 corners and the center see no
        // cardinal connections, so they stay put.
        assert_eq!(
            changes.len(),
            4,
            "expected 4 cardinal-adjacent empty cells to be painted",
        );
        for (_, _, before, after) in &changes {
            assert_eq!(before.index.get(), 0);
            assert_eq!(after.index.get(), 11);
        }

        // Fill all 9 cells with the source tile.
        for cell in &mut data.cells {
            cell.index = TileIndex::new(10);
        }
        let changes = compute_autotile_changes(&data, &AutotileKind::Minimal4, TileIndex::new(10));
        // Center cell now has 4 cardinal neighbors -> index 3, target = 13.
        let center = changes.iter().find(|(x, y, _, _)| *x == 1 && *y == 1);
        assert!(
            center.is_some(),
            "fully-surrounded center should change with Minimal4",
        );
        let (_, _, before, after) = center.unwrap();
        assert_eq!(before.index.get(), 10);
        assert_eq!(after.index.get(), 13);
    }

    #[test]
    fn is_autotile_member_bounds_check() {
        // base=10, region [10, 57)
        assert!(is_autotile_member(10, 10));
        assert!(is_autotile_member(56, 10));
        assert!(!is_autotile_member(57, 10));
        assert!(!is_autotile_member(9, 10));
    }

    #[test]
    fn tile_bulk_command_apply_undo_round_trip() {
        let (mut project, sprite_id, layer_id, _) = project_with_tilemap_layer();
        let mut history = History::new();
        let target = TileCell {
            index: TileIndex::new(7),
            flags: TileFlags::empty(),
        };
        let changes = vec![
            (0, 0, TileCell::empty(), target),
            (3, 2, TileCell::empty(), target),
        ];
        let cmd = TileBulkCommand {
            sprite_id,
            layer_id,
            frame_index: FrameIndex::new(0),
            changes,
        };
        history.push(Box::new(cmd), &mut project).unwrap();
        {
            let data = find_tilemap_data_mut(&mut project, sprite_id, layer_id, FrameIndex::new(0))
                .unwrap();
            assert_eq!(data.cell(0, 0).unwrap(), target);
            assert_eq!(data.cell(3, 2).unwrap(), target);
        }
        history.undo(&mut project).unwrap();
        let data =
            find_tilemap_data_mut(&mut project, sprite_id, layer_id, FrameIndex::new(0)).unwrap();
        assert_eq!(data.cell(0, 0).unwrap(), TileCell::empty());
        assert_eq!(data.cell(3, 2).unwrap(), TileCell::empty());
    }

    #[test]
    fn tileset_set_tile_metadata_command_extends_and_undoes() {
        let (mut project, sprite_id, _, tileset_id) = project_with_tilemap_layer();
        let mut history = History::new();
        let new_meta = TileProperties {
            collision: CollisionShape::Full,
            animation: None,
        };
        let cmd = TilesetSetTileMetadataCommand {
            sprite_id,
            tileset_id,
            tile_index: TileIndex::new(3),
            new_metadata: new_meta.clone(),
            prev: None,
            prev_len: 0,
        };
        history.push(Box::new(cmd), &mut project).unwrap();
        {
            let tileset = find_tileset_mut(&mut project, sprite_id, tileset_id).unwrap();
            assert_eq!(tileset.properties.len(), 4);
            assert_eq!(tileset.properties[3], new_meta);
            // The pad entries before idx 3 should be defaults.
            assert_eq!(tileset.properties[0], TileProperties::default());
        }
        history.undo(&mut project).unwrap();
        let tileset = find_tileset_mut(&mut project, sprite_id, tileset_id).unwrap();
        assert!(tileset.properties.is_empty());
    }

    #[test]
    fn tileset_set_tile_metadata_command_overwrites_existing() {
        let (mut project, sprite_id, _, tileset_id) = project_with_tilemap_layer();
        // Pre-populate properties[1].
        {
            let tileset = find_tileset_mut(&mut project, sprite_id, tileset_id).unwrap();
            tileset.properties.push(TileProperties::default());
            tileset.properties.push(TileProperties::default());
        }
        let mut history = History::new();
        let new_meta = TileProperties {
            collision: CollisionShape::Full,
            animation: None,
        };
        let cmd = TilesetSetTileMetadataCommand {
            sprite_id,
            tileset_id,
            tile_index: TileIndex::new(1),
            new_metadata: new_meta.clone(),
            prev: None,
            prev_len: 0,
        };
        history.push(Box::new(cmd), &mut project).unwrap();
        {
            let tileset = find_tileset_mut(&mut project, sprite_id, tileset_id).unwrap();
            assert_eq!(tileset.properties.len(), 2);
            assert_eq!(tileset.properties[1], new_meta);
        }
        history.undo(&mut project).unwrap();
        let tileset = find_tileset_mut(&mut project, sprite_id, tileset_id).unwrap();
        assert_eq!(tileset.properties.len(), 2);
        assert_eq!(tileset.properties[1], TileProperties::default());
    }

    #[test]
    fn tile_place_args_constructs() {
        let args = TilePlaceArgs {
            sprite_id: SpriteId::new(1),
            layer_id: LayerId::new(2),
            frame_index: FrameIndex::new(0),
            cell_x: 3,
            cell_y: 4,
            cell: TileCell {
                index: TileIndex::new(5),
                flags: TileFlags::empty(),
            },
        };
        assert_eq!(args.cell.index.get(), 5);
    }

    #[test]
    fn tileset_add_args_fields() {
        let args = TilesetAddArgs {
            sprite_id: SpriteId::new(1),
            name: "dungeon".into(),
            tile_width: 16,
            tile_height: 16,
        };
        assert_eq!(args.tile_width, 16);
        assert_eq!(args.tile_height, 16);
    }

    #[test]
    fn tileset_add_uses_default_user_data() {
        let ud = UserData::default();
        assert!(ud.is_empty());
    }

    #[test]
    fn read_tile_pixels_in_bounds_copies_region() {
        // 4×2 source buffer, RGBA. Two distinct pixels at (1, 0) and (2, 1).
        let mut pixels = vec![0u8; 4 * 2 * 4];
        // (1, 0) = red
        pixels[4..8].copy_from_slice(&[255, 0, 0, 255]);
        // (2, 1) at row=1: row_start = 1 * (4*4) = 16; col 2 offset = 8
        pixels[16 + 8..16 + 12].copy_from_slice(&[0, 255, 0, 255]);
        let buffers = vec![pixhaus_io::pixhaus::PixelBufferEntry {
            id: 42,
            width: 4,
            height: 2,
            stride: 16,
            pixels,
        }];
        let captured = read_tile_pixels(&buffers, PixelBufferId::new(42), 1, 0, 2, 2);
        // Output is 2×2, RGBA = 16 bytes. (0, 0) maps to source (1, 0) = red;
        // (1, 1) maps to source (2, 1) = green.
        assert_eq!(&captured[0..4], &[255, 0, 0, 255]);
        assert_eq!(&captured[2 * 4 + 4..2 * 4 + 8], &[0, 255, 0, 255]);
    }

    #[test]
    fn read_tile_pixels_out_of_bounds_reads_transparent() {
        let buffers = vec![pixhaus_io::pixhaus::PixelBufferEntry {
            id: 42,
            width: 4,
            height: 2,
            stride: 16,
            pixels: vec![0xff; 4 * 2 * 4],
        }];
        // source_y = -1 → entire first row of output is OOB and stays 0.
        let captured = read_tile_pixels(&buffers, PixelBufferId::new(42), 0, -1, 2, 2);
        assert_eq!(&captured[0..8], &[0; 8]);
        // Second row (y=0 in source) reads through.
        assert_eq!(&captured[8..16], &[0xff; 8]);
    }

    #[test]
    fn read_tile_pixels_missing_buffer_returns_zeros() {
        let buffers: Vec<pixhaus_io::pixhaus::PixelBufferEntry> = Vec::new();
        let captured = read_tile_pixels(&buffers, PixelBufferId::new(42), 0, 0, 2, 2);
        assert_eq!(captured, vec![0u8; 16]);
    }
}
