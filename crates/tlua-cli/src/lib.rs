mod args;
mod build;
mod check;
mod command;
mod diagnostics;
mod error;
mod source;
mod watch;

pub use args::{parse_args, version_text, CliAction, CommandName, HELP};
pub use command::{run_command, run_lsp_command, run_watch_command};
pub use error::CliError;
