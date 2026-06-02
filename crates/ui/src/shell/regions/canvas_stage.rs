//! Canvas-stage region: the `CentralPanel` (added last). A stage backdrop, the
//! mock artboard rect from zoom/pan, the transparency checkerboard, a manually
//! painted drop shadow, the unchanged `CanvasCallback` embed, grid strokes, and a
//! floating HUD painted with the central panel's `Painter`.

use std::sync::Arc;

use crate::state::Host;
use crate::state::ui_state::GridMode;
use crate::theme::Theme;
use crate::theme::tokens::SurfaceTier;
use crate::{CanvasCallback, CanvasFrame};

/// Render the canvas stage.
//
// The sprite dimensions are small (a few thousand pixels at most), so casting them to
// f32 for the layout math is exact; the `cast_precision_loss` allow documents that.
#[allow(clippy::cast_precision_loss)]
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let Host { state, theme, edit, .. } = &mut *host;

    let frame = egui::Frame::new().fill(theme.surface(SurfaceTier::Stage));

    egui::CentralPanel::default().frame(frame).show_inside(ui, |ui| {
        let stage_rect = ui.available_rect_before_wrap();
        let painter = ui.painter().clone();

        // 1. Stage backdrop already filled by the frame. Compute the artboard from the
        //    active sprite's real size (the default board when nothing is open). A
        //    small sprite at 100% zoom is a speck in a large stage, so we derive a
        //    display scale that fills ~68% of the stage height before applying the user
        //    zoom. `fit` stays integer (one sprite pixel maps to N device pixels) and
        //    floored to >= 1 so the board never collapses. `state.ui.zoom` rides on top,
        //    keeping the HUD/status "100%" honest.
        let (sprite_w, sprite_h) = edit.document.active_sprite_size().unwrap_or(pixhaus_core::DEFAULT_CANVAS_SIZE);
        let sprite_px = egui::vec2(sprite_w as f32, sprite_h as f32);
        let fit = (stage_rect.height() * 0.68 / sprite_px.y).max(1.0).floor();
        let display_scale = fit * state.ui.zoom;
        let scaled = sprite_px * display_scale;
        let artboard = egui::Rect::from_center_size(stage_rect.center() + state.ui.pan, scaled);

        // 2. Manual drop shadow: two stacked offset translucent rects behind the
        //    board for a soft falloff. Shadow is not a paint primitive and cannot be
        //    painter.add-ed here, so the stack stands in for a blurred edge.
        let shadow_far = artboard.expand(3.0).translate(egui::vec2(6.0, 10.0));
        painter.rect_filled(shadow_far, egui::CornerRadius::ZERO, egui::Color32::from_black_alpha(70));
        let shadow_near = artboard.translate(egui::vec2(3.0, 5.0));
        painter.rect_filled(shadow_near, egui::CornerRadius::ZERO, egui::Color32::from_black_alpha(130));

        // 3. Transparency checkerboard behind the artboard.
        paint_checkerboard(&painter, artboard, theme);

        // 4. Recomposite the active sprite only when the document changed since the
        //    last upload (the revision dirty gate), then embed the renderer. The
        //    callback uploads the frame in its `prepare` phase and draws it in the
        //    artboard rect. A `None` frame reuses the retained GPU texture untouched.
        //    INPUT GAP: nothing senses `artboard` yet, so the stage is display-only.
        //    A future phase routes pan/zoom/paint off a sensed rect (see pixhaus-egui).
        let revision = edit.document.revision();
        let canvas_frame = if revision == edit.last_uploaded_revision {
            None
        } else {
            edit.last_uploaded_revision = revision;
            pixhaus_core::composite_active(&edit.document).map(|buf| {
                let width = buf.width();
                let height = buf.height();
                CanvasFrame {
                    rgba: Arc::new(buf.into_bytes()),
                    width,
                    height,
                }
            })
        };
        ui.painter()
            .add(egui_wgpu::Callback::new_paint_callback(artboard, CanvasCallback { frame: canvas_frame }));

        // 5. Grid lines over the artboard, one device step per sprite pixel at the
        //    enlarged display scale (so the grid tracks the board, not raw zoom).
        paint_grid(&painter, artboard, display_scale, state.ui.grid, theme);

        // 6. The board frame: a 1.5px border so its edge reads clearly against the
        //    stage (the drop shadow is already painted behind it in step 2). The
        //    checker now carries the interior, so the edge marks the document bounds.
        painter.rect_stroke(
            artboard,
            egui::CornerRadius::ZERO,
            egui::Stroke::new(1.5, theme.roles.border),
            egui::StrokeKind::Outside,
        );

        // 7. Floating HUD via the central Painter, at the stage's lower-left.
        paint_hud(&painter, stage_rect, (sprite_w, sprite_h), state.ui.zoom, state.ui.grid, theme);
    });
}

// Checker counts come from a bounded screen rect divided by an 8px cell; the
// f32 -> i32 cast cannot overflow for any plausible viewport and the value is
// non-negative by construction. The i32 -> f32 cast back is exact for those small
// row/column counts.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn paint_checkerboard(painter: &egui::Painter, board: egui::Rect, theme: &Theme) {
    let cell = 8.0;
    // The dedicated checker pair, lifted off the stage backdrop so the artboard
    // reads as a lit surface rather than near-black-on-near-black.
    let light = theme.mock.checker[0];
    let dark = theme.mock.checker[1];
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

fn paint_grid(painter: &egui::Painter, board: egui::Rect, display_scale: f32, grid: GridMode, theme: &Theme) {
    if matches!(grid, GridMode::Off) {
        return;
    }
    // Mock chrome: one device step per sprite pixel at the current display scale,
    // clamped to 8px so an extreme zoom-out never produces a sub-pixel step (a tight
    // loop that would stall). The real grid spacing follows the document once core lands.
    let step = display_scale.max(8.0);
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

fn paint_hud(painter: &egui::Painter, stage: egui::Rect, sprite: (u32, u32), zoom: f32, grid: GridMode, theme: &Theme) {
    // Format the live grid the same way the status bar does so the two agree.
    let (sprite_w, sprite_h) = sprite;
    let text = format!("{sprite_w} x {sprite_h}   {:.0}%   Grid {grid:?}   Palette: Bit", zoom * 100.0);
    let font = egui::FontId::monospace(theme.type_scale.mono);
    let galley = painter.layout_no_wrap(text, font, theme.roles.text_secondary);
    let pad = egui::vec2(6.0, 4.0);
    let chip_min = stage.left_bottom() + egui::vec2(8.0, -(galley.size().y + pad.y * 2.0 + 8.0));
    let chip = egui::Rect::from_min_size(chip_min, galley.size() + pad * 2.0);
    painter.rect_filled(chip, egui::CornerRadius::same(2), theme.surface(SurfaceTier::Inset));
    painter.galley(chip.min + pad, galley, theme.roles.text_secondary);
}
