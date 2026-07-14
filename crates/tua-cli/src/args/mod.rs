use crate::error::CliError;
use std::path::PathBuf;

pub const HELP: &str = "\
tua - typed Lua tooling for LuaJIT/LÖVE projects

Usage:
  tua <command> [options]
  tua --help
  tua --version

Commands:
  init      Create a default tua.toml and src/ directory
  check     Parse and type-check the project
  fmt       Format project source files
  lint      Run configured diagnostics as lints
  build     Check and emit Lua output
  traceback Remap generated Lua traceback lines to Tua source
  watch     Rebuild when source files change
  lsp       Start the tua language server over stdio

Options:
  --stdio           With lsp, accepted for editor-client compatibility
  --emit-on-error   With build, emit parseable/lowerable files even if errors exist
  --check           With fmt, report files that would change without writing
  --strict          With lint, enable all lint diagnostics
  --maps <path>     With traceback, source-map directory or .lua.map.json file
  -h, --help        Print command help
  -V, --version     Print version
";

#[derive(Debug, Eq, PartialEq)]
pub enum CliAction {
    Help,
    Version,
    Command {
        name: CommandName,
        options: CommandOptions,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandOptions {
    pub emit_on_error: bool,
    pub fmt_check: bool,
    pub lint_strict: bool,
    pub traceback_maps: Option<PathBuf>,
    pub traceback_input: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandName {
    Init,
    Check,
    Fmt,
    Lint,
    Build,
    Traceback,
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
        "fmt" => CommandName::Fmt,
        "lint" => CommandName::Lint,
        "build" => CommandName::Build,
        "traceback" => CommandName::Traceback,
        "watch" => CommandName::Watch,
        "lsp" => CommandName::Lsp,
        other => {
            return Err(CliError::usage(format!("unknown command `{other}`")));
        }
    };

    let mut options = CommandOptions::default();

    let mut index = 1;

    while index < args.len() {
        let arg = &args[index];

        match arg.as_str() {
            "--emit-on-error" if command == CommandName::Build => options.emit_on_error = true,
            "--check" if command == CommandName::Fmt => options.fmt_check = true,
            "--strict" if command == CommandName::Lint => options.lint_strict = true,
            "--maps" if command == CommandName::Traceback => {
                index += 1;

                let Some(path) = args.get(index) else {
                    return Err(CliError::usage("`--maps` expects a path"));
                };

                options.traceback_maps = Some(PathBuf::from(path));
            }
            raw if command == CommandName::Traceback => {
                if raw.starts_with("--maps=") {
                    let Some((_, path)) = raw.split_once('=') else {
                        unreachable!("prefix check guarantees `=` is present");
                    };

                    if path.is_empty() {
                        return Err(CliError::usage("`--maps` expects a path"));
                    }

                    options.traceback_maps = Some(PathBuf::from(path));
                } else if raw.starts_with('-') {
                    return Err(CliError::usage(format!("unknown option `{raw}`")));
                } else if options.traceback_input.is_some() {
                    return Err(CliError::usage(
                        "`tua traceback` accepts at most one traceback file",
                    ));
                } else {
                    options.traceback_input = Some(PathBuf::from(raw));
                }
            }
            "--stdio" if command == CommandName::Lsp => {}
            "--emit-on-error" => {
                return Err(CliError::usage(
                    "`--emit-on-error` is only valid with `tua build`",
                ));
            }
            "--check" => {
                return Err(CliError::usage("`--check` is only valid with `tua fmt`"));
            }
            "--strict" => {
                return Err(CliError::usage("`--strict` is only valid with `tua lint`"));
            }
            "--maps" => {
                return Err(CliError::usage(
                    "`--maps` is only valid with `tua traceback`",
                ));
            }
            raw if raw.starts_with("--maps=") => {
                return Err(CliError::usage(
                    "`--maps` is only valid with `tua traceback`",
                ));
            }
            other => {
                return Err(CliError::usage(format!("unknown option `{other}`")));
            }
        }

        index += 1;
    }

    Ok(CliAction::Command {
        name: command,
        options,
    })
}

pub fn version_text() -> String {
    format!("tua {}", tua_core::VERSION)
}

#[cfg(test)]
mod tests;
