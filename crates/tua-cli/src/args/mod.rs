use crate::error::CliError;
use crate::package::{PackageOptions, PackageTarget};
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
  lint      Run standard and configured lint rules
  build     Check and emit Lua output
  package   Build, then package the game as a .love (and optionally a fused app)
  traceback Remap generated Lua traceback lines to Tua source
  watch     Rebuild when source files change
  dev       Rebuild on change and hot reload the running LOVE game
  lsp       Start the tua language server over stdio
  api       Start the read-only Tua JSON-RPC API over stdio
  mcp       Start the read-only Tua MCP server over stdio

Options:
  --stdio           With lsp, api, or mcp, use standard input/output
  --workspace PATH  With api or mcp, load PATH instead of the current directory
  --emit-on-error   With build, emit parseable/lowerable files even if errors exist
  --check           With fmt, report files that would change without writing
  --strict          With lint, enable all lint diagnostics
  --maps <path>     With traceback, source-map directory or .lua.map.json file
  --target <kind>   With package, love (default), windows, or macos
  --love <path>     With package, the LOVE for Windows directory or love.app to fuse with;
                    with dev, the LOVE executable to run
  --name <name>     With package, the game name (default: project directory name)
  --identifier <id> With package --target macos, the app bundle identifier
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
    pub dev_love: Option<PathBuf>,
    pub emit_on_error: bool,
    pub package: PackageOptions,
    pub fmt_check: bool,
    pub lint_strict: bool,
    pub traceback_maps: Option<PathBuf>,
    pub traceback_input: Option<PathBuf>,
    pub workspace: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandName {
    Init,
    Check,
    Fmt,
    Lint,
    Build,
    Package,
    Traceback,
    Watch,
    Dev,
    Lsp,
    Api,
    Mcp,
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
        "package" => CommandName::Package,
        "traceback" => CommandName::Traceback,
        "watch" => CommandName::Watch,
        "dev" => CommandName::Dev,
        "lsp" => CommandName::Lsp,
        "api" => CommandName::Api,
        "mcp" => CommandName::Mcp,
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
            "--love" if command == CommandName::Dev => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(CliError::usage("`--love` expects a path"));
                };
                options.dev_love = Some(PathBuf::from(path));
            }
            flag @ ("--target" | "--love" | "--name" | "--identifier")
                if command == CommandName::Package =>
            {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(CliError::usage(format!("`{flag}` expects a value")));
                };
                match flag {
                    "--target" => {
                        options.package.target = PackageTarget::parse(value).ok_or_else(|| {
                            CliError::usage(format!(
                                "unknown package target `{value}`; use love, windows, or macos"
                            ))
                        })?;
                    }
                    "--love" => options.package.love = Some(PathBuf::from(value)),
                    "--name" => options.package.name = Some(value.clone()),
                    _ => options.package.identifier = Some(value.clone()),
                }
            }
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
            "--stdio"
                if matches!(
                    command,
                    CommandName::Lsp | CommandName::Api | CommandName::Mcp
                ) => {}
            "--workspace" if matches!(command, CommandName::Api | CommandName::Mcp) => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(CliError::usage("`--workspace` expects a path"));
                };
                options.workspace = Some(PathBuf::from(path));
            }
            raw if matches!(command, CommandName::Api | CommandName::Mcp)
                && raw.starts_with("--workspace=") =>
            {
                let path = raw.trim_start_matches("--workspace=");
                if path.is_empty() {
                    return Err(CliError::usage("`--workspace` expects a path"));
                }
                options.workspace = Some(PathBuf::from(path));
            }
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
