pub const HELP: &str = "\
tlua - typed Lua tooling for LuaJIT/LÖVE projects

Usage:
  tlua <command> [options]
  tlua --help
  tlua --version

Commands:
  init      Create a default tlua.toml and src/ directory
  check     Parse and type-check the project
  build     Check and emit Lua output
  watch     Rebuild when source files change
  lsp       Start the tlua language server over stdio

Options:
  --emit-on-error   With build, emit parseable/lowerable files even if errors exist
  -h, --help        Print command help
  -V, --version     Print version
";

#[derive(Debug, Eq, PartialEq)]
pub enum CliAction {
    Help,
    Version,
    Command {
        name: CommandName,
        emit_on_error: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandName {
    Init,
    Check,
    Build,
    Watch,
    Lsp,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CliError {
    pub message: String,
}

pub fn parse_args<I, S>(args: I) -> Result<CliAction, CliError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args = args
        .into_iter()
        .map(|arg| arg.as_ref().to_owned())
        .collect::<Vec<_>>();

    if args.is_empty() {
        return Ok(CliAction::Help);
    }

    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        return Ok(CliAction::Help);
    }

    if args.iter().any(|arg| arg == "-V" || arg == "--version") {
        return Ok(CliAction::Version);
    }

    let command = match args[0].as_str() {
        "init" => CommandName::Init,
        "check" => CommandName::Check,
        "build" => CommandName::Build,
        "watch" => CommandName::Watch,
        "lsp" => CommandName::Lsp,
        other => {
            return Err(CliError {
                message: format!("unknown command `{other}`"),
            });
        }
    };

    let mut emit_on_error = false;

    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--emit-on-error" if command == CommandName::Build => emit_on_error = true,
            "--emit-on-error" => {
                return Err(CliError {
                    message: "`--emit-on-error` is only valid with `tlua build`".to_owned(),
                });
            }
            other => {
                return Err(CliError {
                    message: format!("unknown option `{other}`"),
                });
            }
        }
    }

    Ok(CliAction::Command {
        name: command,
        emit_on_error,
    })
}

pub fn version_text() -> String {
    format!("tlua {}", tlua_core::VERSION)
}

pub fn placeholder_text(command: CommandName, emit_on_error: bool) -> String {
    match command {
        CommandName::Init => "tlua init is not implemented yet".to_owned(),
        CommandName::Check => "tlua check is not implemented yet".to_owned(),
        CommandName::Build if emit_on_error => {
            "tlua build --emit-on-error is not implemented yet".to_owned()
        }
        CommandName::Build => "tlua build is not implemented yet".to_owned(),
        CommandName::Watch => "tlua watch is not implemented yet".to_owned(),
        CommandName::Lsp => format!("{} is not implemented yet", tlua_lsp::server_name()),
    }
}

#[cfg(test)]
mod tests {
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
        assert!(version_text().starts_with("tlua "));
    }

    #[test]
    fn parses_known_commands() {
        assert_eq!(
            parse_args(["build"]),
            Ok(CliAction::Command {
                name: CommandName::Build,
                emit_on_error: false,
            })
        );
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
}
