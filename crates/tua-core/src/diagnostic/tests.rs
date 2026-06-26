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
        (5000, DiagnosticCategory::Plugin),
        (5999, DiagnosticCategory::Plugin),
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
