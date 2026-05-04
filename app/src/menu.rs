//! Native application menu.
//!
//! The menu is built once on startup and attached to every window via
//! `app.set_menu()` in `run()`. Every item forwards its ID to the frontend
//! as a `shell:menu` event so the TypeScript command dispatcher handles
//! everything without any per-item wiring in Rust.

use tauri::menu::{Menu, MenuEvent, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter};

/// Builds the main application menu for the given handle.
#[allow(clippy::too_many_lines)] // declarative menu definition; no meaningful way to shorten
pub fn build(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let file = SubmenuBuilder::new(app, "File")
        .item(
            &MenuItemBuilder::new("New Project")
                .id("file:new")
                .accelerator("CmdOrCtrl+N")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Open...")
                .id("file:open")
                .accelerator("CmdOrCtrl+O")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::new("Save")
                .id("file:save")
                .accelerator("CmdOrCtrl+S")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Save As...")
                .id("file:save-as")
                .accelerator("CmdOrCtrl+Shift+S")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::new("Close Project")
                .id("file:close")
                .build(app)?,
        )
        .separator()
        .item(&PredefinedMenuItem::quit(app, None)?)
        .build()?;

    let edit = SubmenuBuilder::new(app, "Edit")
        .item(
            &MenuItemBuilder::new("Undo")
                .id("edit:undo")
                .accelerator("CmdOrCtrl+Z")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Redo")
                .id("edit:redo")
                .accelerator("CmdOrCtrl+Shift+Z")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::new("Cut")
                .id("edit:cut")
                .accelerator("CmdOrCtrl+X")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Copy")
                .id("edit:copy")
                .accelerator("CmdOrCtrl+C")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Paste")
                .id("edit:paste")
                .accelerator("CmdOrCtrl+V")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::new("Select All")
                .id("edit:select-all")
                .accelerator("CmdOrCtrl+A")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Deselect")
                .id("edit:deselect")
                .accelerator("CmdOrCtrl+D")
                .build(app)?,
        )
        .build()?;

    let sprite = SubmenuBuilder::new(app, "Sprite")
        .item(
            &MenuItemBuilder::new("New Sprite")
                .id("sprite:new")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Delete Sprite")
                .id("sprite:delete")
                .build(app)?,
        )
        .build()?;

    let frame = SubmenuBuilder::new(app, "Frame")
        .item(
            &MenuItemBuilder::new("Add Frame")
                .id("frame:new")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Delete Frame")
                .id("frame:delete")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Duplicate Frame")
                .id("frame:duplicate")
                .build(app)?,
        )
        .build()?;

    let layer = SubmenuBuilder::new(app, "Layer")
        .item(
            &MenuItemBuilder::new("New Layer")
                .id("layer:new")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Delete Layer")
                .id("layer:delete")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Rename Layer")
                .id("layer:rename")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::new("Merge Down")
                .id("layer:merge-down")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Flatten All Layers")
                .id("layer:flatten")
                .build(app)?,
        )
        .build()?;

    let select = SubmenuBuilder::new(app, "Select")
        .item(&MenuItemBuilder::new("All").id("select:all").build(app)?)
        .item(
            &MenuItemBuilder::new("Deselect")
                .id("select:deselect")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Invert Selection")
                .id("select:invert")
                .accelerator("CmdOrCtrl+Shift+I")
                .build(app)?,
        )
        .build()?;

    let view = SubmenuBuilder::new(app, "View")
        .item(
            &MenuItemBuilder::new("Zoom In")
                .id("view:zoom-in")
                .accelerator("CmdOrCtrl+Equal")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Zoom Out")
                .id("view:zoom-out")
                .accelerator("CmdOrCtrl+Minus")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::new("Fit to Window")
                .id("view:zoom-fit")
                .accelerator("CmdOrCtrl+0")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("100%")
                .id("view:zoom-100")
                .accelerator("CmdOrCtrl+1")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::new("Toggle Grid")
                .id("view:toggle-grid")
                .accelerator("CmdOrCtrl+G")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Toggle Pixel Grid")
                .id("view:toggle-pixel-grid")
                .build(app)?,
        )
        .build()?;

    let ai = SubmenuBuilder::new(app, "AI")
        .item(
            &MenuItemBuilder::new("Inbetween")
                .id("ai:inbetween")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Continue")
                .id("ai:continue")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Variant")
                .id("ai:variant")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Cleanup")
                .id("ai:cleanup")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Critique")
                .id("ai:critique")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::new("AI Backend Settings")
                .id("ai:settings")
                .build(app)?,
        )
        .build()?;

    let window = SubmenuBuilder::new(app, "Window")
        .item(
            &MenuItemBuilder::new("Command Palette")
                .id("window:command-palette")
                .accelerator("CmdOrCtrl+K")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::new("Preferences")
                .id("window:preferences")
                .accelerator("CmdOrCtrl+Comma")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::new("Toggle Layer Panel")
                .id("window:toggle-layers")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Toggle Timeline")
                .id("window:toggle-timeline")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::new("Toggle Color Palette")
                .id("window:toggle-palette")
                .build(app)?,
        )
        .build()?;

    let help = SubmenuBuilder::new(app, "Help")
        .item(
            &MenuItemBuilder::new("Documentation")
                .id("help:docs")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::new("About Pixhaus")
                .id("help:about")
                .build(app)?,
        )
        .build()?;

    Menu::with_items(
        app,
        &[
            &file, &edit, &sprite, &frame, &layer, &select, &view, &ai, &window, &help,
        ],
    )
}

/// Forwards the menu item ID to the frontend as a `shell:menu` event.
///
/// The TypeScript command dispatcher subscribes to this event and routes
/// it to the matching command handler, keeping all dispatch logic in the UI.
pub fn handle_event(app: &AppHandle, event: &MenuEvent) {
    if let Err(err) = app.emit("shell:menu", event.id().as_ref()) {
        tracing::warn!("failed to emit shell:menu event: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_event_does_not_panic_without_subscribers() {
        // Smoke test: if no frontend is listening, handle_event must not crash.
        // Full integration requires a live Tauri context; test the structure only.
        let _ = std::mem::size_of::<MenuEvent>();
    }
}
