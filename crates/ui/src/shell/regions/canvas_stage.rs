//! Canvas-stage region: the `CentralPanel` (added last). A stage backdrop, the
//! mock artboard rect from zoom/pan, the transparency checkerboard, a manually
//! painted drop shadow, the unchanged `CanvasCallback` embed, grid strokes, and a
//! floating HUD painted with the central panel's `Painter`.

use crate::CanvasCallback;
use crate::state::Host;
use crate::state::ui_state::GridMode;
use crate::theme::Theme;
use crate::theme::tokens::SurfaceTier;

/// Render the canvas stage.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let Host { state, theme, .. } = &mut *host;

    let frame = egui::Frame::new().fill(theme.surface(SurfaceTier::Stage));

    egui::CentralPanel::default().frame(frame).show_inside(ui, |ui| {
        let stage_rect = ui.available_rect_before_wrap();
        let painter = ui.painter().clone();

        // 1. Stage backdrop already filled by the frame. Compute the artboard.
        let sprite_px = egui::vec2(64.0, 64.0);
        let scaled = sprite_px * state.ui.zoom;
        let artboard = egui::Rect::from_center_size(stage_rect.center() + state.ui.pan, scaled);

        // 2. Manual drop shadow: an offset translucent dark rect behind the board.
        //    Shadow is not a paint primitive and cannot be painter.add-ed here.
        let shadow_rect = artboard.translate(egui::vec2(4.0, 6.0));
        painter.rect_filled(shadow_rect, egui::CornerRadius::ZERO, egui::Color32::from_black_alpha(110));

        // 3. Transparency checkerboard behind the artboard.
        paint_checkerboard(&painter, artboard, theme);

        // 4. Embed the renderer UNCHANGED - exactly the app/src/main.rs seam. The
        //    callback rect tracks the artboard, so the wgpu pass draws there.
        //    INPUT GAP: nothing senses `artboard` yet, so the stage is display-only.
        //    A future phase must allocate a sensed rect over the artboard
        //    (`ui.interact`/`allocate_rect` with click+drag) to route pan/zoom/paint
        //    off its `Response` - see the canvas-input routing in pixhaus-egui.
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(artboard, CanvasCallback));

        // 5. Grid lines over the artboard (minor 8px / major 16px per GridMode).
        paint_grid(&painter, artboard, state.ui.zoom, state.ui.grid, theme);

        // 6. Floating HUD via the central Painter, at the stage's lower-left.
        paint_hud(&painter, stage_rect, state.ui.zoom, state.ui.grid, theme);
    });
}

// Checker counts come from a bounded screen rect divided by an 8px cell; the
// f32 -> i32 cast cannot overflow for any plausible viewport and the value is
// non-negative by construction. The i32 -> f32 cast back is exact for those small
// row/column counts.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn paint_checkerboard(painter: &egui::Painter, board: egui::Rect, theme: &Theme) {
    let cell = 8.0;
    let light = theme.surface(SurfaceTier::Inset);
    let dark = theme.surface(SurfaceTier::Stage);
    let cols = (board.width() / cell).ceil() as i32;
    let rows = (board.height() / cell).ceil() as i32;
    for r in 0..rows {
        for c in 0..cols {
            let on = (r + c) % 2 == 0;
            let min = board.min + egui::vec2(c as f32 * cell, r as f32 * cell);
            let rect = egui::Rect::from_min_size(min, egui::vec2(cell, cell)).intersect(board);
            painter.rect_filled(rect, egui::CornerRadius::ZERO, if on { light } else { dark });
        }
    }
}

fn paint_grid(painter: &egui::Painter, board: egui::Rect, zoom: f32, grid: GridMode, theme: &Theme) {
    if matches!(grid, GridMode::Off) {
        return;
    }
    // Mock chrome: one device step per sprite pixel at the current zoom, clamped to
    // 8px so an extreme zoom-out never produces a sub-pixel step (a tight loop that
    // would stall). The real grid spacing follows the document once core lands.
    let step = zoom.max(8.0);
    let stroke = egui::Stroke::new(1.0, theme.roles.border);
    let mut x = board.min.x;
    while x <= board.max.x {
        painter.line_segment([egui::pos2(x, board.min.y), egui::pos2(x, board.max.y)], stroke);
        x += step;
    }
    let mut y = board.min.y;
    while y <= board.max.y {
        painter.line_segment([egui::pos2(board.min.x, y), egui::pos2(board.max.x, y)], stroke);
        y += step;
    }
}

fn paint_hud(painter: &egui::Painter, stage: egui::Rect, zoom: f32, grid: GridMode, theme: &Theme) {
    // Format the live grid the same way the status bar does so the two agree.
    let text = format!("64 x 64   {:.0}%   Grid {grid:?}   Palette: Bit", zoom * 100.0);
    let font = egui::FontId::monospace(theme.type_scale.mono);
    let galley = painter.layout_no_wrap(text, font, theme.roles.text_secondary);
    let pad = egui::vec2(6.0, 4.0);
    let chip_min = stage.left_bottom() + egui::vec2(8.0, -(galley.size().y + pad.y * 2.0 + 8.0));
    let chip = egui::Rect::from_min_size(chip_min, galley.size() + pad * 2.0);
    painter.rect_filled(chip, egui::CornerRadius::same(2), theme.surface(SurfaceTier::Inset));
    painter.galley(chip.min + pad, galley, theme.roles.text_secondary);
}
