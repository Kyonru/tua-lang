mod analysis;
mod auto_import;
mod capabilities;
mod completion_stats;
mod configuration;
mod handlers;
mod protocol;
mod scheduler;
mod transport;
mod uri;

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use analysis::AnalysisHost;
use configuration::WorkspaceSettings;
use scheduler::{request_runs_on_coordinator, RequestScheduler, ServerEvent};
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
    let (event_tx, event_rx) = mpsc::channel();
    let mut scheduler = RequestScheduler::new(event_tx.clone());
    let mut pending_messages = VecDeque::new();
    let mut reader_closed = false;

    thread::spawn(move || {
        let mut reader = BufReader::new(reader);

        loop {
            match read_message(&mut reader) {
                Ok(Some(message)) => {
                    if event_tx
                        .send(ServerEvent::Input(Ok(Some(message))))
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(None) => {
                    let _ = event_tx.send(ServerEvent::Input(Ok(None)));
                    break;
                }
                Err(error) => {
                    let _ = event_tx.send(ServerEvent::Input(Err(error)));
                    break;
                }
            }
        }
    });

    loop {
        if pending_messages.is_empty() && server.should_exit && !scheduler.has_in_flight() {
            break;
        }

        if pending_messages.is_empty() {
            if reader_closed && !scheduler.has_in_flight() {
                for message in server.drain_all_pending_slow_diagnostics() {
                    write_message(&mut writer, &message)?;
                }

                break;
            }

            match server.next_slow_diagnostics_due() {
                Some(deadline) if !reader_closed => {
                    let timeout = deadline.saturating_duration_since(Instant::now());

                    match event_rx.recv_timeout(timeout) {
                        Ok(event) => process_server_event(
                            event,
                            &mut server,
                            &mut scheduler,
                            &mut pending_messages,
                            &mut reader_closed,
                            &mut writer,
                        )?,
                        Err(RecvTimeoutError::Timeout) => match event_rx.try_recv() {
                            Ok(event) => process_server_event(
                                event,
                                &mut server,
                                &mut scheduler,
                                &mut pending_messages,
                                &mut reader_closed,
                                &mut writer,
                            )?,
                            Err(TryRecvError::Empty) => {
                                let now = Instant::now();
                                scheduler.dispatch_due_slow_diagnostic(&mut server, now);
                                if let Some(message) = server.drain_due_pull_diagnostic_refresh(now)
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
                _ => match event_rx.recv() {
                    Ok(event) => process_server_event(
                        event,
                        &mut server,
                        &mut scheduler,
                        &mut pending_messages,
                        &mut reader_closed,
                        &mut writer,
                    )?,
                    Err(_) => reader_closed = true,
                },
            }
        }

        loop {
            match event_rx.try_recv() {
                Ok(event) => process_server_event(
                    event,
                    &mut server,
                    &mut scheduler,
                    &mut pending_messages,
                    &mut reader_closed,
                    &mut writer,
                )?,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    reader_closed = true;
                    break;
                }
            }
        }

        let Some(message) = pop_next_pending_message(&mut pending_messages) else {
            continue;
        };

        server.input_backlog_active = !pending_messages.is_empty();
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let request = message.get("id").is_some();
        let changed_uri = (method == "textDocument/didChange")
            .then(|| document_uri_from_message(&message).map(str::to_owned))
            .flatten();

        let outbound = if request && !request_runs_on_coordinator(&method) {
            scheduler.dispatch(&mut server, message)
        } else {
            let cancelled_id = (method == "$/cancelRequest")
                .then(|| message.get("params")?.get("id").cloned())
                .flatten();
            let mut outbound = server.handle_message(message).unwrap_or_default();

            if let Some(id) = cancelled_id {
                outbound.extend(scheduler.cancel(&id));
            }
            if let Some(uri) = changed_uri {
                let generation = server.document_generations.get(&uri).copied();
                outbound.extend(scheduler.cancel_stale_background(&uri, generation));
            }

            outbound
        };
        for message in outbound {
            write_message(&mut writer, &message)?;
        }
        server.input_backlog_active = false;

        if server.should_exit && !scheduler.has_in_flight() {
            break;
        }
    }

    Ok(())
}

fn process_server_event(
    event: ServerEvent,
    server: &mut LspServer,
    scheduler: &mut RequestScheduler,
    pending_messages: &mut VecDeque<Value>,
    reader_closed: &mut bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    match event {
        ServerEvent::Input(event) => match event? {
            Some(message) => pending_messages.push_back(message),
            None => *reader_closed = true,
        },
        ServerEvent::RequestFinished(result) => {
            for message in scheduler.finish(server, result) {
                write_message(writer, &message)?;
            }
        }
    }

    Ok(())
}

fn document_uri_from_message(message: &Value) -> Option<&str> {
    message
        .get("params")?
        .get("textDocument")?
        .get("uri")?
        .as_str()
}

#[derive(Clone, Default)]
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
    semantic_tokens: Arc<Mutex<SemanticTokensState>>,
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SemanticTokensState {
    results: BTreeMap<String, SemanticTokensCacheEntry>,
    next_result_id: u64,
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
    let method = message.get("method").and_then(Value::as_str)?;

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
        "tua/inspect" | "tua/signalGraph" => Some(3),
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
            | "workspace/symbol"
    )
}

#[cfg(test)]
mod tests;
