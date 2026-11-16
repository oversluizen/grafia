//! `SPEC-006` (`grafia v0.5.0`) keyboard and pointer interaction handlers for
//! `CanvasTool::Text`, interactive spacing (`LSB` / `RSB` / contour shift), and kerning.

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::{
    CanvasHostRequest, FontPoint, ScreenPoint, TextStreamEditKind, TextStreamEditRequest,
};
use crate::interaction::Modifiers;
use crate::tokens::{
    TEXT_MEASUREMENT_LINE_HIT_RADIUS_DP, TEXT_MEASUREMENT_LINE_HIT_TOL_UPM,
    TEXT_SELECTION_DRAG_THRESHOLD_DP,
};
use egui::{Event, Key};

/// Dispatches `SPEC-006` text-stream keyboard events (`T`, `Escape`, `Home`, `End`, spacing/kerning arrows, `Event::Text`).
pub fn dispatch_text_stream_keyboard_shortcut(
    state: &mut GrafiaCanvasState,
    event: &Event,
    is_shift: bool,
    is_alt: bool,
    is_cmd: bool,
    is_ctrl: bool,
) -> bool {
    if let Event::Text(text) = event {
        if state.active_tool == CanvasTool::Text && !is_cmd && !is_ctrl && !is_alt {
            if state.text_transients.text_tool_activated_in_frame && text.eq_ignore_ascii_case("t")
            {
                state.text_transients.text_tool_activated_in_frame = false;
                return true;
            }
            return state.insert_text_at_caret(text) > 0;
        }
        return false;
    }

    let Event::Key {
        key,
        pressed: true,
        modifiers,
        ..
    } = event
    else {
        return false;
    };

    let win_ctrl_only = modifiers.ctrl && modifiers.command && !modifiers.mac_cmd;
    let (shift, alt, ctrl) = (
        modifiers.shift || is_shift,
        modifiers.alt || is_alt,
        modifiers.ctrl || is_ctrl,
    );
    let cmd = !win_ctrl_only && (modifiers.command || modifiers.mac_cmd || is_cmd);
    let key_mods = crate::adapters::keyboard::KeyModifierFlags {
        shift,
        alt,
        cmd,
        ctrl,
    };

    if try_handle_text_tool_toggle_or_escape(state, *key, key_mods) {
        return true;
    }

    let in_text_tool = state.active_tool == CanvasTool::Text;
    let has_slots = !state.text_stream.slots.is_empty();

    if matches!(key, Key::Home | Key::End)
        && (in_text_tool || (has_slots && state.selected_nodes.is_empty()))
    {
        return state.step_active_slot(if *key == Key::Home { -1 } else { 1 });
    }

    if (in_text_tool
        || (has_slots && state.selected_nodes.is_empty() && state.selected_guide.is_none()))
        && state.dispatch_vertical_text_stream_key(*key, in_text_tool, key_mods)
    {
        return true;
    }

    if matches!(key, Key::ArrowLeft | Key::ArrowRight)
        && (in_text_tool
            || (has_slots && state.selected_nodes.is_empty() && state.selected_guide.is_none()))
        && handle_text_stream_horizontal_arrow_key(state, *key, in_text_tool, key_mods)
    {
        return true;
    }

    in_text_tool && handle_in_text_tool_edit_key(state, *key, cmd || ctrl, shift, alt)
}

fn try_handle_text_tool_toggle_or_escape(
    state: &mut GrafiaCanvasState,
    key: Key,
    mods: crate::adapters::keyboard::KeyModifierFlags,
) -> bool {
    let crate::adapters::keyboard::KeyModifierFlags { alt, cmd, ctrl, .. } = mods;
    if key == Key::T
        && !cmd
        && !ctrl
        && !alt
        && state
            .drag_transients
            .keymap
            .chord_for(crate::tokens::CanvasActionId::TextTool)
            == Some(crate::tokens::KeyChord::plain(Key::T))
    {
        if state.active_tool != CanvasTool::Text {
            state.text_transients.last_select_tool = state.active_tool;
        }
        state.text_transients.text_tool_activated_in_frame = true;
        state.activate_tool(CanvasTool::Text);
        return true;
    }
    if key == Key::Escape && state.active_tool == CanvasTool::Text {
        state.text_stream.selection_range = None;
        let prev = state.text_transients.last_select_tool;
        state.activate_tool(if prev == CanvasTool::Text {
            CanvasTool::Select
        } else {
            prev
        });
        return true;
    }
    false
}

fn handle_in_text_tool_edit_key(
    state: &mut GrafiaCanvasState,
    key: Key,
    cmd_or_ctrl: bool,
    shift: bool,
    alt: bool,
) -> bool {
    match key {
        Key::A if cmd_or_ctrl && !shift && !alt => {
            let n = state.text_stream.slots.len();
            if n > 0 {
                state.text_stream.selection_range = Some((0, n));
                state.text_stream.caret_index = n;
            }
            true
        }
        Key::Backspace => state.delete_text_stream_slots(true),
        Key::Delete => state.delete_text_stream_slots(false),
        _ => false,
    }
}

fn handle_text_stream_horizontal_arrow_key(
    state: &mut GrafiaCanvasState,
    key: Key,
    in_text_tool: bool,
    mods: crate::adapters::keyboard::KeyModifierFlags,
) -> bool {
    let crate::adapters::keyboard::KeyModifierFlags {
        shift,
        alt,
        cmd,
        ctrl,
    } = mods;
    let sign = if key == Key::ArrowLeft { -1 } else { 1 };
    let sp_low = (state.effective_spacing_step_upm(false).round() as i32).max(1);
    let sp_high = (state.effective_spacing_step_upm(true).round() as i32).max(1);
    let kn_low = (state.effective_kerning_step_upm(false).round() as i32).max(1);
    let kn_high = (state.effective_kerning_step_upm(true).round() as i32).max(1);
    if ctrl || cmd {
        let sp_step = if shift { sp_high } else { sp_low } * sign;
        let kn_step = if shift { kn_high } else { kn_low } * sign;
        return match (ctrl, cmd, alt) {
            (true, false, false) => state.adjust_active_lsb(sp_step),
            (false, true, false) => state.adjust_active_rsb(sp_step),
            (true, true, false) => state.shift_active_contours_in_width(sp_step),
            (true, false, true) => state.adjust_left_kerning(kn_step),
            (false, true, true) => state.adjust_right_kerning(kn_step),
            _ => false,
        };
    }
    if alt {
        let sp_step = if shift { sp_high } else { sp_low } * sign;
        return state.adjust_active_rsb(sp_step);
    }
    if in_text_tool {
        let n = state.text_stream.slots.len();
        let cur = state.text_stream.caret_index as isize;
        let next = (cur + sign as isize).clamp(0, n as isize) as usize;
        if shift {
            let anchor = state.text_stream.selection_range.map_or(
                state.text_stream.caret_index,
                |(s, e)| {
                    if state.text_stream.caret_index == e {
                        s
                    } else {
                        e
                    }
                },
            );
            let (s, e) = (anchor.min(next), anchor.max(next));
            if n > 0 {
                state.switch_active_slot(next.min(n - 1));
            }
            state.text_stream.caret_index = next;
            state.text_stream.selection_range = (s < e).then_some((s, e));
        } else {
            state.set_caret_index(next);
        }
        return true;
    }
    false
}

/// Handles pointer press in `CanvasTool::Text` or double-click on passive slots (`SPEC-006 §2.1 & §2.2`).
pub fn handle_text_stream_pointer_press(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
    mods: Modifiers,
    is_double: bool,
) -> Option<bool> {
    if is_double && !state.text_stream.slots.is_empty() {
        if let Some(slot_idx) = state.text_stream.resolve_slot_at_upm(font_pt) {
            if state.active_tool == CanvasTool::Text
                || slot_idx != state.text_stream.active_slot_index
            {
                return Some(state.handle_double_click_upm(font_pt));
            }
        }
    }
    if (state.show_text_measurement_line || state.text_transients.temp_show_text_measurement_line)
        && matches!(state.active_tool, CanvasTool::Text | CanvasTool::Select)
    {
        let line_scr_y = state
            .viewport
            .to_screen(FontPoint::new(0.0, state.text_measurement_y_upm))
            .y
            .0;
        if (scr_pt.y.0 - line_scr_y).abs() <= TEXT_MEASUREMENT_LINE_HIT_RADIUS_DP
            || (font_pt.y.0 - state.text_measurement_y_upm).abs()
                <= TEXT_MEASUREMENT_LINE_HIT_TOL_UPM
        {
            state.text_measurement_dragging = true;
            state.set_text_measurement_y(font_pt.y.0);
            return Some(true);
        }
    }
    if state.active_tool != CanvasTool::Text {
        return None;
    }
    if mods.command && mods.control && mods.option {
        state.text_measurement_dragging = true;
        state.set_text_measurement_y(font_pt.y.0);
        return Some(true);
    }
    if mods.control || mods.command {
        state.text_transients.text_drag_anchor_caret =
            Some((usize::MAX, ScreenPoint::new(font_pt.x.0 as f32, 0.0)));
        return Some(true);
    }
    let anchor = state.text_stream.resolve_caret_index_at_upm(font_pt);
    state.place_text_caret_at_upm(font_pt, mods.shift);
    state.text_transients.text_drag_anchor_caret = Some((anchor, scr_pt));
    Some(true)
}

/// Handles pointer move in `CanvasTool::Text` (`SPEC-006 §2.2.2, §2.3.3 & §2.5.1`).
pub fn handle_text_stream_pointer_move(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
) -> Option<bool> {
    if state.text_measurement_dragging {
        state.set_text_measurement_y(font_pt.y.0);
        return Some(true);
    }
    if state.active_tool != CanvasTool::Text {
        return None;
    }
    if let Some((anchor_caret, start_scr)) = state.text_transients.text_drag_anchor_caret {
        if anchor_caret == usize::MAX {
            let last_x = f64::from(start_scr.x.0);
            let delta = (font_pt.x.0 - last_x).round() as i32;
            if delta != 0 {
                let next_x = (last_x + f64::from(delta)) as f32;
                state.text_transients.text_drag_anchor_caret =
                    Some((usize::MAX, ScreenPoint::new(next_x, 0.0)));
                let m = state.drag_transients.last_modifiers;
                if m.control && m.option && !m.command {
                    state.adjust_left_kerning(delta);
                } else if m.command && m.option && !m.control {
                    state.adjust_right_kerning(delta);
                } else if m.control && !m.command && !m.option {
                    state.adjust_active_lsb(delta);
                } else if m.command && !m.control && !m.option {
                    state.adjust_active_rsb(delta);
                } else if m.command && m.control && !m.option {
                    state.shift_active_contours_in_width(delta);
                }
            }
            return Some(true);
        }
        if scr_pt.distance_to(start_scr) >= TEXT_SELECTION_DRAG_THRESHOLD_DP {
            let curr = state.text_stream.resolve_caret_index_at_upm(font_pt);
            let s = anchor_caret.min(curr);
            let e = anchor_caret.max(curr);
            let n = state.text_stream.slots.len();
            if n > 0 {
                state.switch_active_slot(curr.min(n - 1));
            }
            state.text_stream.caret_index = curr;
            state.text_stream.selection_range = if s < e { Some((s, e)) } else { None };
        }
        return Some(true);
    }
    None
}

/// Handles pointer release in `CanvasTool::Text` or active scanline drag.
pub fn handle_text_stream_pointer_release(state: &mut GrafiaCanvasState) -> Option<bool> {
    if state.text_measurement_dragging {
        state.text_measurement_dragging = false;
        state.text_transients.text_drag_anchor_caret = None;
        return Some(true);
    }
    if state.active_tool != CanvasTool::Text {
        return None;
    }
    state.text_measurement_dragging = false;
    state.text_transients.text_drag_anchor_caret = None;
    Some(true)
}

impl GrafiaCanvasState {
    /// Emits `CanvasHostRequest::EditTextStream` with `TextStreamEditKind::InsertText` for characters typed at the caret (`SPEC-006 §2.2`).
    pub fn insert_text_at_caret(&mut self, text: &str) -> usize {
        let cleaned: String = text.chars().filter(|c| !c.is_control()).collect();
        let count = cleaned.chars().count();
        if count == 0 {
            return 0;
        }
        self.drag_transients
            .host_requests
            .push(CanvasHostRequest::EditTextStream(TextStreamEditRequest {
                caret_index: self.text_stream.caret_index,
                selection_range: self.text_stream.selection_range,
                edit: TextStreamEditKind::InsertText(cleaned),
            }));
        self.assert_invariants();
        count
    }

    /// Returns the horizontal pen origin `X_i` in UPM of the active slot (`0.0` when empty).
    #[must_use]
    pub fn active_slot_origin_x_upm(&self) -> f64 {
        self.text_stream
            .slots
            .get(self.text_stream.active_slot_index)
            .map_or(0.0, |s| s.origin_x_upm)
    }

    /// Returns the `ViewportTransform` anchored at the active slot's horizontal and vertical origin `(X_i, Y_i)`.
    #[must_use]
    pub fn active_slot_viewport(&self) -> crate::core::ViewportTransform {
        let mut vp = self
            .viewport
            .with_origin_x_upm(self.active_slot_origin_x_upm());
        let oy = self.active_slot_origin_y_upm();
        if oy.abs() > 0.0 {
            vp.pan_offset.y.0 -= (oy as f32) * vp.zoom;
        }
        vp
    }
    /// Loads a 5-slot canonical `"AonsA"` multi-glyph horizontal strip with negative and positive kerning pairs.
    pub fn load_demo_text_strip(&mut self) {
        let mut s_a = Self::init_glyph_a();
        s_a.correct_path_direction();
        let g_a = s_a.export_glyph();
        s_a.load_glyph_archetype(crate::tokens::GlyphArchetype::N);
        s_a.correct_path_direction();
        let g_n = s_a.export_glyph();
        s_a.load_glyph_archetype(crate::tokens::GlyphArchetype::O);
        s_a.correct_path_direction();
        let g_o = s_a.export_glyph();
        s_a.load_glyph_archetype(crate::tokens::GlyphArchetype::S);
        s_a.correct_path_direction();
        let g_s = s_a.export_glyph();
        self.set_text_stream_slots(
            vec![
                crate::core::GlyphSlot::new(g_a.clone(), 0),
                crate::core::GlyphSlot::new(g_o, crate::tokens::DEMO_STRIP_KERN_AO_UPM),
                crate::core::GlyphSlot::new(g_n, 0),
                crate::core::GlyphSlot::new(g_s, 0),
                crate::core::GlyphSlot::new(g_a, crate::tokens::DEMO_STRIP_KERN_SA_UPM),
            ],
            0,
        );
    }
}
