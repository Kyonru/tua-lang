use std::env;
use std::process::ExitCode;

use tlua_cli::{parse_args, run_command, version_text, CliAction, HELP};

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
            name,
            emit_on_error,
        }) => match env::current_dir() {
            Ok(cwd) => match run_command(name, emit_on_error, cwd) {
                Ok(message) => {
                    println!("{message}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("error: {}", error.message);
                    ExitCode::FAILURE
                }
            },
            Err(error) => {
                eprintln!("error: failed to read current directory: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("error: {}", error.message);
            eprintln!();
            eprintln!("Run `tlua --help` for usage.");
            ExitCode::FAILURE
        }
    }
}
