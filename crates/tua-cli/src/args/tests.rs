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
        ("build", CommandName::Build),
        ("watch", CommandName::Watch),
        ("lsp", CommandName::Lsp),
    ];

    for (raw, expected) in cases {
        assert_eq!(
            parse_args([raw]),
            Ok(CliAction::Command {
                name: expected,
                emit_on_error: false,
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
            emit_on_error: true,
        })
    );
}

#[test]
fn other_commands_reject_emit_on_error() {
    let error = parse_args(["check", "--emit-on-error"]).unwrap_err();

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
