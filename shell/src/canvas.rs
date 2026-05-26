//! The egui-wgpu canvas: the paint callback that drives [`ViewportRenderer`],
//! the input router that turns pointer gestures into `core` editing calls, and
//! the egui overlays (brush cursor, shape preview, selection marching ants).
//!
//! Drawing flows straight into `core`: a pointer drag stamps into the active
//! cel's [`PixelBuffer`], the shell recomposites only the dirty rectangle, and
//! [`ViewportRenderer::upload_dirty_rect`] uploads just that rect. No IPC, and
//! per-move work is bounded by the dirty region, not the canvas size.

use eframe::egui;
use glam::Vec2;
use pixhaus_core::canvas::{BrushShape, PixelBuffer, draw_filled_ellipse, draw_filled_rect, draw_line, draw_rect, draw_stroke, flood_fill, paint_brush};
use pixhaus_core::project::Rgba;
use pixhaus_core::project::{IVec2, Rect, Size};
use pixhaus_core::selection::{Connectivity, magic_wand, select_ellipse, select_polygon, select_rect};
use pixhaus_render::{ViewportRenderer, viewport::clamp_zoom, viewport::snap_zoom};

use crate::app::{ShellApp, ZoomAction};
use crate::commands::{PixelRegionEdit, extract_region};
use crate::editor::{MoveDrag, ShapeDrag, StrokeSession, Tool};

/// Per-frame paint command for the SPRITE program. Cheap to build; holds only
/// the camera state, never GPU handles.
struct SpriteCallback {
    resolution: [f32; 2],
    scroll: [f32; 2],
    zoom: f32,
}

impl egui_wgpu::CallbackTrait for SpriteCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = resources.get_mut::<ViewportRenderer>() {
            renderer.write_uniforms(queue, self.resolution, self.scroll, self.zoom);
        }
        Vec::new()
    }

    fn paint(&self, _info: egui::epaint::PaintCallbackInfo, render_pass: &mut wgpu::RenderPass<'static>, resources: &egui_wgpu::CallbackResources) {
        if let Some(renderer) = resources.get::<ViewportRenderer>() {
            renderer.paint(render_pass);
        }
    }
}

impl ShellApp {
    /// Uploads a composited frame to the GPU via the stored render state.
    /// `refit` marks the viewport to re-fit on the next canvas paint.
    #[allow(clippy::cast_precision_loss)] // canvas dims <= 8192 are exact in f32
    pub(crate) fn upload_frame(&mut self, frame: &PixelBuffer, refit: bool) {
        let Some(rs) = self.render_state.as_ref() else {
            return;
        };
        let mut guard = rs.renderer.write();
        if let Some(renderer) = guard.callback_resources.get_mut::<ViewportRenderer>() {
            renderer.set_frame(&rs.device, &rs.queue, frame);
            self.frame_size = Some([frame.width() as f32, frame.height() as f32]);
            if refit {
                self.needs_fit = true;
            }
        }
    }

    /// Recomposites the rectangle `(x, y, w, h)` of the active frame into the
    /// cached display frame and uploads just that rect to the GPU. The drawing
    /// hot path.
    fn upload_region(&mut self, x: u32, y: u32, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        let Some(df) = self.display_frame.as_mut() else {
            return;
        };
        self.doc.composite_region_into(df, x, y, w, h);
        if let Some(rs) = self.render_state.as_ref() {
            let mut guard = rs.renderer.write();
            if let Some(renderer) = guard.callback_resources.get_mut::<ViewportRenderer>() {
                renderer.upload_dirty_rect(&rs.queue, df, x, y, w, h);
            }
        }
    }

    /// Draws the wgpu canvas, routes input to the active tools, and paints the
    /// egui overlays on top.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(crate) fn canvas_ui(&mut self, ui: &mut egui::Ui) {
        if self.render_state.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("wgpu unavailable — canvas disabled");
            });
            return;
        }

        let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = response.rect;
        let ppp = ui.ctx().pixels_per_point();
        let vp_px = Vec2::new(rect.width() * ppp, rect.height() * ppp);
        if vp_px.x < 1.0 || vp_px.y < 1.0 {
            return;
        }

        if self.needs_fit {
            if let Some(size) = self.frame_size {
                self.viewport.fit(Vec2::from(size), vp_px);
            }
            self.needs_fit = false;
        }

        // Drain a keyboard zoom request now that the viewport size is known, so
        // the zoom stays centred on the viewport.
        if let Some(action) = self.pending_zoom.take() {
            let centre = vp_px * 0.5;
            let new_zoom = match action {
                ZoomAction::In => snap_zoom(self.viewport.zoom, 1),
                ZoomAction::Out => snap_zoom(self.viewport.zoom, -1),
                ZoomAction::Reset => 1.0,
            };
            self.viewport.zoom_at(centre, vp_px, new_zoom);
            ui.ctx().request_repaint();
        }

        // Pointer position in canvas pixels, if the pointer is interacting.
        let to_canvas = |pos: egui::Pos2| -> [i32; 2] {
            let local = Vec2::new((pos.x - rect.min.x) * ppp, (pos.y - rect.min.y) * ppp);
            let c = self.viewport.screen_to_canvas(local, vp_px);
            [c.x.floor() as i32, c.y.floor() as i32]
        };
        let interact_canvas = response.interact_pointer_pos().map(to_canvas);
        let hover_canvas = response.hover_pos().map(to_canvas);

        let space_down = ui.input(|i| i.key_down(egui::Key::Space));
        let panning = response.dragged_by(egui::PointerButton::Middle) || (space_down && response.dragged_by(egui::PointerButton::Primary));

        if panning {
            let d = response.drag_delta();
            self.viewport.pan_by_screen(Vec2::new(d.x * ppp, d.y * ppp));
        } else if !self.preview_active() {
            // A preview (reference sheet, wizard clip, bg-removal) is view-only:
            // pan and zoom, no editing.
            self.route_tools(&response, interact_canvas);
        }

        // Zoom on wheel, keeping the canvas point under the cursor fixed.
        if response.hovered() {
            let scroll_y = ui.ctx().input(|i| i.smooth_scroll_delta.y);
            if scroll_y.abs() > f32::EPSILON {
                if let Some(pos) = response.hover_pos() {
                    let local = Vec2::new((pos.x - rect.min.x) * ppp, (pos.y - rect.min.y) * ppp);
                    let factor = (scroll_y * 0.005).exp();
                    let new_zoom = clamp_zoom(self.viewport.zoom * factor);
                    self.viewport.zoom_at(local, vp_px, new_zoom);
                    ui.ctx().request_repaint();
                }
            }
        }

        painter.add(egui_wgpu::Callback::new_paint_callback(
            rect,
            SpriteCallback {
                resolution: vp_px.into(),
                scroll: self.viewport.scroll.into(),
                zoom: self.viewport.zoom,
            },
        ));

        // Overlays (brush cursor, selection ants) belong to the sprite, not a
        // view-only preview.
        if !self.preview_active() {
            self.paint_overlays(ui, &painter, rect, vp_px, ppp, hover_canvas);
        }
    }

    /// Routes a pointer gesture to the tool bound to the button driving it.
    fn route_tools(&mut self, response: &egui::Response, pointer: Option<[i32; 2]>) {
        let Some(p) = pointer else {
            // No active pointer: a release may still need committing.
            if response.drag_stopped_by(egui::PointerButton::Primary) || response.drag_stopped_by(egui::PointerButton::Secondary) {
                self.commit_gesture();
            }
            return;
        };

        for (btn, primary) in [(egui::PointerButton::Primary, true), (egui::PointerButton::Secondary, false)] {
            let tool = self.editor.tool_for(primary);
            let color = self.editor.color_for(primary);
            if response.drag_started_by(btn) {
                self.begin_gesture(tool, color, p);
            }
            if response.dragged_by(btn) {
                self.update_gesture(tool, p);
            }
            if response.drag_stopped_by(btn) {
                self.commit_gesture();
            }
            if response.clicked_by(btn) {
                self.click_gesture(tool, color, p);
            }
        }
    }

    /// Canvas dimensions of the active sprite, or `None`.
    fn canvas_size(&self) -> Option<Size> {
        self.doc.active_sprite().map(|s| s.canvas)
    }

    // --- gesture lifecycle ---------------------------------------------------

    fn begin_gesture(&mut self, tool: Tool, color: Rgba, p: [i32; 2]) {
        match tool {
            Tool::Pencil | Tool::Eraser => self.begin_stroke(tool, color, p),
            Tool::Line | Tool::Rectangle | Tool::Ellipse => self.begin_shape(p),
            Tool::SelectRect | Tool::SelectEllipse => {
                self.editor.sel_drag = Some((p, p));
            }
            Tool::Lasso => {
                self.editor.lasso.clear();
                self.editor.lasso.push(IVec2::new(p[0], p[1]));
            }
            Tool::Move => self.begin_move(p),
            Tool::Fill | Tool::Wand | Tool::Picker => {}
        }
    }

    fn update_gesture(&mut self, tool: Tool, p: [i32; 2]) {
        match tool {
            Tool::Pencil | Tool::Eraser => self.extend_stroke(p),
            Tool::Line | Tool::Rectangle | Tool::Ellipse => self.update_shape(tool, p),
            Tool::SelectRect | Tool::SelectEllipse => {
                if let Some((_, cur)) = self.editor.sel_drag.as_mut() {
                    *cur = p;
                }
            }
            Tool::Lasso => self.editor.lasso.push(IVec2::new(p[0], p[1])),
            Tool::Move => self.update_move(p),
            Tool::Fill | Tool::Wand | Tool::Picker => {}
        }
    }

    fn click_gesture(&mut self, tool: Tool, color: Rgba, p: [i32; 2]) {
        match tool {
            Tool::Pencil | Tool::Eraser => {
                // A tap with no drag: a single dab.
                self.begin_stroke(tool, color, p);
                self.commit_gesture();
            }
            Tool::Fill => self.do_fill(color, p),
            Tool::Picker => self.do_pick(p),
            Tool::Wand => self.do_wand(p),
            Tool::SelectRect | Tool::SelectEllipse | Tool::Lasso => {
                // A click with no drag clears the selection.
                self.editor.clear_selection();
            }
            _ => {}
        }
    }

    /// Commits whichever gesture is in progress.
    fn commit_gesture(&mut self) {
        if self.editor.stroke.is_some() {
            self.commit_stroke();
        } else if self.editor.shape_drag.is_some() {
            self.commit_shape();
        } else if self.editor.sel_drag.is_some() {
            self.commit_marquee();
        } else if !self.editor.lasso.is_empty() {
            self.commit_lasso();
        } else if self.editor.move_drag.is_some() {
            self.commit_move();
        }
    }

    // --- freehand stroke -----------------------------------------------------

    fn begin_stroke(&mut self, tool: Tool, color: Rgba, p: [i32; 2]) {
        let Some(buffer_id) = self.doc.ensure_drawable() else {
            return;
        };
        let Some(before) = self.doc.pixel_buffers.get(&buffer_id).cloned() else {
            return;
        };
        let erase = tool == Tool::Eraser;
        let paint = if erase { Rgba::transparent() } else { color };
        let mut session = StrokeSession {
            buffer_id,
            before,
            points: vec![[p[0] as f32, p[1] as f32]],
            last_point: Some([p[0] as f32, p[1] as f32]),
            color: paint,
            shape: self.editor.brush_shape,
            size: self.editor.brush_size,
            mirror_x: self.editor.mirror_x,
            mirror_y: self.editor.mirror_y,
            erase,
            pixel_perfect: self.editor.pixel_perfect,
            dirty: None,
        };
        if let Some(buf) = self.doc.pixel_buffers.get_mut(&buffer_id) {
            stamp_point(buf, &mut session, p[0], p[1]);
        }
        self.editor.stroke = Some(session);
        if let Some((x, y, w, h)) = self.take_session_dirty() {
            self.upload_region(x, y, w, h);
        }
    }

    fn extend_stroke(&mut self, p: [i32; 2]) {
        let Some(session) = self.editor.stroke.as_mut() else {
            return;
        };
        let last = session.last_point.unwrap_or([p[0] as f32, p[1] as f32]);
        let from = [last[0] as i32, last[1] as i32];
        session.points.push([p[0] as f32, p[1] as f32]);
        session.last_point = Some([p[0] as f32, p[1] as f32]);
        let buffer_id = session.buffer_id;
        // Borrow the buffer separately from the session to stamp the segment.
        if let Some(buf) = self.doc.pixel_buffers.get_mut(&buffer_id) {
            if let Some(session) = self.editor.stroke.as_mut() {
                stamp_segment_mirrored(buf, session, from, p);
            }
        }
        if let Some((x, y, w, h)) = self.take_session_dirty() {
            self.upload_region(x, y, w, h);
        }
    }

    fn commit_stroke(&mut self) {
        let Some(mut session) = self.editor.stroke.take() else {
            return;
        };
        // Pixel-perfect pencil: redraw cleanly from the snapshot once, on commit.
        if session.pixel_perfect && session.shape == BrushShape::Pixel && session.size == 1 && !session.mirror_x && !session.mirror_y && !session.erase {
            if let Some(buf) = self.doc.pixel_buffers.get_mut(&session.buffer_id) {
                *buf = session.before.clone();
                draw_stroke(buf, &session.points, session.color, BrushShape::Pixel, 1, true);
            }
        }
        let Some((x, y, x1, y1)) = session.dirty else {
            return;
        };
        let (w, h) = (x1 - x + 1, y1 - y + 1);
        self.finish_pixel_edit(&session.before, session.buffer_id, x, y, w, h, "Brush stroke");
        if !session.erase {
            self.maybe_add_palette(session.color);
        }
        session.points.clear();
        self.refresh_canvas(false);
    }

    // --- fill ----------------------------------------------------------------

    fn do_fill(&mut self, color: Rgba, p: [i32; 2]) {
        let Some(buffer_id) = self.doc.ensure_drawable() else {
            return;
        };
        let Some(before) = self.doc.pixel_buffers.get(&buffer_id).cloned() else {
            return;
        };
        let tolerance = self.editor.tolerance;
        if let Some(buf) = self.doc.pixel_buffers.get_mut(&buffer_id) {
            flood_fill(buf, p[0], p[1], color, tolerance);
        }
        let (cw, ch) = match self.canvas_size() {
            Some(s) => (s.width, s.height),
            None => return,
        };
        self.finish_pixel_edit(&before, buffer_id, 0, 0, cw, ch, "Fill");
        self.maybe_add_palette(color);
        self.refresh_canvas(false);
    }

    // --- colour picker -------------------------------------------------------

    fn do_pick(&mut self, p: [i32; 2]) {
        if p[0] < 0 || p[1] < 0 {
            return;
        }
        if let Some(frame) = self.display_frame.as_ref() {
            if let Some(c) = frame.pixel(p[0] as u32, p[1] as u32) {
                if c.a > 0 {
                    self.editor.fg = c;
                }
            }
        }
    }

    // --- shapes (line / rect / ellipse) --------------------------------------

    fn begin_shape(&mut self, p: [i32; 2]) {
        let Some(buffer_id) = self.doc.ensure_drawable() else {
            return;
        };
        let Some(before) = self.doc.pixel_buffers.get(&buffer_id).cloned() else {
            return;
        };
        self.editor.shape_drag = Some(ShapeDrag {
            buffer_id,
            before,
            start: p,
            current: p,
        });
    }

    fn update_shape(&mut self, tool: Tool, p: [i32; 2]) {
        let Some(drag) = self.editor.shape_drag.as_mut() else {
            return;
        };
        drag.current = p;
        let buffer_id = drag.buffer_id;
        let start = drag.start;
        let before = drag.before.clone();
        let color = self.editor.fg;
        let filled = false;
        if let Some(buf) = self.doc.pixel_buffers.get_mut(&buffer_id) {
            *buf = before;
            rasterize_shape(buf, tool, start, p, color, filled);
        }
        self.refresh_canvas(false);
    }

    fn commit_shape(&mut self) {
        let Some(drag) = self.editor.shape_drag.take() else {
            return;
        };
        let (cw, ch) = match self.canvas_size() {
            Some(s) => (s.width, s.height),
            None => return,
        };
        self.finish_pixel_edit(&drag.before, drag.buffer_id, 0, 0, cw, ch, "Shape");
        self.maybe_add_palette(self.editor.fg);
        self.refresh_canvas(false);
    }

    // --- marquee / lasso / wand selections -----------------------------------

    fn commit_marquee(&mut self) {
        let Some((start, end)) = self.editor.sel_drag.take() else {
            return;
        };
        let Some(size) = self.canvas_size() else { return };
        let bounds = rect_from_corners(start, end);
        if bounds.is_empty() {
            self.editor.clear_selection();
            return;
        }
        let ellipse = self.editor.left_tool == Tool::SelectEllipse || self.editor.right_tool == Tool::SelectEllipse;
        let mask = if ellipse {
            select_ellipse(size.width, size.height, bounds)
        } else {
            select_rect(size.width, size.height, bounds)
        };
        if let Ok(mask) = mask {
            self.editor.selection = Some(mask);
        }
    }

    fn commit_lasso(&mut self) {
        let pts = std::mem::take(&mut self.editor.lasso);
        let Some(size) = self.canvas_size() else { return };
        if let Ok(mask) = select_polygon(size.width, size.height, &pts) {
            if mask.selected_count() > 0 {
                self.editor.selection = Some(mask);
            }
        }
    }

    fn do_wand(&mut self, p: [i32; 2]) {
        if p[0] < 0 || p[1] < 0 {
            return;
        }
        let Some(buffer_id) = self.doc.active_buffer_id() else {
            return;
        };
        let tolerance = self.editor.tolerance;
        let Some(buf) = self.doc.pixel_buffers.get(&buffer_id) else {
            return;
        };
        if let Ok(mask) = magic_wand(buf, p[0] as u32, p[1] as u32, tolerance, Connectivity::Four) {
            self.editor.selection = Some(mask);
        }
    }

    // --- move ----------------------------------------------------------------

    fn begin_move(&mut self, p: [i32; 2]) {
        let Some(mask) = self.editor.selection.clone() else {
            return;
        };
        let Some(buffer_id) = self.doc.active_buffer_id() else {
            return;
        };
        let Some(before) = self.doc.pixel_buffers.get(&buffer_id).cloned() else {
            return;
        };
        // Lift selected pixels into a canvas-sized buffer; clear them in place.
        let Ok(mut lifted) = PixelBuffer::new(before.width(), before.height()) else {
            return;
        };
        if let Some(buf) = self.doc.pixel_buffers.get_mut(&buffer_id) {
            for y in 0..before.height() {
                for x in 0..before.width() {
                    if mask.is_selected(x, y) {
                        if let Some(c) = before.pixel(x, y) {
                            lifted.set_pixel(x, y, c);
                        }
                        buf.set_pixel(x, y, Rgba::transparent());
                    }
                }
            }
        }
        self.editor.move_drag = Some(MoveDrag {
            buffer_id,
            before,
            lifted,
            start: p,
            offset: [0, 0],
        });
        self.refresh_canvas(false);
    }

    fn update_move(&mut self, p: [i32; 2]) {
        let Some(drag) = self.editor.move_drag.as_mut() else {
            return;
        };
        drag.offset = [p[0] - drag.start[0], p[1] - drag.start[1]];
        let buffer_id = drag.buffer_id;
        let offset = drag.offset;
        let cleared = drag.before.clone();
        let lifted = drag.lifted.clone();
        let mask_cleared = self.editor.selection.clone();
        if let Some(buf) = self.doc.pixel_buffers.get_mut(&buffer_id) {
            // Reset to the pre-move buffer with the selection cleared, then
            // stamp the lifted pixels at the offset.
            *buf = cleared;
            if let Some(mask) = &mask_cleared {
                for y in 0..buf.height() {
                    for x in 0..buf.width() {
                        if mask.is_selected(x, y) {
                            buf.set_pixel(x, y, Rgba::transparent());
                        }
                    }
                }
            }
            stamp_lifted(buf, &lifted, offset);
        }
        self.refresh_canvas(false);
    }

    fn commit_move(&mut self) {
        let Some(drag) = self.editor.move_drag.take() else {
            return;
        };
        let (cw, ch) = match self.canvas_size() {
            Some(s) => (s.width, s.height),
            None => return,
        };
        self.finish_pixel_edit(&drag.before, drag.buffer_id, 0, 0, cw, ch, "Move");
        self.refresh_canvas(false);
    }

    // --- shared helpers ------------------------------------------------------

    /// Takes the current stroke session's dirty rect (as `x, y, w, h`) without
    /// consuming the session.
    fn take_session_dirty(&mut self) -> Option<(u32, u32, u32, u32)> {
        let s = self.editor.stroke.as_ref()?;
        let (x, y, x1, y1) = s.dirty?;
        Some((x, y, x1 - x + 1, y1 - y + 1))
    }

    /// Builds and pushes a [`PixelRegionEdit`] for the rect `(x, y, w, h)` of
    /// `buffer_id`, capturing `before`/after region bytes. Records one undo
    /// entry; the apply is a no-op re-write of the current (already-edited)
    /// pixels.
    pub(crate) fn finish_pixel_edit(
        &mut self,
        before: &PixelBuffer,
        buffer_id: pixhaus_core::project::PixelBufferId,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        label: &str,
    ) {
        let Some(current) = self.doc.pixel_buffers.get(&buffer_id) else {
            return;
        };
        let before_bytes = extract_region(before, x, y, w, h);
        let after_bytes = extract_region(current, x, y, w, h);
        if before_bytes == after_bytes {
            return; // nothing changed
        }
        let cmd = PixelRegionEdit {
            buffer_id,
            x,
            y,
            w,
            h,
            before: before_bytes,
            after: after_bytes,
            label: label.to_owned(),
        };
        let _ = self.editor.history.push(Box::new(cmd), &mut self.doc);
    }

    /// Adds `color` to the active palette when auto-add is on and the colour is
    /// not already present.
    fn maybe_add_palette(&mut self, color: Rgba) {
        if !self.editor.auto_add_palette || color.a == 0 {
            return;
        }
        let Some(id) = self.doc.project.active_sprite_id() else {
            return;
        };
        if let Some(sprite) = self.doc.project.sprite_mut(id) {
            if let Some(palette) = sprite.palettes.first_mut() {
                if !palette.colors.iter().any(|e| e.color == color) {
                    palette.colors.push(pixhaus_core::project::PaletteEntry::new(color));
                }
            }
        }
    }

    /// Paints the egui overlays over the wgpu canvas: brush cursor, shape
    /// preview, and selection marching ants.
    #[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
    fn paint_overlays(&self, ui: &egui::Ui, painter: &egui::Painter, rect: egui::Rect, vp_px: Vec2, ppp: f32, hover_canvas: Option<[i32; 2]>) {
        let c2s = |cx: f32, cy: f32| -> egui::Pos2 {
            let s = self.viewport.canvas_to_screen(Vec2::new(cx, cy), vp_px);
            egui::pos2(rect.min.x + s.x / ppp, rect.min.y + s.y / ppp)
        };

        // Brush-cursor footprint outline at the hovered pixel.
        if let Some([hx, hy]) = hover_canvas {
            let tool = self.editor.left_tool;
            if tool.paints() {
                let size = self.editor.brush_size.max(1) as i32;
                let half = size / 2;
                let min = c2s((hx - half) as f32, (hy - half) as f32);
                let max = c2s((hx - half + size) as f32, (hy - half + size) as f32);
                painter.rect_stroke(
                    egui::Rect::from_two_pos(min, max),
                    0.0,
                    egui::Stroke::new(1.0, egui::Color32::from_white_alpha(160)),
                    egui::StrokeKind::Middle,
                );
            }
        }

        // Live shape preview bounds.
        if let Some(drag) = &self.editor.shape_drag {
            let min = c2s(drag.start[0] as f32, drag.start[1] as f32);
            let max = c2s((drag.current[0] + 1) as f32, (drag.current[1] + 1) as f32);
            painter.rect_stroke(
                egui::Rect::from_two_pos(min, max),
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 180, 255)),
                egui::StrokeKind::Middle,
            );
        }

        // Marquee preview while dragging.
        if let Some((start, end)) = self.editor.sel_drag {
            let min = c2s(start[0] as f32, start[1] as f32);
            let max = c2s((end[0] + 1) as f32, (end[1] + 1) as f32);
            painter.rect_stroke(
                egui::Rect::from_two_pos(min, max),
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 220, 120)),
                egui::StrokeKind::Middle,
            );
        }

        // Lasso polyline preview.
        if self.editor.lasso.len() > 1 {
            let pts: Vec<egui::Pos2> = self.editor.lasso.iter().map(|p| c2s(p.x as f32, p.y as f32)).collect();
            painter.add(egui::Shape::line(pts, egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 220, 120))));
        }

        // Selection marching ants tracing the real mask boundary.
        if let Some(mask) = &self.editor.selection {
            let segments = mask.boundary_segments();
            if !segments.is_empty() {
                let phase = (ui.input(|i| i.time) * 8.0) as i32;
                // Cap the per-frame segment count; a pathologically large
                // selection falls back to a static bounds outline.
                const MAX_SEGMENTS: usize = 40_000;
                if segments.len() > MAX_SEGMENTS {
                    let b = mask.bounds();
                    let min = c2s(b.origin.x as f32, b.origin.y as f32);
                    let max = c2s((b.origin.x + b.size.width as i32) as f32, (b.origin.y + b.size.height as i32) as f32);
                    painter.rect_stroke(
                        egui::Rect::from_two_pos(min, max),
                        0.0,
                        egui::Stroke::new(1.0, egui::Color32::WHITE),
                        egui::StrokeKind::Middle,
                    );
                } else {
                    paint_selection_ants(painter, &segments, &c2s, phase);
                    ui.ctx().request_repaint();
                }
            }
        }
    }
}

/// Stamps the brush at `(x, y)` plus its mirror images into `buf`, updating the
/// session's dirty bounds.
fn stamp_point(buf: &mut PixelBuffer, session: &mut StrokeSession, x: i32, y: i32) {
    let (cw, ch) = (buf.width() as i32, buf.height() as i32);
    for (px, py) in mirrored_points(x, y, cw, ch, session.mirror_x, session.mirror_y) {
        paint_brush(buf, px, py, session.color, session.shape, session.size);
        mark_point_dirty(session, px, py, cw, ch);
    }
}

/// Bridges a Bresenham segment from `from` to `to` (plus mirror images) and
/// records the dirty bounds.
fn stamp_segment_mirrored(buf: &mut PixelBuffer, session: &mut StrokeSession, from: [i32; 2], to: [i32; 2]) {
    let (cw, ch) = (buf.width() as i32, buf.height() as i32);
    let froms = mirrored_points(from[0], from[1], cw, ch, session.mirror_x, session.mirror_y);
    let tos = mirrored_points(to[0], to[1], cw, ch, session.mirror_x, session.mirror_y);
    for (f, t) in froms.into_iter().zip(tos) {
        draw_line(buf, f.0, f.1, t.0, t.1, session.color, session.shape, session.size);
        mark_point_dirty(session, f.0, f.1, cw, ch);
        mark_point_dirty(session, t.0, t.1, cw, ch);
    }
}

/// Returns `(x, y)` plus its requested mirror images (deduplicated implicitly by
/// the mirror flags).
fn mirrored_points(x: i32, y: i32, cw: i32, ch: i32, mirror_x: bool, mirror_y: bool) -> Vec<(i32, i32)> {
    let mut pts = vec![(x, y)];
    let mx = cw - 1 - x;
    let my = ch - 1 - y;
    if mirror_x {
        pts.push((mx, y));
    }
    if mirror_y {
        pts.push((x, my));
    }
    if mirror_x && mirror_y {
        pts.push((mx, my));
    }
    pts
}

/// Expands the session dirty bounds to include the brush footprint centred at
/// `(x, y)`, clamped to the canvas.
#[allow(clippy::cast_sign_loss)]
fn mark_point_dirty(session: &mut StrokeSession, x: i32, y: i32, cw: i32, ch: i32) {
    let r = (session.size as i32 / 2) + 1;
    let x0 = (x - r).clamp(0, cw - 1);
    let y0 = (y - r).clamp(0, ch - 1);
    let x1 = (x + r).clamp(0, cw - 1);
    let y1 = (y + r).clamp(0, ch - 1);
    if x1 < x0 || y1 < y0 {
        return;
    }
    session.mark_dirty(x0 as u32, y0 as u32, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
}

/// Draws the requested shape outline (or fill) into `buf`.
fn rasterize_shape(buf: &mut PixelBuffer, tool: Tool, start: [i32; 2], end: [i32; 2], color: Rgba, filled: bool) {
    match tool {
        Tool::Line => {
            draw_line(buf, start[0], start[1], end[0], end[1], color, BrushShape::Pixel, 1);
        }
        Tool::Rectangle => {
            if filled {
                draw_filled_rect(buf, start[0], start[1], end[0], end[1], color);
            } else {
                draw_rect(buf, start[0], start[1], end[0], end[1], color);
            }
        }
        Tool::Ellipse => {
            if filled {
                draw_filled_ellipse(buf, start[0], start[1], end[0], end[1], color);
            } else {
                pixhaus_core::canvas::draw_ellipse(buf, start[0], start[1], end[0], end[1], color);
            }
        }
        _ => {}
    }
}

/// Stamps `lifted` (a canvas-sized buffer of selected pixels) into `buf` shifted
/// by `offset`, skipping transparent pixels.
fn stamp_lifted(buf: &mut PixelBuffer, lifted: &PixelBuffer, offset: [i32; 2]) {
    for y in 0..lifted.height() {
        for x in 0..lifted.width() {
            let Some(c) = lifted.pixel(x, y) else { continue };
            if c.a == 0 {
                continue;
            }
            let nx = x as i32 + offset[0];
            let ny = y as i32 + offset[1];
            if nx >= 0 && ny >= 0 {
                buf.set_pixel(nx as u32, ny as u32, c);
            }
        }
    }
}

/// Builds an inclusive-pixel [`Rect`] from two canvas corners.
#[allow(clippy::cast_sign_loss)]
fn rect_from_corners(a: [i32; 2], b: [i32; 2]) -> Rect {
    let x0 = a[0].min(b[0]).max(0);
    let y0 = a[1].min(b[1]).max(0);
    let x1 = a[0].max(b[0]).max(0);
    let y1 = a[1].max(b[1]).max(0);
    Rect::from_xywh(x0, y0, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32)
}

/// Draws marching ants along the selection's real boundary. Each `segments`
/// entry is a unit grid edge in canvas coordinates; `c2s` maps a canvas point to
/// screen space. Edge colour alternates white/black by grid position plus the
/// animation `phase`, so the ticks crawl diagonally along the true silhouette
/// (egui has no per-arc-length dash-phase API, so this approximates it).
fn paint_selection_ants(painter: &egui::Painter, segments: &[[(i32, i32); 2]], c2s: &impl Fn(f32, f32) -> egui::Pos2, phase: i32) {
    const CELL: i32 = 4; // grid units per white/black cell
    let white = egui::Stroke::new(1.0, egui::Color32::WHITE);
    let black = egui::Stroke::new(1.0, egui::Color32::BLACK);
    for [a, b] in segments {
        let pa = c2s(a.0 as f32, a.1 as f32);
        let pb = c2s(b.0 as f32, b.1 as f32);
        let cell = (a.0 + a.1 + phase).rem_euclid(CELL * 2);
        let stroke = if cell < CELL { white } else { black };
        painter.line_segment([pa, pb], stroke);
    }
}
