use super::*;
use crate::{check_parsed_source_with_diagnostics_config, parse_tua_source, DiagnosticsConfig};

#[test]
fn creates_missing_require_target_action() {
    let parsed = parse_tua_source("src/main.tua", "local Player = require(\"player\")\n")
        .expect("source should parse");
    let diagnostics = diagnostics(
        &parsed,
        DiagnosticsConfig {
            missing_requires: true,
            ..DiagnosticsConfig::default()
        },
    );
    let actions = code_actions_parsed_source(
        &parsed,
        &diagnostics,
        range(1, 24),
        std::path::Path::new("src"),
        "tua",
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].title, "Create module `player`");
    assert_eq!(
        actions[0].edit.create_files[0].path,
        std::path::Path::new("src/player.tua")
    );
}

#[test]
fn missing_require_target_action_rejects_unsafe_paths() {
    let parsed = parse_tua_source("src/main.tua", "local Player = require(\"../player\")\n")
        .expect("source should parse");
    let diagnostic = Diagnostic::new(
        "src/main.tua",
        1,
        24,
        crate::DiagnosticSeverity::Warning,
        "bad require file `../player`",
    )
    .with_code(DiagnosticCode::LINT_MISSING_REQUIRE);
    let actions = code_actions_parsed_source(
        &parsed,
        &[diagnostic],
        range(1, 24),
        std::path::Path::new("src"),
        "tua",
    );

    assert!(actions.is_empty());
}

#[test]
fn adds_local_annotation_from_inferred_type() {
    let parsed = parse_tua_source("src/main.tua", "local hp = 10\n").expect("source should parse");
    let actions = code_actions_parsed_source(
        &parsed,
        &[],
        range_line(1),
        std::path::Path::new("src"),
        "tua",
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].title, "Add type annotation `: number`");
    assert_eq!(
        actions[0].edit.changes[&std::path::PathBuf::from("src/main.tua")][0].new_text,
        ": number"
    );
}

#[test]
fn removes_unused_local_and_self_assignment() {
    let parsed = parse_tua_source("src/main.tua", "local unused = 1\nhp = hp\n")
        .expect("source should parse");
    let diagnostics = diagnostics(
        &parsed,
        DiagnosticsConfig {
            unused_locals: true,
            self_assignments: true,
            ..DiagnosticsConfig::default()
        },
    );
    let actions = code_actions_parsed_source(
        &parsed,
        &diagnostics,
        range(1, 7),
        std::path::Path::new("src"),
        "tua",
    );

    assert!(actions
        .iter()
        .any(|action| action.title == "Remove unused local"));

    let actions = code_actions_parsed_source(
        &parsed,
        &diagnostics,
        range(2, 1),
        std::path::Path::new("src"),
        "tua",
    );

    assert!(actions
        .iter()
        .any(|action| action.title == "Remove self-assignment"));
}

#[test]
fn renames_duplicate_table_key() {
    let parsed = parse_tua_source("src/main.tua", "local config = { hp = 10, hp = 20 }\n")
        .expect("source should parse");
    let diagnostics = diagnostics(
        &parsed,
        DiagnosticsConfig {
            duplicate_keys: true,
            ..DiagnosticsConfig::default()
        },
    );
    let actions = code_actions_parsed_source(
        &parsed,
        &diagnostics,
        range_line(1),
        std::path::Path::new("src"),
        "tua",
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].title, "Rename duplicate key to `hp2`");
    assert_eq!(
        actions[0].edit.changes[&std::path::PathBuf::from("src/main.tua")][0].new_text,
        "hp2"
    );
}

fn diagnostics(parsed: &ParsedSource, config: DiagnosticsConfig) -> Vec<Diagnostic> {
    check_parsed_source_with_diagnostics_config(parsed, &config)
        .err()
        .unwrap_or_default()
}

fn range(line: usize, column: usize) -> CodeActionRange {
    CodeActionRange {
        start: SourcePosition { line, column },
        end: SourcePosition { line, column },
    }
}

fn range_line(line: usize) -> CodeActionRange {
    CodeActionRange {
        start: SourcePosition { line, column: 1 },
        end: SourcePosition { line, column: 200 },
    }
}
