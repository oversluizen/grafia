//! 4-Zone Liquid Glass macOS Sonoma / AppKit Toolbar (`Zone A: Studio & Creatie`,
//! `Zone B: Gecentreerde Showcase-Pil`, `Zone C: Weergave, Overlays & Vensters`, `SPEC-018 §3.7`).

use super::super::VIEW_MODE_SPECS;
pub use super::toolbar_specs::{StudioSubWorkspace, StudioWorkspaceMode, TOOL_CATEGORIES};
use super::toolbar_specs::{
    CASE_POPUP_WIDTH_DP, CHAPTER_POPUP_WIDTH_DP, CORE_STUDIO_TOOLS, GLASS_CAPSULE_CORNER_RADIUS_U8,
    GLASS_CAPSULE_PAD_X_F32, GLASS_CAPSULE_PAD_Y_F32, GLASS_HEADER_FONT_PT,
    MOBILE_RESPONSIVE_BREAKPOINT_WIDTH_DP, TOOL_GRID_SPACING_X_DP, TOOL_GRID_SPACING_Y_DP,
};
use super::{DemoPreset, MockFontHost, ShowcaseChapter};
use egui::{CornerRadius, Frame, Grid, Margin, RichText, Stroke, Ui};
use egui_macview::{MacPopUpButton, MacSegmentedControl, MacSymbol, MacTheme};
use grafia::adapters::{CanvasTool, GrafiaCanvasState};
use grafia::core::{PrimitiveSubTool, SymbolColorRenderingMode};
use grafia::tokens::GrafiaTheme;

pub fn liquid_glass_capsule_frame(tm: &MacTheme) -> Frame {
    Frame::NONE
        .fill(tm.control_background())
        .corner_radius(CornerRadius::same(GLASS_CAPSULE_CORNER_RADIUS_U8))
        .stroke(Stroke::new(1.0, tm.specular_rim_top()))
        .inner_margin(Margin::symmetric(
            GLASS_CAPSULE_PAD_X_F32 as i8,
            GLASS_CAPSULE_PAD_Y_F32 as i8,
        ))
}

pub fn section_header_text(label: &str, tm: &MacTheme) -> RichText {
    RichText::new(label)
        .size(GLASS_HEADER_FONT_PT)
        .color(tm.secondary_label())
        .strong()
}

pub fn render_top_toolbar(
    ui: &mut Ui,
    state: &mut GrafiaCanvasState,
    theme: &mut GrafiaTheme,
    host: &mut MockFontHost,
    show_preview_panel: &mut bool,
) {
    let tm = host.active_mac_theme();
    if ui.available_width() < MOBILE_RESPONSIVE_BREAKPOINT_WIDTH_DP {
        ui.horizontal_wrapped(|ui| {
            liquid_glass_capsule_frame(&tm).show(ui, |ui| {
                render_zone_a_workspace_switcher(ui, state, theme, host, &tm);
                render_zone_b_showcase_pill(ui, state, theme, host, &tm);
                host.render_zone_c_overlays_and_windows(ui, state, theme, show_preview_panel, &tm);
            });
        });
        ui.horizontal_wrapped(|ui| {
            liquid_glass_capsule_frame(&tm).show(ui, |ui| {
                render_zone_a_core_tools(ui, state, host, &tm);
                render_zone_c_view_or_color_mode(ui, state, host, &tm);
            });
        });
    } else {
        ui.horizontal(|ui| {
            liquid_glass_capsule_frame(&tm).show(ui, |ui| {
                render_zone_a_workspace_switcher(ui, state, theme, host, &tm);
            });
            liquid_glass_capsule_frame(&tm).show(ui, |ui| {
                render_zone_a_core_tools(ui, state, host, &tm);
                render_zone_b_showcase_pill(ui, state, theme, host, &tm);
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |r_ui| {
                liquid_glass_capsule_frame(&tm).show(r_ui, |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |l_ui| {
                        render_zone_c_view_or_color_mode(l_ui, state, host, &tm);
                        host.render_zone_c_overlays_and_windows(
                            l_ui,
                            state,
                            theme,
                            show_preview_panel,
                            &tm,
                        );
                    });
                });
            });
        });
    }
}

fn render_zone_a_workspace_switcher(
    ui: &mut Ui,
    state: &mut GrafiaCanvasState,
    theme: &mut GrafiaTheme,
    host: &mut MockFontHost,
    tm: &MacTheme,
) {
    let mut sb_on = [host.show_left_sidebar];
    if MacSegmentedControl::new(&[MacSymbol::SidebarLeading.as_str()], 0)
        .id_salt("zone_a_sb_toggle")
        .theme(tm)
        .show_multi(ui, &mut sb_on)
        .is_some()
    {
        host.show_left_sidebar = sb_on[0];
    }
    let ws_idx = usize::from(host.studio_mode == StudioWorkspaceMode::IconStudio);
    MacSegmentedControl::new(&["\u{f892} Aa", "\u{f77e} ❖"], ws_idx)
        .id_salt("zone_a_ws_seg")
        .theme(tm)
        .show(ui, |idx| {
            let sub = if idx == 1 {
                StudioSubWorkspace::IconVectorAndGlass
            } else {
                StudioSubWorkspace::LetterContourAndSpacing
            };
            host.activate_sub_workspace(sub, state, theme);
        });
    let tabs: &[StudioSubWorkspace] = if host.studio_mode == StudioWorkspaceMode::LetterStudio {
        &StudioSubWorkspace::LETTER_TABS
    } else {
        &StudioSubWorkspace::ICON_TABS
    };
    let labels: Vec<&str> = tabs.iter().map(|t| t.tab_label()).collect();
    let cur = tabs
        .iter()
        .position(|&w| w == host.active_workspace)
        .unwrap_or(0);
    MacSegmentedControl::new(&labels, cur)
        .id_salt("zone_a_sub_ws_tabs")
        .theme(tm)
        .show(ui, |idx| {
            host.activate_sub_workspace(tabs[idx], state, theme);
        });
}

fn render_zone_a_core_tools(
    ui: &mut Ui,
    state: &mut GrafiaCanvasState,
    host: &MockFontHost,
    tm: &MacTheme,
) {
    let ws_tools = host.active_workspace.tools();
    let _ = CORE_STUDIO_TOOLS;
    let sel_idx = ws_tools
        .iter()
        .position(|(t, _)| *t == state.active_tool)
        .unwrap_or(0);
    let icons: [&str; 6] = ws_tools.map(|(_, i)| i);
    MacSegmentedControl::new(&icons, sel_idx)
        .id_salt("zone_a_tools_seg")
        .theme(tm)
        .show(ui, |idx| {
            let (tool, _) = ws_tools[idx];
            if tool == CanvasTool::Primitives && state.active_tool == CanvasTool::Primitives {
                state.drawing_tool_state.primitive_sub_tool =
                    if state.drawing_tool_state.primitive_sub_tool == PrimitiveSubTool::Rectangle {
                        PrimitiveSubTool::Circle
                    } else {
                        PrimitiveSubTool::Rectangle
                    };
            }
            state.activate_tool(tool);
        });
    ui.menu_button("•••", |ui| {
        Grid::new("tool_categories_2x2_card")
            .num_columns(2)
            .spacing([TOOL_GRID_SPACING_X_DP, TOOL_GRID_SPACING_Y_DP])
            .show(ui, |card_ui| {
                for (cat_idx, (cat_title, tools)) in TOOL_CATEGORIES.iter().enumerate() {
                    card_ui.vertical(|col_ui| {
                        col_ui.label(section_header_text(cat_title, tm));
                        super::glyph_info_card_ui::render_tool_category_grid(
                            col_ui, cat_idx, tools, state,
                        );
                    });
                    if cat_idx % 2 == 1 {
                        card_ui.end_row();
                    }
                }
            });
    })
    .response
    .on_hover_text("Alle 20 gereedschappen met S-Pen / muis-uitlegballonnetjes & sneltoetsen");
}

fn render_zone_b_showcase_pill(
    ui: &mut Ui,
    st: &mut GrafiaCanvasState,
    theme: &mut GrafiaTheme,
    host: &mut MockFontHost,
    tm: &MacTheme,
) {
    MacSegmentedControl::new(&["◀"], 0)
        .id_salt("zone_b_prev_btn")
        .theme(tm)
        .show_momentary(ui, |_| host.step_tour_prev(st, theme));
    let ch_labels: [&str; 6] = ShowcaseChapter::ALL.map(ShowcaseChapter::short_pill);
    let mut ch_idx = ShowcaseChapter::ALL
        .iter()
        .position(|&c| c == host.active_chapter)
        .unwrap_or(0);
    let prev_ch = ch_idx;
    MacPopUpButton::new(&mut ch_idx, &ch_labels)
        .id("zone_b_chapter_pop")
        .width(CHAPTER_POPUP_WIDTH_DP)
        .theme(tm)
        .show(ui);
    if ch_idx != prev_ch {
        host.active_chapter = ShowcaseChapter::ALL[ch_idx];
        host.apply_preset(host.active_chapter.presets()[0], st, theme);
    }
    let ch_presets = host.active_chapter.presets();
    let case_labels: [&str; 6] = ch_presets.map(|p| super::coach_card(p).short_button);
    let active_preset = DemoPreset::ALL[host.tour_index.min(DemoPreset::ALL.len() - 1)].0;
    let mut case_idx = ch_presets
        .iter()
        .position(|&p| p == active_preset)
        .unwrap_or(0);
    let prev_case = case_idx;
    MacPopUpButton::new(&mut case_idx, &case_labels)
        .id("zone_b_case_pop")
        .width(CASE_POPUP_WIDTH_DP)
        .theme(tm)
        .show(ui);
    if case_idx != prev_case {
        host.apply_preset(ch_presets[case_idx], st, theme);
    }
    MacSegmentedControl::new(&["▶"], 0)
        .id_salt("zone_b_next_btn")
        .theme(tm)
        .show_momentary(ui, |_| host.step_tour_next(st, theme));
}

fn render_zone_c_view_or_color_mode(
    ui: &mut Ui,
    st: &mut GrafiaCanvasState,
    host: &MockFontHost,
    tm: &MacTheme,
) {
    if host.studio_mode == StudioWorkspaceMode::IconStudio {
        let cur_cm = (0..4)
            .find(|&i| {
                let (_, m) = SymbolColorRenderingMode::preset_by_index(i);
                std::mem::discriminant(&st.icon_studio.color_mode) == std::mem::discriminant(&m)
            })
            .unwrap_or(1);
        MacSegmentedControl::new(&["\u{f7f8}", "\u{f7ff}", "\u{f80d}", "\u{f77e}"], cur_cm)
            .id_salt("zone_c_icon_cm_seg")
            .theme(tm)
            .show(ui, |idx| {
                if idx == 3 {
                    st.view_mode = VIEW_MODE_SPECS[3].0;
                } else {
                    st.view_mode = VIEW_MODE_SPECS[0].0;
                    let (_, m) = SymbolColorRenderingMode::preset_by_index(idx);
                    st.set_symbol_color_rendering_mode(m);
                }
            });
    } else {
        let specs = [
            VIEW_MODE_SPECS[0],
            VIEW_MODE_SPECS[1],
            VIEW_MODE_SPECS[3],
            VIEW_MODE_SPECS[2],
        ];
        let icons = specs.map(|s| s.2);
        let cur = specs.iter().position(|s| s.0 == st.view_mode).unwrap_or(0);
        MacSegmentedControl::new(&icons, cur)
            .id_salt("zone_c_view_seg")
            .theme(tm)
            .show(ui, |idx| st.view_mode = specs[idx].0);
    }
}
