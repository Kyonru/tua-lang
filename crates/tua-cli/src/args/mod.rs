use crate::error::CliError;

pub const HELP: &str = "\
tua - typed Lua tooling for LuaJIT/LÖVE projects

Usage:
  tua <command> [options]
  tua --help
  tua --version

Commands:
  init      Create a default tua.toml and src/ directory
  check     Parse and type-check the project
  build     Check and emit Lua output
  watch     Rebuild when source files change
  lsp       Start the tua language server over stdio

Options:
  --stdio           With lsp, accepted for editor-client compatibility
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
            return Err(CliError::usage(format!("unknown command `{other}`")));
        }
    };

    let mut emit_on_error = false;

    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--emit-on-error" if command == CommandName::Build => emit_on_error = true,
            "--stdio" if command == CommandName::Lsp => {}
            "--emit-on-error" => {
                return Err(CliError::usage(
                    "`--emit-on-error` is only valid with `tua build`",
                ));
            }
            other => {
                return Err(CliError::usage(format!("unknown option `{other}`")));
            }
        }
    }

    Ok(CliAction::Command {
        name: command,
        emit_on_error,
    })
}

pub fn version_text() -> String {
    format!("tua {}", tua_core::VERSION)
}

#[cfg(test)]
mod tests;
