use super::*;

#[test]
fn reserves_spec_code_ranges() {
    let cases = [
        (1, DiagnosticCategory::Syntax),
        (999, DiagnosticCategory::Syntax),
        (1000, DiagnosticCategory::Type),
        (1999, DiagnosticCategory::Type),
        (2000, DiagnosticCategory::LoweringEmission),
        (2999, DiagnosticCategory::LoweringEmission),
        (3000, DiagnosticCategory::ProjectConfig),
        (3999, DiagnosticCategory::ProjectConfig),
        (4000, DiagnosticCategory::Lint),
        (4999, DiagnosticCategory::Lint),
        (5000, DiagnosticCategory::Domain),
        (5999, DiagnosticCategory::Domain),
    ];

    for (number, category) in cases {
        let code = DiagnosticCode::new(number).expect("code should be in reserved range");

        assert_eq!(code.category(), category);
        assert_eq!(code.as_u16(), number);
    }

    assert_eq!(DiagnosticCode::new(0), None);
    assert_eq!(DiagnosticCode::new(6000), None);
}

#[test]
fn formats_cli_diagnostic_with_code_snapshot() {
    let diagnostic = Diagnostic::error(
        "src/player.tua",
        12,
        15,
        "type mismatch: expected number, found string",
    )
    .with_code(DiagnosticCode::new(1002).expect("valid code"));

    assert_eq!(
        diagnostic.format_cli(),
        "src/player.tua:12:15 error TL1002: type mismatch: expected number, found string"
    );
}

#[test]
fn formats_cli_diagnostic_without_code_snapshot() {
    let diagnostic = Diagnostic::new(
        "src/main.tua",
        2,
        3,
        DiagnosticSeverity::Warning,
        "unused local `score`",
    );

    assert_eq!(
        diagnostic.format_cli(),
        "src/main.tua:2:3 warning: unused local `score`"
    );
}

#[test]
fn formats_multiple_cli_diagnostics_snapshot() {
    let diagnostics = vec![
        Diagnostic::error("tua.toml", 1, 1, "unknown config key `mode`")
            .with_code(DiagnosticCode::CONFIG_PARSE),
        Diagnostic::new(
            "src/player.tua",
            4,
            7,
            DiagnosticSeverity::Info,
            "inferred type number",
        ),
    ];

    assert_eq!(
        format_diagnostics_for_cli(&diagnostics),
        "tua.toml:1:1 error TL3001: unknown config key `mode`\n\
src/player.tua:4:7 info: inferred type number"
    );
}

#[test]
fn expands_only_default_ranges_that_start_on_identifiers() {
    let source = "local unusedPlayer = missingPlayer + 1\n";
    let mut diagnostics = vec![
        Diagnostic::new(
            "src/player.tua",
            1,
            7,
            DiagnosticSeverity::Warning,
            "unused local `unusedPlayer`",
        ),
        Diagnostic::new(
            "src/player.tua",
            1,
            22,
            DiagnosticSeverity::Error,
            "undefined variable `missingPlayer`",
        ),
        Diagnostic::new(
            "src/player.tua",
            1,
            36,
            DiagnosticSeverity::Error,
            "operator error",
        ),
        Diagnostic::new(
            "src/player.tua",
            1,
            7,
            DiagnosticSeverity::Warning,
            "already precise",
        )
        .with_range(1, 7, 1, 12),
    ];

    expand_identifier_ranges(source, &mut diagnostics);

    assert_eq!(
        (diagnostics[0].end_line, diagnostics[0].end_column),
        (1, 19)
    );
    assert_eq!(
        (diagnostics[1].end_line, diagnostics[1].end_column),
        (1, 35)
    );
    assert_eq!(
        (diagnostics[2].end_line, diagnostics[2].end_column),
        (1, 37)
    );
    assert_eq!(
        (diagnostics[3].end_line, diagnostics[3].end_column),
        (1, 12)
    );
}
