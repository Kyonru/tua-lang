mod analysis;
mod auto_import;
mod capabilities;
mod completion_stats;
mod configuration;
mod handlers;
mod protocol;
mod transport;
mod uri;

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

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
    let mut pending_messages = VecDeque::new();
    let mut reader_closed = false;

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
        if pending_messages.is_empty() && !reader_closed {
            match server.next_slow_diagnostics_due() {
                Some(deadline) => {
                    let timeout = deadline.saturating_duration_since(Instant::now());

                    match reader_rx.recv_timeout(timeout) {
                        Ok(event) => match event? {
                            Some(message) => pending_messages.push_back(message),
                            None => reader_closed = true,
                        },
                        Err(RecvTimeoutError::Timeout) => match reader_rx.try_recv() {
                            Ok(event) => match event? {
                                Some(message) => pending_messages.push_back(message),
                                None => reader_closed = true,
                            },
                            Err(TryRecvError::Empty) => {
                                if let Some(message) =
                                    server.drain_next_due_slow_diagnostic(Instant::now())
                                {
                                    write_message(&mut writer, &message)?;
                                }

                                continue;
                            }
                            Err(TryRecvError::Disconnected) => reader_closed = true,
                        },
                        Err(RecvTimeoutError::Disconnected) => reader_closed = true,
                    }
                }
                None => match reader_rx.recv() {
                    Ok(event) => match event? {
                        Some(message) => pending_messages.push_back(message),
                        None => reader_closed = true,
                    },
                    Err(_) => reader_closed = true,
                },
            }
        }

        while !reader_closed {
            match reader_rx.try_recv() {
                Ok(event) => match event? {
                    Some(message) => pending_messages.push_back(message),
                    None => {
                        reader_closed = true;
                        break;
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    reader_closed = true;
                    break;
                }
            }
        }

        let Some(message) = pop_next_pending_message(&mut pending_messages) else {
            if reader_closed {
                for message in server.drain_all_pending_slow_diagnostics() {
                    write_message(&mut writer, &message)?;
                }

                break;
            }

            continue;
        };

        server.input_backlog_active = !pending_messages.is_empty();
        if let Some(outbound) = server.handle_message(message) {
            for message in outbound {
                write_message(&mut writer, &message)?;
            }
        }
        server.input_backlog_active = false;

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
    pending_pull_fast_diagnostics: BTreeMap<String, PendingDiagnostics>,
    last_published_diagnostics: BTreeMap<String, Vec<serde_json::Value>>,
    document_generations: BTreeMap<String, u64>,
    document_versions: BTreeMap<String, i64>,
    recent_document_changes: BTreeMap<String, Instant>,
    semantic_token_results: BTreeMap<String, SemanticTokensCacheEntry>,
    next_semantic_token_result_id: u64,
    client_watched_files_dynamic_registration: bool,
    client_pull_diagnostics: bool,
    client_diagnostic_refresh: bool,
    client_completion_label_details: bool,
    next_server_request_id: u64,
    trace_config: TraceConfig,
    trace_log_counts: TraceLogCounts,
    input_backlog_active: bool,
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TraceLogCounts {
    requests: BTreeMap<String, usize>,
    notifications: BTreeMap<String, usize>,
    total: usize,
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

fn pop_next_pending_message(messages: &mut VecDeque<Value>) -> Option<Value> {
    let mut best: Option<(usize, u8)> = None;

    for (index, message) in messages.iter().enumerate() {
        if let Some(priority) = message_interactive_priority(message) {
            if best.is_none_or(|(_, best_priority)| priority < best_priority) {
                best = Some((index, priority));
            }

            continue;
        }
        if !message_is_yieldable_background(message) {
            break;
        }
    }

    if let Some((index, _)) = best {
        return messages.remove(index);
    }

    messages.pop_front()
}

fn message_interactive_priority(message: &Value) -> Option<u8> {
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return None;
    };

    match method {
        "initialize"
        | "initialized"
        | "textDocument/didOpen"
        | "textDocument/didChange"
        | "textDocument/didSave"
        | "textDocument/didClose"
        | "tua/setTrace" => Some(0),
        "textDocument/completion" | "completionItem/resolve" => Some(1),
        "textDocument/signatureHelp"
        | "textDocument/definition"
        | "textDocument/typeDefinition"
        | "textDocument/implementation"
        | "textDocument/references"
        | "textDocument/prepareRename"
        | "textDocument/rename"
        | "textDocument/formatting"
        | "textDocument/rangeFormatting"
        | "textDocument/onTypeFormatting" => Some(2),
        "textDocument/hover" | "textDocument/documentHighlight" => Some(3),
        "tua/signalGraph" => Some(3),
        _ => None,
    }
}

fn message_is_yieldable_background(message: &Value) -> bool {
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return false;
    };

    method == "$/cancelRequest" || method_is_background_refresh(method)
}

fn method_is_background_refresh(method: &str) -> bool {
    matches!(
        method,
        "textDocument/codeAction"
            | "textDocument/codeLens"
            | "textDocument/diagnostic"
            | "textDocument/documentLink"
            | "textDocument/documentSymbol"
            | "textDocument/foldingRange"
            | "textDocument/inlayHint"
            | "textDocument/semanticTokens/full"
            | "textDocument/semanticTokens/full/delta"
            | "textDocument/semanticTokens/range"
    )
}

#[cfg(test)]
mod tests;
