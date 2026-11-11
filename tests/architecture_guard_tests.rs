//! Golden Master Architecture Guardrail Suite.
//!
//! Normative References:
//! - GL-1005: Code for anything a machine could tell you got wrong.
//! - STD-ARCH: Industry Software Engineering & Testing Standards:
//!   - Regel 0: De Verankeringswet (Eerst Verankeren, Dan Pas Bouwen).
//!   - Regel 3: Zero Magic Numbers (100% Data-Driven from tokens/theme/constants).
//!   - Regel 4: NASA/Google eSLOC Modularity Limit (< 400 eSLOC, < 599 physical lines).
//!   - Regel 9 / §3.3: Windows Desktop GUI Subsystem & Safe-Renderer Invariant.
//!   - Regel 11: Anti-Spookcode Wet (Zero `#[allow(dead_code)]` in `src/`).
//!   - Regel 12 / §3.5: Verplichte Centrale State-Invariant Conventie (`validate_invariants(&self)`).
//!   - Regel 13 / §3.5: Universele `Actie -> Undo -> Redo` Bit-Exactheid Wet.
//!   - Sectie 2: "Boring Architecture, Strict Invariants" (No premature generalization).
//!
//! This suite serves as the Golden Master template across all ecosystem projects.

use std::fs;
use std::path::{Path, PathBuf};

/// Allowed dimensionless mathematical ratios and identity scalars.
const ALLOWED_RATIOS: &[f64] = &[0.0, 0.5, 1.0, 2.0];

/// Recursively collects all `.rs` files under a directory.
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
}

/// Computes executable Source Lines of Code (eSLOC) excluding comments and empty lines.
fn calculate_esloc(content: &str) -> usize {
    let mut esloc = 0;
    let mut in_block = false;
    for line in content.lines() {
        let mut trimmed = line.trim();
        if in_block {
            if let Some(pos) = trimmed.find("*/") {
                in_block = false;
                trimmed = trimmed[pos + 2..].trim();
            } else {
                continue;
            }
        }
        while let Some(pos) = trimmed.find("/*") {
            if let Some(end) = trimmed[pos + 2..].find("*/") {
                trimmed = trimmed[pos + 2 + end + 2..].trim();
            } else {
                in_block = true;
                trimmed = trimmed[..pos].trim();
                break;
            }
        }
        if !trimmed.is_empty() && !trimmed.starts_with("//") {
            esloc += 1;
        }
    }
    esloc
}

/// Strips comments (`//...` and `/*...*/`) and string literals (`"..."`) from source code,
/// preserving newlines and line indices for accurate violation reporting.
fn strip_comments_and_strings(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();
    let mut idx = 0;
    let mut in_block_comment = 0;

    while idx < len {
        if in_block_comment > 0 {
            if idx + 1 < len && chars[idx] == '*' && chars[idx + 1] == '/' {
                in_block_comment -= 1;
                idx += 2;
            } else {
                if chars[idx] == '\n' {
                    result.push('\n');
                } else {
                    result.push(' ');
                }
                idx += 1;
            }
            continue;
        }

        // Line comment
        if idx + 1 < len && chars[idx] == '/' && chars[idx + 1] == '/' {
            while idx < len && chars[idx] != '\n' {
                result.push(' ');
                idx += 1;
            }
            continue;
        }

        // Block comment start
        if idx + 1 < len && chars[idx] == '/' && chars[idx + 1] == '*' {
            in_block_comment += 1;
            result.push(' ');
            result.push(' ');
            idx += 2;
            continue;
        }

        // String literal
        if chars[idx] == '"' {
            result.push(' ');
            idx += 1;
            while idx < len && chars[idx] != '"' {
                if chars[idx] == '\\' && idx + 1 < len {
                    result.push(' ');
                    result.push(' ');
                    idx += 2;
                } else {
                    if chars[idx] == '\n' {
                        result.push('\n');
                    } else {
                        result.push(' ');
                    }
                    idx += 1;
                }
            }
            if idx < len && chars[idx] == '"' {
                result.push(' ');
                idx += 1;
            }
            continue;
        }

        result.push(chars[idx]);
        idx += 1;
    }

    result
}

/// Extracts all floating point literals from stripped source line.
fn extract_float_literals(line: &str) -> Vec<f64> {
    let mut floats = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut idx = 0;

    while idx < len {
        if chars[idx].is_ascii_digit() {
            if idx > 0
                && (chars[idx - 1].is_alphanumeric()
                    || chars[idx - 1] == '_'
                    || chars[idx - 1] == '.')
            {
                idx += 1;
                continue;
            }

            let start = idx;
            while idx < len && chars[idx].is_ascii_digit() {
                idx += 1;
            }

            if idx < len && chars[idx] == '.' {
                if idx + 1 < len && chars[idx + 1] == '.' {
                    idx += 1;
                    continue;
                }
                if idx + 1 < len && chars[idx + 1].is_ascii_digit() {
                    idx += 1;
                    while idx < len && chars[idx].is_ascii_digit() {
                        idx += 1;
                    }
                    let num_str: String = chars[start..idx].iter().collect();
                    if let Ok(val) = num_str.parse::<f64>() {
                        floats.push(val);
                    }
                }
            }
        }
        idx += 1;
    }

    floats
}

// ============================================================================
// Guardrail 1: Zero Magic Numbers & Hardcoded Geometry (STD-ARCH Regel 3)
// ============================================================================

#[test]
fn test_guard_zero_magic_numbers_in_ui_components() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");

    let mut files = Vec::new();
    collect_rs_files(&src_dir, &mut files);
    assert!(
        !files.is_empty(),
        "Architecture Guard Error: Found zero source files in src/!"
    );

    let mut violations = Vec::new();

    for file_path in &files {
        let file_str = file_path.to_string_lossy();
        // Allow tokens / theme definition files to declare baseline constants
        if file_str.contains("theme") || file_str.contains("token") || file_str.contains("config") {
            continue;
        }

        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let relative_path = file_path
            .strip_prefix(manifest_dir)
            .unwrap_or(file_path)
            .to_string_lossy();

        let stripped = strip_comments_and_strings(&content);
        for (line_idx, line) in stripped.lines().enumerate() {
            let line_num = line_idx + 1;
            let floats = extract_float_literals(line);
            for val in floats {
                let is_allowed = ALLOWED_RATIOS
                    .iter()
                    .any(|&ratio| (val - ratio).abs() < 1e-6);

                if !is_allowed {
                    violations.push(format!(
                        "  - {relative_path}:{line_num}: Disallowed numeric literal `{val}` found. \
                         All geometry dimensions must resolve via theme/tokens/constants. \
                         Allowed ratios: {ALLOWED_RATIOS:?}"
                    ));
                }
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "\n[FATAL ARCHITECTURE GUARD VIOLATION] STD-ARCH Regel 3 (Zero Magic Numbers):\n\
             Found {} hardcoded numeric literals in components:\n{}\n\
             Remediation: Extract dimensions into theme/token constants.\n",
            violations.len(),
            violations.join("\n")
        );
    }
}

// ============================================================================
// Guardrail 2: Zero Hardcoded Colors (STD-ARCH Regel 12)
// ============================================================================

#[test]
fn test_guard_zero_hardcoded_colors_in_ui_components() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");

    let mut files = Vec::new();
    collect_rs_files(&src_dir, &mut files);

    let mut violations = Vec::new();

    for file_path in &files {
        let file_str = file_path.to_string_lossy();
        if file_str.contains("theme") || file_str.contains("token") || file_str.contains("palette")
        {
            continue;
        }

        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let relative_path = file_path
            .strip_prefix(manifest_dir)
            .unwrap_or(file_path)
            .to_string_lossy();

        for (line_idx, original_line) in content.lines().enumerate() {
            let line_num = line_idx + 1;
            let code_part = original_line.split("//").next().unwrap_or("").trim();
            if code_part.is_empty() {
                continue;
            }

            for forbidden_pattern in [
                "Color32::from_rgb(",
                "Color32::from_rgba_premultiplied(",
                "Color32::from_rgba_unmultiplied(",
            ] {
                if code_part.contains(forbidden_pattern) {
                    violations.push(format!(
                        "  - {relative_path}:{line_num}: Inline color constructor `{forbidden_pattern}` forbidden. \
                         All colors must resolve via theme or tokens."
                    ));
                }
            }

            if code_part.contains('"') {
                for part in code_part.split('"') {
                    if part.starts_with('#')
                        && (part.len() == 4 || part.len() == 7 || part.len() == 9)
                        && part[1..].chars().all(|c| c.is_ascii_hexdigit())
                    {
                        violations.push(format!(
                            "  - {relative_path}:{line_num}: Hardcoded hex color `{part}` forbidden in UI code. \
                             Colors must resolve via dynamic theme resolvers."
                        ));
                    }
                }
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "\n[FATAL ARCHITECTURE GUARD VIOLATION] STD-ARCH Regel 12 (Zero Hardcoded Colors):\n\
             Found {} hardcoded color violations in UI components:\n{}\n\
             Remediation: All colors must be sourced from dynamic theme/token resolvers.\n",
            violations.len(),
            violations.join("\n")
        );
    }
}

/// Counts statement-separating semicolons on a single line, ignoring semicolons inside `[...]` array/vec repeat expressions.
fn count_statement_semicolons(code_line: &str) -> usize {
    let mut bracket_depth: usize = 0;
    let mut count = 0;
    for ch in code_line.chars() {
        match ch {
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ';' if bracket_depth == 0 => count += 1,
            _ => {}
        }
    }
    count
}

// ============================================================================
// Guardrail 3: Modularity Limits (< 399 eSLOC & < 599 Physical Lines) (STD-ARCH Regel 4)
// ============================================================================

#[test]
fn test_guard_modularity_limits_gl1015() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");

    let mut files = Vec::new();
    collect_rs_files(&src_dir, &mut files);
    assert!(
        !files.is_empty(),
        "Architecture Guard Error: Found zero source files in src/!"
    );

    let mut violations = Vec::new();

    for file_path in &files {
        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let relative_path = file_path
            .strip_prefix(manifest_dir)
            .unwrap_or(file_path)
            .to_string_lossy();

        let physical_lines = content.lines().count();
        let esloc = calculate_esloc(&content);

        if physical_lines >= 599 {
            violations.push(format!(
                "  - {relative_path}: {physical_lines} physical lines exceeds STD-ARCH hard ceiling (max 598 lines)."
            ));
        }

        if esloc >= 399 {
            violations.push(format!(
                "  - {relative_path}: {esloc} eSLOC exceeds STD-ARCH modularity limit (max 398 eSLOC)."
            ));
        }

        // Anti-Cramming Rule: No multi-statement cramming to fake LOC reduction
        for (line_idx, line) in content.lines().enumerate() {
            let code_part = line.split("//").next().unwrap_or("").trim();
            if !code_part.contains('"')
                && !code_part.ends_with('\\')
                && count_statement_semicolons(code_part) >= 2
            {
                violations.push(format!(
                    "  - {relative_path}:{}: Multi-statement single-line cramming forbidden (>= 2 semicolons): `{code_part}`",
                    line_idx + 1
                ));
            }
        }
    }

    let examples_dir = manifest_dir.join("examples");
    let mut example_files = Vec::new();
    if examples_dir.exists() {
        collect_rs_files(&examples_dir, &mut example_files);
    }
    for file_path in &example_files {
        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let relative_path = file_path
            .strip_prefix(manifest_dir)
            .unwrap_or(file_path)
            .to_string_lossy();
        let esloc = calculate_esloc(&content);
        if esloc >= 300 {
            violations.push(format!(
                "  - {relative_path}: {esloc} eSLOC exceeds example modularity limit (< 300 eSLOC)."
            ));
        }
        for (line_idx, line) in content.lines().enumerate() {
            let code_part = line.split("//").next().unwrap_or("").trim();
            if !code_part.contains('"')
                && !code_part.ends_with('\\')
                && count_statement_semicolons(code_part) >= 2
            {
                violations.push(format!(
                    "  - {relative_path}:{}: Multi-statement single-line cramming forbidden in examples (>= 2 semicolons): `{code_part}`",
                    line_idx + 1
                ));
            }
        }
    }

    let tests_dir = manifest_dir.join("tests");
    let mut test_files = Vec::new();
    if tests_dir.exists() {
        collect_rs_files(&tests_dir, &mut test_files);
    }
    for file_path in &test_files {
        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let relative_path = file_path
            .strip_prefix(manifest_dir)
            .unwrap_or(file_path)
            .to_string_lossy();
        for (line_idx, line) in content.lines().enumerate() {
            let code_part = line.split("//").next().unwrap_or("").trim();
            if !code_part.contains('"')
                && !code_part.ends_with('\\')
                && count_statement_semicolons(code_part) >= 2
            {
                violations.push(format!(
                    "  - {relative_path}:{}: Multi-statement single-line cramming forbidden in tests (>= 2 semicolons): `{code_part}`",
                    line_idx + 1
                ));
            }
        }
    }

    let fmt_status = std::process::Command::new("cargo")
        .args(["fmt", "--all", "--", "--check"])
        .current_dir(manifest_dir)
        .status()
        .expect("Failed to execute `cargo fmt --all -- --check`");
    if !fmt_status.success() {
        violations.push(
            "  - `cargo fmt --all -- --check` failed: repository contains unformatted code."
                .to_string(),
        );
    }

    if !test_files.is_empty() {
        let mut rustfmt_cmd = std::process::Command::new("rustfmt");
        rustfmt_cmd
            .args(["--edition", "2021", "--check"])
            .args(&test_files)
            .current_dir(manifest_dir);
        if let Ok(status) = rustfmt_cmd.status() {
            if !status.success() {
                violations.push(
                    "  - `rustfmt --edition 2021 --check tests/*.rs` failed: unformatted test file(s)."
                        .to_string(),
                );
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "\n[FATAL ARCHITECTURE GUARD VIOLATION] STD-ARCH Regel 4 (Modularity & NASA/Google eSLOC):\n\
             Found {} modularity ceiling violations:\n{}\n\
             Remediation: Refactor large modules into submodules following the Single Responsibility Principle.\n",
            violations.len(),
            violations.join("\n")
        );
    }
}

// ============================================================================
// Guardrail 4: Windows Desktop GUI Subsystem & Safe-Renderer Invariant (STD-ARCH Regel 10 & §3.3)
// ============================================================================

#[test]
fn test_guard_windows_gui_subsystem_attribute() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_bin = manifest_dir.join("src").join("bin");
    let examples = manifest_dir.join("examples");
    let src_main = manifest_dir.join("src").join("main.rs");

    let mut entrypoints = Vec::new();
    if src_main.exists() {
        entrypoints.push(src_main);
    }
    if src_bin.exists() {
        collect_rs_files(&src_bin, &mut entrypoints);
    }
    if examples.exists() {
        collect_rs_files(&examples, &mut entrypoints);
    }

    let required_attr = "#![cfg_attr(not(test), windows_subsystem = \"windows\")]";
    let mut violations = Vec::new();

    for file_path in &entrypoints {
        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let relative_path = file_path
            .strip_prefix(manifest_dir)
            .unwrap_or(file_path)
            .to_string_lossy();

        // Check if this binary/example is a GUI entrypoint (depends on or uses eframe/winit/gui frameworks)
        let is_gui = content.contains("eframe")
            || content.contains("winit")
            || content.contains("run_native")
            || content.contains("iced")
            || content.contains("slint")
            || content.contains("tauri");

        if is_gui && !content.contains(required_attr) {
            if content.contains("windows_subsystem") {
                violations.push(format!(
                    "  - {relative_path}: Incorrect windows_subsystem declaration. \
                     MUST be `#![cfg_attr(not(test), windows_subsystem = \"windows\")]` to avoid console popups in debug builds."
                ));
            } else {
                violations.push(format!(
                    "  - {relative_path}: Missing mandatory Windows GUI Subsystem attribute `{required_attr}`. \
                     STD-ARCH §3.3 mandates all GUI binaries and examples suppress attached console allocation on Windows."
                ));
            }
        }

        // STD-ARCH §3.3 Safe-Renderer Invariant: forbid bare NativeOptions::default() and system_font_fallback: true
        let stripped = strip_comments_and_strings(&content);
        if stripped.contains("run_native") || stripped.contains("NativeOptions") {
            if stripped.contains("NativeOptions::default()") {
                violations.push(format!(
                    "  - {relative_path}: Bare `NativeOptions::default()` is forbidden by STD-ARCH §3.3 (causes Windows Hybrid-GPU wgpu TDR and main-thread font freeze)."
                ));
            }
            if stripped.contains("system_font_fallback: true")
                || !stripped.contains("system_font_fallback: false")
            {
                violations.push(format!(
                    "  - {relative_path}: GUI entrypoint must explicitly set `system_font_fallback: false` per STD-ARCH §3.3."
                ));
            }
            if !stripped.contains("Renderer::Glow") {
                violations.push(format!(
                    "  - {relative_path}: GUI entrypoint must explicitly configure `eframe::Renderer::Glow` per STD-ARCH §3.3."
                ));
            }
        }
    }

    let mut ui_files = Vec::new();
    collect_rs_files(&manifest_dir.join("src"), &mut ui_files);
    if examples.exists() {
        collect_rs_files(&examples, &mut ui_files);
    }
    let needle = "UiBuilder::new()";
    for file_path in &ui_files {
        let content = fs::read_to_string(file_path).unwrap_or_default();
        let rel = file_path
            .strip_prefix(manifest_dir)
            .unwrap_or(file_path)
            .to_string_lossy();
        let stripped = strip_comments_and_strings(&content);
        let mut from = 0;
        while let Some(pos) = stripped[from..].find(needle) {
            let abs = from + pos;
            let line_num = stripped[..abs].lines().count().max(1);
            let after = &stripped[abs + needle.len()..];
            let span = after
                .find(';')
                .unwrap_or(after.len())
                .min(after.find(needle).unwrap_or(after.len()))
                .min(300);
            if !after[..span].contains(".id_salt(") {
                violations.push(format!(
                    "  - {rel}:{line_num}: `UiBuilder::new()` without `.id_salt(...)` violates STD-ARCH §3.4."
                ));
            }
            from = abs + needle.len();
        }
    }

    if !violations.is_empty() {
        panic!(
            "\n[FATAL ARCHITECTURE GUARD VIOLATION] STD-ARCH §3.3 & §3.4 (Windows Desktop GUI Subsystem & Safe-Renderer Wet):\n\
             Found {} GUI entrypoint/hygiene violation(s):\n{}\n\
             Remediation: Add `#![cfg_attr(not(test), windows_subsystem = \"windows\")]`, set `system_font_fallback: false`, and use `InstanceFlags::empty()` / `Renderer::Glow`.\n",
            violations.len(),
            violations.join("\n")
        );
    }
}

// ============================================================================
// Guardrail 5b: Anti-Spookcode Wet — Zero `allow(dead_code)` in `src/` (STD-ARCH Regel 11)
// ============================================================================

#[test]
fn test_guard_zero_allow_dead_code_in_src() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");

    let mut files = Vec::new();
    collect_rs_files(&src_dir, &mut files);
    let mut violations = Vec::new();

    for file_path in &files {
        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let relative_path = file_path
            .strip_prefix(manifest_dir)
            .unwrap_or(file_path)
            .to_string_lossy();

        let stripped = strip_comments_and_strings(&content);
        for (line_idx, line) in stripped.lines().enumerate() {
            let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            if compact.contains("allow(dead_code)") {
                violations.push(format!(
                    "  - {relative_path}:{}: Forbidden `allow(dead_code)` attribute in production code (`src/`).",
                    line_idx + 1
                ));
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "\n[FATAL ARCHITECTURE GUARD VIOLATION] STD-ARCH Regel 11 (Anti-Spookcode Wet — Zero `allow(dead_code)` in `src/`):\n\
             Found {} dead-code suppression(s) in `src/`:\n{}\n\
             Remediation: Remove unused functions/structs/constants or wire them into the active domain logic.\n",
            violations.len(),
            violations.join("\n")
        );
    }
}

// ============================================================================
// Guardrail 6: Central State-Invariant & Action -> Undo -> Redo Bit-Exactness (STD-ARCH Regel 12 & 13, §3.5)
//
// Reference Pattern for Project Integration Tests (`tests/*_tests.rs`):
// ```rust
// fn assert_mutation_undo_redo_roundtrip<S, F>(state: &mut S, mutate: F)
// where
//     F: FnOnce(&mut S),
// {
//     state.validate_invariants().expect("Pre-mutation invariant must hold");
//     let snap_0 = state.snapshot();
//     mutate(state);
//     state.validate_invariants().expect("Post-mutation invariant must hold");
//     let snap_1 = state.snapshot();
//     assert_ne!(snap_0, snap_1, "Mutation must change state");
//     assert!(state.undo(), "Undo must succeed");
//     state.validate_invariants().expect("Post-undo invariant must hold");
//     assert_eq!(state.snapshot(), snap_0, "Undo must restore State_0 bit-exactly");
//     assert!(state.redo(), "Redo must succeed");
//     state.validate_invariants().expect("Post-redo invariant must hold");
//     assert_eq!(state.snapshot(), snap_1, "Redo must restore State_1 bit-exactly");
// }
//
// #[test]
// fn test_cold_start_smoke_and_zero_idle_repaint_leak() {
//     let start = std::time::Instant::now();
//     // Initialize state + run Frame 1 headless tessellation (< 500ms) + verify repaint_delay > Duration::ZERO
// }
// ```
// ============================================================================

#[test]
fn test_guard_central_state_invariant_and_undo_redo_roundtrip_presence() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let tests_dir = manifest_dir.join("tests");

    let mut src_files = Vec::new();
    collect_rs_files(&src_dir, &mut src_files);

    let mut has_mutable_state_or_history = false;
    let mut has_gui_or_canvas = false;
    let mut has_validate_invariants = false;
    let mut has_assert_invariants = false;

    for file_path in &src_files {
        let content = fs::read_to_string(file_path).unwrap_or_default();
        let stripped = strip_comments_and_strings(&content);
        if stripped.contains("CommandHistory")
            || stripped.contains("pub fn undo(")
            || stripped.contains("pub fn record_mutation(")
            || stripped.contains("pub fn restore_snapshot(")
        {
            has_mutable_state_or_history = true;
        }
        if stripped.contains("egui::")
            || stripped.contains("eframe::")
            || stripped.contains("CanvasWidget")
        {
            has_gui_or_canvas = true;
        }
        if stripped.contains("fn validate_invariants(") {
            has_validate_invariants = true;
        }
        if stripped.contains("fn assert_invariants(") {
            has_assert_invariants = true;
        }
    }

    let mut test_files = Vec::new();
    if tests_dir.exists() {
        collect_rs_files(&tests_dir, &mut test_files);
    }
    let mut has_roundtrip_test = false;
    let mut has_cold_start_test = false;
    for file_path in &test_files {
        if file_path
            .file_name()
            .is_some_and(|n| n == "architecture_guard_tests.rs")
        {
            continue;
        }
        let content = fs::read_to_string(file_path).unwrap_or_default();
        let stripped = strip_comments_and_strings(&content);
        if stripped.contains("validate_invariants()")
            && stripped.contains(".undo()")
            && stripped.contains(".redo()")
        {
            has_roundtrip_test = true;
        }
        if stripped.contains("cold_start")
            || (stripped.contains("Instant::now()") && stripped.contains("repaint_delay"))
        {
            has_cold_start_test = true;
        }
    }

    if has_mutable_state_or_history {
        assert!(
            has_validate_invariants && has_assert_invariants,
            "[FATAL ARCHITECTURE GUARD VIOLATION] STD-ARCH Regel 12 (§3.5): Project has mutable state/history in `src/` but is missing `pub fn validate_invariants(&self)` or `pub fn assert_invariants(&self)`."
        );
        assert!(
            has_roundtrip_test,
            "[FATAL ARCHITECTURE GUARD VIOLATION] STD-ARCH Regel 13 (§3.5): Project is missing a universal `State_0 -> Actie -> State_1 -> Undo == State_0 -> Redo == State_1` bit-exact roundtrip test calling `validate_invariants()`."
        );
    }
    if has_gui_or_canvas {
        assert!(
            has_cold_start_test,
            "[FATAL ARCHITECTURE GUARD VIOLATION] STD-ARCH §3.3 & §3.4: Project is missing a Cold-Start Smoke Test (`< 500ms` init + 0% idle repaint leak check)."
        );
    }
}

// ============================================================================
// Guardrail 5: Gekalibreerde Eenvoud — Zero Unused Dependencies & Required Docs
// ============================================================================

#[test]
fn test_guard_v010_calibrated_simplicity_no_unused_dependencies_and_required_docs() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml_path = manifest_dir.join("Cargo.toml");
    let cargo_toml = fs::read_to_string(&cargo_toml_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", cargo_toml_path.display()));

    let src_dir = manifest_dir.join("src");
    let mut rs_files = Vec::new();
    collect_rs_files(&src_dir, &mut rs_files);
    assert!(!rs_files.is_empty(), "src/ must contain .rs files");

    let mut stripped_sources = Vec::with_capacity(rs_files.len());
    for file_path in &rs_files {
        let raw = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        stripped_sources.push(strip_comments_and_strings(&raw));
    }

    let mut in_dependencies = false;
    let mut unused_deps = Vec::new();

    for line in cargo_toml.lines() {
        let trimmed = line.split('#').next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('[') {
            in_dependencies = trimmed == "[dependencies]";
            continue;
        }
        if in_dependencies {
            if let Some((dep_name, _)) = trimmed.split_once('=') {
                let dep_name = dep_name.trim();
                let crate_ident = dep_name.replace('-', "_");
                let pat_path = format!("{crate_ident}::");
                let pat_use = format!("use {crate_ident};");
                let pat_ext = format!("extern crate {crate_ident}");
                let used_in_src = stripped_sources.iter().any(|src| {
                    src.contains(&pat_path) || src.contains(&pat_use) || src.contains(&pat_ext)
                });
                if !used_in_src {
                    unused_deps.push(format!(
                        "  - `{dep_name}` (`{crate_ident}`) is declared in [dependencies] but never referenced in src/"
                    ));
                }
            }
        }
    }

    let required_docs = [
        "README.md",
        "CHANGELOG.md",
        "SPEC-002-V0.1-CANVAS-INTERACTION-AND-RENDERING.md",
    ];
    let mut missing_docs = Vec::new();
    for doc in required_docs {
        let doc_path = manifest_dir.join(doc);
        match fs::read_to_string(&doc_path) {
            Ok(content) if !content.trim().is_empty() => {}
            Ok(_) => missing_docs.push(format!("  - `{doc}` exists but is empty")),
            Err(_) => missing_docs.push(format!("  - `{doc}` is missing from repository root")),
        }
    }

    if !unused_deps.is_empty() || !missing_docs.is_empty() {
        panic!(
            "\n[FATAL ARCHITECTURE GUARD VIOLATION] SPEC-002 §1.1 & STD-ARCH Rule 9 (Gekalibreerde Eenvoud):\n\
             Unused [dependencies] in Cargo.toml ({}):\n{}\n\
             Missing or empty normative documents ({}):\n{}\n",
            unused_deps.len(),
            if unused_deps.is_empty() { "  (none)".to_string() } else { unused_deps.join("\n") },
            missing_docs.len(),
            if missing_docs.is_empty() { "  (none)".to_string() } else { missing_docs.join("\n") },
        );
    }
}

fn contains_hex_offset_4plus(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i + 6 <= bytes.len() {
        if bytes[i] == b'0' && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') {
            let mut hex_len = 0;
            while i + 2 + hex_len < bytes.len() && bytes[i + 2 + hex_len].is_ascii_hexdigit() {
                hex_len += 1;
            }
            if hex_len >= 4 {
                return true;
            }
            i += 2 + hex_len;
        } else {
            i += 1;
        }
    }
    false
}

#[test]
fn tc_arch_008_clean_room_source_and_tests_guard() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_rs_files(&manifest_dir.join("src"), &mut files);
    collect_rs_files(&manifest_dir.join("examples"), &mut files);
    collect_rs_files(&manifest_dir.join("tests"), &mut files);
    if let Ok(entries) = fs::read_dir(manifest_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("md") {
                files.push(p);
            }
        }
    }
    if let Ok(entries) = fs::read_dir(manifest_dir.join("tokens")) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                files.push(p);
            }
        }
    }
    assert!(
        !files.is_empty(),
        "src/, examples/, tests/, and root .md must contain files"
    );

    let forbidden_terms: &[&str] = &[
        concat!("Gly", "phs 4"),
        concat!("Gly", "phs 3"),
        concat!("Gly", "phs.app"),
        concat!("Gly", "phsCore"),
        concat!("Gly", "phsApp"),
        concat!("Gly", "phsPalette"),
        concat!("Gly", "phsPlugin"),
        concat!("Gly", "phsReporter"),
        concat!("Gly", "phsTool"),
        concat!("GS", "Path"),
        concat!("GS", "Node"),
        concat!("GS", "Layer"),
        concat!("GS", "Glyph"),
        concat!("GS", "Font"),
        concat!("GS", "Guide"),
        concat!("GS", "Snapper"),
        concat!("GS", "Tool"),
        concat!("GS", "EditView"),
        concat!("GS", "Inspector"),
        concat!("GS", "Annotation"),
        concat!("Font", "Lab"),
        concat!("Speed", "punk"),
        concat!("Mach", "-O"),
        concat!("llvm-obj", "dump"),
        concat!("obj", "dump"),
        concat!("nm", " -"),
        concat!("disas", "sembly"),
        concat!("Mac", "Book"),
        concat!("192.168.", "188.207"),
        concat!("jaco", "oversluizen"),
        concat!("Object", "ive-C"),
        concat!("SF", "NS"),
        concat!("Pat", "rix"),
        concat!("Obsi", "dian"),
        concat!("GL-", "1015"),
        concat!("my", "PKA"),
        concat!("gli", "fo"),
        concat!("Gli", "fo"),
        concat!("GLI", "FO"),
    ];

    let mut violations = Vec::new();
    for file_path in &files {
        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let rel = file_path
            .strip_prefix(manifest_dir)
            .unwrap_or(file_path)
            .to_string_lossy();
        if rel.to_ascii_lowercase().contains(concat!("gli", "fo")) {
            violations.push(format!("  - {rel}: forbidden legacy filename term"));
        }
        for (idx, line) in content.lines().enumerate() {
            let line_num = idx + 1;
            for &term in forbidden_terms {
                if line.contains(term) {
                    violations.push(format!("  - {rel}:{line_num}: forbidden term `{term}`"));
                }
            }
            let lower = line.to_ascii_lowercase();
            for word in [concat!("o", "tool"), concat!("va", "ult")] {
                if lower
                    .split(|c: char| !c.is_ascii_alphanumeric())
                    .any(|w| w == word)
                {
                    violations.push(format!("  - {rel}:{line_num}: forbidden word `{word}`"));
                }
            }
            if contains_hex_offset_4plus(line) {
                violations.push(format!(
                    "  - {rel}:{line_num}: forbidden hex offset literal"
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Clean-room guard failed with {} violation(s):\n{}",
        violations.len(),
        violations.join("\n")
    );
}
