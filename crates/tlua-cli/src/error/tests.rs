use super::*;

#[test]
fn cli_errors_can_format_diagnostics() {
    let diagnostic = tlua_core::Diagnostic::error("tlua.toml", 1, 1, "unknown config key `mode`")
        .with_code(tlua_core::DiagnosticCode::CONFIG_PARSE);
    let error = CliError {
        message: "project configuration error".to_owned(),
        diagnostics: vec![diagnostic],
    };

    assert_eq!(
        error.format_stderr(),
        "tlua.toml:1:1 error TL3001: unknown config key `mode`"
    );
}
