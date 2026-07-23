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
    let config = DiagnosticsConfig {
        inline_ignores: false,
        ..DiagnosticsConfig::default()
    };
    let output = apply_inline_ignores(
        Path::new("src/main.tua"),
        "-- tua-ignore-next-line TL4018\nplayer.setFilter()\n",
        diagnostics.clone(),
        &config,
    );

    assert_eq!(output, diagnostics);
}

#[test]
fn enabled_config_without_directives_preserves_diagnostic_order() {
    let diagnostics = vec![
        diagnostic(3, DiagnosticCode::LINT_DOT_METHOD_CALL),
        diagnostic(1, DiagnosticCode::TYPE_MISMATCH),
    ];
    let output = apply_inline_ignores(
        Path::new("src/main.tua"),
        "local value = true\n",
        diagnostics.clone(),
        &config(),
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
        "local value = 1 -- tua-ignore-line TL4018 TL4048 TL4998\n",
        vec![
            diagnostic(1, DiagnosticCode::LINT_DOT_METHOD_CALL),
            diagnostic(1, DiagnosticCode::LINT_MISLEADING_AND_OR),
        ],
        &config(),
    );

    assert_eq!(output.len(), 1, "{output:?}");
    assert_eq!(
        output[0].code,
        Some(DiagnosticCode::LINT_INLINE_IGNORE_INVALID)
    );
    assert!(output[0].message.contains("TL4998"));
}

#[test]
fn file_scope_is_available_at_the_top_by_default() {
    let output = apply_inline_ignores(
        Path::new("src/main.tua"),
        "-- tua-ignore-file TL4018\nplayer.setFilter()\nplayer.setFilter()\n",
        vec![
            diagnostic(2, DiagnosticCode::LINT_DOT_METHOD_CALL),
            diagnostic(3, DiagnosticCode::LINT_DOT_METHOD_CALL),
        ],
        &config(),
    );

    assert!(output.is_empty(), "{output:?}");
}

#[test]
fn file_scope_is_rejected_after_source_code() {
    let output = apply_inline_ignores(
        Path::new("src/main.tua"),
        "local ready = true\n-- tua-ignore-file TL4018\nplayer.setFilter()\n",
        vec![diagnostic(3, DiagnosticCode::LINT_DOT_METHOD_CALL)],
        &config(),
    );

    assert_eq!(output.len(), 2, "{output:?}");
    assert!(output
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_INLINE_IGNORE_INVALID)));
    assert!(output
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_DOT_METHOD_CALL)));
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
fn broad_and_empty_suppressions_are_rejected() {
    for directive in [
        "-- tua-ignore-next-line",
        "-- tua-ignore-next-line TL4000",
        "-- tua-ignore-next-line lint",
    ] {
        let source = format!("{directive}\nplayer.setFilter()\n");
        let output = apply_inline_ignores(
            Path::new("src/main.tua"),
            &source,
            vec![diagnostic(2, DiagnosticCode::LINT_DOT_METHOD_CALL)],
            &config(),
        );

        assert_eq!(output.len(), 2, "{directive}: {output:?}");
        assert!(output
            .iter()
            .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_INLINE_IGNORE_INVALID)));
        assert!(output
            .iter()
            .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_DOT_METHOD_CALL)));
    }
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
