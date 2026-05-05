//! Pixhaus application shell entry point.
//!
//! IPC commands are defined in `commands/` per the catalog in B4. The
//! shell wires `core`, `io`, `ai`, and `scripting` into a Tauri 2 process.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc,
        clippy::disallowed_methods
    )
)]

pub mod commands;
pub mod error;
pub mod menu;
pub mod state;

pub use error::{AppCommandError, CommandResult};

use tracing_subscriber::EnvFilter;

use state::AppState;

/// Application errors at the shell layer.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The Tauri runtime returned an error during startup or operation.
    #[error("tauri runtime error: {0}")]
    Tauri(#[from] tauri::Error),
}

/// Returns the application name. Used by tracing setup and the window title.
#[must_use]
pub fn app_name() -> &'static str {
    "Pixhaus"
}

/// Builds and runs the Tauri application.
///
/// # Errors
/// Returns [`AppError::Tauri`] when the Tauri runtime fails to start.
pub fn run() -> Result<(), AppError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("starting {}", app_name());

    // tauri::generate_context! still expands to code that calls .unwrap() on
    // baked-in results in Tauri 2.11; the disallowed_methods lint can't see
    // through the macro. Re-test on each Tauri minor bump and remove if fixed.
    #[allow(clippy::disallowed_methods)]
    let context = tauri::generate_context!();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(AppState::new())
        .setup(|app| {
            let m = menu::build(app.handle())?;
            app.set_menu(m)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            menu::handle_event(app, &event);
        })
        .invoke_handler(tauri::generate_handler![
            // canvas
            commands::canvas::canvas_composite,
            commands::canvas::canvas_draw_stroke,
            commands::canvas::canvas_fill,
            commands::canvas::canvas_set_selection,
            commands::canvas::canvas_set_viewport,
            commands::canvas::canvas_transform,
            // frames
            commands::frames::frame_add,
            commands::frames::frame_delete,
            commands::frames::frame_duplicate,
            commands::frames::frame_list,
            commands::frames::frame_reorder,
            commands::frames::frame_set_duration,
            commands::frames::frame_tag_create,
            commands::frames::frame_tag_delete,
            // layers
            commands::layers::layer_add,
            commands::layers::layer_delete,
            commands::layers::layer_list,
            commands::layers::layer_rename,
            commands::layers::layer_reorder,
            commands::layers::layer_set_blend_mode,
            commands::layers::layer_set_locked,
            commands::layers::layer_set_opacity,
            commands::layers::layer_set_visibility,
            // palette
            commands::palette::palette_add,
            commands::palette::palette_add_color,
            commands::palette::palette_delete,
            commands::palette::palette_list,
            commands::palette::palette_remove_color,
            commands::palette::palette_reorder_colors,
            commands::palette::palette_set_color,
            commands::palette::palette_swap,
            // project
            commands::project::project_close,
            commands::project::project_get,
            commands::project::project_import_psd,
            commands::project::project_new,
            commands::project::project_open,
            commands::project::project_save,
            commands::project::sprite_add,
            commands::project::sprite_delete,
            commands::project::sprite_list,
            // tiles
            commands::tiles::tile_autotile_apply,
            commands::tiles::tile_erase,
            commands::tiles::tile_place,
            commands::tiles::tileset_add,
            commands::tiles::tileset_list,
            commands::tiles::tileset_rename,
            // undo/redo
            commands::undo::redo,
            commands::undo::undo,
            // verbs
            commands::verbs::verb_cancel,
            commands::verbs::verb_invoke,
            commands::verbs::verb_list,
        ])
        .run(context)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_name_is_stable() {
        assert_eq!(app_name(), "Pixhaus");
    }
}
