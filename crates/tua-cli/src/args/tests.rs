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
fn command_help_is_contextual() {
    assert_eq!(
        parse_args(["init", "--help"]),
        Ok(CliAction::CommandHelp(CommandName::Init))
    );
    assert!(command_help(CommandName::Init).contains("tua init"));
    assert!(!command_help(CommandName::Init).contains("--strict"));
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
        ("traceback", CommandName::Traceback),
        ("watch", CommandName::Watch),
        ("dev", CommandName::Dev),
        ("lsp", CommandName::Lsp),
        ("api", CommandName::Api),
        ("mcp", CommandName::Mcp),
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
fn api_and_mcp_accept_workspace_and_stdio() {
    for command in ["api", "mcp"] {
        assert_eq!(
            parse_args([command, "--stdio", "--workspace", "example"]),
            Ok(CliAction::Command {
                name: if command == "api" {
                    CommandName::Api
                } else {
                    CommandName::Mcp
                },
                options: CommandOptions {
                    workspace: Some("example".into()),
                    ..CommandOptions::default()
                },
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
fn traceback_accepts_maps_and_input_file() {
    assert_eq!(
        parse_args(["traceback", "--maps", "build", "sentry.txt"]),
        Ok(CliAction::Command {
            name: CommandName::Traceback,
            options: CommandOptions {
                traceback_maps: Some("build".into()),
                traceback_input: Some("sentry.txt".into()),
                ..CommandOptions::default()
            },
        })
    );
}

#[test]
fn traceback_accepts_maps_equals() {
    assert_eq!(
        parse_args(["traceback", "--maps=build"]),
        Ok(CliAction::Command {
            name: CommandName::Traceback,
            options: CommandOptions {
                traceback_maps: Some("build".into()),
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
fn check_accepts_deny_warnings() {
    assert_eq!(
        parse_args(["check", "--deny-warnings"]),
        Ok(CliAction::Command {
            name: CommandName::Check,
            options: CommandOptions {
                deny_warnings: true,
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
fn other_commands_reject_deny_warnings() {
    let error = parse_args(["lint", "--deny-warnings"]).unwrap_err();

    assert!(error.message.contains("only valid"));
}

#[test]
fn other_commands_reject_maps() {
    let error = parse_args(["build", "--maps", "build"]).unwrap_err();

    assert!(error.message.contains("only valid"));
}

#[test]
fn traceback_rejects_multiple_input_files() {
    let error = parse_args(["traceback", "one.txt", "two.txt"]).unwrap_err();

    assert!(error.message.contains("at most one"));
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

#[test]
fn package_accepts_a_target_runtime_name_and_identifier() {
    assert_eq!(
        parse_args([
            "package",
            "--target",
            "macos",
            "--love",
            "/Applications/love.app",
            "--name",
            "Star Dust",
            "--identifier",
            "io.example.stardust",
        ]),
        Ok(CliAction::Command {
            name: CommandName::Package,
            options: CommandOptions {
                package: crate::package::PackageOptions {
                    target: crate::package::PackageTarget::Macos,
                    love: Some("/Applications/love.app".into()),
                    name: Some("Star Dust".to_owned()),
                    identifier: Some("io.example.stardust".to_owned()),
                },
                ..CommandOptions::default()
            },
        })
    );
    assert!(parse_args(["package", "--target", "linux"]).is_err());
    assert!(parse_args(["package", "--love"]).is_err());
    assert!(parse_args(["build", "--target", "love"]).is_err());
}

#[test]
fn dev_can_prepare_hot_reload_without_launching_a_game() {
    let action = parse_args(["dev", "--no-launch"]).unwrap();
    match action {
        CliAction::Command { name, options } => {
            assert_eq!(name, CommandName::Dev);
            assert!(options.dev_no_launch);
        }
        other => panic!("unexpected action: {other:?}"),
    }
    assert!(parse_args(["build", "--no-launch"]).is_err());
}
