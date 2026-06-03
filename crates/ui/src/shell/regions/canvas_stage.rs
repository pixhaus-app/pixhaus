//! Canvas-stage region: the `CentralPanel` (added last). It owns the navigable view —
//! the camera (a true scale + pan), cursor-anchored wheel zoom, drag-pan, auto-fit, the
//! grid and pixel grid, a hover crosshair, and a floating zoom control — over the
//! GPU-blitted sprite.
//!
//! View state (`zoom`/`pan`/`last_fit_size`/`pixel_perfect_zoom`) is the UI bucket the
//! shell owns; this region mutates it directly for interactive gestures (a one-frame
//! intent round-trip is visible during a continuous wheel or drag), the same blessed
//! direct-mutation carve-out it already uses for the upload-dedup fields. Discrete view
//! actions from outside the canvas (menus, the zoom control, the `+`/`-`/fit keys) route
//! through `Intent` instead, since their callers cannot reach the stage geometry.

use std::sync::Arc;

use pixhaus_services::i18n;

use crate::canvas::view;
use crate::state::Host;
use crate::state::intent::Intent;
use crate::state::ui_state::GridMode;
use crate::theme::Theme;
use crate::theme::tokens::SurfaceTier;
use crate::{CanvasCallback, CanvasFrame};

/// Continuous-mode zoom per wheel notch: the scale multiplies by this zooming in, and
/// divides by it zooming out. Magnitude-independent, so it is robust to the OS's
/// lines-per-notch setting.
const WHEEL_ZOOM_FACTOR: f32 = 1.2;

/// `Ctrl/Cmd+Shift+0` resets to actual pixels (1:1). Built explicitly so the shift is
/// part of the match (plain `Ctrl/Cmd+0` is fit-to-window).
const ACTUAL_PIXELS_MODS: egui::Modifiers = egui::Modifiers {
    alt: false,
    ctrl: false,
    shift: true,
    mac_cmd: false,
    command: true,
};

/// Render the canvas stage.
//
// The sprite dimensions are small (a few thousand pixels at most), so casting them to
// f32 for the layout math is exact; the `cast_precision_loss` allow documents that.
#[allow(clippy::cast_precision_loss)]
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let Host {
        state, theme, edit, intents, ..
    } = &mut *host;

    let frame = egui::Frame::new().fill(theme.surface(SurfaceTier::Stage));

    egui::CentralPanel::default().frame(frame).show_inside(ui, |ui| {
        let stage_rect = ui.available_rect_before_wrap();
        // The HUD paints on the full painter; everything that tracks the artboard paints
        // on a stage-clipped clone, so a zoomed-in board is scissored to the panel and
        // never bleeds over the docks. egui-wgpu reads this clip for the GPU scissor too.
        let painter = ui.painter().clone();
        let canvas = painter.with_clip_rect(stage_rect);

        let (sprite_w, sprite_h) = edit.document.active_sprite_size().unwrap_or(pixhaus_core::DEFAULT_CANVAS_SIZE);
        let sprite_px = egui::vec2(sprite_w as f32, sprite_h as f32);

        // 1. Auto-fit bootstrap. When the active sprite's dimensions differ from the last
        //    fit (first open, switching sprites, a resize, or a FitView request that
        //    cleared the record), re-fit the board to the stage so a small sprite is not
        //    a speck. A manual zoom leaves the dimensions unchanged, so it is never
        //    undone. This runs before input so the fitted view is what the user navigates.
        let fit_key = (sprite_w, sprite_h);
        if state.ui.last_fit_size != Some(fit_key) {
            let fit = view::fit_scale(stage_rect.size(), sprite_px, view::FIT_MARGIN);
            state.ui.zoom = if state.ui.pixel_perfect_zoom { view::snap_scale(fit) } else { fit };
            state.ui.pan = egui::Vec2::ZERO;
            state.ui.last_fit_size = Some(fit_key);
        }

        // 2. Interactive navigation, mutating view state directly for immediacy. Gated on
        //    no modal so the palette/about overlays own the pointer when open.
        let response = ui.interact(stage_rect, ui.id().with("canvas"), egui::Sense::click_and_drag());
        if state.ui.modal.is_none() {
            handle_navigation(ui, &response, state, sprite_px, stage_rect);
            handle_view_keys(ui, intents, state.ui.modal.is_some());
        }

        // 3. Resolve the on-screen artboard from the (possibly just-updated) view, with a
        //    pan clamp from the authoritative geometry so the board stays reachable.
        let scale = view::clamp_scale(state.ui.zoom);
        state.ui.pan = view::clamp_pan(stage_rect, sprite_px, scale, state.ui.pan);
        let artboard = view::artboard_rect(stage_rect, sprite_px, scale, state.ui.pan);

        // 4. Manual drop shadow: two stacked offset translucent rects behind the board.
        let shadow_far = artboard.expand(3.0).translate(egui::vec2(6.0, 10.0));
        canvas.rect_filled(shadow_far, egui::CornerRadius::ZERO, egui::Color32::from_black_alpha(70));
        let shadow_near = artboard.translate(egui::vec2(3.0, 5.0));
        canvas.rect_filled(shadow_near, egui::CornerRadius::ZERO, egui::Color32::from_black_alpha(130));

        // 5. Resolve which frame to display (the playhead picks it during playback;
        //    otherwise the range start / active frame). A transient view choice — the
        //    document is untouched, so the dirty gate below fires on a revision OR
        //    displayed-frame change.
        let displayed_frame: Option<pixhaus_core::FrameId> = edit.document.active_sprite().and_then(|id| edit.document.sprite(id)).and_then(|sprite| {
            let range = crate::playback::resolve_range(sprite, state.ui.playback.clip);
            let offset = crate::playback::playhead_index(state.ui.playback.playhead_seconds, range.fps, range.frame_count, range.loop_mode);
            let index = range.start.saturating_add(offset) as usize;
            sprite.frames().get(index).or_else(|| sprite.active_frame()).map(|frame| frame.id)
        });

        // 6. Recomposite + upload the current frame only when it changed; `None` reuses
        //    the retained GPU texture. The wgpu callback draws the transparency
        //    checkerboard, the grid, and the sprite in one fragment pass — O(visible
        //    pixels), so it stays fast at any zoom (the per-cell CPU loops are gone).
        let has_tile = edit.document.active_sprite().is_some();
        let revision = edit.document.revision();
        let needs_upload = revision != edit.last_uploaded_revision || displayed_frame != edit.last_uploaded_frame;
        let canvas_frame = if needs_upload {
            edit.last_uploaded_revision = revision;
            edit.last_uploaded_frame = displayed_frame;
            let composited = match (edit.document.active_sprite(), displayed_frame) {
                (Some(sprite_id), Some(frame_id)) => pixhaus_core::composite_frame(&edit.document, sprite_id, frame_id).ok(),
                _ => pixhaus_core::composite_active(&edit.document),
            };
            composited.map(|buf| {
                let width = buf.width();
                let height = buf.height();
                CanvasFrame {
                    rgba: Arc::new(buf.into_bytes()),
                    width,
                    height,
                }
            })
        } else {
            None
        };
        let chrome = crate::chrome_params(state.ui.grid, scale, theme, has_tile);
        canvas.add(egui_wgpu::Callback::new_paint_callback(
            artboard,
            CanvasCallback {
                frame: canvas_frame,
                artboard,
                chrome,
            },
        ));

        // 7. Onion-skin ghosts of the neighbor frames, as a translucent overlay over the
        //    current frame (the GPU board is opaque, so the ghosts sit on top). Real data
        //    (frames live in core); off by default. The cache frees when disabled.
        if state.ui.onion_skin
            && let (Some(sprite_id), Some(center)) = (edit.document.active_sprite(), displayed_frame)
        {
            paint_onion(&canvas, artboard, edit.onion.ghosts(ui.ctx(), &edit.document, sprite_id, center), theme);
        } else {
            edit.onion.clear();
        }

        // 8. View overlays over the frame, under the HUD: the selection marquee and the
        //    active tool's preview. Scaffolds — no data until core's selection model and
        //    the tools land, so they draw nothing today but hold their place in the order.
        crate::canvas::overlay::paint_selection(&canvas, artboard, scale, None, theme);
        crate::canvas::overlay::paint_tool_preview(&canvas, artboard, scale, None, theme);

        // 9. Hover crosshair through the pixel under the pointer (Tier-2 tool
        //    foundation), suppressed while panning so a drag stays clean.
        let hover_pixel = hovered_pixel(response.hover_pos(), artboard, scale, (sprite_w, sprite_h));
        let panning = is_panning(ui, &response);
        if let Some(pixel) = hover_pixel
            && !panning
        {
            paint_crosshair(&canvas, artboard, scale, pixel, theme);
        }

        // 10. The board frame: a 1.5px border so its edge reads against the stage.
        canvas.rect_stroke(
            artboard,
            egui::CornerRadius::ZERO,
            egui::Stroke::new(1.5, theme.roles.border),
            egui::StrokeKind::Outside,
        );

        // 11. Floating HUD (lower-left) and zoom control (lower-right).
        let readout = HudReadout {
            size: (sprite_w, sprite_h),
            zoom: state.ui.zoom,
            pixel_perfect: state.ui.pixel_perfect_zoom,
            grid: state.ui.grid,
            hover: hover_pixel,
        };
        paint_hud(&painter, stage_rect, &readout, theme);
        zoom_control(ui, stage_rect, state.ui.zoom, state.ui.pixel_perfect_zoom, theme, intents);
    });
}

/// Whether a pan gesture is active this frame: middle-drag, or Space + left-drag.
fn is_panning(ui: &egui::Ui, response: &egui::Response) -> bool {
    let space_down = ui.ctx().input(|i| i.key_down(egui::Key::Space));
    response.dragged_by(egui::PointerButton::Middle) || (space_down && response.dragged_by(egui::PointerButton::Primary))
}

/// Apply pan-drag and cursor-anchored wheel/pinch zoom to the view, in points.
fn handle_navigation(ui: &egui::Ui, response: &egui::Response, state: &mut crate::state::ShellState, sprite_px: egui::Vec2, stage_rect: egui::Rect) {
    let panning = is_panning(ui, response);
    if panning {
        state.ui.pan += response.drag_delta();
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if ui.ctx().input(|i| i.key_down(egui::Key::Space)) && response.contains_pointer() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }

    // Cursor-anchored zoom. The wheel is read from the raw per-frame `MouseWheel` events
    // rather than `smooth_scroll_delta` (which spreads one notch over several frames and
    // would step the zoom repeatedly), so one notch is one discrete event: pixel-perfect
    // mode steps to the next whole zoom level (a small multiplicative factor would round
    // straight back to the current integer and the wheel would never leave it — the bug
    // this fixes), continuous mode scales by a fixed per-notch factor. Command/ctrl+wheel
    // and trackpad pinch arrive through `zoom_delta` instead.
    let (wheel_y, pinch) = ui.ctx().input(|i| {
        let mut wheel_y = 0.0;
        for event in &i.events {
            if let egui::Event::MouseWheel { delta, modifiers, .. } = event
                && !modifiers.command
                && !modifiers.ctrl
            {
                wheel_y += delta.y;
            }
        }
        (wheel_y, i.zoom_delta())
    });
    if !panning
        && response.contains_pointer()
        && let Some(cursor) = response.hover_pos()
    {
        let scale = view::clamp_scale(state.ui.zoom);
        let mut new_scale = scale;
        if wheel_y.abs() > f32::EPSILON {
            let zoom_in = wheel_y > 0.0;
            new_scale = if state.ui.pixel_perfect_zoom {
                view::zoom_step(scale, zoom_in, true)
            } else {
                view::clamp_scale(scale * if zoom_in { WHEEL_ZOOM_FACTOR } else { 1.0 / WHEEL_ZOOM_FACTOR })
            };
        }
        if (pinch - 1.0).abs() > f32::EPSILON {
            new_scale = view::clamp_scale(new_scale * pinch);
            if state.ui.pixel_perfect_zoom {
                new_scale = view::snap_scale(new_scale);
            }
        }
        if (new_scale - scale).abs() > f32::EPSILON {
            state.ui.pan = view::zoom_anchored(stage_rect.center(), sprite_px, scale, state.ui.pan, cursor, new_scale);
            state.ui.zoom = new_scale;
        }
    }
}

/// Read the canvas keyboard shortcuts and queue their intents. Gated by the caller on no
/// modal; bare `+`/`-` are also gated on no focused text field so typing never zooms.
fn handle_view_keys(ui: &egui::Ui, intents: &mut crate::state::intent::IntentSink, modal_open: bool) {
    if modal_open {
        return;
    }
    let text_focused = ui.ctx().text_edit_focused();
    let (mut step_in, mut step_out, mut fit, mut actual) = (false, false, false, false);
    ui.ctx().input_mut(|i| {
        if !text_focused {
            step_in = i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals);
            step_out = i.key_pressed(egui::Key::Minus);
        }
        // Command-modified, so they fire even with a text field focused.
        actual = i.consume_key(ACTUAL_PIXELS_MODS, egui::Key::Num0);
        fit = i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num0);
    });
    if actual {
        intents.push(Intent::SetZoom(1.0));
        intents.push(Intent::SetPan(egui::Vec2::ZERO));
    } else if fit {
        intents.push(Intent::FitView);
    }
    if step_in {
        intents.push(Intent::ZoomStep { zoom_in: true });
    }
    if step_out {
        intents.push(Intent::ZoomStep { zoom_in: false });
    }
}

/// The sprite pixel under `hover`, or `None` when the pointer is off the board.
// Floors a bounded, non-negative in-board coordinate to a pixel index; the f32 -> u32
// casts cannot truncate or lose a sign for any real sprite size.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn hovered_pixel(hover: Option<egui::Pos2>, artboard: egui::Rect, scale: f32, size: (u32, u32)) -> Option<(u32, u32)> {
    let hover = hover?;
    if !artboard.contains(hover) {
        return None;
    }
    let sprite = view::screen_to_sprite(hover, artboard.min, scale);
    let (w, h) = size;
    if sprite.x < 0.0 || sprite.y < 0.0 || sprite.x >= w as f32 || sprite.y >= h as f32 {
        return None;
    }
    Some((sprite.x.floor() as u32, sprite.y.floor() as u32))
}

/// Draw a faint full-board crosshair through the center of the hovered pixel.
// The pixel index is small and non-negative; the u32 -> f32 cast is exact here.
#[allow(clippy::cast_precision_loss)]
fn paint_crosshair(painter: &egui::Painter, board: egui::Rect, scale: f32, pixel: (u32, u32), theme: &Theme) {
    let (px, py) = pixel;
    let center = view::sprite_to_screen(egui::vec2(px as f32 + 0.5, py as f32 + 0.5), board.min, scale);
    let stroke = egui::Stroke::new(1.0, theme.roles.text_secondary.gamma_multiply(0.5));
    painter.line_segment([egui::pos2(board.min.x, center.y), egui::pos2(board.max.x, center.y)], stroke);
    painter.line_segment([egui::pos2(center.x, board.min.y), egui::pos2(center.x, board.max.y)], stroke);
}

/// Draw the onion-skin ghosts over the artboard, tinting past frames with the warning
/// role and future frames with the accent, both dimmed and translucent so they read as
/// references behind the current frame.
fn paint_onion(painter: &egui::Painter, artboard: egui::Rect, ghosts: &[crate::canvas::onion::Ghost], theme: &Theme) {
    use crate::canvas::onion::Neighbor;
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    for ghost in ghosts {
        let tint = match ghost.neighbor {
            Neighbor::Past => theme.roles.warning.gamma_multiply(0.5),
            Neighbor::Future => theme.accent.base.gamma_multiply(0.5),
        };
        painter.image(ghost.texture.id(), artboard, uv, tint);
    }
}

/// The live values the canvas HUD prints, bundled so the painter stays under the
/// argument-count lint.
struct HudReadout {
    /// Sprite dimensions in pixels.
    size: (u32, u32),
    /// The true scale (screen points per sprite pixel); shown as a percentage.
    zoom: f32,
    /// Whether pixel-perfect zoom is active.
    pixel_perfect: bool,
    /// The active grid mode.
    grid: GridMode,
    /// The sprite pixel under the pointer, if any.
    hover: Option<(u32, u32)>,
}

/// The floating HUD at the stage's lower-left: size, honest zoom %, zoom mode, grid, and
/// the live pixel coordinate under the pointer.
// The mock comment from the status bar applies: the numeric size/zoom and the short
// technical mode tokens are locale-neutral and deliberately not i18n keys; the real word
// "Grid" routes through the shared key so the HUD and status bar agree.
fn paint_hud(painter: &egui::Painter, stage: egui::Rect, readout: &HudReadout, theme: &Theme) {
    let (sprite_w, sprite_h) = readout.size;
    let grid_text = i18n::tr_args("app.ui.status.grid", &[("mode", &format!("{:?}", readout.grid))]);
    let mode = if readout.pixel_perfect { "Pixel Perfect" } else { "Smooth" };
    let coord = readout.hover.map_or_else(String::new, |(x, y)| format!("   [{x}, {y}]"));
    let text = format!(
        "{sprite_w} x {sprite_h}   {:.0}%   {mode}   {grid_text}{coord}   Palette: Bit",
        readout.zoom * 100.0
    );
    let font = egui::FontId::monospace(theme.type_scale.mono);
    let galley = painter.layout_no_wrap(text, font, theme.roles.text_secondary);
    let pad = egui::vec2(6.0, 4.0);
    let chip_min = stage.left_bottom() + egui::vec2(8.0, -(galley.size().y + pad.y * 2.0 + 8.0));
    let chip = egui::Rect::from_min_size(chip_min, galley.size() + pad * 2.0);
    painter.rect_filled(chip, egui::CornerRadius::same(2), theme.surface(SurfaceTier::Inset));
    painter.galley(chip.min + pad, galley, theme.roles.text_secondary);
}

/// The floating zoom control at the stage's lower-right: zoom out, the live percentage,
/// zoom in, a mode toggle, and fit-to-window. Buttons queue intents (the cursor-anchored
/// path is the wheel); the control floats in its own foreground area.
#[allow(clippy::cast_precision_loss)]
fn zoom_control(ui: &egui::Ui, stage: egui::Rect, zoom: f32, pixel_perfect: bool, theme: &Theme, intents: &mut crate::state::intent::IntentSink) {
    let pos = stage.right_bottom() + egui::vec2(-176.0, -38.0);
    let frame = egui::Frame::new()
        .fill(theme.surface(SurfaceTier::Inset))
        .inner_margin(theme.spacing.xs)
        .corner_radius(egui::CornerRadius::same(4));
    egui::Area::new(ui.id().with("zoom_control"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = theme.spacing.xs;
                    if icon_button(ui, theme, crate::icons::REMOVE).clicked() {
                        intents.push(Intent::ZoomStep { zoom_in: false });
                    }
                    ui.colored_label(theme.roles.text_secondary, egui::RichText::new(format!("{:>4.0}%", zoom * 100.0)).monospace());
                    if icon_button(ui, theme, crate::icons::ADD).clicked() {
                        intents.push(Intent::ZoomStep { zoom_in: true });
                    }
                    ui.separator();
                    // The mode toggle reads in the active accent when pixel-perfect is on.
                    let mode_color = if pixel_perfect { theme.accent.base } else { theme.roles.text_secondary };
                    if ui
                        .add(egui::Button::new(egui::RichText::new(crate::icons::GRID).color(mode_color)).frame(false))
                        .on_hover_text(i18n::tr("app.ui.canvas.pixel_perfect_zoom"))
                        .clicked()
                    {
                        intents.push(Intent::ToggleZoomMode);
                    }
                    if icon_button(ui, theme, crate::icons::FIT)
                        .on_hover_text(i18n::tr("app.ui.canvas.fit_to_window"))
                        .clicked()
                    {
                        intents.push(Intent::FitView);
                    }
                });
            });
        });
}

/// A small frameless icon button in the secondary text color.
fn icon_button(ui: &mut egui::Ui, theme: &Theme, icon: char) -> egui::Response {
    ui.add(egui::Button::new(egui::RichText::new(icon).color(theme.roles.text_secondary)).frame(false))
}
