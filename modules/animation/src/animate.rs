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
    ActionDesc, ActionId, HostRegistrar, MenuGroup, MenuItem, MsgKey, PENCIL, Panel, PanelId, PanelMeta, PanelScope, StatusItem, TOOL_RAIL, Workspace,
    WorkspaceId, WorkspaceLayout, WorkspaceMeta,
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

/// The Animate workspace: editing the sprite over time. Reuses the shared editing
/// panels by id (bible rule 2); owns layout only, no data.
pub struct AnimateWorkspace;

impl Workspace for AnimateWorkspace {
    fn id(&self) -> WorkspaceId {
        ANIMATE
    }

    fn meta(&self) -> WorkspaceMeta {
        WorkspaceMeta {
            name: MsgKey("workspace.animate.title"),
            icon: icons::ANIMATE,
            purpose: MsgKey("workspace.animate.purpose"),
            shortcut: KeyboardShortcut::new(Modifiers::COMMAND, Key::Num2),
        }
    }

    fn layout(&self) -> WorkspaceLayout {
        WorkspaceLayout {
            right_dock: vec![LAYERS, SPRITES, FRAMES, CLIP_PROPERTIES, AI_ANIM_ASSISTANT],
            bottom_tray: vec![TIMELINE, FRAMES, CONSOLE],
            primary_tools: TOOL_RAIL.to_vec(),
            default_tool: PENCIL,
            // The live frame count and fps now read off the Timeline panel (driven by
            // the active sprite); the status bar keeps only the onion-skin chrome
            // rather than the old hardcoded "15 frames"/"12 FPS" placeholders.
            status_items: vec![StatusItem {
                icon: icons::EYE,
                text: MsgKey("workspace.animate.status.onion-skin").tr(),
            }],
        }
    }
}

/// The Clip Properties dock panel. Reads the selected (or first) clip from the
/// read-only playback mirror and shows its name, frame range, and fps. The Loop
/// checkbox stays inert this round - `loop_mode` is a durable clip property, so
/// wiring it is a follow-up `Command`, not transient view state.
pub struct ClipPropertiesPanel;

impl Panel for ClipPropertiesPanel {
    fn id(&self) -> PanelId {
        CLIP_PROPERTIES
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.clip-properties.title"),
            icon: icons::TIMELINE,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        let playback = &scope.ctx.session.playback;
        let selected = scope.ctx.ui_state.playback.clip;
        // The selected clip if still valid, else the sprite's first clip.
        let clip = selected
            .and_then(|id| playback.clips.iter().find(|c| c.id == id))
            .or_else(|| playback.clips.first());
        if let Some(clip) = clip {
            widgets::mock_row(ui, theme, &format!("Clip: {}", clip.name));
            widgets::mock_row(ui, theme, &format!("Frames {}-{}", clip.start, clip.end));
            widgets::mock_row(ui, theme, &format!("FPS {}", clip.fps));
            // Inert this round: loop_mode is a durable clip property (a follow-up command).
            let mut looping = false;
            ui.checkbox(&mut looping, "Loop");
        } else {
            ui.label(egui::RichText::new("No clip on the active sprite.").color(theme.roles.text_secondary));
        }
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
            title: MsgKey("panel.ai-animation-assistant.title"),
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

/// The Timeline tray panel: the four-band animation timeline. Band 1 (transport),
/// Band 2 (the sprite's real clips), and Band 3 (the frame ruler + live playhead)
/// are driven by the read-only playback mirror and push playback intents; Band 4
/// (layer tracks) is still decorative - per-layer cel data is not modeled yet.
pub struct TimelinePanel;

impl Panel for TimelinePanel {
    fn id(&self) -> PanelId {
        TIMELINE
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.timeline.title"),
            icon: icons::TIMELINE,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    // Frame indices and band partitions are small bounded values; the f32 casts of
    // loop indices, the click hit-test, and the radius token cannot truncate or lose
    // a sign in practice (frame counts are small). The four painted bands plus the
    // hit-test make one cohesive draw routine, hence the line-count allow.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss, clippy::too_many_lines)]
    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        // Copy the mirror's scalars and clone its (tiny) clip rows up front, so the
        // render holds no borrow of `scope` and can push a collected intent at the end.
        let playing = scope.ctx.ui_state.playback.playing;
        let selected_clip = scope.ctx.ui_state.playback.clip;
        let playable = scope.ctx.session.playback.playable;
        let frame_count = scope.ctx.session.playback.frame_count;
        let range_start = scope.ctx.session.playback.range_start;
        let range_fps = scope.ctx.session.playback.range_fps;
        let playhead_offset = scope.ctx.session.playback.playhead_offset;
        let clips = scope.ctx.session.playback.clips.clone();
        // At most one click per frame, applied after the render (the panel reads
        // `scope.ctx` here, so it cannot also push mid-render).
        let mut intent: Option<Intent> = None;

        // Band 1 - transport. Play/pause, stop, step a frame; disabled when nothing plays.
        ui.horizontal(|ui| {
            let play_glyph = if playing { icons::PAUSE } else { icons::PLAY };
            if ui.add_enabled(playable, egui::Button::new(play_glyph.to_string())).clicked() {
                intent = Some(Intent::TogglePlayback);
            }
            if ui.add_enabled(playable, egui::Button::new(icons::STOP.to_string())).clicked() {
                intent = Some(Intent::StopPlayback);
            }
            if ui.add_enabled(playable, egui::Button::new(icons::PREV.to_string())).clicked() {
                intent = Some(Intent::ScrubToFrame(playhead_offset.saturating_sub(1)));
            }
            if ui.add_enabled(playable, egui::Button::new(icons::NEXT.to_string())).clicked() {
                intent = Some(Intent::ScrubToFrame(playhead_offset.saturating_add(1)));
            }
            ui.label(format!("{frame_count} frames"));
            ui.label(format!("{range_fps} FPS"));
        });

        // A still or empty sprite has nothing to scrub: show a hint, skip the bands.
        if !playable {
            ui.label(
                egui::RichText::new("No animation to play - insert an animated sprite from Generate.")
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_secondary),
            );
            if let Some(intent) = intent {
                scope.ctx.intents.push(intent);
            }
            return;
        }

        // Bands 2-4 paint on one clickable rect: a click in the clips band selects a
        // clip; a click in the ruler band scrubs the playhead.
        let desired = Vec2::new(ui.available_width(), 120.0);
        let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
        let painter = ui.painter_at(rect);
        let band_h = rect.height() / 3.0;
        let label_size = theme.type_scale.label;
        let frames = frame_count.max(1);
        let frame_w = rect.width() / frames as f32;

        // Band 2 - the sprite's real clips, each a span over its frame range; the
        // selected clip gets an accent outline. Clip names are mid-light, so dark ink.
        let clips_top = rect.top();
        let clip_ink = theme.mock.clip_ink;
        for (i, clip) in clips.iter().enumerate() {
            let start = clip.start.min(frames - 1);
            let end = clip.end.min(frames - 1);
            let span_x = rect.left() + (start as f32 / frames as f32) * rect.width();
            let span_w = (((end.saturating_sub(start) + 1) as f32 / frames as f32) * rect.width() - 4.0).max(2.0);
            let span = egui::Rect::from_min_size(egui::pos2(span_x + 2.0, clips_top + 2.0), Vec2::new(span_w, band_h - 4.0));
            painter.rect_filled(span, theme.radius.sm, theme.mock.clips[i % theme.mock.clips.len()]);
            if selected_clip == Some(clip.id) {
                painter.rect_stroke(span, theme.radius.sm, Stroke::new(1.5, theme.accent.base), egui::StrokeKind::Inside);
            }
            painter.text(
                span.left_center() + Vec2::new(5.0, 0.0),
                Align2::LEFT_CENTER,
                &clip.name,
                FontId::proportional(label_size),
                clip_ink,
            );
        }

        // Band 3 - the frame ruler with the live playhead at range_start + offset.
        let ruler_top = clips_top + band_h;
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
        let playhead_frame = range_start.saturating_add(playhead_offset).min(frames - 1);
        let sel = egui::Rect::from_min_size(egui::pos2(rect.left() + playhead_frame as f32 * frame_w, ruler_top), Vec2::new(frame_w, band_h));
        painter.rect_stroke(sel, theme.radius.sm, Stroke::new(1.5, theme.accent.base), egui::StrokeKind::Inside);
        let playhead_x = rect.left() + playhead_frame as f32 * frame_w;
        painter.line_segment(
            [egui::pos2(playhead_x, ruler_top), egui::pos2(playhead_x, rect.bottom())],
            Stroke::new(2.0, theme.accent.base),
        );

        // Band 4 - decorative layer tracks over the real frame count. Per-layer cel
        // data is not modeled yet, so the keyed cells are a placeholder (follow-up).
        let tracks_top = ruler_top + band_h;
        let tracks = ["Body", "Effects", "Shadow"];
        let track_h = band_h / tracks.len() as f32;
        let label_gutter = 52.0;
        for (i, track) in tracks.iter().enumerate() {
            let y = tracks_top + i as f32 * track_h;
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
            let track_color = theme.mock.clips[i % theme.mock.clips.len()];
            let cell_area_left = rect.left() + label_gutter;
            let cell_w = (rect.width() - label_gutter) / frames as f32;
            for f in 0..frames {
                let cx = cell_area_left + f as f32 * cell_w;
                let cell = egui::Rect::from_min_size(egui::pos2(cx + 1.0, y + 1.0), Vec2::new(cell_w - 2.0, track_h - 2.0));
                let keyed = (f as usize + i) % 3 == 0;
                let fill = if keyed { track_color } else { theme.surfaces.inset };
                painter.rect_filled(cell, 0.0, fill);
            }
        }

        // A click scrubs (ruler band) or selects a clip (clips band).
        if resp.clicked()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let local_y = pos.y - rect.top();
            if local_y < band_h {
                for clip in &clips {
                    let start = clip.start.min(frames - 1);
                    let end = clip.end.min(frames - 1);
                    let span_x = rect.left() + (start as f32 / frames as f32) * rect.width();
                    let span_w = ((end.saturating_sub(start) + 1) as f32 / frames as f32) * rect.width();
                    if pos.x >= span_x && pos.x < span_x + span_w {
                        intent = Some(Intent::SelectClip(Some(clip.id)));
                        break;
                    }
                }
            } else if local_y < band_h * 2.0 {
                let frame = (((pos.x - rect.left()) / frame_w).floor().max(0.0) as u32).min(frames - 1);
                intent = Some(Intent::ScrubToFrame(frame.saturating_sub(range_start)));
            }
        }

        if let Some(intent) = intent {
            scope.ctx.intents.push(intent);
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
        (ANIM_INBETWEEN, MsgKey("command.anim.in-between-frames")),
        (ANIM_EXTEND, MsgKey("command.anim.extend-animation")),
        (ANIM_CLEAN, MsgKey("command.anim.clean-frames")),
        (ANIM_REDUCE, MsgKey("command.anim.reduce-colors")),
        (ANIM_VARIATIONS, MsgKey("command.anim.create-variations")),
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
        label: MsgKey("app.menu.frame"),
        items: vec![
            MenuItem {
                label: MsgKey("command.frame.add"),
                shortcut: None,
                action: FRAME_ADD,
            },
            MenuItem {
                label: MsgKey("command.frame.duplicate"),
                shortcut: None,
                action: FRAME_DUPLICATE,
            },
            MenuItem {
                label: MsgKey("command.frame.delete"),
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
        assert_eq!(layout.status_items.len(), 1, "the fake frame/FPS status items moved to the live timeline");
        assert_eq!(layout.status_items[0].text, MsgKey("workspace.animate.status.onion-skin").tr());
    }

    #[test]
    fn animate_meta_uses_cmd_2() {
        assert_eq!(AnimateWorkspace.id(), ANIMATE);
        assert_eq!(AnimateWorkspace.meta().name, MsgKey("workspace.animate.title"));
        assert_eq!(AnimateWorkspace.meta().shortcut, KeyboardShortcut::new(Modifiers::COMMAND, Key::Num2));
    }

    #[test]
    fn animate_uses_the_canonical_tool_rail() {
        // Animate draws the same shared rail every workspace consumes, in order.
        assert_eq!(AnimateWorkspace.layout().primary_tools, TOOL_RAIL.to_vec());
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
