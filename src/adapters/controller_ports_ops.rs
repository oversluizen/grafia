//! `SPEC-015`: Universal System-Agnostic Controller Ports ("Stekkers") on `GrafiaCanvasState`:
//! 1. `CanvasIntent` & `CanvasIntentMask` (Semantic Modifier Flags),
//! 3. `ControllerTelemetryDescriptor` (`controller_telemetry` for OLEDs, LCDs & `egui-macview` PieMenu),
//! 4. `CanvasHapticEvent` (`drain_haptic_events` for Taptic Engine, Pencil Pro & Trackpad),
//! 5. Mobile/Stylus Precision Ports (`InputDeviceKind`, `StylusAccessoryEvent`, `PrecisionLoupeGeometry`).

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::math_consts::{
    BRUSH_MAX_RADIUS_DP, BRUSH_MIN_RADIUS_DP, LOUPE_OFFSET_DP, LOUPE_RADIUS_DP,
    LOUPE_TOP_EDGE_FLIP_THRESHOLD_DP, LOUPE_ZOOM_FACTOR, TOUCH_HIT_MIN_FLOOR_DP,
    TOUCH_HIT_RADIUS_MULTIPLIER,
};
use crate::core::{
    CanvasHapticEvent, CanvasIntent, CanvasIntentMask, ControllerTelemetryDescriptor,
    InputDeviceKind, IntentTelemetryItem, PrecisionLoupeGeometry, ScreenPoint,
    StylusAccessoryEvent,
};
use crate::interaction::{Modifiers, PenToolState, SelectToolState};

impl GrafiaCanvasState {
    pub(crate) fn push_haptic_event(&mut self, event: CanvasHapticEvent) {
        self.drawing_tool_state
            .controller_ports
            .pending_haptic_events
            .push(event);
    }

    /// Sets the momentary (held) external [`CanvasIntentMask`] (`SPEC-015 §2`).
    pub fn set_external_intents(&mut self, intents: CanvasIntentMask) {
        self.drawing_tool_state.controller_ports.external_intents = intents;
    }

    /// Latches or unlatches a single [`CanvasIntent`] for one-handed or toggle operation (`SPEC-015 §2`).
    pub fn set_intent_latched(&mut self, intent: CanvasIntent, latched: bool) {
        let was_latched = self
            .drawing_tool_state
            .controller_ports
            .latched_intents
            .contains(intent);
        if latched {
            self.drawing_tool_state
                .controller_ports
                .latched_intents
                .insert(intent);
        } else {
            self.drawing_tool_state
                .controller_ports
                .latched_intents
                .remove(intent);
        }
        if was_latched != latched {
            self.push_haptic_event(CanvasHapticEvent::IntentLatchToggled { intent, latched });
        }
    }

    /// Returns whether `intent` is currently latched (`SPEC-015 §2`).
    #[must_use]
    pub fn is_intent_latched(&self, intent: CanvasIntent) -> bool {
        self.drawing_tool_state
            .controller_ports
            .latched_intents
            .contains(intent)
    }

    /// Clears all momentary and latched external [`CanvasIntent`] flags (`SPEC-015 §2`).
    pub fn clear_external_intents(&mut self) {
        self.drawing_tool_state.controller_ports.external_intents = CanvasIntentMask::NONE;
        self.drawing_tool_state.controller_ports.latched_intents = CanvasIntentMask::NONE;
    }

    /// Computes the union of external intents, latched intents, stylus barrel hold, and hardware modifiers (`SPEC-015 §2`).
    #[must_use]
    pub fn effective_intents(&self, hw_modifiers: Modifiers, hw_space: bool) -> CanvasIntentMask {
        let cp = &self.drawing_tool_state.controller_ports;
        let mut mask = cp.external_intents.union(cp.latched_intents);
        if cp.barrel_hold_active {
            mask.insert(CanvasIntent::BreakCuspHandle);
        }
        if hw_modifiers.shift {
            mask.insert(CanvasIntent::ConstrainAngle);
            mask.insert(CanvasIntent::LockAspectRatio);
        }
        if hw_modifiers.option {
            mask.insert(CanvasIntent::OriginFromCenter);
            mask.insert(CanvasIntent::BreakCuspHandle);
            mask.insert(CanvasIntent::DuplicateOnDrag);
            mask.insert(CanvasIntent::KnifeAddNodesOnly);
        }
        if hw_modifiers.control || hw_modifiers.command {
            mask.insert(CanvasIntent::SlideAlongCurve);
        }
        if (hw_modifiers.control || hw_modifiers.command) && hw_modifiers.option {
            mask.insert(CanvasIntent::ShearTransform);
        }
        if hw_space || hw_modifiers.space {
            mask.insert(CanvasIntent::MoveOriginOrPan);
        }
        mask
    }

    #[must_use]
    pub(crate) fn resolve_modifiers_with_intents(
        &self,
        hw_modifiers: Modifiers,
        hw_space: bool,
    ) -> Modifiers {
        let eff = self.effective_intents(hw_modifiers, hw_space);
        let shift = hw_modifiers.shift
            || eff.contains(CanvasIntent::ConstrainAngle)
            || eff.contains(CanvasIntent::LockAspectRatio);
        let option = hw_modifiers.option
            || eff.contains(CanvasIntent::OriginFromCenter)
            || eff.contains(CanvasIntent::BreakCuspHandle)
            || eff.contains(CanvasIntent::DuplicateOnDrag)
            || eff.contains(CanvasIntent::KnifeAddNodesOnly)
            || eff.contains(CanvasIntent::ShearTransform);
        let control = hw_modifiers.control
            || eff.contains(CanvasIntent::SlideAlongCurve)
            || eff.contains(CanvasIntent::ShearTransform);
        let command = hw_modifiers.command;
        let space = hw_space || hw_modifiers.space || eff.contains(CanvasIntent::MoveOriginOrPan);
        Modifiers {
            shift,
            option,
            command,
            control,
            space,
        }
    }

    /// Drains all pending [`CanvasHapticEvent`] items emitted since the last poll (`SPEC-015 §5`).
    pub fn drain_haptic_events(&mut self) -> Vec<CanvasHapticEvent> {
        std::mem::take(
            &mut self
                .drawing_tool_state
                .controller_ports
                .pending_haptic_events,
        )
    }

    /// Returns a live [`ControllerTelemetryDescriptor`] for OLED displays, LCD buttons, and radial HUDs (`SPEC-015 §4`).
    #[must_use]
    pub fn controller_telemetry(&self) -> ControllerTelemetryDescriptor {
        let eff = self.effective_intents(
            self.drag_transients.last_modifiers,
            self.drag_transients.last_modifiers.space,
        );
        let specs: &[(CanvasIntent, &'static str, &'static str)] = match self.active_tool {
            CanvasTool::Pen => &[
                (CanvasIntent::ConstrainAngle, "∠ 45°", "Shift"),
                (CanvasIntent::BreakCuspHandle, "∧ Cusp", "Alt"),
                (CanvasIntent::MoveOriginOrPan, "✥ Move Pt", "Space"),
            ],
            CanvasTool::Primitives | CanvasTool::Scale => &[
                (CanvasIntent::LockAspectRatio, "1:1", "Shift"),
                (CanvasIntent::OriginFromCenter, "⊙ Center", "Alt"),
                (CanvasIntent::ShearTransform, "⇄ Shear", "Ctrl+Alt"),
                (CanvasIntent::MoveOriginOrPan, "✥ Move", "Space"),
            ],
            CanvasTool::Knife => &[
                (CanvasIntent::ConstrainAngle, "∠ 15°", "Shift"),
                (CanvasIntent::KnifeAddNodesOnly, "⊕ Nodes", "Alt"),
                (CanvasIntent::MoveOriginOrPan, "✋ Pan", "Space"),
            ],
            _ => &[
                (CanvasIntent::ConstrainAngle, "∠ Axis", "Shift"),
                (CanvasIntent::DuplicateOnDrag, "⧉ Clone", "Alt"),
                (CanvasIntent::SlideAlongCurve, "↝ Slide", "Ctrl"),
                (CanvasIntent::ShearTransform, "⇄ Shear", "Ctrl+Alt"),
                (CanvasIntent::MoveOriginOrPan, "✋ Pan", "Space"),
            ],
        };
        let available_intents = specs
            .iter()
            .map(
                |&(intent, short_label, shortcut_hint)| IntentTelemetryItem {
                    intent,
                    short_label,
                    shortcut_hint,
                    active: eff.contains(intent),
                },
            )
            .collect();
        ControllerTelemetryDescriptor {
            active_tool: self.active_tool,
            available_intents,
            primary_rotary: self.compute_primary_rotary_telemetry(),
        }
    }

    /// Sets the active physical input device classification (`Mouse`, `Stylus`, `TouchFinger`, `SPEC-015 §6`).
    pub fn set_active_input_device(&mut self, device: InputDeviceKind) {
        self.drawing_tool_state.controller_ports.active_input_device = device;
    }

    /// Returns the active [`InputDeviceKind`] (`SPEC-015 §6`).
    #[must_use]
    pub fn active_input_device(&self) -> InputDeviceKind {
        self.drawing_tool_state.controller_ports.active_input_device
    }

    /// Returns the effective brush radius in screen `dp` for `Trowel` and `Cleaner`.
    #[must_use]
    pub fn effective_brush_radius_dp(&self) -> f32 {
        self.drawing_tool_state
            .controller_ports
            .brush_radius_dp
            .clamp(BRUSH_MIN_RADIUS_DP, BRUSH_MAX_RADIUS_DP)
    }

    /// Returns the effective node hit-test radius in `dp`, inflating by `1.6x` (floor `14.0 dp`) for `TouchFinger` (`SPEC-015 §6`).
    #[must_use]
    pub fn effective_hit_radius_dp(&self) -> f32 {
        let base = self.preferences.hit_test_radius_dp;
        if self.active_input_device() == InputDeviceKind::TouchFinger {
            (base * TOUCH_HIT_RADIUS_MULTIPLIER).max(TOUCH_HIT_MIN_FLOOR_DP)
        } else {
            base
        }
    }

    /// Resolves the effective pointer screen coordinate with finger-offset applied when `TouchFinger` is active (`SPEC-015 §6`).
    #[must_use]
    pub fn resolve_effective_pointer_screen_pos(&self, raw_pos: ScreenPoint) -> ScreenPoint {
        if self.active_input_device() == InputDeviceKind::TouchFinger
            || !self.drag_transients.touch_state.contacts.is_empty()
            || self.drag_transients.touch_state.egui_any_touches_in_frame
        {
            ScreenPoint::new(
                raw_pos.x.0,
                raw_pos.y.0 - self.preferences.touch_finger_offset_dp,
            )
        } else {
            raw_pos
        }
    }

    /// Computes the edge-flipping magnification loupe geometry for `canvas_rect` (`SPEC-015 §6`).
    #[must_use]
    pub fn compute_precision_loupe_overlay(
        &self,
        canvas_rect: egui::Rect,
    ) -> Option<PrecisionLoupeGeometry> {
        if !self.preferences.show_touch_precision_loupe {
            return None;
        }
        let target_screen_pos = self.cursor_screen_pos.or(self.last_pointer_screen_pos)?;
        let target_upm_pos = self.active_slot_viewport().to_font(target_screen_pos);
        let pad = LOUPE_RADIUS_DP;
        let min_x = canvas_rect.left() + pad;
        let max_x = (canvas_rect.right() - pad).max(min_x);
        let min_y = canvas_rect.top() + pad;
        let max_y = (canvas_rect.bottom() - pad).max(min_y);
        let flipped_from_top_edge =
            (target_screen_pos.y.0 - canvas_rect.top()) < LOUPE_TOP_EDGE_FLIP_THRESHOLD_DP;
        let raw_cy = if flipped_from_top_edge {
            target_screen_pos.y.0 + LOUPE_OFFSET_DP
        } else {
            target_screen_pos.y.0 - LOUPE_OFFSET_DP
        };
        let cx = target_screen_pos.x.0.clamp(min_x, max_x);
        let cy = raw_cy.clamp(min_y, max_y);
        Some(PrecisionLoupeGeometry {
            target_screen_pos,
            target_upm_pos,
            lens_center_screen_pos: ScreenPoint::new(cx, cy),
            lens_radius_dp: LOUPE_RADIUS_DP,
            zoom_factor: LOUPE_ZOOM_FACTOR,
            flipped_from_top_edge,
        })
    }

    /// Cleanly rolls back any uncommitted 1-finger drag or tentative `Pen` node placement on 1-to-2 finger promotion (`SPEC-015 §6`).
    pub fn promote_to_multitouch_gesture(&mut self) -> bool {
        let Some(before) = self.drag_transients.active_drag_before_snapshot.take() else {
            return false;
        };
        self.restore_snapshot(&before);
        self.select_state = SelectToolState::Idle;
        self.pen_state = PenToolState::Idle;
        self.drawing_tool_state.reset_in_flight_drags();
        self.active_snappers.clear();
        self.curve_harmony.active_tangent_snap = None;
        self.drag_transients.touch_state.two_finger_peak = true;
        true
    }

    /// Dispatches stylus hardware events (`DoubleTap`, `Squeeze`, `BarrelHold`, `SPEC-015 §6`).
    pub fn dispatch_stylus_accessory_event(&mut self, event: StylusAccessoryEvent) -> bool {
        match event {
            StylusAccessoryEvent::DoubleTap => {
                if self.active_tool == CanvasTool::Select {
                    let prev = self.drawing_tool_state.controller_ports.last_drawing_tool;
                    let target = if prev == CanvasTool::Select {
                        CanvasTool::Pen
                    } else {
                        prev
                    };
                    self.activate_tool(target);
                } else {
                    self.drawing_tool_state.controller_ports.last_drawing_tool = self.active_tool;
                    self.activate_tool(CanvasTool::Select);
                }
                true
            }
            StylusAccessoryEvent::Squeeze { pos } => {
                self.cursor_screen_pos = Some(pos);
                self.last_pointer_screen_pos = Some(pos);
                true
            }
            StylusAccessoryEvent::BarrelHold(held) => {
                self.drawing_tool_state.controller_ports.barrel_hold_active = held;
                true
            }
        }
    }
}
