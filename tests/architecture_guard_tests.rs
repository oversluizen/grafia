#![allow(clippy::too_many_lines, clippy::cognitive_complexity)]
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
//!   - Regel 19..22: COTS AST Lints (`[lints]` + `clippy.toml`), Domain Naming, Generation-Cached Render Loop, DRY & Design-by-Contract (`debug_assert!`).
//!   - Sectie 2: "Boring Architecture, Strict Invariants" (No premature generalization).
//!
//! This suite serves as the Golden Master template across all single-crate and multi-crate workspace projects.

use std::fs;
use std::path::{Path, PathBuf};

/// Allowed dimensionless mathematical ratios and identity scalars.
const ALLOWED_RATIOS: &[f64] = &[0.0, 0.5, 1.0, 2.0];

/// Resolves the workspace root directory (supports both single-crate repos and `crates/*` Cargo workspaces).
fn resolve_workspace_root(manifest_dir: &Path) -> PathBuf {
    if let Some(parent) = manifest_dir.parent() {
        if parent.file_name().is_some_and(|n| n == "crates") {
            if let Some(root) = parent.parent() {
                if root.join("Cargo.toml").exists() {
                    return root.to_path_buf();
                }
            }
        }
    }
    manifest_dir.to_path_buf()
}

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

/// Collects all production `.rs` files under `src/` (single-crate) or `crates/*/src/` (workspace).
fn collect_workspace_src_files(manifest_dir: &Path, out: &mut Vec<PathBuf>) {
    let ws_root = resolve_workspace_root(manifest_dir);
    let crates_dir = ws_root.join("crates");
    if crates_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&crates_dir) {
            for entry in entries.flatten() {
                let crate_src = entry.path().join("src");
                if crate_src.is_dir() {
                    collect_rs_files(&crate_src, out);
                }
            }
        }
    } else {
        collect_rs_files(&manifest_dir.join("src"), out);
    }
}

/// Collects all integration test `.rs` files under `tests/` (single-crate) or `crates/*/tests/` (workspace).
fn collect_workspace_test_files(manifest_dir: &Path, out: &mut Vec<PathBuf>) {
    let ws_root = resolve_workspace_root(manifest_dir);
    let crates_dir = ws_root.join("crates");
    if crates_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&crates_dir) {
            for entry in entries.flatten() {
                let crate_tests = entry.path().join("tests");
                if crate_tests.is_dir() {
                    collect_rs_files(&crate_tests, out);
                }
            }
        }
    }
    let root_tests = ws_root.join("tests");
    if root_tests.is_dir() {
        collect_rs_files(&root_tests, out);
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

        if idx + 1 < len && chars[idx] == '/' && chars[idx + 1] == '/' {
            while idx < len && chars[idx] != '\n' {
                result.push(' ');
                idx += 1;
            }
            continue;
        }

        if idx + 1 < len && chars[idx] == '/' && chars[idx + 1] == '*' {
            in_block_comment += 1;
            result.push(' ');
            result.push(' ');
            idx += 2;
            continue;
        }

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
    let ws_root = resolve_workspace_root(manifest_dir);

    let mut files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut files);
    assert!(
        !files.is_empty(),
        "Architecture Guard Error: Found zero source files in src/!"
    );

    let mut violations = Vec::new();

    for file_path in &files {
        let file_str = file_path.to_string_lossy();
        if file_str.contains("theme")
            || file_str.contains("token")
            || file_str.contains("config")
            || file_str.contains("math_consts")
        {
            continue;
        }

        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let relative_path = file_path
            .strip_prefix(&ws_root)
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
    let ws_root = resolve_workspace_root(manifest_dir);

    let mut files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut files);

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
            .strip_prefix(&ws_root)
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
    let ws_root = resolve_workspace_root(manifest_dir);

    let mut files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut files);
    assert!(
        !files.is_empty(),
        "Architecture Guard Error: Found zero source files in src/!"
    );

    let mut violations = Vec::new();

    for file_path in &files {
        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let relative_path = file_path
            .strip_prefix(&ws_root)
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

    let examples_dir = ws_root.join("examples");
    let mut example_files = Vec::new();
    if examples_dir.exists() {
        collect_rs_files(&examples_dir, &mut example_files);
    }
    for file_path in &example_files {
        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let relative_path = file_path
            .strip_prefix(&ws_root)
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

    let mut test_files = Vec::new();
    collect_workspace_test_files(manifest_dir, &mut test_files);
    for file_path in &test_files {
        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let relative_path = file_path
            .strip_prefix(&ws_root)
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
        .args(["fmt", "--", "--check"])
        .current_dir(&ws_root)
        .status()
        .expect("Failed to execute `cargo fmt -- --check`");
    if !fmt_status.success() {
        violations.push(
            "  - `cargo fmt --all -- --check` failed: repository contains unformatted code."
                .to_string(),
        );
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
    let ws_root = resolve_workspace_root(manifest_dir);
    let src_bin = manifest_dir.join("src").join("bin");
    let examples = ws_root.join("examples");
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
            .strip_prefix(&ws_root)
            .unwrap_or(file_path)
            .to_string_lossy();

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
    collect_workspace_src_files(manifest_dir, &mut ui_files);
    if examples.exists() {
        collect_rs_files(&examples, &mut ui_files);
    }
    let needle = "UiBuilder::new()";
    for file_path in &ui_files {
        let content = fs::read_to_string(file_path).unwrap_or_default();
        let rel = file_path
            .strip_prefix(&ws_root)
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
    let ws_root = resolve_workspace_root(manifest_dir);

    let mut files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut files);
    let mut violations = Vec::new();

    for file_path in &files {
        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let relative_path = file_path
            .strip_prefix(&ws_root)
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
// ============================================================================

#[test]
fn test_guard_central_state_invariant_and_undo_redo_roundtrip_presence() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut src_files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut src_files);

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
    collect_workspace_test_files(manifest_dir, &mut test_files);
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
    let ws_root = resolve_workspace_root(manifest_dir);
    let cargo_toml_path = manifest_dir.join("Cargo.toml");
    let cargo_toml = fs::read_to_string(&cargo_toml_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", cargo_toml_path.display()));

    let mut rs_files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut rs_files);
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
                let dep_name = dep_name.trim().split('.').next().unwrap_or("").trim();
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

    let required_docs = ["README.md", "CHANGELOG.md"];
    let mut missing_docs = Vec::new();
    for doc in required_docs {
        let doc_path = ws_root.join(doc);
        match fs::read_to_string(&doc_path) {
            Ok(content) if !content.trim().is_empty() => {}
            Ok(_) => missing_docs.push(format!("  - `{doc}` exists but is empty")),
            Err(_) => missing_docs.push(format!("  - `{doc}` is missing from repository root")),
        }
    }

    if !unused_deps.is_empty() || !missing_docs.is_empty() {
        panic!(
            "\n[FATAL ARCHITECTURE GUARD VIOLATION] STD-ARCH Rule 9 (Gekalibreerde Eenvoud):\n\
             Unused [dependencies] in Cargo.toml ({}):\n{}\n\
             Missing or empty normative documents ({}):\n{}\n",
            unused_deps.len(),
            if unused_deps.is_empty() {
                "  (none)".to_string()
            } else {
                unused_deps.join("\n")
            },
            missing_docs.len(),
            if missing_docs.is_empty() {
                "  (none)".to_string()
            } else {
                missing_docs.join("\n")
            },
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
    let ws_root = resolve_workspace_root(manifest_dir);
    let is_patrix_repo = ws_root
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("patrix"));

    let mut files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut files);
    collect_rs_files(&ws_root.join("examples"), &mut files);
    collect_workspace_test_files(manifest_dir, &mut files);
    if let Ok(entries) = fs::read_dir(&ws_root) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("md") {
                files.push(p);
            }
        }
    }
    if let Ok(entries) = fs::read_dir(ws_root.join("tokens")) {
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

    let mut forbidden_terms: Vec<&str> = vec![
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
        concat!("Obsi", "dian"),
        concat!("GL-", "1015"),
        concat!("my", "PKA"),
        concat!("gli", "fo"),
        concat!("Gli", "fo"),
        concat!("GLI", "FO"),
    ];
    if !is_patrix_repo {
        forbidden_terms.push(concat!("Pat", "rix"));
    }

    let mut violations = Vec::new();
    for file_path in &files {
        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", file_path.display()));
        let rel = file_path
            .strip_prefix(&ws_root)
            .unwrap_or(file_path)
            .to_string_lossy();
        if rel.to_ascii_lowercase().contains(concat!("gli", "fo")) {
            violations.push(format!("  - {rel}: forbidden legacy filename term"));
        }
        for (idx, line) in content.lines().enumerate() {
            let line_num = idx + 1;
            for &term in &forbidden_terms {
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

// ============================================================================
// Universal COTS Anti-Bloat, Readability, DRY & Battery-Efficiency Guards
// (`tc_arch_009` .. `tc_arch_015` — Portable across any Rust/egui workspace)
// ============================================================================

fn has_version_or_slice_stamp(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    if lower.contains("part_a")
        || lower.contains("part_b")
        || lower.contains("slice_1")
        || lower.contains("slice_2")
        || lower.contains("slice_3")
        || lower.contains("slice_4")
    {
        return true;
    }
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if bytes[i] == b'v'
            && bytes[i + 1] == b'0'
            && bytes[i + 2].is_ascii_digit()
            && bytes[i + 3].is_ascii_digit()
        {
            let prev_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            if prev_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

#[test]
fn tc_arch_009_no_shadow_test_only_renderers_in_src() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws_root = resolve_workspace_root(manifest_dir);
    let mut files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut files);

    let mut violations = Vec::new();
    let mut non_shadow_sources = String::new();

    for file_path in &files {
        let rel = file_path
            .strip_prefix(&ws_root)
            .unwrap_or(file_path)
            .to_string_lossy();
        let file_name = file_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let content = fs::read_to_string(file_path).unwrap_or_default();
        let stripped = strip_comments_and_strings(&content);

        if file_name.ends_with("_ts.rs") {
            violations.push(format!(
                "  - {rel}: shadow `*_ts.rs` overlay renderer file in `src/` (duplicates `egui::Painter` geometry exclusively for headless tests)"
            ));
        } else {
            non_shadow_sources.push_str(&stripped);
            non_shadow_sources.push('\n');
        }

        for (idx, line) in stripped.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.contains("CanvasViewMode::ShapeThumbnail") && trimmed.ends_with("{}") {
                violations.push(format!(
                    "  - {rel}:{}: empty no-op `CanvasViewMode::ShapeThumbnail => {{}}` in live `egui` renderer",
                    idx + 1
                ));
            }
        }
    }

    if non_shadow_sources.contains("pub struct PreviewStripCache")
        && !non_shadow_sources.contains("PreviewStripCache::new")
        && !non_shadow_sources.contains("preview_strip_cache")
    {
        violations.push(
            "  - src/render: `PreviewStripCache` is defined in `src/` but never wired into any production widget/state".to_string(),
        );
    }

    assert!(
        violations.is_empty(),
        "\n[FATAL ARCHITECTURE GUARD VIOLATION] tc_arch_009 (No Shadow Test-Only Renderers or Unwired Caches in src/):\n\
         Found {} violation(s):\n{}\n",
        violations.len(),
        violations.join("\n")
    );
}

#[test]
fn tc_arch_010_no_version_or_slice_archaeology_in_identifiers() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws_root = resolve_workspace_root(manifest_dir);
    let mut files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut files);

    let mut violations = Vec::new();

    for file_path in &files {
        let rel = file_path
            .strip_prefix(&ws_root)
            .unwrap_or(file_path)
            .to_string_lossy();
        let file_name = file_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        if has_version_or_slice_stamp(file_name) {
            violations.push(format!(
                "  - {rel}: chronological version/slice filename `{file_name}` (must use domain-driven module naming)"
            ));
        }

        let content = fs::read_to_string(file_path).unwrap_or_default();
        let stripped = strip_comments_and_strings(&content);
        for (idx, line) in stripped.lines().enumerate() {
            let trimmed = line.trim();
            for kw in ["fn ", "struct ", "enum ", "mod "] {
                if let Some(pos) = trimmed.find(kw) {
                    let after = &trimmed[pos + kw.len()..];
                    let ident: String = after
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !ident.is_empty() && has_version_or_slice_stamp(&ident) {
                        violations.push(format!(
                            "  - {rel}:{}: version/slice-stamped identifier `{ident}`",
                            idx + 1
                        ));
                    }
                }
            }
            if let Some(after_pub) = trimmed.strip_prefix("pub ") {
                let ident: String = after_pub
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                let rest = after_pub[ident.len()..].trim_start();
                if !ident.is_empty()
                    && rest.starts_with(':')
                    && !rest.starts_with("::")
                    && has_version_or_slice_stamp(&ident)
                {
                    violations.push(format!(
                        "  - {rel}:{}: version/slice-stamped struct field `{ident}`",
                        idx + 1
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "\n[FATAL ARCHITECTURE GUARD VIOLATION] tc_arch_010 (Zero Version/Slice Archaeology in src/):\n\
         Found {} version- or slice-stamped file/item name(s):\n{}\n",
        violations.len(),
        violations.join("\n")
    );
}

#[test]
fn tc_arch_011_no_linter_evasion_math_or_token_dumping() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws_root = resolve_workspace_root(manifest_dir);
    let mut files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut files);

    let mut violations = Vec::new();
    let evasion_patterns = [
        "(2.0 + 1.0)",
        "(1.0 + 2.0)",
        "2.0 * 2.0",
        "(2.0 * 2.0)",
        "(1.0 + 1.0)",
    ];
    let forbidden_token_domain_types = [
        "struct ComponentId",
        "enum TextStreamEditKind",
        "struct TextStreamEditRequest",
        "enum CanvasHostRequest",
        "enum HobbyDirConstraint",
        "struct ExtraNodeKey",
        "struct VirtualExtraNode",
        "struct CanvasSmartHandle",
        "struct CanvasBackgroundImage",
        "struct HobbyNodeInfo",
        "struct BackgroundImageInfo",
        "struct BackgroundImageMutation",
        "enum LayerPathOp",
        "struct TouchInteractionState",
    ];

    for file_path in &files {
        let rel = file_path
            .strip_prefix(&ws_root)
            .unwrap_or(file_path)
            .to_string_lossy();
        let rel_norm = rel.replace('\\', "/");
        let content = fs::read_to_string(file_path).unwrap_or_default();
        let stripped = strip_comments_and_strings(&content);

        if rel_norm.starts_with("src/core/")
            || rel_norm.starts_with("src/snapping/")
            || rel_norm.contains("patrix-core/src/")
        {
            for (idx, line) in stripped.lines().enumerate() {
                if line.contains("crate::tokens") || line.contains("patrix_gui::") {
                    violations.push(format!(
                        "  - {rel}:{}: upward layer dependency `core -> tokens/gui` (move math constants to `math_consts.rs` and domain types to `core`)",
                        idx + 1
                    ));
                }
            }
        }

        if rel_norm.starts_with("src/tokens/") {
            for (idx, line) in stripped.lines().enumerate() {
                for pat in forbidden_token_domain_types {
                    if line.contains(pat) {
                        violations.push(format!(
                            "  - {rel}:{}: domain/interaction type `{pat}` misplaced in `src/tokens/`",
                            idx + 1
                        ));
                    }
                }
            }
        }

        for (idx, line) in stripped.lines().enumerate() {
            for pat in evasion_patterns {
                if line.contains(pat) {
                    violations.push(format!(
                        "  - {rel}:{}: linter-evading constant arithmetic `{pat}` (use a named constant in `math_consts.rs`)",
                        idx + 1
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "\n[FATAL ARCHITECTURE GUARD VIOLATION] tc_arch_011 (No Layer Cycles, Token Dumping, or Linter-Evasion Arithmetic):\n\
         Found {} violation(s):\n{}\n",
        violations.len(),
        violations.join("\n")
    );
}

#[test]
fn tc_arch_012_no_cross_domain_exile_or_write_only_state_fields() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws_root = resolve_workspace_root(manifest_dir);
    let mut files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut files);

    let mut violations = Vec::new();
    let mut all_stripped = Vec::new();

    for file_path in &files {
        let rel = file_path
            .strip_prefix(&ws_root)
            .unwrap_or(file_path)
            .to_string_lossy();
        let rel_norm = rel.replace('\\', "/");
        let content = fs::read_to_string(file_path).unwrap_or_default();
        let stripped = strip_comments_and_strings(&content);

        if rel_norm.starts_with("src/core/") && rel_norm != "src/core/mod.rs" {
            for (idx, line) in stripped.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("pub use crate::core::") {
                    violations.push(format!(
                        "  - {rel}:{}: cross-domain sibling re-export `{trimmed}` (do not exile overflow items into unrelated sibling files)",
                        idx + 1
                    ));
                }
            }
        }

        if rel_norm == "src/core/clipboard_ops.rs" && stripped.contains("fn flatten_contour_points")
        {
            violations.push(
                "  - src/core/clipboard_ops.rs: path/contour flattening exiled into clipboard module"
                    .to_string(),
            );
        }
        if rel_norm == "src/core/boolean_geom.rs"
            && stripped.contains("fn split_closed_ring_at_indices")
        {
            violations.push(
                "  - src/core/boolean_geom.rs: knife ring-slicing helper exiled into boolean geometry module"
                    .to_string(),
            );
        }

        all_stripped.push((rel_norm, stripped));
    }

    for shadow_field in [
        "editing_background",
        "center_in_width_count",
        "hovered_pen_close_node",
    ] {
        let decl_pat = format!("pub {shadow_field}:");
        let declared = all_stripped.iter().any(|(_, src)| src.contains(&decl_pat));
        if declared {
            violations.push(format!(
                "  - state: redundant/write-only shadow field `pub {shadow_field}`"
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "\n[FATAL ARCHITECTURE GUARD VIOLATION] tc_arch_012 (No Cross-Domain Exile or Write-Only Shadow State Fields):\n\
         Found {} violation(s):\n{}\n",
        violations.len(),
        violations.join("\n")
    );
}

#[test]
fn tc_arch_013_no_unbounded_per_frame_allocations_or_gpu_uploads_in_paint() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws_root = resolve_workspace_root(manifest_dir);
    let mut files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut files);

    let mut violations = Vec::new();
    let mut pen_hover_hit_test_files = Vec::new();

    for file_path in &files {
        let rel = file_path
            .strip_prefix(&ws_root)
            .unwrap_or(file_path)
            .to_string_lossy();
        let rel_norm = rel.replace('\\', "/");
        let content = fs::read_to_string(file_path).unwrap_or_default();
        let stripped = strip_comments_and_strings(&content);

        if (rel_norm.starts_with("src/render/")
            || rel_norm.contains("patrix-render/src/")
            || rel_norm.contains("painter")
            || rel_norm.contains("desktop_shell"))
            && !rel_norm.contains("cache")
            && !rel_norm.contains("v060_preview")
        {
            for (idx, line) in stripped.lines().enumerate() {
                let line_num = idx + 1;
                if line.contains(".load_texture(") {
                    violations.push(format!(
                        "  - {rel}:{line_num}: per-frame GPU `.load_texture(` inside render pass (must cache texture handle by `content_generation`)"
                    ));
                }
                if line.contains("compute_active_extra_nodes(")
                    || line.contains("compute_layer_extra_nodes(")
                {
                    violations.push(format!(
                        "  - {rel}:{line_num}: per-frame O(S^2) cubic intersection solver `compute_*_extra_nodes(` inside render pass (must read from generation cache)"
                    ));
                }
                if line.contains("compute_expanded_stroke_contours(") {
                    violations.push(format!(
                        "  - {rel}:{line_num}: per-frame stroke offset expansion `compute_expanded_stroke_contours(` inside render pass (must read from generation cache)"
                    ));
                }
                if rel_norm == "src/render/tessellation.rs"
                    && line.contains("contour_to_bezpath_with_splines(")
                {
                    violations.push(format!(
                        "  - {rel}:{line_num}: `tessellation.rs` re-runs `contour_to_bezpath_with_splines` solver on every frame instead of using cached `BezPath`s"
                    ));
                }
            }
        }

        if stripped.contains("hit_test_pen_node(")
            && !rel_norm.ends_with("hit_test.rs")
            && !rel_norm.ends_with("mod.rs")
        {
            pen_hover_hit_test_files.push(rel_norm);
        }
    }

    if pen_hover_hit_test_files.len() > 1 {
        violations.push(format!(
            "  - duplicate hover hit-testing (`hit_test_pen_node`) executed across {} files in a single frame: {}",
            pen_hover_hit_test_files.len(),
            pen_hover_hit_test_files.join(", ")
        ));
    }

    assert!(
        violations.is_empty(),
        "\n[FATAL ARCHITECTURE GUARD VIOLATION] tc_arch_013 (Zero Uncached Solvers, GPU Texture Uploads, or Triple Hit-Tests in Render Loop):\n\
         Found {} battery/CPU hotspot violation(s):\n{}\n",
        violations.len(),
        violations.join("\n")
    );
}

#[test]
fn tc_arch_014_no_copy_pasted_functions_or_duplicate_solvers() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws_root = resolve_workspace_root(manifest_dir);
    let mut files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut files);

    let mut violations = Vec::new();
    let mut eval_cubic_defs = Vec::new();
    let mut bool_seg_extractors = Vec::new();
    let mut schneider_fitters = Vec::new();
    let mut gauss_solvers = Vec::new();
    let mut diamond_helpers = Vec::new();
    let mut dashed_line_helpers = Vec::new();

    for file_path in &files {
        let rel = file_path
            .strip_prefix(&ws_root)
            .unwrap_or(file_path)
            .to_string_lossy();
        let rel_norm = rel.replace('\\', "/");
        let content = fs::read_to_string(file_path).unwrap_or_default();
        let stripped = strip_comments_and_strings(&content);

        for (idx, line) in stripped.lines().enumerate() {
            let line_num = idx + 1;
            let loc = format!("{rel_norm}:{line_num}");
            if line.contains("fn eval_cubic(") || line.contains("fn eval_cubic_pt(") {
                eval_cubic_defs.push(loc.clone());
            }
            if line.contains("fn extract_bool_segs(")
                || line.contains("fn extract_contour_bool_segs(")
            {
                bool_seg_extractors.push(loc.clone());
            }
            if line.contains("fn fit_cubic_to_segment_chain(")
                || line.contains("fn fit_single_cubic(")
            {
                schneider_fitters.push(loc.clone());
            }
            if line.contains("fn solve_linear_system(") || line.contains("fn solve_homography_8x8(")
            {
                gauss_solvers.push(loc.clone());
            }
            if line.contains("fn diamond_points(") || line.contains("fn paint_diamond_") {
                diamond_helpers.push(loc.clone());
            }
            if line.contains("fn paint_dashed_line_")
                || line.contains("fn paint_dashed_segment_")
                || line.contains("fn draw_dashed_line_")
            {
                dashed_line_helpers.push(loc);
            }
        }
    }

    if !eval_cubic_defs.is_empty() {
        violations.push(format!(
            "  - handwritten cubic Bernstein evaluators found ({}); use `kurbo::CubicBez::eval` instead: {}",
            eval_cubic_defs.len(),
            eval_cubic_defs.join(", ")
        ));
    }
    if bool_seg_extractors.len() > 1 {
        violations.push(format!(
            "  - copy-pasted contour-to-cubic-segments extractor defined {} times: {}",
            bool_seg_extractors.len(),
            bool_seg_extractors.join(", ")
        ));
    }
    if schneider_fitters.len() > 1 {
        violations.push(format!(
            "  - duplicate Schneider cubic curve-fitting solver defined {} times: {}",
            schneider_fitters.len(),
            schneider_fitters.join(", ")
        ));
    }
    if gauss_solvers.len() > 1 {
        violations.push(format!(
            "  - duplicate Gaussian elimination linear solver defined {} times: {}",
            gauss_solvers.len(),
            gauss_solvers.join(", ")
        ));
    }
    if diamond_helpers.len() > 1 {
        violations.push(format!(
            "  - duplicate diamond geometry helper defined {} times: {}",
            diamond_helpers.len(),
            diamond_helpers.join(", ")
        ));
    }
    if dashed_line_helpers.len() > 1 {
        violations.push(format!(
            "  - duplicate dashed-line walker defined {} times: {}",
            dashed_line_helpers.len(),
            dashed_line_helpers.join(", ")
        ));
    }

    assert!(
        violations.is_empty(),
        "\n[FATAL ARCHITECTURE GUARD VIOLATION] tc_arch_014 (Zero Copy-Pasted Functions or Duplicate CAGD Solvers):\n\
         Found {} DRY violation group(s):\n{}\n",
        violations.len(),
        violations.join("\n")
    );
}

#[test]
fn tc_arch_015_std_arch_complete_guardrails_and_nasa_power_of_ten() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws_root = resolve_workspace_root(manifest_dir);
    let mut files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut files);

    let mut violations = Vec::new();

    // 1. Verify Cargo.toml [lints] / [workspace.lints] & clippy.toml COTS AST Gate (STD-ARCH Regel 19)
    let cargo_toml = fs::read_to_string(ws_root.join("Cargo.toml")).unwrap_or_default();
    let has_lints_section =
        cargo_toml.contains("[lints.rust]") || cargo_toml.contains("[workspace.lints.rust]");
    if !has_lints_section {
        violations.push(
            "  - Cargo.toml: missing `[lints.rust]` / `[workspace.lints.rust]` section (STD-ARCH Regel 19)"
                .to_string(),
        );
    }
    for required_lint in [
        "unsafe_code = \"forbid\"",
        "dead_code = \"forbid\"",
        "unwrap_used = \"deny\"",
        "expect_used = \"deny\"",
        "iter_over_hash_type = \"deny\"",
        "cognitive_complexity = \"deny\"",
        "too_many_lines = \"deny\"",
    ] {
        if !cargo_toml.contains(required_lint) {
            violations.push(format!(
                "  - Cargo.toml: missing required COTS AST lint `{required_lint}` (STD-ARCH Regel 19)"
            ));
        }
    }
    if !ws_root.join("clippy.toml").exists() {
        violations
            .push("  - clippy.toml: missing from repository root (STD-ARCH Regel 19)".to_string());
    }

    // 2. Scan src/ for #[allow(clippy::...)], token arithmetic, throwaway state, render I/O, fonts, UiBuilder salt, and debug_assert! contracts
    let forbidden_bypass = [
        "fn pt(",
        "fn half_pt(",
        "fn tenth_pt(",
        "MacNibPrimitives",
        "rect_box(",
        "let mut checked =",
        "let mut _ =",
    ];
    let forbidden_fonts = [
        concat!("SF", "NS.ttf"),
        concat!("SF", "NS.otf"),
        "SFPro.ttf",
        "SFCompact.ttf",
        "Menlo.ttc",
        "LucidaGrande.ttc",
    ];
    let mut core_debug_assert_count = 0usize;

    for file_path in &files {
        let rel = file_path
            .strip_prefix(&ws_root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .replace('\\', "/");
        let content = fs::read_to_string(file_path).unwrap_or_default();
        let stripped = strip_comments_and_strings(&content);

        if rel.starts_with("src/core/") || rel.contains("patrix-core/src/") {
            core_debug_assert_count += stripped.matches("debug_assert!").count();
        }

        let is_render = rel.starts_with("src/render/")
            || rel.contains("patrix-render/src/")
            || rel.ends_with("painter_pass.rs")
            || rel.ends_with("egui_adapter.rs");

        for (idx, line) in stripped.lines().enumerate() {
            let lnum = idx + 1;
            let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            if compact.contains("allow(clippy::") {
                violations.push(format!(
                    "  - {rel}:{lnum}: forbidden `#[allow(clippy::...)]` lint suppression in `src/` (NASA JPL Rule 4 / STD-ARCH Regel 19)"
                ));
            }
            for helper in &forbidden_bypass {
                if line.contains(helper) {
                    violations.push(format!(
                        "  - {rel}:{lnum}: forbidden bypass/throwaway `{helper}` (STD-ARCH Guardrail 1/9)"
                    ));
                }
            }
            if is_render {
                for bad_io in [
                    "fs::read",
                    "std::fs::",
                    "File::open",
                    "serde_json::from_",
                    "request_repaint(",
                    "request_repaint_after(",
                ] {
                    if line.contains(bad_io) {
                        violations.push(format!(
                            "  - {rel}:{lnum}: forbidden `{bad_io}` in render/paint module (STD-ARCH Regel 18)"
                        ));
                    }
                }
            }
        }

        for (idx, line) in content.lines().enumerate() {
            let code_line = line.split("//").next().unwrap_or("");
            for bad_font in &forbidden_fonts {
                if code_line.contains(bad_font) && code_line.contains("include_bytes!") {
                    violations.push(format!(
                        "  - {rel}:{}: forbidden proprietary font `{bad_font}` (STD-ARCH Guardrail 13)",
                        idx + 1
                    ));
                }
            }
        }

        let needle = "UiBuilder::new()";
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
                    "  - {rel}:{line_num}: `UiBuilder::new()` missing `.id_salt(...)` (STD-ARCH §3.4)"
                ));
            }
            from = abs + needle.len();
        }
    }

    if core_debug_assert_count < 10 {
        violations.push(format!(
            "  - core: insufficient Design-by-Contract `debug_assert!` pre/postcondition checks (found {core_debug_assert_count}, require >= 10 per NASA JPL Rule 5 / STD-ARCH Regel 22)"
        ));
    }

    assert!(
        violations.is_empty(),
        "\n[FATAL ARCHITECTURE GUARD VIOLATION] tc_arch_015 (Complete STD-ARCH & NASA Power of Ten Guardrails):\n\
         Found {} violation(s):\n{}\n",
        violations.len(),
        violations.join("\n")
    );
}

#[test]
fn tc_arch_016_senior_maturity_no_tuple_smuggling_or_unbounded_god_structs() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws_root = resolve_workspace_root(manifest_dir);
    let mut files = Vec::new();
    collect_workspace_src_files(manifest_dir, &mut files);

    let mut violations = Vec::new();

    // 1. Enforce `too_many_arguments = "deny"` in Cargo.toml [lints.clippy]
    let cargo_toml = fs::read_to_string(ws_root.join("Cargo.toml")).unwrap_or_default();
    if !cargo_toml.contains("too_many_arguments = \"deny\"") {
        violations.push(
            "  - Cargo.toml: missing `too_many_arguments = \"deny\"` in `[lints.clippy]` / `[workspace.lints.clippy]` (SPEC-014 §2.1)"
                .to_string(),
        );
    }

    for file_path in &files {
        let rel = file_path
            .strip_prefix(&ws_root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .replace('\\', "/");
        let content = fs::read_to_string(file_path).unwrap_or_default();
        let stripped = strip_comments_and_strings(&content);

        // 2. Scan all `fn` parameter lists for anonymous >= 3-element tuple parameters `(A, B, C, ...)`
        let bytes = stripped.as_bytes();
        let mut idx = 0;
        while idx + 3 < bytes.len() {
            if bytes[idx..].starts_with(b"fn ")
                && (idx == 0 || !bytes[idx - 1].is_ascii_alphanumeric() && bytes[idx - 1] != b'_')
            {
                let fn_line = stripped[..idx].lines().count().max(1);
                let after_fn = &stripped[idx + 3..];
                let name_end = after_fn
                    .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .unwrap_or(0);
                let fn_name = &after_fn[..name_end];
                if let Some(open_rel) = after_fn.find('(') {
                    let sig_start = idx + 3 + open_rel + 1;
                    let mut depth = 1usize;
                    let mut p = sig_start;
                    while p < bytes.len() && depth > 0 {
                        if bytes[p] == b'(' {
                            depth += 1;
                        } else if bytes[p] == b')' {
                            depth -= 1;
                        }
                        p += 1;
                    }
                    let sig = &stripped[sig_start..p.saturating_sub(1)];
                    for (t_idx, _) in sig.match_indices(':') {
                        let rest = sig[t_idx + 1..].trim_start();
                        let rest = rest
                            .strip_prefix('&')
                            .map_or(rest, str::trim_start)
                            .strip_prefix("mut")
                            .map_or(
                                rest.strip_prefix('&').map_or(rest, str::trim_start),
                                str::trim_start,
                            );
                        if rest.starts_with('(') {
                            let mut t_depth = 1usize;
                            let mut q = 1usize;
                            let rbytes = rest.as_bytes();
                            let mut top_commas = 0usize;
                            while q < rbytes.len() && t_depth > 0 {
                                match rbytes[q] {
                                    b'(' | b'<' | b'[' => t_depth += 1,
                                    b')' | b'>' | b']' => t_depth = t_depth.saturating_sub(1),
                                    b',' if t_depth == 1 => top_commas += 1,
                                    _ => {}
                                }
                                q += 1;
                            }
                            if top_commas >= 2 {
                                let tuple_ty = &rest[..q];
                                violations.push(format!(
                                    "  - {rel}:{fn_line}: `fn {fn_name}` uses anonymous {}-tuple parameter `{tuple_ty}` (SPEC-014 §2.1: replace with a named domain struct)",
                                    top_commas + 1
                                ));
                            }
                        }
                    }
                    idx = p;
                    continue;
                }
            }
            idx += 1;
        }

        // 3. Enforce struct field ceilings (<= 25 for DragSessionTransients, <= 35 for domain/snapshot/token structs, <= 90 for root canvas state)
        let mut s_idx = 0;
        while let Some(pos) = stripped[s_idx..].find("struct ") {
            let abs = s_idx + pos;
            if abs > 0 && (bytes[abs - 1].is_ascii_alphanumeric() || bytes[abs - 1] == b'_') {
                s_idx = abs + 7;
                continue;
            }
            let struct_line = stripped[..abs].lines().count().max(1);
            let after_kw = &stripped[abs + 7..];
            let name_len = after_kw
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(0);
            let struct_name = &after_kw[..name_len];
            let header_end = after_kw.find(['{', ';']).unwrap_or(after_kw.len());
            if after_kw.as_bytes().get(header_end) == Some(&b'{') {
                let body_start = abs + 7 + header_end + 1;
                let mut b_depth = 1usize;
                let mut q = body_start;
                while q < bytes.len() && b_depth > 0 {
                    if bytes[q] == b'{' {
                        b_depth += 1;
                    } else if bytes[q] == b'}' {
                        b_depth -= 1;
                    }
                    q += 1;
                }
                let body = &stripped[body_start..q.saturating_sub(1)];
                if struct_name == "DrawingToolState" && body.contains("controller_ports") {
                    violations.push(format!(
                        "  - {rel}:{struct_line}: `DrawingToolState` must not contain hardware `controller_ports` (SPEC-014 §2.2 / §2.7)"
                    ));
                }
                if struct_name == "GrafiaCanvasState" && body.contains("pub smart_corners:") {
                    violations.push(format!(
                        "  - {rel}:{struct_line}: `GrafiaCanvasState` must not duplicate `pub smart_corners` alongside `curve_harmony.smart_corners` (SPEC-014 §2.7)"
                    ));
                }
                let field_count = body
                    .lines()
                    .filter(|l| {
                        let t = l.trim();
                        !t.is_empty() && !t.starts_with('#') && t.contains(':')
                    })
                    .count();
                let limit = if struct_name == "GrafiaCanvasState" {
                    89
                } else if struct_name == "DragSessionTransients" {
                    25
                } else {
                    35
                };
                if field_count > limit {
                    violations.push(format!(
                        "  - {rel}:{struct_line}: `struct {struct_name}` has {field_count} fields (exceeds SPEC-014 §2.2 limit of <= {limit} fields; decompose into cohesive sub-structs)"
                    ));
                }
                s_idx = q;
            } else {
                s_idx = abs + 7 + header_end;
            }
        }
    }

    // 4. Orphan-Test Guard & Ban on `.rs` Source-File String-Grepping inside Functional Integration Tests (`SPEC-014 §2.7`)
    let tests_dir = ws_root.join("tests");
    if let Ok(entries) = fs::read_dir(&tests_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().is_some_and(|e| e == "rs") {
                let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or_default();
                let rel_test = format!("tests/{fname}");
                if cargo_toml.contains("autotests = false") && !cargo_toml.contains(&rel_test) {
                    violations.push(format!(
                        "  - Cargo.toml: orphan test file `{rel_test}` is not registered in a `[[test]]` block while `autotests = false` (SPEC-014 §2.7)"
                    ));
                }
                if fname != "architecture_guard_tests.rs" {
                    let t_content = fs::read_to_string(&p).unwrap_or_default();
                    let has_read_to_string = t_content.contains("read_to_string");
                    for (l_idx, line) in t_content.lines().enumerate() {
                        let code_part = line.split("//").next().unwrap_or("").trim();
                        if !code_part.starts_with("#[path")
                            && (code_part.contains("read_to_string") && code_part.contains(".rs\"")
                                || (has_read_to_string
                                    && code_part.contains(".rs\"")
                                    && (code_part.contains("join(")
                                        || code_part.contains("Path::new("))))
                        {
                            violations.push(format!(
                                "  - {rel_test}:{}: forbidden `.rs` source-file string-grepping in functional integration test `{}` (SPEC-014 §2.7: test runtime behavior via public APIs)",
                                l_idx + 1,
                                code_part
                            ));
                        }
                    }
                }
            }
        }
    }

    // 5. Examples UI Layout Magic-Number Hygiene (`inspector_ui.rs`, `demo_panels.rs`, `desktop_shell.rs`)
    for ui_rel in [
        "examples/demo_host_shell/inspector_ui.rs",
        "examples/demo_host_shell/demo_panels.rs",
        "examples/demo_host_shell/desktop_shell.rs",
        "examples/demo_host_shell/spline_and_trace_presets.rs",
    ] {
        let ui_path = ws_root.join(ui_rel);
        if !ui_path.exists() {
            continue;
        }
        let content = fs::read_to_string(&ui_path).unwrap_or_default();
        let stripped = strip_comments_and_strings(&content);
        for (line_idx, line) in stripped.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("const ") || trimmed.starts_with("pub const ") {
                continue;
            }
            for val in extract_float_literals(line) {
                let is_allowed = ALLOWED_RATIOS
                    .iter()
                    .any(|&ratio| (val - ratio).abs() < 1e-6);
                if !is_allowed {
                    violations.push(format!(
                        "  - {ui_rel}:{}: raw float literal `{val}` in example UI layout (SPEC-014 §2.7: move to named constant in `examples/demo_host_shell/toolbar_specs.rs`)",
                        line_idx + 1
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "\n[FATAL ARCHITECTURE GUARD VIOLATION] tc_arch_016 (Senior Architectural Maturity — Zero Tuple Smuggling & Struct Field Ceiling):\n\
         Found {} violation(s):\n{}\n",
        violations.len(),
        violations.join("\n")
    );
}
