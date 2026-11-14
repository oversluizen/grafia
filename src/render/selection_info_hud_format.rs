//! Text formatting for the contextual Grey Info Box HUD (`SPEC-007` & `SPEC-009`).

use crate::core::{GuideAlignment, NodeType, SelectionInfo, TransformGridAnchor};

fn anchor_short_code(anchor: TransformGridAnchor) -> &'static str {
    match anchor {
        TransformGridAnchor::TopLeft => "TL",
        TransformGridAnchor::TopCenter => "TC",
        TransformGridAnchor::TopRight => "TR",
        TransformGridAnchor::MiddleLeft => "ML",
        TransformGridAnchor::Center => "C",
        TransformGridAnchor::MiddleRight => "MR",
        TransformGridAnchor::BottomLeft => "BL",
        TransformGridAnchor::BottomCenter => "BC",
        TransformGridAnchor::BottomRight => "BR",
    }
}

/// Formats the primary and secondary HUD text lines for any `SelectionInfo` mode.
#[must_use]
pub fn format_selection_info_hud_lines(info: &SelectionInfo) -> (String, String) {
    match info {
        SelectionInfo::GlyphBase(b) => format_glyph_base_hud_lines(b),
        SelectionInfo::SingleOnCurve(s) => format_single_oncurve_hud_lines(s),
        SelectionInfo::SingleOffCurve(h) => (
            format!(
                "OFFCURVE  X:{:.0}  Y:{:.0}  DX:{:.0}  DY:{:.0}",
                h.x, h.y, h.delta_x, h.delta_y
            ),
            format!("L:{:.1}  ANCHOR:{}", h.length, h.tethered_oncurve_id.0),
        ),
        SelectionInfo::MultiNode(m) => {
            let prop = if m.proportional_scale { "ON" } else { "OFF" };
            let code = anchor_short_code(m.origin_anchor);
            (
                format!(
                    "X:{:.0}  Y:{:.0}  W:{:.0}  H:{:.0}",
                    m.origin_x, m.origin_y, m.width, m.height
                ),
                format!(
                    "SEL:{}/{}  PROP:{prop}  REF:{code}",
                    m.selected_node_count, m.total_layer_node_count
                ),
            )
        }
        SelectionInfo::Guide(g) => format_guide_hud_lines(g),
        SelectionInfo::HobbyNode(h) => (
            format!("HOBBY  X:{:.0}  Y:{:.0}  ID:{}", h.x, h.y, h.node_id.0),
            format!(
                "TIN:{:.2}  TOUT:{:.2}  DIR:{:?}",
                h.tension_in, h.tension_out, h.dir_constraint
            ),
        ),
        SelectionInfo::BackgroundImage(bg) => {
            let lock = if bg.locked { "ON" } else { "OFF" };
            (
                format!(
                    "IMAGE  X:{:.0}  Y:{:.0}  SX:{:.0}%  SY:{:.0}%",
                    bg.x, bg.y, bg.scale_x_pct, bg.scale_y_pct
                ),
                format!(
                    "ROT:{:.1}  ALPHA:{:.2}  LOCK:{lock}",
                    bg.rotation_deg, bg.alpha
                ),
            )
        }
        other => format_color_or_hinting_hud_lines(other),
    }
}

fn format_color_or_hinting_hud_lines(info: &SelectionInfo) -> (String, String) {
    match info {
        SelectionInfo::GradientStop {
            sublayer_index,
            stop_index,
            offset_pct,
            rgba,
        } => (
            format!("GRAD STOP  LAYER:{sublayer_index}  STOP:{stop_index}  POS:{offset_pct:.1}%"),
            format!("RGBA:{},{},{},{}", rgba[0], rgba[1], rgba[2], rgba[3]),
        ),
        SelectionInfo::PostScriptHint {
            hint_id,
            horizontal,
            origin_upm,
            width_upm,
            is_ghost,
            ..
        } => {
            let axis = if *horizontal { "H" } else { "V" };
            let ghost = if *is_ghost { "GHOST" } else { "STEM" };
            (
                format!("PS {ghost} ({axis})  ID:{hint_id}"),
                format!("POS:{origin_upm:.1}  W:{width_upm:.1}"),
            )
        }
        SelectionInfo::TrueTypeInstruction {
            instruction_id,
            kind,
            ppem,
            round_mode,
            stem_index,
            zone_index,
        } => (
            format!("TT {kind:?}  ID:{instruction_id}  PPEM:{ppem}"),
            format!("RND:{round_mode:?}  STEM:{stem_index:?}  ZONE:{zone_index:?}"),
        ),
        SelectionInfo::HoiTiming {
            c1_x,
            c1_y,
            c2_x,
            c2_y,
        } => (
            format!("HOI TIMING  C1:({c1_x:.2},{c1_y:.2})"),
            format!("C2:({c2_x:.2},{c2_y:.2})"),
        ),
        _ => (String::new(), String::new()),
    }
}

fn format_glyph_base_hud_lines(b: &crate::core::GlyphBaseInfo) -> (String, String) {
    let name = if b.glyph_name.is_empty() {
        "GLYPH"
    } else {
        b.glyph_name.as_str()
    };
    let line1 = format!(
        "{name}  LSB:{:.0}  RSB:{:.0}  W:{:.0}  BBOX:{:.0}x{:.0}",
        b.lsb, b.rsb, b.advance_width, b.bbox_w, b.bbox_h
    );
    let kl = b
        .kern_with_prev
        .map_or_else(|| "-".to_string(), |v| v.to_string());
    let kr = b
        .kern_with_next
        .map_or_else(|| "-".to_string(), |v| v.to_string());
    let line2 = format!(
        "KL:{kl}  KR:{kr}  C:{}  N:{}",
        b.contour_count, b.total_node_count
    );
    (line1, line2)
}

fn format_single_oncurve_hud_lines(s: &crate::core::SingleOnCurveInfo) -> (String, String) {
    let kind = match s.node_type {
        NodeType::Corner => "CORNER",
        NodeType::Smooth => "SMOOTH",
        NodeType::OffCurve => "OFFCURVE",
        NodeType::HobbyStar { .. } => "HOBBY",
        NodeType::EulerSpiro { .. } => "EULER",
    };
    (
        format!("ONCURVE  X:{:.0}  Y:{:.0}", s.x, s.y),
        format!("TYPE:{kind}  ID:{}", s.node_id.0),
    )
}

fn format_guide_hud_lines(g: &crate::core::GuideInfo) -> (String, String) {
    let align = match g.alignment {
        GuideAlignment::Lsb => "LSB",
        GuideAlignment::Center => "CTR",
        GuideAlignment::Rsb => "RSB",
    };
    let lock = if g.lock_angle { "ON" } else { "OFF" };
    let meas = if g.show_measurement { "ON" } else { "OFF" };
    (
        format!("GUIDE  X:{:.0}  Y:{:.0}  ANG:{:.1}", g.x, g.y, g.angle_deg),
        format!("ALIGN:{align}  LOCK:{lock}  MEAS:{meas}"),
    )
}
