use std::env;
use std::process::ExitCode;

use tua_cli::{
    parse_args, run_command, run_lsp_command, run_watch_command, version_text, CliAction,
    CommandName, HELP,
};

fn main() -> ExitCode {
    match parse_args(env::args().skip(1)) {
        Ok(CliAction::Help) => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Ok(CliAction::Version) => {
            println!("{}", version_text());
            ExitCode::SUCCESS
        }
        Ok(CliAction::Command {
            name: CommandName::Watch,
            ..
        }) => match env::current_dir() {
            Ok(cwd) => match run_watch_command(cwd) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{}", error.format_stderr());
                    ExitCode::FAILURE
                }
            },
            Err(error) => {
                eprintln!("error: failed to read current directory: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(CliAction::Command {
            name: CommandName::Lsp,
            ..
        }) => match run_lsp_command() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{}", error.format_stderr());
                ExitCode::FAILURE
            }
        },
        Ok(CliAction::Command { name, options }) => match env::current_dir() {
            Ok(cwd) => match run_command(name, options, cwd) {
                Ok(message) => {
                    if name == CommandName::Traceback {
                        print!("{message}");
                    } else {
                        println!("{message}");
                    }

                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{}", error.format_stderr());
                    ExitCode::FAILURE
                }
            },
            Err(error) => {
                eprintln!("error: failed to read current directory: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("{}", error.format_stderr());
            eprintln!();
            eprintln!("Run `tua --help` for usage.");
            ExitCode::FAILURE
        }
    }
}
