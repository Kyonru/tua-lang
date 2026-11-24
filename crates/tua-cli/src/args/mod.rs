use crate::error::CliError;
use crate::package::{PackageOptions, PackageTarget};
use std::path::PathBuf;

pub const HELP: &str = "\
tua - typed Lua tooling for LuaJIT/LÖVE projects

Usage:
  tua <command> [options]
  tua --help
  tua --version [--json]

Commands:
  init      Create a default tua.toml and src/ directory
  actions   Add GitHub Actions for game packaging and draft releases
  check     Parse and type-check the project
  fmt       Format project source files
  lint      Run standard and configured lint rules
  test      Build and run unit tests with Busted
  build     Check and emit Lua output
  package   Build, then package the game as a .love, desktop app, or web site
  traceback Remap generated Lua traceback lines to Tua source
  watch     Rebuild when source files change
  dev       Rebuild on change and hot reload the running LOVE game
  dialogue-extract Export dialogue source text as JSON without changing catalogs
  shader-tools Test shader validators with built-in source and report JSON
  lsp       Start the tua language server over stdio
  api       Start the read-only Tua JSON-RPC API over stdio
  mcp       Start the read-only Tua MCP server over stdio

Options:
  --stdio           With lsp, api, or mcp, use standard input/output
  --workspace PATH  With api or mcp, load PATH instead of the current directory
  --emit-on-error   With build, emit parseable/lowerable files even if errors exist
  --no-cache        With build, bypass the persistent successful-build cache
  --build-report    With build, explain cache decisions and phase wall times on stderr
  --check           With fmt, report files that would change without writing
  --strict          With lint, enable all lint diagnostics
  --deny-warnings   With check, fail when warning diagnostics are present
  --maps <path>     With traceback, source-map directory or .lua.map.json file
  --target <kind>   With package, love (default), windows, macos, web, or custom:<name>
  --list-targets    With package, print configured custom targets as JSON without building
  --love <path>     With package, the LOVE for Windows directory or love.app to fuse with;
                    with dev or check, the LOVE executable to run
  --web-player <path> With package --target web, the LÖVE 11.5 standalone player directory
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
    VersionJson,
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
    pub no_cache: bool,
    pub build_report: bool,
    pub package: Box<PackageOptions>,
    pub fmt_check: bool,
    pub fmt_paths: Vec<PathBuf>,
    pub fmt_stdin_filepath: Option<PathBuf>,
    pub lint_strict: bool,
    pub deny_warnings: bool,
    pub traceback_maps: Option<PathBuf>,
    pub traceback_input: Option<PathBuf>,
    pub workspace: Option<PathBuf>,
    pub test: Box<crate::testing::TestOptions>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandName {
    Init,
    Actions,
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
    ShaderTools,
    DialogueExtract,
    Test,
}

impl CommandName {
    fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "test" => Self::Test,
            "init" => Self::Init,
            "actions" => Self::Actions,
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
            "shader-tools" => Self::ShaderTools,
            "dialogue-extract" => Self::DialogueExtract,
            _ => return None,
        })
    }
}

pub fn command_help(command: CommandName) -> &'static str {
    match command {
        CommandName::Test => "tua test - build and run unit tests with Busted\n\nUsage:\n  tua test [SOURCE-PATH] [--busted PATH] [--filter PATTERN] [--tags TAGS] [--lua PATH] [--list] [--report PATH] [--coverage PATH]\n\n--list returns JSON discovery without executing tests.\n--report writes structured JSON results with mapped source locations.\n--coverage writes source-mapped JSON line coverage (requires LuaCov).\n\nDiscovers *_spec.tua and *_spec.lua below [test].source or source.\nSOURCE-PATH is relative to that discovery root. Builds into temporary .tua output,\nthen runs an installed Busted executable (TUA_BUSTED or busted on PATH).\nTests run with the generated output as their working directory.\n",
        CommandName::DialogueExtract => "tua dialogue-extract - export source text IDs and defaults as JSON\n\nUsage:\n  tua dialogue-extract\n\nReads project dialogue sources only; never executes Lua or overwrites catalogs.\nReview differences and merge additions into configured Lua catalogs.\n",
        CommandName::ShaderTools => "tua shader-tools - test installed shader validators\n\nUsage:\n  tua shader-tools\n\nReports JSON using a built-in shader in an isolated directory; does not load the project.\n",
        CommandName::Init => "tua init - create a Tua project\n\nUsage:\n  tua init\n\nCreates tua.toml and src/main.tua without overwriting existing files, and adds generated folders to .gitignore.\n",
        CommandName::Actions => "tua actions - add GitHub Actions to a Tua project\n\nUsage:\n  tua actions init\n\nAdds .github/workflows/tua-game.yml without overwriting an existing workflow.\nThe workflow builds a .love archive, uploads an Actions artifact, and creates a draft release for v* tags.\n",
        CommandName::Check => "tua check - parse and type-check the project\n\nUsage:\n  tua check [--deny-warnings] [--love PATH]\n",
        CommandName::Fmt => "tua fmt - format project source files\n\nUsage:\n  tua fmt [--check] [PATH ...]\n  tua fmt [--check] --stdin-filepath PATH\n\nExplicit paths accept Tua and Lua files within the current project.\nStdin mode prints only formatted source and never writes files.\n",
        CommandName::Lint => "tua lint - run lint rules\n\nUsage:\n  tua lint [--strict]\n",
        CommandName::Build => "tua build - check and emit Lua output\n\nUsage:\n  tua build [--emit-on-error] [--no-cache] [--build-report]\n\n--no-cache bypasses the persistent successful-build cache.\n--build-report explains cache decisions and reports phase durations on stderr.\n",
        CommandName::Package => "tua package - build and package a LOVE game\n\nUsage:\n  tua package [--target love|windows|macos|web|custom:<name>] [--love PATH] [--web-player PATH] [--name NAME] [--identifier ID]\n  tua package --list-targets\n",
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
        if args.iter().any(|arg| arg == "--json") {
            if args.len() != 2 || !matches!(args[0].as_str(), "-V" | "--version") {
                return Err(CliError::usage("usage: tua --version [--json]"));
            }
            return Ok(CliAction::VersionJson);
        }
        return Ok(CliAction::Version);
    }

    let command = CommandName::parse(&args[0])
        .ok_or_else(|| CliError::usage(format!("unknown command `{}`", args[0])))?;

    if command == CommandName::Actions {
        return if args.get(1).map(String::as_str) == Some("init") && args.len() == 2 {
            Ok(CliAction::Command {
                name: command,
                options: CommandOptions::default(),
            })
        } else {
            Err(CliError::usage("usage: tua actions init"))
        };
    }

    let mut options = CommandOptions::default();

    let mut index = 1;

    while index < args.len() {
        let arg = &args[index];

        match arg.as_str() {
            "--list" if command == CommandName::Test => options.test.list = true,
            flag @ ("--busted" | "--filter" | "--tags" | "--lua" | "--report" | "--coverage")
                if command == CommandName::Test =>
            {
                index += 1;
                let value = args
                    .get(index)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| CliError::usage(format!("`{flag}` expects a value")))?;
                match flag {
                    "--busted" => options.test.busted = Some(PathBuf::from(value)),
                    "--filter" => options.test.filter = Some(value.clone()),
                    "--tags" => options.test.tags = Some(value.clone()),
                    "--report" => options.test.report = Some(PathBuf::from(value)),
                    "--coverage" => options.test.coverage = Some(PathBuf::from(value)),
                    _ => options.test.lua = Some(PathBuf::from(value)),
                }
            }
            raw if command == CommandName::Test && !raw.starts_with('-') => {
                if options.test.path.replace(PathBuf::from(raw)).is_some() {
                    return Err(CliError::usage(
                        "`tua test` accepts at most one source path",
                    ));
                }
            }
            "--no-launch" if command == CommandName::Dev => options.dev_no_launch = true,
            "--list-targets" if command == CommandName::Package => {
                options.package.list_targets = true
            }
            "--emit-on-error" if command == CommandName::Build => options.emit_on_error = true,
            "--no-cache" if command == CommandName::Build => options.no_cache = true,
            "--build-report" if command == CommandName::Build => options.build_report = true,
            "--love" if matches!(command, CommandName::Dev | CommandName::Check) => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(CliError::usage("`--love` expects a path"));
                };
                options.love_executable = Some(PathBuf::from(path));
            }
            flag @ ("--target" | "--love" | "--web-player" | "--name" | "--identifier")
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
                                "unknown package target `{value}`; use love, windows, macos, web, or custom:<name>"
                            ))
                        })?;
                    }
                    "--love" => options.package.love = Some(PathBuf::from(value)),
                    "--web-player" => options.package.web_player = Some(PathBuf::from(value)),
                    "--name" => options.package.name = Some(value.clone()),
                    _ => options.package.identifier = Some(value.clone()),
                }
            }
            "--check" if command == CommandName::Fmt => options.fmt_check = true,
            "--stdin-filepath" if command == CommandName::Fmt => {
                index += 1;
                let path = args
                    .get(index)
                    .filter(|path| !path.is_empty() && !path.starts_with('-'))
                    .ok_or_else(|| CliError::usage("`--stdin-filepath` expects a source path"))?;
                if options
                    .fmt_stdin_filepath
                    .replace(PathBuf::from(path))
                    .is_some()
                {
                    return Err(CliError::usage(
                        "`--stdin-filepath` can only be supplied once",
                    ));
                }
            }
            "--" if command == CommandName::Fmt => {
                options
                    .fmt_paths
                    .extend(args[index + 1..].iter().map(PathBuf::from));
                break;
            }
            path if command == CommandName::Fmt && !path.starts_with('-') => {
                options.fmt_paths.push(PathBuf::from(path));
            }
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

    if options.fmt_stdin_filepath.is_some() && !options.fmt_paths.is_empty() {
        return Err(CliError::usage(
            "stdin formatting cannot be combined with file paths",
        ));
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
