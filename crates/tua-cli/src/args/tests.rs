use super::*;

#[test]
fn no_args_prints_help() {
    assert_eq!(parse_args(Vec::<String>::new()), Ok(CliAction::Help));
}

#[test]
fn help_flag_prints_help() {
    assert_eq!(parse_args(["--help"]), Ok(CliAction::Help));
    assert!(HELP.contains("Commands:"));
}

#[test]
fn version_flag_prints_version() {
    assert_eq!(parse_args(["--version"]), Ok(CliAction::Version));
    assert!(version_text().starts_with("tua "));
}

#[test]
fn parses_known_commands() {
    let cases = [
        ("init", CommandName::Init),
        ("check", CommandName::Check),
        ("fmt", CommandName::Fmt),
        ("lint", CommandName::Lint),
        ("build", CommandName::Build),
        ("watch", CommandName::Watch),
        ("lsp", CommandName::Lsp),
    ];

    for (raw, expected) in cases {
        assert_eq!(
            parse_args([raw]),
            Ok(CliAction::Command {
                name: expected,
                options: CommandOptions::default(),
            })
        );
    }
}

#[test]
fn build_accepts_emit_on_error() {
    assert_eq!(
        parse_args(["build", "--emit-on-error"]),
        Ok(CliAction::Command {
            name: CommandName::Build,
            options: CommandOptions {
                emit_on_error: true,
                ..CommandOptions::default()
            },
        })
    );
}

#[test]
fn fmt_accepts_check() {
    assert_eq!(
        parse_args(["fmt", "--check"]),
        Ok(CliAction::Command {
            name: CommandName::Fmt,
            options: CommandOptions {
                fmt_check: true,
                ..CommandOptions::default()
            },
        })
    );
}

#[test]
fn lint_accepts_strict() {
    assert_eq!(
        parse_args(["lint", "--strict"]),
        Ok(CliAction::Command {
            name: CommandName::Lint,
            options: CommandOptions {
                lint_strict: true,
                ..CommandOptions::default()
            },
        })
    );
}

#[test]
fn lsp_accepts_stdio_for_editor_client_compatibility() {
    assert_eq!(
        parse_args(["lsp", "--stdio"]),
        Ok(CliAction::Command {
            name: CommandName::Lsp,
            options: CommandOptions::default(),
        })
    );
}

#[test]
fn other_commands_reject_emit_on_error() {
    let error = parse_args(["check", "--emit-on-error"]).unwrap_err();

    assert!(error.message.contains("only valid"));
}

#[test]
fn other_commands_reject_check() {
    let error = parse_args(["check", "--check"]).unwrap_err();

    assert!(error.message.contains("only valid"));
}

#[test]
fn other_commands_reject_strict() {
    let error = parse_args(["check", "--strict"]).unwrap_err();

    assert!(error.message.contains("only valid"));
}

#[test]
fn rejects_unknown_commands() {
    let error = parse_args(["compile"]).unwrap_err();

    assert!(error.message.contains("unknown command"));
}

#[test]
fn rejects_unknown_options() {
    let error = parse_args(["build", "--fast"]).unwrap_err();

    assert!(error.message.contains("unknown option"));
    assert!(error.message.contains("--fast"));
}
