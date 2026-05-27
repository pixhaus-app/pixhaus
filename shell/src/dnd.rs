//! Drag-and-drop helpers for the panels.

use std::any::Any;
use std::sync::Arc;

use eframe::egui::{self, Margin, Sense, Stroke, StrokeKind, Ui};

/// How a drop target indicates where a dragged item will land.
#[derive(Clone, Copy)]
pub(crate) enum DropHint {
    /// Drop *inside* the target (a folder or group): the whole row highlights.
    Into,
    /// Reorder *above* the target in top-first display order: an insertion line
    /// marks the gap at the target's top edge.
    Before,
}

/// Whether a payload of type `P` is currently being dragged. Drop indicators and
/// the [`top_level_strip`] only appear while this is true.
pub(crate) fn is_dragging<P: Any + Send + Sync>(ctx: &egui::Context) -> bool {
    egui::DragAndDrop::has_payload_of_type::<P>(ctx)
}

/// A drag-and-drop target that paints nothing while idle. Unlike
/// [`egui::Ui::dnd_drop_zone`], which always fills the zone with the inactive
/// widget background (leaving a stray box under our lists), this renders the
/// contents bare and, only while a compatible payload hovers, draws the
/// indicator chosen by `hint`. Returns the inner result and the payload released
/// over it this frame.
pub(crate) fn drop_target<P, R>(ui: &mut Ui, hint: DropHint, add_contents: impl FnOnce(&mut Ui) -> R) -> (R, Option<Arc<P>>)
where
    P: Any + Send + Sync,
{
    let inner = ui.scope(add_contents);
    let rect = inner.response.rect;
    let response = ui.interact(rect, inner.response.id.with("drop_target"), Sense::hover());

    if is_dragging::<P>(ui.ctx()) && response.contains_pointer() {
        let accent = ui.visuals().widgets.hovered.bg_stroke.color;
        match hint {
            DropHint::Into => {
                let radius = ui.visuals().widgets.active.corner_radius;
                let fill = ui.visuals().selection.bg_fill.gamma_multiply(0.4);
                let painter = ui.painter();
                painter.rect_filled(rect, radius, fill);
                painter.rect_stroke(rect, radius, Stroke::new(1.0, accent), StrokeKind::Inside);
            }
            DropHint::Before => {
                ui.painter().hline(rect.x_range(), rect.top(), Stroke::new(2.0, accent));
            }
        }
    }

    (inner.inner, response.dnd_release_payload::<P>())
}

/// Renders the drag-only "move to top level" strip and returns the payload
/// dropped on it, if any. Reserves no space and renders nothing unless a
/// compatible payload is being dragged.
pub(crate) fn top_level_strip<P: Any + Send + Sync>(ui: &mut Ui, label: &str) -> Option<Arc<P>> {
    if !is_dragging::<P>(ui.ctx()) {
        return None;
    }
    ui.add_space(6.0);
    let border = ui.visuals().widgets.inactive.bg_stroke.color;
    let radius = ui.visuals().widgets.active.corner_radius;
    let ((), payload) = drop_target::<P, _>(ui, DropHint::Into, |ui| {
        egui::Frame::NONE
            .stroke(Stroke::new(1.0, border))
            .corner_radius(radius)
            .inner_margin(Margin::symmetric(8, 8))
            .show(ui, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 16.0),
                    egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| {
                        ui.label(egui::RichText::new(label).weak());
                    },
                );
            });
    });
    payload
}
