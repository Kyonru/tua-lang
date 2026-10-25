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
  --deny-warnings   With check, fail when warning diagnostics are present
  --maps <path>     With traceback, source-map directory or .lua.map.json file
  --target <kind>   With package, love (default), windows, or macos
  --love <path>     With package, the LOVE for Windows directory or love.app to fuse with;
                    with dev or check, the LOVE executable to run
  --no-launch       With dev, watch and prepare hot reload output without starting LOVE
  --name <name>     With package, the game name (default: project directory name)
  --identifier <id> With package --target macos, the app bundle identifier
  -h, --help        Print command help
  -V, --version     Print version
";

#[derive(Debug, Eq, PartialEq)]
pub enum CliAction {
    Help,
    CommandHelp(CommandName),
    Version,
    Command {
        name: CommandName,
        options: CommandOptions,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandOptions {
    pub love_executable: Option<PathBuf>,
    pub dev_no_launch: bool,
    pub emit_on_error: bool,
    pub package: PackageOptions,
    pub fmt_check: bool,
    pub lint_strict: bool,
    pub deny_warnings: bool,
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

impl CommandName {
    fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "init" => Self::Init,
            "check" => Self::Check,
            "fmt" => Self::Fmt,
            "lint" => Self::Lint,
            "build" => Self::Build,
            "package" => Self::Package,
            "traceback" => Self::Traceback,
            "watch" => Self::Watch,
            "dev" => Self::Dev,
            "lsp" => Self::Lsp,
            "api" => Self::Api,
            "mcp" => Self::Mcp,
            _ => return None,
        })
    }
}

pub fn command_help(command: CommandName) -> &'static str {
    match command {
        CommandName::Init => "tua init - create a Tua project\n\nUsage:\n  tua init\n\nCreates tua.toml and src/main.tua without overwriting existing files, and adds generated folders to .gitignore.\n",
        CommandName::Check => "tua check - parse and type-check the project\n\nUsage:\n  tua check [--deny-warnings] [--love PATH]\n",
        CommandName::Fmt => "tua fmt - format project source files\n\nUsage:\n  tua fmt [--check]\n",
        CommandName::Lint => "tua lint - run lint rules\n\nUsage:\n  tua lint [--strict]\n",
        CommandName::Build => "tua build - check and emit Lua output\n\nUsage:\n  tua build [--emit-on-error]\n",
        CommandName::Package => "tua package - build and package a LOVE game\n\nUsage:\n  tua package [--target love|windows|macos] [--love PATH] [--name NAME] [--identifier ID]\n",
        CommandName::Traceback => "tua traceback - remap generated Lua traceback lines\n\nUsage:\n  tua traceback [--maps PATH] [FILE]\n",
        CommandName::Watch => "tua watch - rebuild when source files change\n\nUsage:\n  tua watch\n",
        CommandName::Dev => "tua dev - rebuild and hot reload a LOVE game\n\nUsage:\n  tua dev [--love PATH] [--no-launch]\n",
        CommandName::Lsp => "tua lsp - start the language server\n\nUsage:\n  tua lsp [--stdio]\n",
        CommandName::Api => "tua api - start the read-only JSON-RPC API\n\nUsage:\n  tua api [--stdio] [--workspace PATH]\n",
        CommandName::Mcp => "tua mcp - start the read-only MCP server\n\nUsage:\n  tua mcp [--stdio] [--workspace PATH]\n",
    }
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
        return match args.first().and_then(|arg| CommandName::parse(arg)) {
            Some(command) => Ok(CliAction::CommandHelp(command)),
            None => Ok(CliAction::Help),
        };
    }

    if args.iter().any(|arg| arg == "-V" || arg == "--version") {
        return Ok(CliAction::Version);
    }

    let command = CommandName::parse(&args[0])
        .ok_or_else(|| CliError::usage(format!("unknown command `{}`", args[0])))?;

    let mut options = CommandOptions::default();

    let mut index = 1;

    while index < args.len() {
        let arg = &args[index];

        match arg.as_str() {
            "--no-launch" if command == CommandName::Dev => options.dev_no_launch = true,
            "--emit-on-error" if command == CommandName::Build => options.emit_on_error = true,
            "--love" if matches!(command, CommandName::Dev | CommandName::Check) => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(CliError::usage("`--love` expects a path"));
                };
                options.love_executable = Some(PathBuf::from(path));
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
            "--deny-warnings" if command == CommandName::Check => options.deny_warnings = true,
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
            "--deny-warnings" => {
                return Err(CliError::usage(
                    "`--deny-warnings` is only valid with `tua check`",
                ));
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
