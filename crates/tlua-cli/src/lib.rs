use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tlua_core::{
    emit_tlua_source, format_diagnostics_for_cli, ConfigError, Diagnostic, DiagnosticCode,
    LoadedConfig,
};

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
    pub diagnostics: Vec<Diagnostic>,
}

impl CliError {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            diagnostics: Vec::new(),
        }
    }

    fn from_config_error(error: ConfigError) -> Self {
        Self {
            message: "project configuration error".to_owned(),
            diagnostics: vec![error.to_diagnostic()],
        }
    }

    fn from_diagnostics(message: impl Into<String>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            message: message.into(),
            diagnostics,
        }
    }

    pub fn format_stderr(&self) -> String {
        if self.diagnostics.is_empty() {
            format!("error: {}", self.message)
        } else {
            format_diagnostics_for_cli(&self.diagnostics)
        }
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
            "--emit-on-error" => {
                return Err(CliError::usage(
                    "`--emit-on-error` is only valid with `tlua build`",
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
    format!("tlua {}", tlua_core::VERSION)
}

fn placeholder_text(command: CommandName) -> String {
    match command {
        CommandName::Init => unreachable!("tlua init is handled before placeholder_text"),
        CommandName::Check => "tlua check is not implemented yet".to_owned(),
        CommandName::Build => unreachable!("tlua build is handled before placeholder_text"),
        CommandName::Watch => "tlua watch is not implemented yet".to_owned(),
        CommandName::Lsp => format!("{} is not implemented yet", tlua_lsp::server_name()),
    }
}

pub fn run_command(
    command: CommandName,
    emit_on_error: bool,
    cwd: impl AsRef<Path>,
) -> Result<String, CliError> {
    match command {
        CommandName::Init => tlua_core::init_project(cwd)
            .map(format_init_result)
            .map_err(CliError::from_config_error),
        CommandName::Build => {
            let loaded =
                tlua_core::load_config_from_dir(&cwd).map_err(CliError::from_config_error)?;

            run_build(cwd, loaded, emit_on_error)
        }
        CommandName::Check | CommandName::Watch => {
            tlua_core::load_config_from_dir(cwd).map_err(CliError::from_config_error)?;

            Ok(placeholder_text(command))
        }
        _ => Ok(placeholder_text(command)),
    }
}

fn run_build(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    _emit_on_error: bool,
) -> Result<String, CliError> {
    let cwd = cwd.as_ref();
    let source_dir = cwd.join(&loaded.config.source);
    let out_dir = cwd.join(&loaded.config.out);
    let source_files =
        discover_source_files(&source_dir, &loaded.config.extension).map_err(|error| {
            CliError::from_diagnostics("failed to discover source files", vec![error])
        })?;
    let mut emitted = 0usize;

    for source_path in source_files {
        let relative_path = source_path
            .strip_prefix(&source_dir)
            .expect("discovered source file should be under source directory");
        let output_path = out_dir.join(relative_path).with_extension("lua");
        let source = fs::read_to_string(&source_path).map_err(|source| {
            CliError::from_diagnostics(
                "failed to read source file",
                vec![io_diagnostic(&source_path, source)],
            )
        })?;
        let output = emit_tlua_source(&source_path, source)
            .map_err(|diagnostics| CliError::from_diagnostics("failed to emit Lua", diagnostics))?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                CliError::from_diagnostics(
                    "failed to create output directory",
                    vec![io_diagnostic(parent, source)],
                )
            })?;
        }

        fs::write(&output_path, output).map_err(|source| {
            CliError::from_diagnostics(
                "failed to write output file",
                vec![io_diagnostic(&output_path, source)],
            )
        })?;

        emitted += 1;
    }

    Ok(format!("built {emitted} file(s)"))
}

fn discover_source_files(source_dir: &Path, extension: &str) -> Result<Vec<PathBuf>, Diagnostic> {
    let mut files = Vec::new();

    discover_source_files_into(source_dir, extension, &mut files)?;
    files.sort();

    Ok(files)
}

fn discover_source_files_into(
    dir: &Path,
    extension: &str,
    files: &mut Vec<PathBuf>,
) -> Result<(), Diagnostic> {
    let entries = fs::read_dir(dir).map_err(|source| io_diagnostic(dir, source))?;

    for entry in entries {
        let entry = entry.map_err(|source| io_diagnostic(dir, source))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| io_diagnostic(&path, source))?;

        if file_type.is_dir() {
            discover_source_files_into(&path, extension, files)?;
        } else if file_type.is_file() && has_extension(&path, extension) {
            files.push(path);
        }
    }

    Ok(())
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value == extension)
}

fn io_diagnostic(path: &Path, source: io::Error) -> Diagnostic {
    Diagnostic::error(path, 1, 1, source.to_string()).with_code(DiagnosticCode::EMIT_IO)
}

fn format_init_result(result: tlua_core::InitResult) -> String {
    let mut lines = vec!["initialized tlua project".to_owned()];

    if result.config_created {
        lines.push("created tlua.toml".to_owned());
    } else {
        lines.push("tlua.toml already exists; left unchanged".to_owned());
    }

    if result.source_created {
        lines.push("created src/".to_owned());
    } else {
        lines.push("src/ already exists".to_owned());
    }

    lines.join("\n")
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

    #[test]
    fn command_placeholders_are_explicitly_unimplemented() {
        let commands = [CommandName::Check, CommandName::Watch, CommandName::Lsp];

        for command in commands {
            assert!(placeholder_text(command).contains("not implemented yet"));
        }
    }

    #[test]
    fn cli_errors_can_format_diagnostics() {
        let diagnostic = Diagnostic::error("tlua.toml", 1, 1, "unknown config key `mode`")
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
}
