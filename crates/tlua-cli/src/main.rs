use std::env;
use std::process::ExitCode;

use tlua_cli::{parse_args, placeholder_text, version_text, CliAction, HELP};

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
        }) => {
            println!("{}", placeholder_text(name, emit_on_error));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {}", error.message);
            eprintln!();
            eprintln!("Run `tlua --help` for usage.");
            ExitCode::FAILURE
        }
    }
}
