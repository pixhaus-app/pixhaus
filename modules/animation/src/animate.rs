//! The Animate workspace, its panels, and the timeline.
//!
//! Animate is editing in space over time atop the shared sprite-editing core
//! (bible rule 2). Its layout reuses the Layers/Sprites/Frames/Console panels by
//! id - sprite-edit owns and registers those, so they are referenced here, never
//! re-registered. This module adds the Clip Properties and AI Animation Assistant
//! dock panels, the Timeline tray panel, the AI-animation actions, and the Frame
//! menu group.

use egui::{Align2, FontId, Key, KeyboardShortcut, Modifiers, Sense, Stroke, Vec2};
use pixhaus_ui::contrib_api::{
    ActionDesc, ActionId, HostRegistrar, MenuGroup, MenuItem, Panel, PanelId, PanelMeta, PanelScope, StatusItem, ToolId, Workspace, WorkspaceId,
    WorkspaceLayout, WorkspaceMeta,
};
use pixhaus_ui::region::Region;
use pixhaus_ui::state::intent::Intent;
use pixhaus_ui::{icons, widgets};

/// The Animate workspace id.
pub const ANIMATE: WorkspaceId = WorkspaceId("animate");

/// The Clip Properties dock panel id (animation-owned).
pub const CLIP_PROPERTIES: PanelId = PanelId("clip-properties");
/// The AI Animation Assistant dock panel id (animation-owned).
pub const AI_ANIM_ASSISTANT: PanelId = PanelId("ai-animation-assistant");
/// The Timeline tray panel id (animation-owned).
pub const TIMELINE: PanelId = PanelId("timeline");

// Shared panels owned by sprite-edit, referenced by id - never re-registered here.
const LAYERS: PanelId = PanelId("layers");
const SPRITES: PanelId = PanelId("sprites");
const FRAMES: PanelId = PanelId("frames");
const CONSOLE: PanelId = PanelId("console");

// The default tool is the shared Pencil (owned by sprite-edit).
const PENCIL: ToolId = ToolId("pencil");

// AI Animation Assistant quick-actions. Namespaced under `anim.` so they never
// collide with sprite-edit's `ai.*` action ids.
const ANIM_INBETWEEN: ActionId = ActionId("anim.in-between-frames");
const ANIM_EXTEND: ActionId = ActionId("anim.extend-animation");
const ANIM_CLEAN: ActionId = ActionId("anim.clean-frames");
const ANIM_REDUCE: ActionId = ActionId("anim.reduce-colors");
const ANIM_VARIATIONS: ActionId = ActionId("anim.create-variations");

// Frame menu actions. sprite-edit already registers these via `add_action`; the
// Frame menu group below only references them, it does not re-register them.
const FRAME_ADD: ActionId = ActionId("frame.add");
const FRAME_DUPLICATE: ActionId = ActionId("frame.duplicate");
const FRAME_DELETE: ActionId = ActionId("frame.delete");

/// The full 15-tool rail in rail order (shared editing tools owned by sprite-edit).
fn full_rail() -> Vec<ToolId> {
    [
        "pencil",
        "eraser",
        "fill",
        "line",
        "rectangle",
        "ellipse",
        "eyedropper",
        "selection",
        "lasso",
        "move",
        "transform",
        "text",
        "hand",
        "zoom",
        "ai_brush",
    ]
    .into_iter()
    .map(ToolId)
    .collect()
}

/// The Animate workspace: editing the sprite over time. Reuses the shared editing
/// panels by id (bible rule 2); owns layout only, no data.
pub struct AnimateWorkspace;

impl Workspace for AnimateWorkspace {
    fn id(&self) -> WorkspaceId {
        ANIMATE
    }

    fn meta(&self) -> WorkspaceMeta {
        WorkspaceMeta {
            name: "Animate",
            icon: icons::ANIMATE,
            purpose: "Animate the sprite across frames",
            shortcut: KeyboardShortcut::new(Modifiers::COMMAND, Key::Num2),
        }
    }

    fn layout(&self) -> WorkspaceLayout {
        WorkspaceLayout {
            right_dock: vec![LAYERS, SPRITES, FRAMES, CLIP_PROPERTIES, AI_ANIM_ASSISTANT],
            bottom_tray: vec![TIMELINE, FRAMES, CONSOLE],
            primary_tools: full_rail(),
            default_tool: PENCIL,
            status_items: vec![
                StatusItem {
                    icon: icons::FRAMES,
                    text: "15 frames".to_owned(),
                },
                StatusItem {
                    icon: icons::EYE,
                    text: "Onion Skin Off".to_owned(),
                },
                StatusItem {
                    icon: icons::STATUS_DOT,
                    text: "12 FPS".to_owned(),
                },
            ],
        }
    }
}

/// The Clip Properties dock panel. Mock content: the current clip's frame range,
/// FPS, an inert Loop checkbox, and the export name as a read-only label (only the
/// Prompt panel owns a scratch buffer, so the name is shown, not editable here).
pub struct ClipPropertiesPanel;

impl Panel for ClipPropertiesPanel {
    fn id(&self) -> PanelId {
        CLIP_PROPERTIES
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "Clip Properties",
            icon: icons::TIMELINE,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        widgets::mock_row(ui, theme, "Clip: jump");
        widgets::mock_row(ui, theme, "Frames 8-15");
        widgets::mock_row(ui, theme, "FPS 12");
        // Inert mock control; the value resets each frame (drives nothing).
        let mut looping = false;
        ui.checkbox(&mut looping, "Loop");
        ui.horizontal(|ui| {
            widgets::mock_row(ui, theme, "Export name");
            ui.label(egui::RichText::new("bit_jump").color(theme.roles.text_primary));
        });
    }
}

/// The AI Animation Assistant dock panel: the UX quick-action list. Each row is a
/// full-width button pushing a distinct `RunAction` intent (mock job this round).
/// The header is sparkle-marked in the AI accent.
pub struct AiAnimationAssistantPanel;

impl Panel for AiAnimationAssistantPanel {
    fn id(&self) -> PanelId {
        AI_ANIM_ASSISTANT
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "AI Animation Assistant",
            icon: icons::SPARKLE,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let actions = [
            ("In-between frames", ANIM_INBETWEEN),
            ("Extend animation", ANIM_EXTEND),
            ("Clean frames", ANIM_CLEAN),
            ("Reduce colors", ANIM_REDUCE),
            ("Create variations", ANIM_VARIATIONS),
        ];
        for (label, action) in actions {
            if ui.add_sized([ui.available_width(), 24.0], egui::Button::new(label)).clicked() {
                scope.ctx.intents.push(Intent::RunAction(action));
            }
        }
    }
}

/// The Timeline tray panel: the four-band animation timeline drawn with a Painter.
/// Bands top-to-bottom: Playback controls, Animation clips, the frame ruler with
/// the violet playhead, and Layer tracks. All content is mock.
pub struct TimelinePanel;

impl Panel for TimelinePanel {
    fn id(&self) -> PanelId {
        TIMELINE
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "Timeline",
            icon: icons::TIMELINE,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    // The frame count, band partitions, and radius token are small bounded
    // constants; the f32 casts of loop indices and `theme.radius.sm` cannot
    // truncate or lose a sign.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;

        // Band 1 - Playback: real widgets in a horizontal row (interactive controls).
        ui.horizontal(|ui| {
            let _ = ui.button(icons::PLAY.to_string());
            let _ = ui.button(icons::PREV.to_string());
            let _ = ui.button(icons::NEXT.to_string());
            ui.label("100ms");
            ui.label("1.00x");
            ui.label("12 FPS");
            let mut looping = false;
            ui.checkbox(&mut looping, "Loop");
        });

        // Bands 2-4 are painted: clips, the frame ruler + playhead, layer tracks.
        let desired = Vec2::new(ui.available_width(), 120.0);
        let (rect, _resp) = ui.allocate_exact_size(desired, Sense::hover());
        let painter = ui.painter_at(rect);
        let band_h = rect.height() / 3.0;
        let label_size = theme.type_scale.label;

        // Band 2 - Animation clips: each clip is a distinct colored span from the
        // `mock.clips` hue set. The clip name is painted in a near-black ink (the
        // clip hues are mid-to-light, so dark text reads; these are decorative mock
        // labels, not role-gated by the WCAG floors).
        let clips_top = rect.top();
        let clips = ["idle", "walk", "run", "jump", "attack"];
        let clip_w = rect.width() / clips.len() as f32;
        let clip_ink = egui::Color32::from_rgb(0x14, 0x12, 0x18);
        for (i, name) in clips.iter().enumerate() {
            let x = rect.left() + i as f32 * clip_w;
            let span = egui::Rect::from_min_size(egui::pos2(x + 2.0, clips_top + 2.0), Vec2::new(clip_w - 4.0, band_h - 4.0));
            painter.rect_filled(span, theme.radius.sm, theme.mock.clips[i % theme.mock.clips.len()]);
            painter.text(
                span.left_center() + Vec2::new(5.0, 0.0),
                Align2::LEFT_CENTER,
                name,
                FontId::proportional(label_size),
                clip_ink,
            );
        }

        // Band 3 - Frame ruler 0..14 with the violet playhead at frame 11.
        let ruler_top = clips_top + band_h;
        let frames = 15;
        let frame_w = rect.width() / frames as f32;
        for f in 0..frames {
            let x = rect.left() + f as f32 * frame_w;
            painter.line_segment(
                [egui::pos2(x, ruler_top), egui::pos2(x, ruler_top + band_h)],
                Stroke::new(1.0, theme.roles.border),
            );
            painter.text(
                egui::pos2(x + 2.0, ruler_top + 2.0),
                Align2::LEFT_TOP,
                f.to_string(),
                FontId::monospace(label_size),
                theme.roles.text_secondary,
            );
        }
        // The selected frame cell (frame 11) drawn with a violet outline.
        let sel = egui::Rect::from_min_size(egui::pos2(rect.left() + 11.0 * frame_w, ruler_top), Vec2::new(frame_w, band_h));
        painter.rect_stroke(sel, theme.radius.sm, Stroke::new(1.5, theme.accent.base), egui::StrokeKind::Inside);
        // The playhead at frame 11, spanning the ruler and the tracks below.
        let playhead_x = rect.left() + 11.0 * frame_w;
        painter.line_segment(
            [egui::pos2(playhead_x, ruler_top), egui::pos2(playhead_x, rect.bottom())],
            Stroke::new(2.0, theme.accent.base),
        );

        // Band 4 - Layer tracks: a labeled row per track over an alternating band,
        // with a label gutter so the name never collides with the first cell. Each
        // track's keyed cells are tinted in that track's clip color; the rest read
        // as empty wells. The active-frame column (11) gets a muted-accent wash.
        let tracks_top = ruler_top + band_h;
        let tracks = ["Body", "Effects", "Shadow"];
        let track_h = band_h / tracks.len() as f32;
        let label_gutter = 52.0;
        for (i, track) in tracks.iter().enumerate() {
            let y = tracks_top + i as f32 * track_h;
            // Alternating row band so the tracks read as distinct lanes.
            let row = egui::Rect::from_min_size(egui::pos2(rect.left(), y), Vec2::new(rect.width(), track_h));
            let band = if i % 2 == 0 { theme.surfaces.elevated } else { theme.surfaces.panel };
            painter.rect_filled(row, 0.0, band);
            painter.text(
                egui::pos2(rect.left() + 4.0, y + track_h / 2.0),
                Align2::LEFT_CENTER,
                *track,
                FontId::proportional(label_size),
                theme.roles.text_secondary,
            );
            // Keyed cells for this track, starting after the label gutter. A cell is
            // "keyed" on a stride offset per track so the rows differ; keyed cells
            // carry the track's clip color, the rest a faint inset well.
            let track_color = theme.mock.clips[i % theme.mock.clips.len()];
            let cell_area_left = rect.left() + label_gutter;
            let cell_w = (rect.width() - label_gutter) / frames as f32;
            for f in 0..frames {
                let cx = cell_area_left + f as f32 * cell_w;
                let cell = egui::Rect::from_min_size(egui::pos2(cx + 1.0, y + 1.0), Vec2::new(cell_w - 2.0, track_h - 2.0));
                let keyed = (f + i) % 3 == 0;
                let fill = if keyed { track_color } else { theme.surfaces.inset };
                painter.rect_filled(cell, 0.0, fill);
            }
        }
    }
}

/// Register the Animate workspace, the Clip Properties / AI Animation Assistant
/// dock panels, the Timeline tray panel, the AI-animation actions, and the Frame
/// menu group.
///
/// The shared Layers/Sprites/Frames/Console panels are owned by sprite-edit and
/// referenced by id, not re-registered. The Frame menu's items reference the
/// `frame.*` actions sprite-edit already registers, so this fn only registers the
/// new `anim.*` AI-animation actions.
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_workspace(Box::new(AnimateWorkspace));

    host.add_panel(Box::new(ClipPropertiesPanel));
    host.add_panel(Box::new(AiAnimationAssistantPanel));
    host.add_panel(Box::new(TimelinePanel));

    // The AI-animation quick-actions the assistant panel dispatches.
    for (id, label) in [
        (ANIM_INBETWEEN, "In-between frames"),
        (ANIM_EXTEND, "Extend animation"),
        (ANIM_CLEAN, "Clean frames"),
        (ANIM_REDUCE, "Reduce colors"),
        (ANIM_VARIATIONS, "Create variations"),
    ] {
        host.add_action(ActionDesc {
            id,
            label,
            icon: icons::SPARKLE,
            palette_visible: true,
        });
    }

    // The Frame menu group. Its items reference sprite-edit's already-registered
    // frame.* actions (menu groups are an ordered Vec, so adding one never collides).
    host.add_menu_group(MenuGroup {
        label: "Frame",
        items: vec![
            MenuItem {
                label: "Add Frame",
                shortcut: None,
                action: FRAME_ADD,
            },
            MenuItem {
                label: "Duplicate Frame",
                shortcut: None,
                action: FRAME_DUPLICATE,
            },
            MenuItem {
                label: "Delete Frame",
                shortcut: None,
                action: FRAME_DELETE,
            },
        ],
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animate_reuses_shared_panels_by_id() {
        let layout = AnimateWorkspace.layout();
        assert_eq!(layout.right_dock, vec![LAYERS, SPRITES, FRAMES, CLIP_PROPERTIES, AI_ANIM_ASSISTANT]);
        assert_eq!(layout.bottom_tray, vec![TIMELINE, FRAMES, CONSOLE]);
        assert_eq!(layout.default_tool, PENCIL);
        assert_eq!(layout.primary_tools.len(), 15);
        assert_eq!(layout.status_items.len(), 3);
        assert_eq!(layout.status_items[0].text, "15 frames");
    }

    #[test]
    fn animate_meta_uses_cmd_2() {
        assert_eq!(AnimateWorkspace.id(), ANIMATE);
        assert_eq!(AnimateWorkspace.meta().name, "Animate");
        assert_eq!(AnimateWorkspace.meta().shortcut, KeyboardShortcut::new(Modifiers::COMMAND, Key::Num2));
    }

    #[test]
    fn full_rail_is_the_fifteen_tools_in_order() {
        let rail = full_rail();
        assert_eq!(rail.len(), 15);
        assert_eq!(rail[0], PENCIL);
        assert_eq!(rail[14], ToolId("ai_brush"));
    }

    #[test]
    fn animation_panel_ids_and_regions() {
        assert_eq!(ClipPropertiesPanel.id(), CLIP_PROPERTIES);
        assert_eq!(AiAnimationAssistantPanel.id(), AI_ANIM_ASSISTANT);
        assert_eq!(TimelinePanel.id(), TIMELINE);
        assert_eq!(ClipPropertiesPanel.meta().default_region, Region::RightDock);
        assert_eq!(AiAnimationAssistantPanel.meta().default_region, Region::RightDock);
        assert_eq!(TimelinePanel.meta().default_region, Region::BottomTray);
    }
}
