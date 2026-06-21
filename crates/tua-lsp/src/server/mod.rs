mod capabilities;
mod handlers;
mod protocol;
mod transport;
mod uri;

use std::collections::HashMap;
use std::io::{self, BufReader, Read, Write};
use std::path::PathBuf;

use transport::{read_message, write_message};

pub fn server_name() -> &'static str {
    capabilities::SERVER_NAME
}

pub fn server_description() -> String {
    capabilities::server_description()
}

pub fn run_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();

    run(stdin.lock(), stdout.lock())
}

pub(crate) fn run(reader: impl Read, writer: impl Write) -> io::Result<()> {
    let mut server = LspServer::default();
    let mut reader = BufReader::new(reader);
    let mut writer = writer;

    while let Some(message) = read_message(&mut reader)? {
        if let Some(outbound) = server.handle_message(message) {
            for message in outbound {
                write_message(&mut writer, &message)?;
            }
        }

        if server.should_exit {
            break;
        }
    }

    Ok(())
}

#[derive(Default)]
struct LspServer {
    documents: HashMap<String, String>,
    root_dir: Option<PathBuf>,
    client_watched_files_dynamic_registration: bool,
    next_server_request_id: u64,
    shutdown_requested: bool,
    should_exit: bool,
}

impl LspServer {
    fn next_request_id(&mut self) -> u64 {
        if self.next_server_request_id == 0 {
            self.next_server_request_id = 1;
        }

        let id = self.next_server_request_id;

        self.next_server_request_id += 1;
        id
    }
}

#[cfg(test)]
mod tests;
