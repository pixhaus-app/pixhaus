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
use pixhaus_core::canvas::{
    BrushShape, PixelBuffer, brush_covers, draw_filled_ellipse, draw_filled_rect, draw_line, draw_rect, draw_stroke, flood_fill, paint_brush,
};
use pixhaus_core::project::Rgba;
use pixhaus_core::project::{IVec2, Rect, Size};
use pixhaus_core::selection::{
    Connectivity, GapCloseConfig, SelectionMask, color_range, magic_wand_with_gap_close, select_ellipse, select_polygon, select_rect,
};
use pixhaus_render::{ViewportRenderer, viewport::clamp_zoom, viewport::snap_zoom};

use crate::app::{ShellApp, ZoomAction};
use crate::commands::{PixelRegionEdit, extract_region};
use crate::editor::{FreeTransformDrag, MoveDrag, SelectionMode, ShapeDrag, StrokeSession, Tool};
use crate::gizmo::{BoxGizmo, GizmoHandle};

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
        // A pending free transform commits the moment the active tool stops being
        // Transform (the user switched tools), so it stays one undo entry and the
        // resampled pixels land in the document.
        if self.editor.free_transform.is_some() && self.editor.left_tool != Tool::Transform && self.editor.right_tool != Tool::Transform {
            self.commit_free_transform();
        }
        let Some(p) = pointer else {
            // No active pointer: a release may still need committing.
            if response.drag_stopped_by(egui::PointerButton::Primary) || response.drag_stopped_by(egui::PointerButton::Secondary) {
                self.commit_gesture();
            }
            return;
        };

        // Resolve the selection-combine mode the gesture will commit with. A held
        // Shift/Alt overrides the context-bar default for this gesture; with no
        // modifier the current (UI-set) mode applies. Capture it at the gesture's
        // start — egui reports modifiers per-frame, so reading them at the commit
        // would miss a Shift/Alt released before the pointer.
        let modifiers = response.ctx.input(|i| i.modifiers);
        for (btn, primary) in [(egui::PointerButton::Primary, true), (egui::PointerButton::Secondary, false)] {
            let tool = self.editor.tool_for(primary);
            let color = self.editor.color_for(primary);
            if response.drag_started_by(btn) {
                if is_marquee_or_lasso(tool) {
                    self.resolve_selection_mode(modifiers);
                }
                self.begin_gesture(tool, color, p);
            }
            if response.dragged_by(btn) {
                self.update_gesture(tool, p, modifiers);
            }
            if response.drag_stopped_by(btn) {
                self.commit_gesture();
            }
            if response.clicked_by(btn) {
                if matches!(tool, Tool::Wand | Tool::ColorRange) {
                    self.resolve_selection_mode(modifiers);
                }
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
            Tool::Line | Tool::Rectangle | Tool::Ellipse => self.begin_shape(tool, p),
            Tool::SelectRect | Tool::SelectEllipse => {
                self.editor.sel_drag = Some((p, p));
            }
            Tool::Lasso => {
                self.editor.lasso.clear();
                self.editor.lasso.push(IVec2::new(p[0], p[1]));
            }
            Tool::Move => self.begin_move(p),
            Tool::Transform => self.begin_free_transform(p),
            Tool::Fill | Tool::Wand | Tool::ColorRange | Tool::Picker => {}
        }
    }

    fn update_gesture(&mut self, tool: Tool, p: [i32; 2], modifiers: egui::Modifiers) {
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
            Tool::Transform => self.update_free_transform(p, modifiers.shift),
            Tool::Fill | Tool::Wand | Tool::ColorRange | Tool::Picker => {}
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
            Tool::ColorRange => self.do_color_range(p),
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
        } else if let Some(drag) = self.editor.free_transform.as_mut() {
            // A free transform persists across drags: a single drag release only
            // ends the handle grab. The whole transform commits once (one undo
            // entry) when the tool changes or the user deselects, via
            // `commit_free_transform`.
            drag.gizmo_drag = None;
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
            pending: None,
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
        // Clip the painted footprint to the active selection: outside the mask,
        // restore the pre-stroke bytes. Bounded to the stroke's dirty rect.
        if let Some(mask) = self.editor.selection.clone() {
            if let Some(buf) = self.doc.pixel_buffers.get_mut(&session.buffer_id) {
                clip_to_mask(buf, &session.before, &mask, (x, y, x1, y1));
            }
        }
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
        // Flood fill is unbounded, so clip over the whole canvas.
        if let Some(mask) = self.editor.selection.clone() {
            if let Some(buf) = self.doc.pixel_buffers.get_mut(&buffer_id) {
                clip_to_mask(buf, &before, &mask, (0, 0, cw - 1, ch - 1));
            }
        }
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

    fn begin_shape(&mut self, tool: Tool, p: [i32; 2]) {
        let Some(buffer_id) = self.doc.ensure_drawable() else {
            return;
        };
        let Some(before) = self.doc.pixel_buffers.get(&buffer_id).cloned() else {
            return;
        };
        self.editor.shape_drag = Some(ShapeDrag {
            buffer_id,
            tool,
            before,
            start: p,
            current: p,
            last_dirty: None,
        });
    }

    fn update_shape(&mut self, tool: Tool, p: [i32; 2]) {
        // Read what we need, then drop the drag borrow so the disjoint
        // editor/doc field borrows below are allowed.
        let (buffer_id, start, last_dirty, cw, ch) = {
            let Some(drag) = self.editor.shape_drag.as_mut() else {
                return;
            };
            drag.current = p;
            (drag.buffer_id, drag.start, drag.last_dirty, drag.before.width(), drag.before.height())
        };
        let color = self.editor.fg;
        let new_bounds = shape_bounds(start, p, cw, ch);
        // Repaint only the previous preview footprint (to erase it) unioned with
        // the new one — never the whole canvas.
        let dirty = union_opt(last_dirty, new_bounds);

        if let Some(drag) = self.editor.shape_drag.as_ref() {
            let before = &drag.before;
            if let Some(buf) = self.doc.pixel_buffers.get_mut(&buffer_id) {
                if let Some((rx, ry, rx1, ry1)) = last_dirty {
                    restore_region(buf, before, rx, ry, rx1 - rx + 1, ry1 - ry + 1);
                }
                rasterize_shape(buf, tool, start, p, color, false);
            }
        }

        if let Some((x, y, x1, y1)) = dirty {
            self.upload_region(x, y, x1 - x + 1, y1 - y + 1);
        }
        if let Some(drag) = self.editor.shape_drag.as_mut() {
            drag.last_dirty = new_bounds;
        }
    }

    fn commit_shape(&mut self) {
        let Some(drag) = self.editor.shape_drag.take() else {
            return;
        };
        let (cw, ch) = match self.canvas_size() {
            Some(s) => (s.width, s.height),
            None => return,
        };
        // Shapes can span the canvas, so clip over the canvas bounds.
        if let Some(mask) = self.editor.selection.clone() {
            if let Some(buf) = self.doc.pixel_buffers.get_mut(&drag.buffer_id) {
                clip_to_mask(buf, &drag.before, &mask, (0, 0, cw - 1, ch - 1));
            }
        }
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
            self.combine_selection(mask);
        }
    }

    fn commit_lasso(&mut self) {
        let pts = std::mem::take(&mut self.editor.lasso);
        let Some(size) = self.canvas_size() else { return };
        if let Ok(mask) = select_polygon(size.width, size.height, &pts) {
            if mask.selected_count() > 0 {
                self.combine_selection(mask);
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
        let connectivity = if self.editor.wand_eight { Connectivity::Eight } else { Connectivity::Four };
        let gap_config = self.editor.wand_gap_close.then(|| GapCloseConfig {
            closing_distance: self.editor.wand_gap_distance,
            ..GapCloseConfig::default()
        });
        let Some(buf) = self.doc.pixel_buffers.get(&buffer_id) else {
            return;
        };
        if let Ok(mask) = magic_wand_with_gap_close(buf, IVec2::new(p[0], p[1]), tolerance, connectivity, gap_config) {
            self.combine_selection(mask);
        }
    }

    /// Selects every pixel matching the clicked colour within
    /// [`crate::editor::EditorState::color_range_tolerance`], canvas-wide and
    /// regardless of contiguity (the wand's contiguous twin). Samples the
    /// reference colour from the active buffer, not the composited
    /// `display_frame`, so onion skins and other layers do not pollute it.
    fn do_color_range(&mut self, p: [i32; 2]) {
        if p[0] < 0 || p[1] < 0 {
            return;
        }
        let Some(buffer_id) = self.doc.active_buffer_id() else {
            return;
        };
        let tolerance = self.editor.color_range_tolerance;
        let Some(buf) = self.doc.pixel_buffers.get(&buffer_id) else {
            return;
        };
        let Some(reference) = buf.pixel(p[0] as u32, p[1] as u32) else {
            return;
        };
        if let Ok(mask) = color_range(buf, reference, tolerance) {
            self.combine_selection(mask);
        }
    }

    /// Sets [`crate::editor::EditorState::selection_mode`] from held modifiers
    /// at gesture start. A held Shift/Alt picks the mode (Shift -> Add, Alt ->
    /// Subtract, Shift+Alt -> Intersect); with neither held the mode is left as
    /// the context-bar default, so a keyboard-free user can still pick a mode.
    fn resolve_selection_mode(&mut self, modifiers: egui::Modifiers) {
        if modifiers.shift || modifiers.alt {
            self.editor.selection_mode = SelectionMode::from_modifiers(modifiers);
        }
    }

    /// Stores `new_mask` combined with the existing selection per
    /// [`crate::editor::EditorState::selection_mode`], via [`combine_masks`].
    /// Clears the selection to `None` when the combine leaves nothing selected.
    fn combine_selection(&mut self, new_mask: SelectionMask) {
        self.editor.selection = combine_masks(self.editor.selection.as_ref(), new_mask, self.editor.selection_mode);
    }

    /// Selects the whole canvas (the Ctrl+A / Select menu command). Builds a
    /// fully-covered mask at the active sprite's size and replaces the current
    /// selection. A no-op when there is no sprite or the dimensions overflow.
    pub(crate) fn select_all(&mut self) {
        let Some(size) = self.canvas_size() else { return };
        if let Ok(mask) = SelectionMask::full(size.width, size.height) {
            self.editor.selection = Some(mask);
        }
    }

    /// Inverts the selection (the Ctrl+Shift+I / Select menu command). Flips an
    /// existing mask's coverage; with nothing selected, the invert of the empty
    /// selection is the whole canvas (the Photoshop model), so this selects all.
    /// A no-op when there is no sprite or the dimensions overflow.
    pub(crate) fn invert_selection(&mut self) {
        let Some(size) = self.canvas_size() else { return };
        self.editor.selection = inverted_selection(self.editor.selection.as_ref(), size);
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
        // Lift selected pixels into a canvas-sized buffer, clear them in place,
        // and track the selection's bounding box for bounded per-move work.
        let Ok(mut lifted) = PixelBuffer::new(before.width(), before.height()) else {
            return;
        };
        let mut sel: Option<(u32, u32, u32, u32)> = None;
        if let Some(buf) = self.doc.pixel_buffers.get_mut(&buffer_id) {
            for y in 0..before.height() {
                for x in 0..before.width() {
                    if mask.is_selected(x, y) {
                        if let Some(c) = before.pixel(x, y) {
                            lifted.set_pixel(x, y, c);
                        }
                        buf.set_pixel(x, y, Rgba::transparent());
                        sel = Some(match sel {
                            None => (x, y, x, y),
                            Some((ax, ay, bx, by)) => (ax.min(x), ay.min(y), bx.max(x), by.max(y)),
                        });
                    }
                }
            }
        }
        // The live buffer now holds the cleared background — keep a copy as the
        // per-move restore source so the move never recopies the whole buffer.
        let Some(base) = self.doc.pixel_buffers.get(&buffer_id).cloned() else {
            return;
        };
        self.editor.move_drag = Some(MoveDrag {
            buffer_id,
            before,
            base,
            lifted,
            sel_bounds: sel,
            last_dirty: sel,
            start: p,
            offset: [0, 0],
        });
        self.refresh_canvas(false);
    }

    fn update_move(&mut self, p: [i32; 2]) {
        let (buffer_id, offset, sel_bounds, last_dirty, cw, ch) = {
            let Some(drag) = self.editor.move_drag.as_mut() else {
                return;
            };
            drag.offset = [p[0] - drag.start[0], p[1] - drag.start[1]];
            (
                drag.buffer_id,
                drag.offset,
                drag.sel_bounds,
                drag.last_dirty,
                drag.base.width(),
                drag.base.height(),
            )
        };
        // The footprint the lifted pixels now occupy, clamped to the canvas.
        let new_dirty = sel_bounds.and_then(|b| translate_clamp(b, offset, cw, ch));
        // Restore the previous footprint and the new one, then stamp.
        let dirty = union_opt(last_dirty, new_dirty);

        if let Some(drag) = self.editor.move_drag.as_ref() {
            let base = &drag.base;
            let lifted = &drag.lifted;
            if let Some(buf) = self.doc.pixel_buffers.get_mut(&buffer_id) {
                if let Some((rx, ry, rx1, ry1)) = dirty {
                    restore_region(buf, base, rx, ry, rx1 - rx + 1, ry1 - ry + 1);
                }
                if let Some(b) = sel_bounds {
                    stamp_lifted_region(buf, lifted, b, offset);
                }
            }
        }

        if let Some((x, y, x1, y1)) = dirty {
            self.upload_region(x, y, x1 - x + 1, y1 - y + 1);
        }
        if let Some(drag) = self.editor.move_drag.as_mut() {
            drag.last_dirty = new_dirty;
        }
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

    // --- free transform ------------------------------------------------------

    /// Starts (or continues) a free transform. The first press lifts the
    /// selection into a canvas-sized `lifted` buffer, clears it from the live
    /// buffer, and seeds the box gizmo to the selection's bounding box — the same
    /// lift `begin_move` does. Either way, the press then picks the gizmo handle
    /// under the pointer (a corner, the rotate stem, or the body) so the next
    /// drag transforms the lifted pixels. A press that misses both an active
    /// transform and a selection is a no-op.
    #[allow(clippy::cast_precision_loss)]
    fn begin_free_transform(&mut self, p: [i32; 2]) {
        if self.editor.free_transform.is_none() {
            let Some(mask) = self.editor.selection.clone() else {
                return;
            };
            let Some(buffer_id) = self.doc.active_buffer_id() else {
                return;
            };
            let Some(before) = self.doc.pixel_buffers.get(&buffer_id).cloned() else {
                return;
            };
            let Ok(mut lifted) = PixelBuffer::new(before.width(), before.height()) else {
                return;
            };
            let mut sel: Option<(u32, u32, u32, u32)> = None;
            if let Some(buf) = self.doc.pixel_buffers.get_mut(&buffer_id) {
                for y in 0..before.height() {
                    for x in 0..before.width() {
                        if mask.is_selected(x, y) {
                            if let Some(c) = before.pixel(x, y) {
                                lifted.set_pixel(x, y, c);
                            }
                            buf.set_pixel(x, y, Rgba::transparent());
                            sel = Some(match sel {
                                None => (x, y, x, y),
                                Some((ax, ay, bx, by)) => (ax.min(x), ay.min(y), bx.max(x), by.max(y)),
                            });
                        }
                    }
                }
            }
            let Some((x0, y0, x1, y1)) = sel else {
                // Nothing was selected; leave the buffer untouched.
                return;
            };
            let Some(base) = self.doc.pixel_buffers.get(&buffer_id).cloned() else {
                return;
            };
            self.editor.free_transform = Some(FreeTransformDrag {
                buffer_id,
                before,
                base,
                lifted,
                sel_bounds: sel,
                last_dirty: sel,
                gizmo: BoxGizmo::from_bounds(x0, y0, x1, y1),
                gizmo_drag: None,
            });
            self.refresh_canvas(false);
        }

        // Pick the handle under the pointer on the (now-present) gizmo. Tolerance
        // is in canvas pixels scaled so the grab radius is ~8 screen pixels at any
        // zoom; the pointer's pixel-centre gives sub-cell precision.
        let tol = (8.0 / self.viewport.zoom.max(0.01)).max(1.5);
        let pc = (p[0] as f32 + 0.5, p[1] as f32 + 0.5);
        if let Some(drag) = self.editor.free_transform.as_mut() {
            drag.gizmo_drag = pick_handle_canvas(&drag.gizmo, pc, tol);
        }
    }

    /// Drives the active gizmo handle to the pointer and live-previews the
    /// resample: restore the previous footprint from `base`, then inverse-map
    /// `lifted` through the gizmo affine into the live buffer, bounded by the
    /// gizmo's axis-aligned bounding box. Per-frame cost is O(gizmo area), not
    /// O(canvas) — the 8K constraint.
    #[allow(clippy::cast_precision_loss)]
    fn update_free_transform(&mut self, p: [i32; 2], uniform: bool) {
        let (buffer_id, cw, ch) = {
            let Some(drag) = self.editor.free_transform.as_ref() else {
                return;
            };
            (drag.buffer_id, drag.base.width(), drag.base.height())
        };
        // The gizmo reads the pointer's absolute image position for every handle:
        // a corner sets the half-extents from the pointer's local distance, the
        // stem points at it, and the body re-centres on it. Shift on a corner
        // locks the aspect ratio (uniform scale).
        let pc = (p[0] as f32 + 0.5, p[1] as f32 + 0.5);
        let prev_aabb = {
            let Some(drag) = self.editor.free_transform.as_mut() else {
                return;
            };
            let Some(handle) = drag.gizmo_drag else {
                return;
            };
            let prev = drag.gizmo.aabb(cw, ch);
            match handle {
                GizmoHandle::Move => {
                    // Re-centre the box so the grabbed body tracks the pointer.
                    drag.gizmo.cx = pc.0;
                    drag.gizmo.cy = pc.1;
                }
                handle => drag.gizmo.drag_handle(handle, pc.0, pc.1, 0.0, 0.0, uniform),
            }
            prev
        };

        // The footprints to rewrite: the previous stamp and the new gizmo box,
        // unioned so the trail is cleared as the box moves.
        let new_aabb = self.editor.free_transform.as_ref().map(|d| d.gizmo.aabb(cw, ch));
        let prev_rect = aabb_to_inclusive(prev_aabb);
        let new_rect = new_aabb.and_then(aabb_to_inclusive);
        let dirty = union_opt(prev_rect, new_rect);

        if let Some(drag) = self.editor.free_transform.as_ref() {
            let base = &drag.base;
            let lifted = &drag.lifted;
            let gizmo = drag.gizmo;
            let sel = drag.sel_bounds;
            if let Some(buf) = self.doc.pixel_buffers.get_mut(&buffer_id) {
                if let Some((rx, ry, rx1, ry1)) = dirty {
                    restore_region(buf, base, rx, ry, rx1 - rx + 1, ry1 - ry + 1);
                }
                if let Some(sel_bounds) = sel {
                    resample_lifted(buf, lifted, &gizmo, sel_bounds, cw, ch);
                }
            }
        }

        if let Some((x, y, x1, y1)) = dirty {
            self.upload_region(x, y, x1 - x + 1, y1 - y + 1);
        }
        if let Some(drag) = self.editor.free_transform.as_mut() {
            drag.last_dirty = new_rect;
        }
    }

    /// Commits the active free transform as one undo entry over the union of the
    /// original selection bounds and the final gizmo box, then drops the drag and
    /// clears the selection. A no-op gizmo (the live buffer matches its
    /// pre-transform bytes) records nothing — `finish_pixel_edit` skips an
    /// unchanged region. Called when the tool changes away from Transform or the
    /// user deselects, not on each drag release.
    pub(crate) fn commit_free_transform(&mut self) {
        let Some(drag) = self.editor.free_transform.take() else {
            return;
        };
        let (cw, ch) = match self.canvas_size() {
            Some(s) => (s.width, s.height),
            None => return,
        };
        self.finish_pixel_edit(&drag.before, drag.buffer_id, 0, 0, cw, ch, "Free transform");
        self.editor.clear_selection();
        self.refresh_canvas(false);
    }

    // --- shared helpers ------------------------------------------------------

    /// Takes the stroke session's *pending* dirty rect (the footprint stamped
    /// since the last upload) as `x, y, w, h`, clearing it. Bounds the per-move
    /// recomposite and GPU upload to the latest dab, not the whole stroke.
    fn take_session_dirty(&mut self) -> Option<(u32, u32, u32, u32)> {
        self.editor.stroke.as_mut()?.take_pending()
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

        // Cursor gizmo at the hovered pixel: the exact brush footprint for the
        // paint brushes, a single target cell for click/shape/selection tools.
        if let Some([hx, hy]) = hover_canvas {
            let cursor = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(160));
            match self.editor.left_tool {
                Tool::Pencil | Tool::Eraser => {
                    for [a, b] in brush_outline_segments(self.editor.brush_shape, self.editor.brush_size, hx, hy) {
                        painter.line_segment([c2s(a.0 as f32, a.1 as f32), c2s(b.0 as f32, b.1 as f32)], cursor);
                    }
                }
                // Move and Transform act on an existing selection / the gizmo,
                // not a pixel under the cursor.
                Tool::Move | Tool::Transform => {}
                _ => {
                    let min = c2s(hx as f32, hy as f32);
                    let max = c2s((hx + 1) as f32, (hy + 1) as f32);
                    painter.rect_stroke(egui::Rect::from_two_pos(min, max), 0.0, cursor, egui::StrokeKind::Middle);
                }
            }
        }

        // Live shape preview: the real geometry the tool will draw.
        if let Some(drag) = &self.editor.shape_drag {
            let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 180, 255));
            match drag.tool {
                Tool::Line => {
                    // Pixel centres, so the gizmo sits over the rasterized line.
                    let a = c2s(drag.start[0] as f32 + 0.5, drag.start[1] as f32 + 0.5);
                    let b = c2s(drag.current[0] as f32 + 0.5, drag.current[1] as f32 + 0.5);
                    painter.line_segment([a, b], stroke);
                }
                Tool::Ellipse => {
                    let pts: Vec<egui::Pos2> = ellipse_points(drag.start, drag.current).into_iter().map(|(x, y)| c2s(x, y)).collect();
                    painter.add(egui::Shape::closed_line(pts, stroke));
                }
                _ => {
                    let min = c2s(drag.start[0] as f32, drag.start[1] as f32);
                    let max = c2s((drag.current[0] + 1) as f32, (drag.current[1] + 1) as f32);
                    painter.rect_stroke(egui::Rect::from_two_pos(min, max), 0.0, stroke, egui::StrokeKind::Middle);
                }
            }
        }

        // Marquee preview while dragging: an ellipse for the ellipse-select tool
        // (its only feedback — selection tools draw no pixels), else a rect.
        if let Some((start, end)) = self.editor.sel_drag {
            let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 220, 120));
            let ellipse = self.editor.left_tool == Tool::SelectEllipse || self.editor.right_tool == Tool::SelectEllipse;
            if ellipse {
                let pts: Vec<egui::Pos2> = ellipse_points(start, end).into_iter().map(|(x, y)| c2s(x, y)).collect();
                painter.add(egui::Shape::closed_line(pts, stroke));
            } else {
                let min = c2s(start[0] as f32, start[1] as f32);
                let max = c2s((end[0] + 1) as f32, (end[1] + 1) as f32);
                painter.rect_stroke(egui::Rect::from_two_pos(min, max), 0.0, stroke, egui::StrokeKind::Middle);
            }
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

        // Free-transform box gizmo: the four corner handles, the body outline,
        // and the rotate stem, drawn over the live resample preview.
        if let Some(drag) = &self.editor.free_transform {
            let accent = ui.visuals().selection.stroke.color;
            let g = &drag.gizmo;
            let pts: Vec<egui::Pos2> = g.corners().iter().map(|&(x, y)| c2s(x, y)).collect();
            for k in 0..4 {
                painter.line_segment([pts[k], pts[(k + 1) % 4]], egui::Stroke::new(1.5, accent));
            }
            for p in &pts {
                painter.circle_filled(*p, 4.0, accent);
            }
            let (tx, ty) = g.point(0.0, -g.hh);
            let top_mid = c2s(tx, ty);
            let (rx, ry) = g.rotate_handle();
            let stem = c2s(rx, ry);
            painter.line_segment([top_mid, stem], egui::Stroke::new(1.5, accent));
            painter.circle_filled(stem, 4.0, accent);
        }
    }
}

/// Whether `tool` is a drag-to-select tool whose combine mode is captured at
/// `drag_started_by` (marquee rect/ellipse, lasso). The wand combines on click,
/// so it captures its mode separately.
fn is_marquee_or_lasso(tool: Tool) -> bool {
    matches!(tool, Tool::SelectRect | Tool::SelectEllipse | Tool::Lasso)
}

/// Combines `new_mask` with the `current` selection per `mode` and returns the
/// resulting selection: `Some(mask)` when something stays selected, `None` when
/// the combine leaves nothing (so the caller clears the selection and drawing is
/// unclipped again). Replace, or any mode with no current selection, keeps
/// `new_mask`. The set ops need equal dimensions; both masks are canvas-sized so
/// they always match, but an `Err(SizeMismatch)` falls back to `new_mask` rather
/// than panicking.
fn combine_masks(current: Option<&SelectionMask>, new_mask: SelectionMask, mode: SelectionMode) -> Option<SelectionMask> {
    let combined = match (current, mode) {
        (Some(cur), SelectionMode::Add) => cur.union(&new_mask),
        (Some(cur), SelectionMode::Subtract) => cur.subtract(&new_mask),
        (Some(cur), SelectionMode::Intersect) => cur.intersect(&new_mask),
        _ => Ok(new_mask.clone()),
    };
    let mask = combined.unwrap_or(new_mask);
    (mask.selected_count() > 0).then_some(mask)
}

/// The inverted selection for a canvas of `size`. An existing mask flips its
/// coverage; with nothing selected the invert of the empty set is the whole
/// canvas (the Photoshop model), so this returns a full mask. `None` only when
/// the canvas dimensions overflow `usize`, which the caller treats as a no-op.
fn inverted_selection(current: Option<&SelectionMask>, size: Size) -> Option<SelectionMask> {
    match current {
        Some(mask) => Some(mask.invert()),
        None => SelectionMask::full(size.width, size.height).ok(),
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

/// Picks the gizmo handle a click at canvas-pixel position `pc` grabs, with
/// hit-testing entirely in canvas-pixel space (tolerance `tol`). The screen-space
/// pick the studio uses needs the per-frame `to_screen` map; here the canvas
/// pointer is already in image pixels and `tol` is pre-scaled for the zoom, so a
/// canvas-space distance test is enough and keeps the gizmo widget caller-agnostic.
fn pick_handle_canvas(g: &BoxGizmo, pc: (f32, f32), tol: f32) -> Option<GizmoHandle> {
    let dist = |a: (f32, f32)| ((a.0 - pc.0).powi(2) + (a.1 - pc.1).powi(2)).sqrt();
    for (k, &corner) in g.corners().iter().enumerate() {
        if dist(corner) <= tol {
            return Some(GizmoHandle::Corner(k));
        }
    }
    if dist(g.rotate_handle()) <= tol {
        return Some(GizmoHandle::Rotate);
    }
    g.contains(pc.0, pc.1).then_some(GizmoHandle::Move)
}

/// Converts a gizmo `[x, y, w, h]` aabb to an inclusive `(x0, y0, x1, y1)` rect,
/// or `None` for a zero-area box.
fn aabb_to_inclusive([x, y, w, h]: [u32; 4]) -> Option<(u32, u32, u32, u32)> {
    (w > 0 && h > 0).then(|| (x, y, x + w - 1, y + h - 1))
}

/// Inverse-maps the lifted selection through the gizmo's affine and stamps the
/// result into `buf`, bounded to the gizmo's axis-aligned bounding box so the
/// cost is O(box area), not O(canvas) — the 8K constraint.
///
/// At its seed the gizmo covers the selection's continuous extent exactly
/// (`BoxGizmo::from_bounds`), so the box's local `[-hw, hw] x [-hh, hh]` frame
/// maps linearly to the source rectangle `sel_bounds`. For each destination pixel
/// in the box's aabb we undo the gizmo transform to a local position, normalize it
/// to `[0, 1]` across the box, scale to the source rect, and sample `lifted` with
/// nearest-neighbour — the responsive preview default; a crisper resample is a
/// commit-time choice. Transparent source pixels are skipped so the lift's hole
/// shape is preserved.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn resample_lifted(buf: &mut PixelBuffer, lifted: &PixelBuffer, gizmo: &BoxGizmo, sel_bounds: (u32, u32, u32, u32), cw: u32, ch: u32) {
    let (sx0, sy0, sx1, sy1) = sel_bounds;
    let left = sx0 as f32;
    let top = sy0 as f32;
    let right = sx1 as f32 + 1.0;
    let bottom = sy1 as f32 + 1.0;
    let src_w = right - left;
    let src_h = bottom - top;
    let (hw, hh) = (gizmo.hw.max(f32::EPSILON), gizmo.hh.max(f32::EPSILON));

    let [ax, ay, aw, ah] = gizmo.aabb(cw, ch);
    for dy in ay..ay + ah {
        for dx in ax..ax + aw {
            let (lx, ly) = gizmo.to_local(dx as f32 + 0.5, dy as f32 + 0.5);
            if lx.abs() > hw || ly.abs() > hh {
                continue;
            }
            // Local -> [0, 1] across the box -> source-rect pixel.
            let u = (lx + hw) / (2.0 * hw);
            let v = (ly + hh) / (2.0 * hh);
            let src_x = left + u * src_w - 0.5;
            let src_y = top + v * src_h - 0.5;
            let sx = src_x.round();
            let sy = src_y.round();
            if sx < sx0 as f32 || sx > sx1 as f32 || sy < sy0 as f32 || sy > sy1 as f32 {
                continue;
            }
            let Some(c) = lifted.pixel(sx as u32, sy as u32) else { continue };
            if c.a == 0 {
                continue;
            }
            buf.set_pixel(dx, dy, c);
        }
    }
}

/// Stamps the lifted pixels into `buf` shifted by `offset`, skipping transparent
/// pixels. Iteration is bounded to `sel_bounds` (the inclusive pre-offset
/// selection box) so move previews cost O(selection), not O(canvas).
fn stamp_lifted_region(buf: &mut PixelBuffer, lifted: &PixelBuffer, sel_bounds: (u32, u32, u32, u32), offset: [i32; 2]) {
    let (x0, y0, x1, y1) = sel_bounds;
    for y in y0..=y1 {
        for x in x0..=x1 {
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

/// Copies the rect `(x, y, w, h)` from `src` into `dst` (both full-canvas, same
/// dims). Bounded by the rect, the per-move restore primitive for shape and
/// move previews.
fn restore_region(dst: &mut PixelBuffer, src: &PixelBuffer, x: u32, y: u32, w: u32, h: u32) {
    let x1 = (x + w).min(dst.width());
    let y1 = (y + h).min(dst.height());
    for py in y..y1 {
        for px in x..x1 {
            if let Some(c) = src.pixel(px, py) {
                dst.set_pixel(px, py, c);
            }
        }
    }
}

/// Blends every painted pixel of `buf` toward its pre-gesture value in `before`
/// by the inverse of the mask's coverage, bounded to the inclusive rect
/// `(x0, y0, x1, y1)`. The single clip primitive for stroke/fill/shape commits:
/// paint runs unclipped, then this reconciles each pixel with the selection —
/// `out = lerp(before, painted, coverage / 255)`. Full coverage (255) keeps the
/// painted pixel, zero coverage restores `before`, and a feathered edge lands
/// partway, so soft selections soften the clipped paint with no separate branch.
/// For a hard-edged (binary 0/255) mask the blend reduces to the keep/restore
/// case and is byte-identical to it.
///
/// Work is O(rect ∩ mask), never O(canvas) — pixels outside `mask.bounds()` have
/// zero coverage and restore `before` directly, so a small selection or a small
/// gesture stays cheap at 8K. The eraser flows through the same path: pixels
/// outside the mask keep their original (non-erased) value from `before`.
fn clip_to_mask(buf: &mut PixelBuffer, before: &PixelBuffer, mask: &SelectionMask, bounds: (u32, u32, u32, u32)) {
    let (bx0, by0, bx1, by1) = bounds;
    let mb = mask.bounds();
    // An empty mask covers nothing: restore the whole gesture footprint.
    if mb.is_empty() {
        for y in by0..=by1 {
            for x in bx0..=bx1 {
                if let Some(c) = before.pixel(x, y) {
                    buf.set_pixel(x, y, c);
                }
            }
        }
        return;
    }
    // The mask's bounding box. Pixels outside it have zero coverage, so they
    // restore `before` without reading the coverage byte. The box only tightens
    // the per-pixel blend to the selected region; iteration stays over the
    // gesture bounds (already the dirty rect).
    let mx0 = mb.origin.x.max(0) as u32;
    let my0 = mb.origin.y.max(0) as u32;
    let mx1 = mx0 + mb.size.width.saturating_sub(1);
    let my1 = my0 + mb.size.height.saturating_sub(1);
    for y in by0..=by1 {
        for x in bx0..=bx1 {
            let coverage = if x >= mx0 && x <= mx1 && y >= my0 && y <= my1 {
                mask.get(x, y).unwrap_or(0)
            } else {
                0
            };
            // Fully covered: keep the painted pixel untouched.
            if coverage == 255 {
                continue;
            }
            let Some(was) = before.pixel(x, y) else { continue };
            // Zero coverage: restore the pre-gesture pixel outright.
            if coverage == 0 {
                buf.set_pixel(x, y, was);
                continue;
            }
            // Partial (feathered) coverage: blend painted toward `was`.
            let Some(now) = buf.pixel(x, y) else { continue };
            buf.set_pixel(x, y, blend_coverage(was, now, coverage));
        }
    }
}

/// Linearly blends `painted` toward `before` by `coverage`:
/// `out = lerp(before, painted, coverage / 255)`, per channel. `coverage == 255`
/// returns `painted`, `0` returns `before`, `128` lands halfway. Channels are
/// non-premultiplied, so the blend runs straight on each component including
/// alpha — matching how the rest of the shell treats `Rgba`.
fn blend_coverage(before: Rgba, painted: Rgba, coverage: u8) -> Rgba {
    Rgba::new(
        lerp_u8(before.r, painted.r, coverage),
        lerp_u8(before.g, painted.g, coverage),
        lerp_u8(before.b, painted.b, coverage),
        lerp_u8(before.a, painted.a, coverage),
    )
}

/// Rounds `a + (b - a) * t / 255` to the nearest `u8`, with `t` the coverage in
/// `0..=255`. Symmetric and exact at the endpoints (`t == 0` -> `a`,
/// `t == 255` -> `b`); `t == 128` lands on the rounded midpoint.
fn lerp_u8(a: u8, b: u8, t: u8) -> u8 {
    let a = i32::from(a);
    let b = i32::from(b);
    let t = i32::from(t);
    // Round-to-nearest: add half the divisor before the integer divide.
    let v = a + ((b - a) * t + 127) / 255;
    v.clamp(0, 255) as u8
}

/// Inclusive canvas-pixel bounds touched by a shape drawn between two corners,
/// padded one pixel for outline safety and clamped to the canvas. `None` when
/// the box falls fully off-canvas.
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn shape_bounds(a: [i32; 2], b: [i32; 2], cw: u32, ch: u32) -> Option<(u32, u32, u32, u32)> {
    let x0 = (a[0].min(b[0]) - 1).clamp(0, cw as i32 - 1);
    let y0 = (a[1].min(b[1]) - 1).clamp(0, ch as i32 - 1);
    let x1 = (a[0].max(b[0]) + 1).clamp(0, cw as i32 - 1);
    let y1 = (a[1].max(b[1]) + 1).clamp(0, ch as i32 - 1);
    if x1 < x0 || y1 < y0 {
        return None;
    }
    Some((x0 as u32, y0 as u32, x1 as u32, y1 as u32))
}

/// Translates an inclusive bounds box by `offset` and clamps it to the canvas.
/// `None` when the translated box no longer overlaps the canvas.
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn translate_clamp(bounds: (u32, u32, u32, u32), offset: [i32; 2], cw: u32, ch: u32) -> Option<(u32, u32, u32, u32)> {
    let (bx0, by0, bx1, by1) = bounds;
    let nx0 = bx0 as i32 + offset[0];
    let ny0 = by0 as i32 + offset[1];
    let nx1 = bx1 as i32 + offset[0];
    let ny1 = by1 as i32 + offset[1];
    if nx1 < 0 || ny1 < 0 || nx0 > cw as i32 - 1 || ny0 > ch as i32 - 1 {
        return None;
    }
    Some((
        nx0.clamp(0, cw as i32 - 1) as u32,
        ny0.clamp(0, ch as i32 - 1) as u32,
        nx1.clamp(0, cw as i32 - 1) as u32,
        ny1.clamp(0, ch as i32 - 1) as u32,
    ))
}

/// Unions two optional inclusive bounds boxes.
fn union_opt(a: Option<(u32, u32, u32, u32)>, b: Option<(u32, u32, u32, u32)>) -> Option<(u32, u32, u32, u32)> {
    match (a, b) {
        (Some((ax, ay, ax1, ay1)), Some((bx, by, bx1, by1))) => Some((ax.min(bx), ay.min(by), ax1.max(bx1), ay1.max(by1))),
        (some, None) | (None, some) => some,
    }
}

/// The boundary edges of the brush footprint centred at `(cx, cy)`, as
/// canvas-grid segments `[(x0, y0), (x1, y1)]`. Traces the exact set of pixels
/// [`brush_covers`] reports painted: an edge is emitted wherever a painted cell
/// borders an unpainted one, so the cursor outline hugs the real footprint
/// (round for Circle, square for Square, one cell for Pixel).
#[allow(clippy::cast_possible_wrap)]
fn brush_outline_segments(shape: BrushShape, size: u32, cx: i32, cy: i32) -> Vec<[(i32, i32); 2]> {
    // One cell beyond the half-extent covers every painted cell plus the
    // unpainted neighbours the boundary test needs.
    let ext = (size.max(1) as i32) / 2 + 1;
    let mut segments = Vec::new();
    for dy in -ext..=ext {
        for dx in -ext..=ext {
            if !brush_covers(shape, size, dx, dy) {
                continue;
            }
            let (gx, gy) = (cx + dx, cy + dy);
            if !brush_covers(shape, size, dx, dy - 1) {
                segments.push([(gx, gy), (gx + 1, gy)]); // top
            }
            if !brush_covers(shape, size, dx, dy + 1) {
                segments.push([(gx, gy + 1), (gx + 1, gy + 1)]); // bottom
            }
            if !brush_covers(shape, size, dx - 1, dy) {
                segments.push([(gx, gy), (gx, gy + 1)]); // left
            }
            if !brush_covers(shape, size, dx + 1, dy) {
                segments.push([(gx + 1, gy), (gx + 1, gy + 1)]); // right
            }
        }
    }
    segments
}

/// A closed polyline approximating the ellipse inscribed in the inclusive cell
/// box spanned by corners `a` and `b`, in canvas coordinates. Used for the
/// ellipse-draw and ellipse-select previews.
#[allow(clippy::cast_precision_loss)]
fn ellipse_points(a: [i32; 2], b: [i32; 2]) -> Vec<(f32, f32)> {
    const N: usize = 64;
    let x0 = a[0].min(b[0]) as f32;
    let y0 = a[1].min(b[1]) as f32;
    // +1 so the box spans the far cells' outer edges, matching the rect preview.
    let x1 = (a[0].max(b[0]) + 1) as f32;
    let y1 = (a[1].max(b[1]) + 1) as f32;
    let (cx, cy) = (f32::midpoint(x0, x1), f32::midpoint(y0, y1));
    let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    (0..N)
        .map(|i| {
            let t = i as f32 / N as f32 * std::f32::consts::TAU;
            (cx + rx * t.cos(), cy + ry * t.sin())
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::write_region;

    /// A closed outline visits every grid point an even number of times (each
    /// vertex is shared by an incoming and outgoing edge).
    fn is_closed(segments: &[[(i32, i32); 2]]) -> bool {
        let mut degree: std::collections::HashMap<(i32, i32), i32> = std::collections::HashMap::new();
        for [a, b] in segments {
            *degree.entry(*a).or_default() += 1;
            *degree.entry(*b).or_default() += 1;
        }
        !segments.is_empty() && degree.values().all(|d| d % 2 == 0)
    }

    #[test]
    fn pixel_brush_outline_is_one_cell() {
        let segs = brush_outline_segments(BrushShape::Pixel, 1, 5, 7);
        // Four edges around the single cell (5,7)-(6,8).
        assert_eq!(segs.len(), 4);
        assert!(is_closed(&segs));
    }

    #[test]
    fn square_brush_outline_is_rect_perimeter() {
        // Size 2 square: a 2x2 block -> 8 unit boundary edges, closed.
        let segs = brush_outline_segments(BrushShape::Square, 2, 0, 0);
        assert_eq!(segs.len(), 8);
        assert!(is_closed(&segs));
    }

    #[test]
    fn circle_brush_outline_is_closed_and_symmetric() {
        let segs = brush_outline_segments(BrushShape::Circle, 8, 0, 0);
        assert!(is_closed(&segs), "circle outline forms a closed loop");
        // The footprint is symmetric across the cell-row centre line y = 0.5
        // (covers(dx, dy) == covers(dx, -dy)), so reflecting a boundary edge by
        // y -> 1 - y yields another boundary edge (endpoints may be swapped).
        let set: std::collections::HashSet<[(i32, i32); 2]> = segs.iter().copied().collect();
        for [a, b] in &segs {
            let ma = (a.0, 1 - a.1);
            let mb = (b.0, 1 - b.1);
            assert!(set.contains(&[ma, mb]) || set.contains(&[mb, ma]), "missing mirror of {a:?}-{b:?}");
        }
    }

    #[test]
    fn circle_outline_differs_from_square_outline() {
        // The bug was a square gizmo for a circle brush; the two must differ.
        let circle = brush_outline_segments(BrushShape::Circle, 8, 0, 0);
        let square = brush_outline_segments(BrushShape::Square, 8, 0, 0);
        assert_ne!(circle.len(), square.len());
    }

    #[test]
    fn ellipse_points_form_closed_ring_centered_on_bbox() {
        let pts = ellipse_points([0, 0], [9, 5]);
        assert_eq!(pts.len(), 64);
        // Centre of the inclusive box [0,10) x [0,6) is (5, 3).
        let cx = pts.iter().map(|p| p.0).sum::<f32>() / pts.len() as f32;
        let cy = pts.iter().map(|p| p.1).sum::<f32>() / pts.len() as f32;
        assert!((cx - 5.0).abs() < 0.01, "cx={cx}");
        assert!((cy - 3.0).abs() < 0.01, "cy={cy}");
    }

    // --- clip_to_mask: selection-constrained drawing ------------------------

    const RED: Rgba = Rgba::opaque(220, 40, 40);
    const BLANK: Rgba = Rgba::transparent();

    /// A canvas-sized buffer filled with `fill`.
    fn buf(w: u32, h: u32, fill: Rgba) -> PixelBuffer {
        PixelBuffer::filled(w, h, fill).expect("buffer")
    }

    /// A rect mask selecting the inclusive box `(x0, y0, x1, y1)` on a `w x h`
    /// canvas.
    fn rect_mask(w: u32, h: u32, x0: i32, y0: i32, x1: i32, y1: i32) -> SelectionMask {
        select_rect(w, h, Rect::from_xywh(x0, y0, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32)).expect("rect mask")
    }

    #[test]
    fn clip_keeps_paint_inside_mask_restores_outside() {
        // Paint a horizontal red row across a 16x16 canvas, with a rect mask
        // covering the left half (x 0..=7). After clipping, only the masked
        // half keeps paint; the right half reverts to the pre-paint blank.
        let before = buf(16, 16, BLANK);
        let mut after = before.clone();
        for x in 0..16 {
            after.set_pixel(x, 8, RED);
        }
        let mask = rect_mask(16, 16, 0, 0, 7, 15);
        clip_to_mask(&mut after, &before, &mask, (0, 8, 15, 8));

        for x in 0..16 {
            let got = after.pixel(x, 8).expect("pixel");
            if x <= 7 {
                assert_eq!(got, RED, "masked pixel ({x}, 8) keeps paint");
            } else {
                assert_eq!(got, BLANK, "unmasked pixel ({x}, 8) reverts to before");
            }
        }
    }

    #[test]
    fn clip_to_ellipse_mask_fills_only_inside() {
        // Flood-fill a blank canvas red, then clip to an ellipse mask. Only the
        // ellipse interior stays red; everything else reverts to blank.
        let before = buf(24, 24, BLANK);
        let mut after = before.clone();
        flood_fill(&mut after, 0, 0, RED, 0);
        let mask = select_ellipse(24, 24, Rect::from_xywh(4, 4, 16, 16)).expect("ellipse mask");
        clip_to_mask(&mut after, &before, &mask, (0, 0, 23, 23));

        for y in 0..24 {
            for x in 0..24 {
                let got = after.pixel(x, y).expect("pixel");
                if mask.is_selected(x, y) {
                    assert_eq!(got, RED, "inside-ellipse ({x}, {y}) is filled");
                } else {
                    assert_eq!(got, BLANK, "outside-ellipse ({x}, {y}) reverts");
                }
            }
        }
    }

    #[test]
    fn clip_crops_rect_at_mask_edge() {
        // A filled rect spanning x 2..=12 crossing a mask that ends at x=7.
        // The committed rect is cropped to the mask boundary.
        let before = buf(16, 16, BLANK);
        let mut after = before.clone();
        draw_filled_rect(&mut after, 2, 2, 12, 12, RED);
        let mask = rect_mask(16, 16, 0, 0, 7, 15);
        clip_to_mask(&mut after, &before, &mask, (0, 0, 15, 15));

        // The last painted column inside the mask is x=7; x=8 onward is cropped.
        assert_eq!(after.pixel(7, 6).expect("pixel"), RED, "rect kept up to the mask edge");
        assert_eq!(after.pixel(8, 6).expect("pixel"), BLANK, "rect cropped past the mask edge");
    }

    #[test]
    fn empty_mask_restores_everything_in_bounds() {
        // An empty (nothing-selected) mask reverts the whole gesture footprint.
        let before = buf(8, 8, BLANK);
        let mut after = before.clone();
        for x in 0..8 {
            after.set_pixel(x, 3, RED);
        }
        let empty = SelectionMask::new(8, 8).expect("empty mask");
        assert_eq!(empty.selected_count(), 0);
        clip_to_mask(&mut after, &before, &empty, (0, 3, 7, 3));
        for x in 0..8 {
            assert_eq!(after.pixel(x, 3).expect("pixel"), BLANK, "empty mask reverts ({x}, 3)");
        }
    }

    #[test]
    fn no_selection_leaves_paint_unclipped() {
        // Regression guard for the call-site short-circuit: when the editor has
        // no selection, clip_to_mask is never invoked, so paint survives whole.
        // This mirrors the `if let Some(mask) = self.editor.selection` guard in
        // commit_stroke / do_fill / commit_shape.
        let selection: Option<SelectionMask> = None;
        let before = buf(8, 8, BLANK);
        let mut after = before.clone();
        for x in 0..8 {
            after.set_pixel(x, 3, RED);
        }
        if let Some(mask) = selection {
            clip_to_mask(&mut after, &before, &mask, (0, 3, 7, 3));
        }
        for x in 0..8 {
            assert_eq!(after.pixel(x, 3).expect("pixel"), RED, "no selection paints unclipped ({x}, 3)");
        }
    }

    // --- coverage-weighted clip blend (feather) -----------------------------

    #[test]
    fn lerp_u8_hits_endpoints_and_midpoint() {
        // The blend weight is exact at the ends and rounds the midpoint.
        assert_eq!(lerp_u8(0, 255, 0), 0, "coverage 0 keeps `before`");
        assert_eq!(lerp_u8(0, 255, 255), 255, "coverage 255 keeps `painted`");
        assert_eq!(lerp_u8(0, 255, 128), 128, "coverage 128 lands on the rounded midpoint");
        assert_eq!(lerp_u8(40, 200, 0), 40);
        assert_eq!(lerp_u8(40, 200, 255), 200);
    }

    #[test]
    fn blend_clip_at_coverage_128_lands_halfway() {
        // A single pixel painted RED over a BLACK `before`, with the mask at
        // half coverage (128), commits to the rounded midpoint of the two —
        // the soft-edge case feathering produces.
        let black = Rgba::opaque(0, 0, 0);
        let before = buf(4, 4, black);
        let mut after = before.clone();
        after.set_pixel(1, 1, RED);

        let mut mask = SelectionMask::new(4, 4).expect("mask");
        mask.set(1, 1, 128);
        clip_to_mask(&mut after, &before, &mask, (1, 1, 1, 1));

        let got = after.pixel(1, 1).expect("pixel");
        // lerp(black, RED, 128/255): each channel halfway (rounded).
        assert_eq!(got, Rgba::opaque(110, 20, 20), "coverage-128 paint lands halfway: got {got:?}");
    }

    #[test]
    fn feathered_boundary_pixel_keeps_intermediate_coverage() {
        // Feather a left-half row mask, then paint a full red row and clip. The
        // boundary pixel the feather softened (coverage neither 0 nor 255) must
        // commit to a colour strictly between `before` and the paint, not a hard
        // snap to either.
        use pixhaus_core::selection::feather;
        let before = buf(10, 1, BLANK);
        let mut hard = SelectionMask::new(10, 1).expect("mask");
        for x in 0..5u32 {
            hard.set(x, 0, 255);
        }
        let soft = feather(&hard, 2).expect("feather");
        // Find a boundary pixel the feather left partially covered.
        let edge = (0..10u32).find(|&x| matches!(soft.get(x, 0), Some(c) if c > 0 && c < 255)).expect("a softened edge pixel");
        let coverage = soft.get(edge, 0).expect("coverage");

        let mut after = before.clone();
        for x in 0..10 {
            after.set_pixel(x, 0, RED);
        }
        clip_to_mask(&mut after, &before, &soft, (0, 0, 9, 0));

        let got = after.pixel(edge, 0).expect("pixel");
        assert_eq!(got, blend_coverage(BLANK, RED, coverage), "edge pixel blends by its coverage");
        assert!(got != BLANK && got != RED, "softened edge is neither fully reverted nor fully painted: {got:?}");
    }

    #[test]
    fn hard_edged_clip_is_byte_identical_under_the_blend() {
        // Switching clip_to_mask to a coverage blend must not change the binary
        // (0/255) path. Paint a diagonal stroke, clip it to a hard rect mask,
        // and prove every byte matches a hand-built keep/restore reference —
        // the image-compare guard the plan asks for.
        let (w, h) = (32u32, 32u32);
        let before = buf(w, h, BLANK);
        let points: Vec<[f32; 2]> = (0..32).map(|i| [i as f32, i as f32]).collect();
        let mask = rect_mask(w, h, 8, 8, 23, 23);

        let mut actual = before.clone();
        draw_stroke(&mut actual, &points, RED, BrushShape::Pixel, 1, true);
        clip_to_mask(&mut actual, &before, &mask, (0, 0, w - 1, h - 1));

        // Reference: paint, then hard keep/restore by is_selected (the pre-blend
        // semantics). The blend must reproduce this exactly for a binary mask.
        let mut expected = before.clone();
        draw_stroke(&mut expected, &points, RED, BrushShape::Pixel, 1, true);
        for y in 0..h {
            for x in 0..w {
                if !mask.is_selected(x, y) {
                    expected.set_pixel(x, y, before.pixel(x, y).expect("pixel"));
                }
            }
        }

        // Byte-for-byte equality, stronger than an image-compare score.
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "hard-edged clip is byte-identical under the blend");
        // And confirm via image-compare for the visual-regression record.
        let score = image_compare::rgba_hybrid_compare(&to_rgba_image(&actual), &to_rgba_image(&expected))
            .expect("compare")
            .score;
        assert!((score - 1.0).abs() < f64::EPSILON, "hard-edged clip score is exactly 1.0: {score}");
    }

    // --- combine_masks / selection modifiers --------------------------------

    #[test]
    fn replace_mode_keeps_only_the_new_mask() {
        // Replace ignores the current selection entirely.
        let current = rect_mask(16, 16, 0, 0, 7, 15); // left half, 128 px
        let new = rect_mask(16, 16, 8, 0, 15, 15); // right half, 128 px
        let out = combine_masks(Some(&current), new, SelectionMode::Replace).expect("non-empty");
        assert_eq!(out.selected_count(), 128, "replace yields just the new mask");
    }

    #[test]
    fn add_mode_unions_overlapping_masks() {
        // Two 8x8 boxes overlapping in a 4x4 corner: union = 64 + 64 - 16 = 112.
        let current = rect_mask(16, 16, 0, 0, 7, 7);
        let new = rect_mask(16, 16, 4, 4, 11, 11);
        let out = combine_masks(Some(&current), new, SelectionMode::Add).expect("non-empty");
        assert_eq!(out.selected_count(), 112, "add yields the union count");
    }

    #[test]
    fn subtract_mode_removes_the_overlap() {
        // Subtract the second box from the first leaves 64 - 16 = 48 px.
        let current = rect_mask(16, 16, 0, 0, 7, 7);
        let new = rect_mask(16, 16, 4, 4, 11, 11);
        let out = combine_masks(Some(&current), new, SelectionMode::Subtract).expect("non-empty");
        assert_eq!(out.selected_count(), 48, "subtract removes the overlap");
    }

    #[test]
    fn intersect_mode_keeps_only_the_overlap() {
        // Intersect keeps just the shared 4x4 corner: 16 px.
        let current = rect_mask(16, 16, 0, 0, 7, 7);
        let new = rect_mask(16, 16, 4, 4, 11, 11);
        let out = combine_masks(Some(&current), new, SelectionMode::Intersect).expect("non-empty");
        assert_eq!(out.selected_count(), 16, "intersect keeps only the overlap");
    }

    #[test]
    fn subtract_to_empty_clears_the_selection() {
        // Subtracting a superset of the current selection empties it, which
        // combine_masks reports as None so the caller drops the selection and
        // drawing is unclipped again.
        let current = rect_mask(16, 16, 4, 4, 11, 11);
        let cover_all = rect_mask(16, 16, 0, 0, 15, 15);
        let out = combine_masks(Some(&current), cover_all, SelectionMode::Subtract);
        assert!(out.is_none(), "subtracting everything clears the selection");
    }

    #[test]
    fn combine_with_no_current_selection_keeps_the_new_mask() {
        // Add/Subtract/Intersect with nothing selected fall through to the new
        // mask (there is nothing to combine against).
        let new = rect_mask(16, 16, 0, 0, 7, 7);
        for mode in [SelectionMode::Add, SelectionMode::Subtract, SelectionMode::Intersect] {
            let out = combine_masks(None, new.clone(), mode).expect("non-empty");
            assert_eq!(out.selected_count(), 64, "{mode:?} with no current keeps the new mask");
        }
    }

    #[test]
    fn selection_mode_resolves_from_modifiers() {
        use egui::Modifiers;
        let none = Modifiers::default();
        let shift = Modifiers { shift: true, ..Modifiers::default() };
        let alt = Modifiers { alt: true, ..Modifiers::default() };
        let both = Modifiers { shift: true, alt: true, ..Modifiers::default() };
        assert_eq!(SelectionMode::from_modifiers(none), SelectionMode::Replace);
        assert_eq!(SelectionMode::from_modifiers(shift), SelectionMode::Add);
        assert_eq!(SelectionMode::from_modifiers(alt), SelectionMode::Subtract);
        assert_eq!(SelectionMode::from_modifiers(both), SelectionMode::Intersect);
    }

    // --- select all / invert -----------------------------------------------

    #[test]
    fn select_all_covers_the_whole_canvas() {
        // The full mask over a 16x16 canvas selects every one of its 256 pixels
        // (the body of select_all, minus the ShellApp size lookup).
        let mask = SelectionMask::full(16, 16).expect("full mask");
        assert_eq!(mask.selected_count(), 256, "select all covers the whole canvas");
    }

    #[test]
    fn invert_flips_a_half_canvas_mask_to_the_complement() {
        // A left-half mask (128 of 256 px) inverts to the right half.
        let left = rect_mask(16, 16, 0, 0, 7, 15);
        assert_eq!(left.selected_count(), 128);
        let out = inverted_selection(Some(&left), Size::new(16, 16)).expect("inverted mask");
        assert_eq!(out.selected_count(), 128, "invert keeps the complementary count");
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(out.is_selected(x, y), !left.is_selected(x, y), "({x}, {y}) flips coverage");
            }
        }
    }

    #[test]
    fn invert_with_no_selection_yields_full() {
        // Inverting the empty selection selects everything (the Photoshop model).
        let out = inverted_selection(None, Size::new(16, 16)).expect("full mask");
        assert_eq!(out.selected_count(), 256, "invert of nothing selects all");
    }

    /// Converts a tightly-packed [`PixelBuffer`] to an owned [`image::RgbaImage`]
    /// for `image-compare`.
    fn to_rgba_image(b: &PixelBuffer) -> image::RgbaImage {
        let mut bytes = Vec::with_capacity((b.width() as usize) * (b.height() as usize) * 4);
        for y in 0..b.height() {
            bytes.extend_from_slice(b.row(y).expect("row"));
        }
        image::RgbaImage::from_raw(b.width(), b.height(), bytes).expect("rgba image")
    }

    #[test]
    fn clipped_stroke_matches_expected_render() {
        // image-compare visual regression: a diagonal stroke clipped to a rect
        // mask must match an independently-built reference that paints only the
        // masked pixels. Catches boundary off-by-one in the clip.
        let (w, h) = (32u32, 32u32);
        let before = buf(w, h, BLANK);
        let points: Vec<[f32; 2]> = (0..32).map(|i| [i as f32, i as f32]).collect();
        let mask = rect_mask(w, h, 8, 8, 23, 23);

        let mut actual = before.clone();
        draw_stroke(&mut actual, &points, RED, BrushShape::Pixel, 1, true);
        clip_to_mask(&mut actual, &before, &mask, (0, 0, w - 1, h - 1));

        // Reference: the same stroke, then hand-revert every unselected pixel.
        let mut expected = before.clone();
        draw_stroke(&mut expected, &points, RED, BrushShape::Pixel, 1, true);
        for y in 0..h {
            for x in 0..w {
                if !mask.is_selected(x, y) {
                    expected.set_pixel(x, y, before.pixel(x, y).expect("pixel"));
                }
            }
        }

        let score = image_compare::rgba_hybrid_compare(&to_rgba_image(&actual), &to_rgba_image(&expected))
            .expect("compare")
            .score;
        assert!(score >= 0.999, "clipped stroke diverged from reference: score = {score}");
    }

    proptest::proptest! {
        /// The load-bearing property: clip_to_mask never changes a pixel the
        /// mask does not select, and leaves every selected pixel equal to the
        /// unclipped paint.
        #[test]
        fn clip_never_touches_unselected_and_keeps_selected(
            mx0 in 0u32..16, my0 in 0u32..16, mw in 1u32..16, mh in 1u32..16,
            seed in 0u64..1_000_000,
        ) {
            const W: u32 = 16;
            const H: u32 = 16;
            // A deterministic pseudo-random paint over a blank canvas.
            let before = buf(W, H, BLANK);
            let mut painted = before.clone();
            let mut state = seed.wrapping_add(1);
            for y in 0..H {
                for x in 0..W {
                    state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                    if (state >> 33) & 1 == 1 {
                        let r = (state >> 8) as u8;
                        let g = (state >> 16) as u8;
                        let b = (state >> 24) as u8;
                        painted.set_pixel(x, y, Rgba::opaque(r, g, b));
                    }
                }
            }
            let mask = rect_mask(W, H, mx0 as i32, my0 as i32, (mx0 + mw - 1) as i32, (my0 + mh - 1) as i32);

            let mut clipped = painted.clone();
            clip_to_mask(&mut clipped, &before, &mask, (0, 0, W - 1, H - 1));

            for y in 0..H {
                for x in 0..W {
                    let got = clipped.pixel(x, y).expect("pixel");
                    if mask.is_selected(x, y) {
                        proptest::prop_assert_eq!(got, painted.pixel(x, y).expect("pixel"), "selected pixel matches unclipped paint at ({}, {})", x, y);
                    } else {
                        proptest::prop_assert_eq!(got, before.pixel(x, y).expect("pixel"), "unselected pixel is untouched at ({}, {})", x, y);
                    }
                }
            }
        }
    }

    // --- magic-wand connectivity + gap-close --------------------------------

    /// Builds the connectivity / gap-config exactly as `do_wand` does, then
    /// runs the core flood. Keeps these tests honest about the wiring `do_wand`
    /// uses without booting a full `ShellApp`/`DocumentStore`.
    fn wand(buf: &PixelBuffer, seed: [i32; 2], tolerance: u8, eight: bool, gap_close: bool, gap_distance: u32) -> SelectionMask {
        let connectivity = if eight { Connectivity::Eight } else { Connectivity::Four };
        let gap_config = gap_close.then(|| GapCloseConfig {
            closing_distance: gap_distance,
            ..GapCloseConfig::default()
        });
        magic_wand_with_gap_close(buf, IVec2::new(seed[0], seed[1]), tolerance, connectivity, gap_config).expect("wand")
    }

    /// Two same-colour regions that touch only at a single diagonal corner. A
    /// 4-connected flood from one region cannot reach the other; an 8-connected
    /// flood can cross the diagonal. The corner pixel between them is a
    /// different colour so the two regions are distinct masses.
    ///
    /// Layout (R = region colour, X = barrier):
    ///   R X
    ///   X R
    fn diagonal_two_region() -> PixelBuffer {
        let region = Rgba::opaque(40, 120, 220);
        let barrier = Rgba::opaque(220, 40, 40);
        let mut b = PixelBuffer::filled(2, 2, barrier).expect("buffer");
        b.set_pixel(0, 0, region);
        b.set_pixel(1, 1, region);
        b
    }

    #[test]
    fn wand_four_does_not_cross_the_diagonal() {
        // 4-connectivity treats the two region pixels as separate masses, so
        // flooding from (0,0) selects only that pixel.
        let buf = diagonal_two_region();
        let m = wand(&buf, [0, 0], 0, false, false, 10);
        assert!(m.is_selected(0, 0), "seed pixel is selected");
        assert!(!m.is_selected(1, 1), "Four does not cross the diagonal");
        assert_eq!(m.selected_count(), 1);
    }

    #[test]
    fn wand_eight_crosses_the_diagonal() {
        // 8-connectivity bridges the diagonal corner, so both region pixels
        // join one mass.
        let buf = diagonal_two_region();
        let m = wand(&buf, [0, 0], 0, true, false, 10);
        assert!(m.is_selected(0, 0), "seed pixel is selected");
        assert!(m.is_selected(1, 1), "Eight crosses the diagonal");
        assert_eq!(m.selected_count(), 2);
    }

    /// A black square outline on a white field with a `gap_pixels`-wide break in
    /// the top edge, leaving `margin` of exterior white above it. Mirrors the
    /// core `square_outline_with_top_gap` fixture.
    fn square_outline_with_top_gap(size: u32, margin: u32, gap_pixels: u32) -> PixelBuffer {
        let white = Rgba::opaque(255, 255, 255);
        let ink = Rgba::opaque(0, 0, 0);
        let mut b = PixelBuffer::filled(size, size, white).expect("buffer");
        let top = margin;
        let bottom = size - margin - 1;
        let left = margin;
        let right = size - margin - 1;
        let gap_start = size / 2 - gap_pixels / 2;
        let gap_end = gap_start + gap_pixels;
        for x in left..=right {
            if !(gap_start..gap_end).contains(&x) {
                b.set_pixel(x, top, ink);
            }
            b.set_pixel(x, bottom, ink);
        }
        for y in top..=bottom {
            b.set_pixel(left, y, ink);
            b.set_pixel(right, y, ink);
        }
        b
    }

    #[test]
    fn wand_gap_close_stops_the_leak_a_plain_wand_shows() {
        // A plain wand from the interior leaks through the 2px top gap into the
        // exterior white margin; turning gap-close on bridges the gap so the
        // flood stays inside the outline.
        let buf = square_outline_with_top_gap(20, 3, 2);
        let seed = [10, 10];

        let leaked = wand(&buf, seed, 0, false, false, 10);
        assert!(leaked.is_selected(10, 10), "baseline fill selects the interior");
        assert!(leaked.is_selected(0, 0), "baseline fill leaks to the exterior corner");

        let closed = wand(&buf, seed, 0, false, true, 10);
        assert!(closed.is_selected(10, 10), "gap-close still selects the interior");
        assert!(!closed.is_selected(0, 0), "gap-close stops the leak to the exterior");
    }

    // --- colour range (select-by-colour) ------------------------------------

    /// Samples the reference colour at `seed` from `buf` and runs the core
    /// `color_range`, exactly as `do_color_range` wires it — keeps the test
    /// honest about the wiring without booting a `ShellApp`.
    fn select_color_range(buf: &PixelBuffer, seed: [u32; 2], tolerance: u8) -> SelectionMask {
        let reference = buf.pixel(seed[0], seed[1]).expect("reference pixel");
        color_range(buf, reference, tolerance).expect("color_range")
    }

    /// A two-colour checkerboard: red on the even cells, blue on the odd. Red
    /// pixels never touch each other 4-connected, so a contiguous wand would
    /// catch only one — `color_range` must catch them all.
    fn two_color_checkerboard(w: u32, h: u32) -> PixelBuffer {
        let red = Rgba::opaque(220, 40, 40);
        let blue = Rgba::opaque(40, 80, 220);
        let mut b = PixelBuffer::filled(w, h, blue).expect("buffer");
        for y in 0..h {
            for x in 0..w {
                if (x + y) % 2 == 0 {
                    b.set_pixel(x, y, red);
                }
            }
        }
        b
    }

    #[test]
    fn color_range_selects_all_matching_regardless_of_contiguity() {
        // Click a red cell; every red cell must select even though no two red
        // cells touch, and no blue cell may.
        let buf = two_color_checkerboard(8, 8);
        let mask = select_color_range(&buf, [0, 0], 0);
        for y in 0..8 {
            for x in 0..8 {
                let red = (x + y) % 2 == 0;
                assert_eq!(mask.is_selected(x, y), red, "({x}, {y}) selected == is-red");
            }
        }
        // 8x8 board, half the cells are red.
        assert_eq!(mask.selected_count(), 32);
    }

    #[test]
    fn color_range_tolerance_255_selects_everything() {
        // Maximum tolerance treats every colour as a match, so the whole canvas
        // selects regardless of the clicked colour.
        let buf = two_color_checkerboard(8, 8);
        let mask = select_color_range(&buf, [0, 0], 255);
        assert_eq!(mask.selected_count(), 64);
    }

    // --- free transform: gizmo affine resample ------------------------------

    /// Lifts the inclusive selection `(x0, y0, x1, y1)` of `src` into a
    /// canvas-sized `lifted` buffer and returns it alongside the seed gizmo (the
    /// box covering the selection bounds, unrotated). Mirrors the lift
    /// `begin_free_transform` performs, without a `ShellApp`.
    fn lift(src: &PixelBuffer, x0: u32, y0: u32, x1: u32, y1: u32) -> (PixelBuffer, BoxGizmo) {
        let mut lifted = PixelBuffer::new(src.width(), src.height()).expect("lifted");
        for y in y0..=y1 {
            for x in x0..=x1 {
                if let Some(c) = src.pixel(x, y) {
                    lifted.set_pixel(x, y, c);
                }
            }
        }
        (lifted, BoxGizmo::from_bounds(x0, y0, x1, y1))
    }

    #[test]
    fn identity_gizmo_resample_reproduces_the_lift() {
        // With the seed gizmo (no scale, rotate, or move) the resample reproduces
        // the lifted region exactly, so a commit over a cleared `base` lands the
        // pixels back where they started — and `finish_pixel_edit`'s before==after
        // guard records no history entry.
        let mut src = buf(8, 8, BLANK);
        src.set_pixel(2, 2, RED);
        src.set_pixel(5, 5, Rgba::opaque(20, 220, 40));
        src.set_pixel(3, 4, Rgba::opaque(40, 60, 220));
        let (lifted, gizmo) = lift(&src, 2, 2, 5, 5);

        // `base` is the live buffer with the selection cleared (transparent hole).
        let mut out = src.clone();
        for y in 2..=5u32 {
            for x in 2..=5u32 {
                out.set_pixel(x, y, BLANK);
            }
        }
        resample_lifted(&mut out, &lifted, &gizmo, (2, 2, 5, 5), 8, 8);
        assert_eq!(out.as_bytes(), src.as_bytes(), "identity resample reproduces the source");
    }

    #[test]
    fn uniform_scale_2x_of_a_4x4_selection_fills_an_8x8_region() {
        // A solid 4x4 selection scaled 2x about its centre covers an 8x8 region.
        // The seed box spans continuous [2, 6] x [2, 6] (centre (4, 4), half 2);
        // doubling the half-extents to 4 grows it to [0, 8] x [0, 8].
        let fill = Rgba::opaque(180, 60, 90);
        let mut src = buf(8, 8, BLANK);
        for y in 2..=5u32 {
            for x in 2..=5u32 {
                src.set_pixel(x, y, fill);
            }
        }
        let (lifted, mut gizmo) = lift(&src, 2, 2, 5, 5);
        gizmo.hw = 4.0;
        gizmo.hh = 4.0;

        let mut out = buf(8, 8, BLANK);
        resample_lifted(&mut out, &lifted, &gizmo, (2, 2, 5, 5), 8, 8);

        // Every one of the 64 pixels is the fill colour; the scaled box covers the
        // whole canvas with the solid selection.
        let covered = out.pixels().filter(|p| *p == fill).count();
        assert_eq!(covered, 64, "2x scale of the solid 4x4 fills the 8x8 region");
    }

    #[test]
    fn lifted_hole_round_trips_through_a_before_after_swap() {
        // The undo contract: a free transform's `before` is the pre-lift buffer,
        // its `after` is base (the hole) plus the resampled stamp. Swapping after
        // -> before restores the original, including the cleared hole. This is the
        // exact apply/undo `PixelRegionEdit` performs over the committed region.
        let mut before = buf(8, 8, BLANK);
        before.set_pixel(3, 3, RED);
        before.set_pixel(4, 3, RED);
        let (lifted, mut gizmo) = lift(&before, 3, 3, 4, 3);

        // Move the box two pixels right (the body drag) and resample over a hole.
        gizmo.cx += 2.0;
        let mut after = before.clone();
        for x in 3..=4u32 {
            after.set_pixel(x, 3, BLANK); // lift the hole
        }
        resample_lifted(&mut after, &lifted, &gizmo, (3, 3, 4, 3), 8, 8);
        // After the move the original cells are empty and the shifted ones filled.
        assert_eq!(after.pixel(3, 3), Some(BLANK), "original cell vacated");
        assert_eq!(after.pixel(5, 3), Some(RED), "moved cell filled");

        // The undo restores `before` byte-for-byte (the hole is closed again).
        let region_before = extract_region(&before, 0, 0, 8, 8);
        let mut undone = after.clone();
        write_region(&mut undone, 0, 0, 8, 8, &region_before);
        assert_eq!(undone.as_bytes(), before.as_bytes(), "undo restores the lifted hole");
    }

    #[test]
    fn pick_handle_finds_corner_stem_and_body() {
        let g = BoxGizmo {
            cx: 4.0,
            cy: 4.0,
            hw: 2.0,
            hh: 2.0,
            angle: 0.0,
        };
        // TL corner at (2, 2); a click on it within tolerance grabs Corner(0).
        assert_eq!(pick_handle_canvas(&g, (2.0, 2.0), 1.0), Some(GizmoHandle::Corner(0)));
        // The body centre grabs Move.
        assert_eq!(pick_handle_canvas(&g, (4.0, 4.0), 1.0), Some(GizmoHandle::Move));
        // The rotate stem sits above the top edge.
        let (rx, ry) = g.rotate_handle();
        assert_eq!(pick_handle_canvas(&g, (rx, ry), 1.0), Some(GizmoHandle::Rotate));
        // A click far outside the box misses everything.
        assert_eq!(pick_handle_canvas(&g, (7.9, 7.9), 0.5), None);
    }

    #[test]
    fn aabb_to_inclusive_drops_zero_area() {
        assert_eq!(aabb_to_inclusive([2, 3, 4, 5]), Some((2, 3, 5, 7)));
        assert_eq!(aabb_to_inclusive([2, 3, 0, 5]), None);
        assert_eq!(aabb_to_inclusive([2, 3, 4, 0]), None);
    }

    /// Visual regression: a rotated free-transform selection compared against a
    /// committed baseline PNG. Regenerate with `PIXHAUS_UPDATE_SNAPSHOTS=1
    /// cargo test -p pixhaus-shell rotated_free_transform_matches_baseline` after
    /// an intentional change, and audit the new image in the PR.
    #[test]
    fn rotated_free_transform_matches_baseline() {
        use std::path::Path;

        // A 16x16 canvas with an off-centre coloured block as the selection.
        let (w, h) = (16u32, 16u32);
        let mut src = buf(w, h, BLANK);
        for y in 4..=11u32 {
            for x in 4..=11u32 {
                let shade = if (x + y) % 2 == 0 { Rgba::opaque(220, 80, 60) } else { Rgba::opaque(60, 120, 220) };
                src.set_pixel(x, y, shade);
            }
        }
        let (lifted, mut gizmo) = lift(&src, 4, 4, 11, 11);
        gizmo.angle = std::f32::consts::FRAC_PI_6; // 30 degrees

        let mut out = buf(w, h, BLANK);
        resample_lifted(&mut out, &lifted, &gizmo, (4, 4, 11, 11), w, h);

        let actual = to_rgba_image(&out);
        let baseline_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/rotated_free_transform.png");
        if std::env::var_os("PIXHAUS_UPDATE_SNAPSHOTS").is_some() {
            if let Some(dir) = baseline_path.parent() {
                std::fs::create_dir_all(dir).expect("create snapshot dir");
            }
            actual.save(&baseline_path).expect("write baseline png");
            return;
        }
        let baseline = image::open(&baseline_path)
            .expect("baseline png is present; regenerate with PIXHAUS_UPDATE_SNAPSHOTS=1")
            .to_rgba8();
        let score = image_compare::rgba_hybrid_compare(&actual, &baseline).expect("compare").score;
        assert!(score >= 0.999, "rotated free transform diverged from baseline: score = {score}");
    }
}
