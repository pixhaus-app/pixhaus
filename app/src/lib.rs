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
pub mod crash_reporting;
pub mod error;
pub mod menu;
pub mod pixel_history;
pub mod state;

pub use error::{AppCommandError, CommandResult};

use tauri::Manager as _;
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
#[allow(clippy::too_many_lines)]
pub fn run() -> Result<(), AppError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Initialise crash reporting early so the panic hook is in place
    // before any async work starts. The guard must be kept alive for the
    // full process lifetime; events are gated by `crash_reporting::ENABLED`
    // which the UI sets via `crash_reporting_set_enabled` on startup.
    let _crash_guard = crash_reporting::init();

    tracing::info!("starting {}", app_name());

    // tauri::generate_context! still expands to code that calls .unwrap() on
    // baked-in results in Tauri 2.11; the disallowed_methods lint can't see
    // through the macro. Re-test on each Tauri minor bump and remove if fixed.
    #[allow(clippy::disallowed_methods)]
    let context = tauri::generate_context!();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(AppState::new())
        .setup(|app| {
            let m = menu::build(app.handle())?;
            app.set_menu(m)?;

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                commands::updater::check_on_startup(handle).await;
            });

            // Scan user-installed plugins asynchronously so startup is not
            // blocked by WASM compilation or slow storage. Any plugin that
            // fails to load is logged as a warning and skipped.
            let plugins = app.state::<AppState>().plugins.clone();
            tauri::async_runtime::spawn(async move {
                match plugins.scan().await {
                    Ok(n) => tracing::info!(count = n, "plugins loaded"),
                    Err(e) => tracing::warn!(error = %e, "plugin scan failed"),
                }
            });

            Ok(())
        })
        .on_menu_event(|app, event| {
            menu::handle_event(app, &event);
        })
        .invoke_handler(tauri::generate_handler![
            // app info
            commands::app_info::app_about,
            // crash reporting
            commands::crash_reporting::crash_reporting_get_enabled,
            commands::crash_reporting::crash_reporting_set_enabled,
            // exports
            commands::exports::export_animated_gif,
            commands::exports::export_animated_webp,
            commands::exports::export_png_sprite_sheet,
            commands::exports::export_tmx,
            // canvas
            commands::canvas::canvas_begin_stroke,
            commands::canvas::canvas_composite,
            commands::canvas::canvas_draw_stroke,
            commands::canvas::canvas_end_stroke,
            commands::canvas::canvas_extend_stroke,
            commands::canvas::canvas_fill,
            commands::canvas::canvas_invert_selection,
            commands::canvas::canvas_recomposite_frame,
            commands::canvas::canvas_select_all,
            commands::canvas::canvas_select_color_range,
            commands::canvas::canvas_select_lasso,
            commands::canvas::canvas_select_magic_wand,
            commands::canvas::canvas_set_selection,
            commands::canvas::canvas_set_viewport,
            commands::canvas::canvas_transform,
            // frames
            commands::frames::cel_list,
            commands::frames::frame_add,
            commands::frames::frame_delete,
            commands::frames::frame_duplicate,
            commands::frames::frame_list,
            commands::frames::frame_reorder,
            commands::frames::frame_set_duration,
            commands::frames::frame_tag_create,
            commands::frames::frame_tag_delete,
            commands::frames::frame_tag_list,
            commands::frames::frame_tag_rename,
            // layers
            commands::layers::layer_add,
            commands::layers::layer_convert_to_tilemap,
            commands::layers::layer_delete,
            commands::layers::layer_flatten_visible,
            commands::layers::layer_list,
            commands::layers::layer_merge_down,
            commands::layers::layer_merge_selected,
            commands::layers::layer_rename,
            commands::layers::layer_reorder,
            commands::layers::layer_set_blend_mode,
            commands::layers::layer_set_locked,
            commands::layers::layer_set_opacity,
            commands::layers::layer_set_parent,
            commands::layers::layer_set_visibility,
            commands::layers::layer_wrap_in_group,
            // palette
            commands::palette::palette_add,
            commands::palette::palette_add_color,
            commands::palette::palette_delete,
            commands::palette::palette_list,
            commands::palette::palette_remove_color,
            commands::palette::palette_reorder_colors,
            commands::palette::palette_set_color,
            commands::palette::palette_swap,
            // library
            commands::library::library_add_state,
            commands::library::library_add_tag,
            commands::library::library_approve_sheet_variant,
            commands::library::library_create_entity,
            commands::library::library_create_group,
            commands::library::library_delete_entity,
            commands::library::library_delete_group,
            commands::library::library_delete_state,
            commands::library::library_delete_tag,
            commands::library::library_get_active_target,
            commands::library::library_get_anchor_payload,
            commands::library::library_get_entity,
            commands::library::library_list_entities,
            commands::library::library_list_groups,
            commands::library::library_list_tags,
            commands::library::library_move_entity_to_group,
            commands::library::library_rename_entity,
            commands::library::library_rename_group,
            commands::library::library_rename_state,
            commands::library::library_rename_tag,
            commands::library::library_reorder_entities,
            commands::library::library_search,
            commands::library::library_set_active_target,
            commands::library::library_set_entity_anchor,
            commands::library::library_set_group_parent,
            commands::library::library_tag_entity,
            commands::library::library_untag_entity,
            // project
            commands::project::project_close,
            commands::project::project_get,
            commands::project::project_import_aseprite,
            commands::project::project_import_psd,
            commands::project::project_new,
            commands::project::project_open,
            commands::project::project_save,
            commands::project::sprite_add,
            commands::project::sprite_delete,
            commands::project::sprite_list,
            // samples
            commands::samples::list_samples,
            // tiles
            commands::tiles::tile_autotile_apply,
            commands::tiles::tile_erase,
            commands::tiles::tile_place,
            commands::tiles::tileset_add,
            commands::tiles::tileset_add_tile,
            commands::tiles::tileset_list,
            commands::tiles::tileset_rename,
            commands::tiles::tileset_set_autotile,
            commands::tiles::tileset_set_tile_metadata,
            // plugins
            commands::plugins::plugin_list,
            commands::plugins::plugin_reload,
            commands::plugins::plugin_scan,
            commands::plugins::plugin_unload,
            // undo/redo
            commands::undo::redo,
            commands::undo::undo,
            // updater
            commands::updater::updater_check,
            commands::updater::updater_install,
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
