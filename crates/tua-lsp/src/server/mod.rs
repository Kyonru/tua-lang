mod analysis;
mod auto_import;
mod capabilities;
mod configuration;
mod handlers;
mod protocol;
mod transport;
mod uri;

use std::collections::BTreeMap;
use std::io::{self, BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use analysis::AnalysisHost;
use configuration::WorkspaceSettings;
use transport::{read_message, write_message};

const SLOW_DIAGNOSTICS_DEBOUNCE: Duration = Duration::from_millis(200);

pub fn server_name() -> &'static str {
    capabilities::SERVER_NAME
}

pub fn server_description() -> String {
    capabilities::server_description()
}

pub fn run_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();

    run(stdin, stdout.lock())
}

pub(crate) fn run(reader: impl Read + Send + 'static, writer: impl Write) -> io::Result<()> {
    let mut server = LspServer::default();
    let mut writer = writer;
    let (reader_tx, reader_rx) = mpsc::channel();

    thread::spawn(move || {
        let mut reader = BufReader::new(reader);

        loop {
            match read_message(&mut reader) {
                Ok(Some(message)) => {
                    if reader_tx.send(Ok(Some(message))).is_err() {
                        break;
                    }
                }
                Ok(None) => {
                    let _ = reader_tx.send(Ok(None));
                    break;
                }
                Err(error) => {
                    let _ = reader_tx.send(Err(error));
                    break;
                }
            }
        }
    });

    loop {
        let event = match server.next_slow_diagnostics_due() {
            Some(deadline) => {
                let timeout = deadline.saturating_duration_since(Instant::now());

                match reader_rx.recv_timeout(timeout) {
                    Ok(event) => Some(event),
                    Err(RecvTimeoutError::Timeout) => match reader_rx.try_recv() {
                        Ok(event) => Some(event),
                        Err(TryRecvError::Empty) => {
                            if let Some(message) =
                                server.drain_next_due_slow_diagnostic(Instant::now())
                            {
                                write_message(&mut writer, &message)?;
                            }

                            continue;
                        }
                        Err(TryRecvError::Disconnected) => None,
                    },
                    Err(RecvTimeoutError::Disconnected) => None,
                }
            }
            None => reader_rx.recv().ok(),
        };

        let Some(event) = event else {
            for message in server.drain_all_pending_slow_diagnostics() {
                write_message(&mut writer, &message)?;
            }

            break;
        };

        let Some(message) = event? else {
            for message in server.drain_all_pending_slow_diagnostics() {
                write_message(&mut writer, &message)?;
            }

            break;
        };

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
    analysis: AnalysisHost,
    workspace_folders: Vec<PathBuf>,
    settings: WorkspaceSettings,
    pending_slow_diagnostics: BTreeMap<String, PendingDiagnostics>,
    last_published_diagnostics: BTreeMap<String, Vec<serde_json::Value>>,
    document_generations: BTreeMap<String, u64>,
    document_versions: BTreeMap<String, i64>,
    semantic_token_results: BTreeMap<String, SemanticTokensCacheEntry>,
    next_semantic_token_result_id: u64,
    client_watched_files_dynamic_registration: bool,
    client_pull_diagnostics: bool,
    client_diagnostic_refresh: bool,
    client_completion_label_details: bool,
    next_server_request_id: u64,
    trace_config: TraceConfig,
    shutdown_requested: bool,
    should_exit: bool,
}

#[derive(Clone, Copy, Debug)]
struct PendingDiagnostics {
    generation: u64,
    version: Option<i64>,
    due_at: Instant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SemanticTokensCacheEntry {
    uri: String,
    data: Vec<usize>,
    document_version: Option<i64>,
    analysis_revision: u64,
    sequence: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TraceConfig {
    server: ServerTraceLevel,
    log_verbosity: LogVerbosity,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ServerTraceLevel {
    #[default]
    Off,
    Messages,
    Verbose,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
enum LogVerbosity {
    #[default]
    Off,
    Error,
    Info,
    Debug,
    Trace,
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
