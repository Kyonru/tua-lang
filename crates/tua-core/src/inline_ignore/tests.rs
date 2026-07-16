use std::path::Path;

use super::*;

fn config() -> DiagnosticsConfig {
    DiagnosticsConfig {
        inline_ignores: true,
        ..DiagnosticsConfig::default()
    }
}

fn diagnostic(line: usize, code: DiagnosticCode) -> Diagnostic {
    Diagnostic::new(
        Path::new("src/main.tua"),
        line,
        3,
        DiagnosticSeverity::Warning,
        "diagnostic",
    )
    .with_code(code)
}

#[test]
fn disabled_config_leaves_diagnostics_unchanged() {
    let diagnostics = vec![diagnostic(2, DiagnosticCode::LINT_DOT_METHOD_CALL)];
    let output = apply_inline_ignores(
        Path::new("src/main.tua"),
        "-- tua-ignore-next-line TL4018\nplayer.setFilter()\n",
        diagnostics.clone(),
        &DiagnosticsConfig::default(),
    );

    assert_eq!(output, diagnostics);
}

#[test]
fn next_line_exact_code_suppresses_matching_diagnostic() {
    let output = apply_inline_ignores(
        Path::new("src/main.tua"),
        "-- tua-ignore-next-line TL4018\nplayer.setFilter()\n",
        vec![diagnostic(2, DiagnosticCode::LINT_DOT_METHOD_CALL)],
        &config(),
    );

    assert!(output.is_empty(), "{output:?}");
}

#[test]
fn line_scope_supports_multiple_codes_and_reports_unknown_codes() {
    let output = apply_inline_ignores(
        Path::new("src/main.tua"),
        "local value = 1 -- tua-ignore-line TL4018 TL4042\n",
        vec![diagnostic(1, DiagnosticCode::LINT_DOT_METHOD_CALL)],
        &config(),
    );

    assert_eq!(output.len(), 1, "{output:?}");
    assert_eq!(
        output[0].code,
        Some(DiagnosticCode::LINT_INLINE_IGNORE_INVALID)
    );
    assert!(output[0].message.contains("TL4042"));
}

#[test]
fn file_scope_is_available_when_configured() {
    let mut config = config();

    config
        .inline_ignore_scopes
        .push("tua-ignore-file".to_owned());

    let output = apply_inline_ignores(
        Path::new("src/main.tua"),
        "-- tua-ignore-file TL4018\nplayer.setFilter()\nplayer.setFilter()\n",
        vec![
            diagnostic(2, DiagnosticCode::LINT_DOT_METHOD_CALL),
            diagnostic(3, DiagnosticCode::LINT_DOT_METHOD_CALL),
        ],
        &config,
    );

    assert!(output.is_empty(), "{output:?}");
}

#[test]
fn unused_suppression_reports_warning() {
    let output = apply_inline_ignores(
        Path::new("src/main.tua"),
        "-- tua-ignore-next-line TL4018\nplayer:setFilter()\n",
        Vec::new(),
        &config(),
    );

    assert_eq!(output.len(), 1, "{output:?}");
    assert_eq!(
        output[0].code,
        Some(DiagnosticCode::LINT_INLINE_IGNORE_UNUSED)
    );
}

#[test]
fn category_groups_suppress_matching_codes() {
    let output = apply_inline_ignores(
        Path::new("src/main.tua"),
        "-- tua-ignore-next-line TL4000\nplayer.setFilter()\n",
        vec![diagnostic(2, DiagnosticCode::LINT_DOT_METHOD_CALL)],
        &config(),
    );

    assert!(output.is_empty(), "{output:?}");
}

#[test]
fn domain_category_suppresses_domain_diagnostics() {
    let output = apply_inline_ignores(
        Path::new("src/main.tua"),
        "-- tua-ignore-next-line domain\nlocal sprites = {}\n",
        vec![diagnostic(2, DiagnosticCode::DOMAIN_SPRITE)],
        &config(),
    );

    assert!(output.is_empty(), "{output:?}");
}

#[test]
fn comments_inside_strings_are_ignored() {
    let output = apply_inline_ignores(
        Path::new("src/main.tua"),
        "local value = \"-- tua-ignore-next-line TL4018\"\nplayer.setFilter()\n",
        vec![diagnostic(2, DiagnosticCode::LINT_DOT_METHOD_CALL)],
        &config(),
    );

    assert_eq!(output.len(), 1, "{output:?}");
    assert_eq!(output[0].code, Some(DiagnosticCode::LINT_DOT_METHOD_CALL));
}
