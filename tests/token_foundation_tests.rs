//! Anchoring tests for Milestone M1: Dependencies & Token Foundation.
//!
//! Normative References:
//! - STD-ARCH Regel 0: De Verankeringswet (Red then Green).
//! - STD-ARCH Regel 3 & Regel 12: Token-driven UI colors and geometry.

use approx::assert_relative_eq;
use grafia::core::{compute_signed_area, FontPoint, ScreenPoint, ViewportTransform};
use grafia::render::{PreviewFilledProjection, ShapeThumbnailProjection};
use grafia::tokens::{GlyphAGoldenMaster, GrafiaTheme};
use kurbo::{BezPath, Shape};

#[test]
fn test_grafia_theme_color32_helpers() {
    let theme = GrafiaTheme::sonoma_default();

    // Edit Canvas Token Helpers
    let corner = theme.edit_canvas.corner_node_color32();
    assert_eq!(corner, egui::Color32::from_rgb(0x2B, 0x78, 0xE4));

    let smooth = theme.edit_canvas.smooth_node_color32();
    assert_eq!(smooth, egui::Color32::from_rgb(0x3C, 0xAD, 0x66));

    let bcp_handle = theme.edit_canvas.bcp_handle_color32();
    assert_eq!(bcp_handle, egui::Color32::from_rgb(0x8C, 0x68, 0x45));

    let bcp_line = theme.edit_canvas.bcp_line_color32();
    assert_eq!(bcp_line, egui::Color32::from_rgb(0xB4, 0x9A, 0x80));

    let selected_glow = theme.edit_canvas.selected_glow_color32();
    assert_eq!(selected_glow, egui::Color32::from_rgb(0x00, 0x7A, 0xFF));

    let rubberband_fill = theme.edit_canvas.rubberband_fill_color32();
    assert_eq!(
        rubberband_fill,
        egui::Color32::from_rgba_unmultiplied(0x00, 0x7A, 0xFF, 0x26)
    );

    let rubberband_stroke = theme.edit_canvas.rubberband_stroke_color32();
    assert_eq!(rubberband_stroke, egui::Color32::from_rgb(0x00, 0x7A, 0xFF));

    // Preview Canvas Token Helpers
    let fill = theme.preview_canvas.fill_color32();
    assert_eq!(fill, egui::Color32::from_rgb(0x00, 0x00, 0x00));

    let baseline = theme.preview_canvas.baseline_color32();
    assert_eq!(baseline, egui::Color32::from_rgb(0xCC, 0xCC, 0xCC));

    // Shape Canvas Token Helpers
    let accent = theme.shape_canvas.accent_selection_color32();
    assert_eq!(accent, egui::Color32::from_rgb(0x00, 0x7A, 0xFF));

    let text_active = theme.shape_canvas.text_active_color32();
    assert_eq!(text_active, egui::Color32::from_rgb(0xFF, 0xFF, 0xFF));

    let text_inactive = theme.shape_canvas.text_inactive_color32();
    assert_eq!(text_inactive, egui::Color32::from_rgb(0x1C, 0x1C, 0x1E));
}

#[test]
fn test_glyph_a_golden_master_deserialization() {
    let master = GlyphAGoldenMaster::load();

    assert_eq!(master.glyphname, "A");
    assert_eq!(master.upm, 1000);
    assert_eq!(master.width, 700.0);
    assert_eq!(master.ascender, 800.0);
    assert_eq!(master.cap_height, 700.0);
    assert_eq!(master.x_height, 500.0);
    assert_eq!(master.descender, -200.0);

    // Verify outer contour: 8 points
    assert_eq!(master.outer_contour.len(), 8);
    let expected_outer = [
        (30.0, 0.0),
        (300.0, 700.0),
        (400.0, 700.0),
        (670.0, 0.0),
        (540.0, 0.0),
        (470.0, 180.0),
        (230.0, 180.0),
        (160.0, 0.0),
    ];
    for (i, &(ex, ey)) in expected_outer.iter().enumerate() {
        assert_eq!(
            (master.outer_contour[i].x, master.outer_contour[i].y),
            (ex, ey),
            "Outer contour point {i} mismatch"
        );
    }

    // Verify inner counter: 3 points
    assert_eq!(master.inner_counter.len(), 3);
    let expected_inner = [(260.0, 260.0), (440.0, 260.0), (350.0, 520.0)];
    for (i, &(ex, ey)) in expected_inner.iter().enumerate() {
        assert_eq!(
            (master.inner_counter[i].x, master.inner_counter[i].y),
            (ex, ey),
            "Inner counter point {i} mismatch"
        );
    }
}

#[test]
fn test_hex_to_color32_boundary_inputs() {
    use grafia::tokens::hex_to_color32;

    // 1. 3-digit #RGB (expands to RRGGBB)
    assert_eq!(
        hex_to_color32("#FFF"),
        egui::Color32::from_rgb(255, 255, 255)
    );
    assert_eq!(hex_to_color32("#000"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(
        hex_to_color32("#123"),
        egui::Color32::from_rgb(0x11, 0x22, 0x33)
    );
    assert_eq!(
        hex_to_color32("#abc"),
        egui::Color32::from_rgb(0xaa, 0xbb, 0xcc)
    );
    assert_eq!(
        hex_to_color32("#AbC"),
        egui::Color32::from_rgb(0xaa, 0xbb, 0xcc)
    );
    assert_eq!(
        hex_to_color32("123"),
        egui::Color32::from_rgb(0x11, 0x22, 0x33)
    ); // without #

    // 2. 4-digit #RGBA (expands to RRGGBBAA)
    assert_eq!(
        hex_to_color32("#FFFF"),
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 255)
    );
    assert_eq!(
        hex_to_color32("#0000"),
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 0)
    );
    assert_eq!(
        hex_to_color32("#1234"),
        egui::Color32::from_rgba_unmultiplied(0x11, 0x22, 0x33, 0x44)
    );
    assert_eq!(
        hex_to_color32("#abcd"),
        egui::Color32::from_rgba_unmultiplied(0xaa, 0xbb, 0xcc, 0xdd)
    );
    assert_eq!(
        hex_to_color32("1234"),
        egui::Color32::from_rgba_unmultiplied(0x11, 0x22, 0x33, 0x44)
    ); // without #

    // 3. 6-digit #RRGGBB
    assert_eq!(
        hex_to_color32("#FFFFFF"),
        egui::Color32::from_rgb(255, 255, 255)
    );
    assert_eq!(hex_to_color32("#000000"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(
        hex_to_color32("#2B78E4"),
        egui::Color32::from_rgb(0x2B, 0x78, 0xE4)
    );
    assert_eq!(
        hex_to_color32("#2b78e4"),
        egui::Color32::from_rgb(0x2B, 0x78, 0xE4)
    );
    assert_eq!(
        hex_to_color32("2B78E4"),
        egui::Color32::from_rgb(0x2B, 0x78, 0xE4)
    ); // without #
    assert_eq!(
        hex_to_color32("  #2B78E4  "),
        egui::Color32::from_rgb(0x2B, 0x78, 0xE4)
    ); // whitespace trimming
    assert_eq!(
        hex_to_color32("##2B78E4"),
        egui::Color32::from_rgb(0x2B, 0x78, 0xE4)
    ); // multiple #

    // 4. 8-digit #RRGGBBAA
    assert_eq!(
        hex_to_color32("#007AFF26"),
        egui::Color32::from_rgba_unmultiplied(0x00, 0x7A, 0xFF, 0x26)
    );
    assert_eq!(
        hex_to_color32("#007aff26"),
        egui::Color32::from_rgba_unmultiplied(0x00, 0x7A, 0xFF, 0x26)
    );
    assert_eq!(
        hex_to_color32("007AFF26"),
        egui::Color32::from_rgba_unmultiplied(0x00, 0x7A, 0xFF, 0x26)
    ); // without #

    // 5. Empty strings, whitespace, and bare hashes
    assert_eq!(hex_to_color32(""), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(hex_to_color32("   "), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(hex_to_color32("#"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(hex_to_color32("  #  "), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(hex_to_color32("###"), egui::Color32::from_rgb(0, 0, 0));

    // 6. Invalid lengths (must fallback safely to black without crashing)
    assert_eq!(hex_to_color32("1"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(hex_to_color32("#1"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(hex_to_color32("12"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(hex_to_color32("#12"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(hex_to_color32("12345"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(hex_to_color32("#12345"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(hex_to_color32("1234567"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(hex_to_color32("#1234567"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(
        hex_to_color32("123456789"),
        egui::Color32::from_rgb(0, 0, 0)
    );
    assert_eq!(
        hex_to_color32("#123456789"),
        egui::Color32::from_rgb(0, 0, 0)
    );

    // 7. Invalid hex characters (ASCII) - fallback to (0, 0, 0)
    assert_eq!(hex_to_color32("#GGGGGG"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(hex_to_color32("#ZZZ"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(hex_to_color32("#ZZZZ"), egui::Color32::from_rgb(0, 0, 0));
    assert_eq!(
        hex_to_color32("#ZZZZZZZZ"),
        egui::Color32::from_rgb(0, 0, 0)
    );
    assert_eq!(hex_to_color32("#2B78ZZ"), egui::Color32::from_rgb(0, 0, 0));
}

#[test]
fn test_hex_to_color32_valid_formats_channel_accuracy() {
    use grafia::tokens::hex_to_color32;

    // 1. 3-digit (#RGB) expansion and accuracy
    let c = hex_to_color32("#F0A");
    assert_eq!(c.r(), 0xFF);
    assert_eq!(c.g(), 0x00);
    assert_eq!(c.b(), 0xAA);
    assert_eq!(c.a(), 0xFF);

    // 2. 4-digit (#RGBA) unmultiplied channel integrity
    let c = hex_to_color32("#F0A8");
    assert_eq!(
        c,
        egui::Color32::from_rgba_unmultiplied(0xFF, 0x00, 0xAA, 0x88)
    );
    assert_eq!(c.a(), 0x88);

    // 3. 6-digit (#RRGGBB) channel accuracy
    let c = hex_to_color32("#2B78E4");
    assert_eq!(c.r(), 0x2B);
    assert_eq!(c.g(), 0x78);
    assert_eq!(c.b(), 0xE4);
    assert_eq!(c.a(), 0xFF);

    // 4. 8-digit (#RRGGBBAA) channel accuracy
    let c = hex_to_color32("#007AFF26");
    assert_eq!(
        c,
        egui::Color32::from_rgba_unmultiplied(0x00, 0x7A, 0xFF, 0x26)
    );
    assert_eq!(c.a(), 0x26);
    assert_eq!(c.r(), 0);
    assert_eq!(c.g(), 18); // premultiplied
    assert_eq!(c.b(), 38); // premultiplied
}

#[test]
fn test_hex_to_color32_utf8_multibyte_does_not_panic_and_returns_fallback() {
    use grafia::tokens::hex_to_color32;
    use std::panic::catch_unwind;

    // Hardened stress testing with non-ASCII multi-byte UTF-8 strings.
    // Byte slicing across UTF-8 code point boundaries must be prevented.
    // All non-ASCII or multi-byte strings must safely return fallback Color32::from_rgb(0, 0, 0) without panicking.
    let fallback = egui::Color32::from_rgb(0, 0, 0);

    let test_cases = [
        // 1. Previously failing inputs reported in M1 Challenge
        "€",      // 3 bytes (0xE2, 0x82, 0xAC)
        "#€",     // 1 + 3 bytes
        "😀",     // 4 bytes (0xF0, 0x9F, 0x98, 0x80)
        "#😀",    // 1 + 4 bytes
        "aébcd",  // 6 bytes (contains 2-byte 'é' at index 1..3)
        "#aébcd", // 1 + 6 bytes
        "😀😀",   // 8 bytes (two 4-byte emojis)
        "#😀😀",  // 1 + 8 bytes
        // 2. Arabic & RTL multi-byte scripts
        "مرحبا",   // Arabic greeting
        "العربية", // Arabic script
        "ألوان",   // Arabic "colors"
        "١٢٣٤٥٦",  // Arabic-Indic numerals
        "#مرحبا",
        // 3. Cyrillic (2 bytes per character)
        "Привет", // Cyrillic greeting (12 bytes)
        "мир",    // Cyrillic (6 bytes - matching 6-digit branch!)
        "абв",    // Cyrillic (6 bytes - matching 6-digit branch!)
        "абвг",   // Cyrillic (8 bytes - matching 8-digit branch!)
        "#абв",
        "#абвг",
        // 4. CJK (Chinese, Japanese, Korean - 3 bytes per character)
        "字",       // 3 bytes CJK (matching 3-digit branch!)
        "色",       // 3 bytes CJK
        "你好",     // 6 bytes CJK (matching 6-digit branch!)
        "中文测试", // 12 bytes CJK
        "日本語",   // 9 bytes CJK
        "한국어",   // 9 bytes CJK
        "#字",
        "#色",
        "#你好",
        // 5. Emojis, symbols, and zero-width/combining sequences
        "🦀",       // 4 bytes Rust crab emoji (matching 4-digit branch!)
        "🚀",       // 4 bytes emoji
        "🔥✨",     // 4 + 3 = 7 bytes
        "🎉🎊🎈🎨", // 16 bytes emojis
        "❤️",       // 3 + 3 = 6 bytes (heart + variation selector)
        "  #€  ",   // leading/trailing whitespace with 3-byte unicode
        // 6. Latin Extended and accented strings
        "aé",      // 3 bytes (1 + 2)
        "aébc",    // 5 bytes (1 + 2 + 1 + 1)
        "aébcde",  // 7 bytes
        "aébcdef", // 8 bytes (1 + 2 + 5 = 8 bytes - matching 8-digit branch!)
        "ééé",     // 6 bytes (three 2-byte chars - matching 6-digit branch!)
        "éééé",    // 8 bytes (four 2-byte chars - matching 8-digit branch!)
        "café",    // 5 bytes
        "naïve",   // 6 bytes (matching 6-digit branch!)
        "façade",  // 7 bytes
        "München", // 8 bytes (matching 8-digit branch!)
        "§±!@#$",  // assorted symbols
        "©®™",     // multi-byte legal symbols
        // 7. Mixed ASCII hex digits and multi-byte UTF-8 sequences
        "12€",     // 2 + 3 = 5 bytes
        "1€3",     // 1 + 3 + 1 = 5 bytes
        "1234€",   // 4 + 3 = 7 bytes
        "12345€",  // 5 + 3 = 8 bytes (matching 8-digit branch!)
        "12😀",    // 2 + 4 = 6 bytes (matching 6-digit branch!)
        "1234😀",  // 4 + 4 = 8 bytes (matching 8-digit branch!)
        "😀12",    // 4 + 2 = 6 bytes (matching 6-digit branch!)
        "😀1234",  // 4 + 4 = 8 bytes (matching 8-digit branch!)
        "é1234",   // 2 + 4 = 6 bytes (matching 6-digit branch!)
        "é123456", // 2 + 6 = 8 bytes (matching 8-digit branch!)
        "12é345",  // 2 + 2 + 3 = 7 bytes
        "123456é", // 6 + 2 = 8 bytes (matching 8-digit branch!)
        // 8. Special UTF-8 whitespace and control codes
        "\u{00A0}",         // Non-breaking space (2 bytes)
        "\u{200B}",         // Zero-width space (3 bytes)
        "\u{200C}",         // Zero-width non-joiner (3 bytes)
        "\u{200D}",         // Zero-width joiner (3 bytes)
        "\u{FEFF}",         // Zero-width no-break space / BOM (3 bytes)
        "\0",               // NUL byte
        "\0\0\0",           // 3 NUL bytes (matching 3-digit branch!)
        "\0\0\0\0",         // 4 NUL bytes (matching 4-digit branch!)
        "\0\0\0\0\0\0",     // 6 NUL bytes (matching 6-digit branch!)
        "\0\0\0\0\0\0\0\0", // 8 NUL bytes (matching 8-digit branch!)
        // 9. Non-hex ASCII patterns
        "xyz",
        "GGGGGG",
        "#ZZZZZZ",
        "12345G",
        "0x123",
        "#0x12",
    ];

    for &input in &test_cases {
        let res = catch_unwind(|| hex_to_color32(input));
        assert!(
            res.is_ok(),
            "hex_to_color32 must NOT panic on non-ASCII UTF-8 string: {:?}",
            input
        );
        let color = res.unwrap();
        assert_eq!(
            color, fallback,
            "hex_to_color32 must return fallback black (0, 0, 0) for non-ASCII input: {:?}",
            input
        );
    }
}

#[test]
fn test_sonoma_ground_truth_color_channels_exact() {
    let theme = GrafiaTheme::sonoma_default();

    // 1. Edit Canvas Tokens
    assert_eq!(theme.edit_canvas.corner_node_color, "#2B78E4");
    let c = theme.edit_canvas.corner_node_color32();
    assert_eq!(c.r(), 0x2B, "corner_node_color R channel mismatch");
    assert_eq!(c.g(), 0x78, "corner_node_color G channel mismatch");
    assert_eq!(c.b(), 0xE4, "corner_node_color B channel mismatch");
    assert_eq!(c.a(), 0xFF, "corner_node_color A channel mismatch");

    assert_eq!(theme.edit_canvas.smooth_node_color, "#3CAD66");
    let c = theme.edit_canvas.smooth_node_color32();
    assert_eq!(c.r(), 0x3C, "smooth_node_color R channel mismatch");
    assert_eq!(c.g(), 0xAD, "smooth_node_color G channel mismatch");
    assert_eq!(c.b(), 0x66, "smooth_node_color B channel mismatch");
    assert_eq!(c.a(), 0xFF, "smooth_node_color A channel mismatch");

    assert_eq!(theme.edit_canvas.bcp_handle_color, "#8C6845");
    let c = theme.edit_canvas.bcp_handle_color32();
    assert_eq!(c.r(), 0x8C, "bcp_handle_color R channel mismatch");
    assert_eq!(c.g(), 0x68, "bcp_handle_color G channel mismatch");
    assert_eq!(c.b(), 0x45, "bcp_handle_color B channel mismatch");
    assert_eq!(c.a(), 0xFF, "bcp_handle_color A channel mismatch");

    assert_eq!(theme.edit_canvas.bcp_line_color, "#B49A80");
    let c = theme.edit_canvas.bcp_line_color32();
    assert_eq!(c.r(), 0xB4, "bcp_line_color R channel mismatch");
    assert_eq!(c.g(), 0x9A, "bcp_line_color G channel mismatch");
    assert_eq!(c.b(), 0x80, "bcp_line_color B channel mismatch");
    assert_eq!(c.a(), 0xFF, "bcp_line_color A channel mismatch");

    assert_eq!(theme.edit_canvas.selected_glow_color, "#007AFF");
    let c = theme.edit_canvas.selected_glow_color32();
    assert_eq!(c.r(), 0x00, "selected_glow_color R channel mismatch");
    assert_eq!(c.g(), 0x7A, "selected_glow_color G channel mismatch");
    assert_eq!(c.b(), 0xFF, "selected_glow_color B channel mismatch");
    assert_eq!(c.a(), 0xFF, "selected_glow_color A channel mismatch");

    assert_eq!(theme.edit_canvas.rubberband_fill_color, "#007AFF26");
    let c = theme.edit_canvas.rubberband_fill_color32();
    assert_eq!(
        c,
        egui::Color32::from_rgba_unmultiplied(0x00, 0x7A, 0xFF, 0x26)
    );
    assert_eq!(
        c.a(),
        0x26,
        "rubberband_fill_color A channel mismatch (0x26 = 38 alpha)"
    );
    // Note: egui::Color32 stores premultiplied channels internally.
    // In premultiplied space: g = 122 * 38 / 255 = 18.
    assert_eq!(c.r(), 0);
    assert_eq!(c.g(), 18, "premultiplied G channel is 18");
    assert_eq!(c.b(), 38, "premultiplied B channel is 38");

    assert_eq!(theme.edit_canvas.rubberband_stroke_color, "#007AFF");
    let c = theme.edit_canvas.rubberband_stroke_color32();
    assert_eq!(c.r(), 0x00, "rubberband_stroke_color R channel mismatch");
    assert_eq!(c.g(), 0x7A, "rubberband_stroke_color G channel mismatch");
    assert_eq!(c.b(), 0xFF, "rubberband_stroke_color B channel mismatch");
    assert_eq!(c.a(), 0xFF, "rubberband_stroke_color A channel mismatch");

    // 2. Preview Canvas Tokens
    assert_eq!(theme.preview_canvas.fill_color, "#000000");
    let c = theme.preview_canvas.fill_color32();
    assert_eq!(c.r(), 0x00, "fill_color R channel mismatch");
    assert_eq!(c.g(), 0x00, "fill_color G channel mismatch");
    assert_eq!(c.b(), 0x00, "fill_color B channel mismatch");
    assert_eq!(c.a(), 0xFF, "fill_color A channel mismatch");

    assert_eq!(theme.preview_canvas.baseline_color, "#CCCCCC");
    let c = theme.preview_canvas.baseline_color32();
    assert_eq!(c.r(), 0xCC, "baseline_color R channel mismatch");
    assert_eq!(c.g(), 0xCC, "baseline_color G channel mismatch");
    assert_eq!(c.b(), 0xCC, "baseline_color B channel mismatch");
    assert_eq!(c.a(), 0xFF, "baseline_color A channel mismatch");

    // 3. Shape Canvas Tokens
    assert_eq!(theme.shape_canvas.accent_selection_color, "#007AFF");
    let c = theme.shape_canvas.accent_selection_color32();
    assert_eq!(c.r(), 0x00, "accent_selection_color R channel mismatch");
    assert_eq!(c.g(), 0x7A, "accent_selection_color G channel mismatch");
    assert_eq!(c.b(), 0xFF, "accent_selection_color B channel mismatch");
    assert_eq!(c.a(), 0xFF, "accent_selection_color A channel mismatch");

    assert_eq!(theme.shape_canvas.text_active_color, "#FFFFFF");
    let c = theme.shape_canvas.text_active_color32();
    assert_eq!(c.r(), 0xFF, "text_active_color R channel mismatch");
    assert_eq!(c.g(), 0xFF, "text_active_color G channel mismatch");
    assert_eq!(c.b(), 0xFF, "text_active_color B channel mismatch");
    assert_eq!(c.a(), 0xFF, "text_active_color A channel mismatch");

    assert_eq!(theme.shape_canvas.text_inactive_color, "#1C1C1E");
    let c = theme.shape_canvas.text_inactive_color32();
    assert_eq!(c.r(), 0x1C, "text_inactive_color R channel mismatch");
    assert_eq!(c.g(), 0x1C, "text_inactive_color G channel mismatch");
    assert_eq!(c.b(), 0x1E, "text_inactive_color B channel mismatch");
    assert_eq!(c.a(), 0xFF, "text_inactive_color A channel mismatch");
}

#[test]
fn test_grafia_theme_deserialization_stress() {
    // Malformed JSON should fail cleanly via serde
    let malformed_json = r#"{"theme_name": "Incomplete"}"#;
    let res: Result<GrafiaTheme, _> = serde_json::from_str(malformed_json);
    assert!(
        res.is_err(),
        "Expected error when required fields are missing"
    );

    // Completely broken JSON syntax
    let broken_syntax = r#"{ this is not valid json"#;
    let res: Result<GrafiaTheme, _> = serde_json::from_str(broken_syntax);
    assert!(res.is_err(), "Expected error on invalid JSON syntax");
}

#[test]
fn test_glyph_a_contour_signed_areas_and_winding() {
    let master = GlyphAGoldenMaster::load();

    // Map GlyphPointToken to FontPoint
    let outer_pts: Vec<FontPoint> = master
        .outer_contour
        .iter()
        .map(|pt| FontPoint::new(pt.x, pt.y))
        .collect();

    let inner_pts: Vec<FontPoint> = master
        .inner_counter
        .iter()
        .map(|pt| FontPoint::new(pt.x, pt.y))
        .collect();

    // Calculate signed areas using Shoelace formula
    let outer_area = compute_signed_area(&outer_pts);
    let inner_area = compute_signed_area(&inner_pts);

    // Verify exact mathematical signed areas in Y-up font design space
    assert_relative_eq!(outer_area, -203_200.0, epsilon = 1e-4);
    assert_relative_eq!(inner_area, 23_400.0, epsilon = 1e-4);

    // Verify winding directions:
    // Under compute_signed_area rules (Y-up Cartesian):
    // Negative area indicates Clockwise (CW) winding.
    // Positive area indicates Counter-Clockwise (CCW) winding.
    assert!(
        outer_area < 0.0,
        "Outer contour must be Clockwise (negative signed area: {outer_area}) in TrueType/OpenType convention"
    );
    assert!(
        inner_area > 0.0,
        "Inner counter must be Counter-Clockwise (positive signed area: {inner_area}) in TrueType/OpenType convention"
    );

    // Opposing signs ensure the Non-Zero Winding Rule cuts out the inner counter as a transparent hole
    assert!(
        (outer_area * inner_area) < 0.0,
        "Outer and inner contours must have opposing winding signs for hole cutout"
    );
}

#[test]
fn test_glyph_a_geometry_symmetry_and_metrics() {
    let master = GlyphAGoldenMaster::load();

    // Verify Golden Master font metrics
    assert_eq!(master.glyphname, "A");
    assert_eq!(master.upm, 1000);
    assert_relative_eq!(master.width, 700.0);
    assert_relative_eq!(master.ascender, 800.0);
    assert_relative_eq!(master.cap_height, 700.0);
    assert_relative_eq!(master.x_height, 500.0);
    assert_relative_eq!(master.descender, -200.0);

    // Bounding box computation from nodes
    let min_x = master
        .outer_contour
        .iter()
        .map(|p| p.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = master
        .outer_contour
        .iter()
        .map(|p| p.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = master
        .outer_contour
        .iter()
        .map(|p| p.y)
        .fold(f64::INFINITY, f64::min);
    let max_y = master
        .outer_contour
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max);

    assert_relative_eq!(min_x, 30.0);
    assert_relative_eq!(max_x, 670.0);
    assert_relative_eq!(min_y, 0.0);
    assert_relative_eq!(max_y, 700.0);

    // Sidebearings: LSB = 30.0, RSB = 700.0 - 670.0 = 30.0
    let lsb = min_x;
    let rsb = master.width - max_x;
    assert_relative_eq!(lsb, 30.0);
    assert_relative_eq!(rsb, 30.0);
    assert_relative_eq!(lsb, rsb, epsilon = 1e-6);

    // Glyph horizontal symmetry axis is at x = 350.0
    let center_x = 350.0;
    assert_relative_eq!(
        (master.outer_contour[0].x + master.outer_contour[3].x) / 2.0,
        center_x
    ); // outer bases (30 + 670)/2
    assert_relative_eq!(
        (master.outer_contour[1].x + master.outer_contour[2].x) / 2.0,
        center_x
    ); // apexes (300 + 400)/2
    assert_relative_eq!(
        (master.outer_contour[4].x + master.outer_contour[7].x) / 2.0,
        center_x
    ); // inner bases (540 + 160)/2
    assert_relative_eq!(
        (master.outer_contour[5].x + master.outer_contour[6].x) / 2.0,
        center_x
    ); // crossbar (470 + 230)/2
    assert_relative_eq!(master.inner_counter[2].x, center_x); // counter apex at x=350
    assert_relative_eq!(
        (master.inner_counter[0].x + master.inner_counter[1].x) / 2.0,
        center_x
    ); // counter base (260 + 440)/2
}

#[test]
fn test_glyph_a_kurbo_conversion_and_rendering_projections() {
    let master = GlyphAGoldenMaster::load();
    let theme = GrafiaTheme::sonoma_default();

    // Convert outer contour + inner counter into kurbo::BezPath
    let mut bez_path = BezPath::new();

    // Outer contour
    let first_outer = master.outer_contour[0];
    bez_path.move_to((first_outer.x, first_outer.y));
    for pt in &master.outer_contour[1..] {
        bez_path.line_to((pt.x, pt.y));
    }
    bez_path.close_path();

    // Inner counter
    let first_inner = master.inner_counter[0];
    bez_path.move_to((first_inner.x, first_inner.y));
    for pt in &master.inner_counter[1..] {
        bez_path.line_to((pt.x, pt.y));
    }
    bez_path.close_path();

    // Verify kurbo BezPath bounding box
    let bbox = bez_path.bounding_box();
    assert_relative_eq!(bbox.min_x(), 30.0);
    assert_relative_eq!(bbox.max_x(), 670.0);
    assert_relative_eq!(bbox.min_y(), 0.0);
    assert_relative_eq!(bbox.max_y(), 700.0);
    assert_relative_eq!(bbox.width(), 640.0);
    assert_relative_eq!(bbox.height(), 700.0);

    // Verify integration with PreviewFilledProjection
    let vp = ViewportTransform::new(0.5, ScreenPoint::new(400.0, 500.0), master.upm);
    let preview = PreviewFilledProjection::compute(&bez_path, &vp);
    assert!(!preview.screen_path.is_empty());
    assert_relative_eq!(preview.baseline_y_screen, 500.0);

    // Verify integration with ShapeThumbnailProjection
    let total_nodes = master.outer_contour.len() + master.inner_counter.len();
    assert_eq!(total_nodes, 11);
    let thumbnail =
        ShapeThumbnailProjection::compute(&bez_path, &theme.shape_canvas, total_nodes, true);
    let thumb_bbox = thumbnail.fitted_path.bounding_box();
    assert!(thumb_bbox.width() <= 20.01);
    assert!(thumb_bbox.height() <= 12.01);
    assert_eq!(thumbnail.node_count, 11);
}

#[test]
fn test_glyph_a_golden_master_deserialization_edge_cases() {
    let json_alternate = r#"{
        "glyphname": "A",
        "UPM": 1000,
        "advanceWidth": 700.0,
        "ascender": 800.0,
        "capHeight": 700.0,
        "xHeight": 500.0,
        "descender": -200.0,
        "outerContour": [
            { "x": 30.0, "y": 0.0 },
            { "x": 160.0, "y": 0.0 }
        ],
        "innerCounter": [
            { "x": 260.0, "y": 260.0 }
        ]
    }"#;

    let parsed: GlyphAGoldenMaster =
        serde_json::from_str(json_alternate).expect("CamelCase aliases must parse");
    assert_eq!(parsed.upm, 1000);
    assert_eq!(parsed.width, 700.0);
    assert_eq!(parsed.cap_height, 700.0);
    assert_eq!(parsed.x_height, 500.0);
    assert_eq!(parsed.outer_contour.len(), 2);
    assert_eq!(parsed.inner_counter.len(), 1);

    // Malformed JSON (missing required field)
    let incomplete = r#"{"glyphname": "A"}"#;
    let res: Result<GlyphAGoldenMaster, _> = serde_json::from_str(incomplete);
    assert!(res.is_err(), "Missing fields must fail deserialization");
}
