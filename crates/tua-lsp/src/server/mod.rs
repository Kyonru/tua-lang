mod analysis;
mod auto_import;
mod capabilities;
mod completion_stats;
mod config;
mod configuration;
mod handlers;
mod protocol;
mod request_registry;
mod scheduler;
mod transport;
mod uri;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
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
const RECENT_REQUEST_LATENCY_LIMIT: usize = 128;

#[derive(Default)]
struct ReceivedInputState {
    document_versions: Mutex<BTreeMap<String, Option<i64>>>,
    request_document_versions: Mutex<BTreeMap<String, i64>>,
}

type SharedReceivedInputState = Arc<ReceivedInputState>;

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
    let received_input = SharedReceivedInputState::default();
    let mut scheduler = RequestScheduler::new(event_tx.clone(), Arc::clone(&received_input));
    let mut pending_messages = VecDeque::new();
    let mut reader_closed = false;

    let reader_input = Arc::clone(&received_input);
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);

        loop {
            match read_message(&mut reader) {
                Ok(Some(message)) => {
                    record_received_input(&message, &reader_input);
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
        if pending_messages.is_empty()
            && !reader_closed
            && !server.shutdown_requested
            && !server.should_exit
        {
            scheduler.dispatch_next_workspace_warm(&mut server);
        }

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
        let received_document_version = take_received_request_version(&message, &received_input);
        let changed_uri = (method == "textDocument/didChange")
            .then(|| document_uri_from_message(&message).map(str::to_owned))
            .flatten();

        let outbound = if request && !request_runs_on_coordinator(&method) {
            scheduler.dispatch(&mut server, message, received_document_version)
        } else {
            let cancelled_id = (method == "$/cancelRequest")
                .then(|| message.get("params")?.get("id").cloned())
                .flatten();
            server.update_scheduler_health(pending_messages.len(), &scheduler);
            let mut outbound = server.handle_message(message).unwrap_or_default();

            if let Some(id) = cancelled_id {
                outbound.extend(scheduler.cancel(&id));
            }
            if matches!(method.as_str(), "shutdown" | "exit") {
                scheduler.abandon_workspace_warm();
            }
            if let Some(uri) = changed_uri {
                let generation = server.document_generations.get(&uri).copied();
                let version = server.document_versions.get(&uri).copied();
                outbound
                    .extend(scheduler.cancel_stale_document_requests(&uri, generation, version));
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

fn record_received_input(message: &Value, input: &ReceivedInputState) {
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return;
    };
    let uri = document_uri_from_message(message);

    if let Some(uri) = uri {
        let state = match method {
            "textDocument/didOpen" | "textDocument/didChange" => message
                .get("params")
                .and_then(|params| params.get("textDocument"))
                .and_then(|document| document.get("version"))
                .and_then(Value::as_i64)
                .map(Some),
            "textDocument/didClose" => Some(None),
            _ => None,
        };

        if let Some(state) = state {
            input
                .document_versions
                .lock()
                .expect("received document version lock poisoned")
                .insert(uri.to_owned(), state);
        }
    }

    let Some(request_id) = message.get("id") else {
        return;
    };
    let Some(uri) = uri else {
        return;
    };
    let Some(version) = input
        .document_versions
        .lock()
        .expect("received document version lock poisoned")
        .get(uri)
        .copied()
        .flatten()
    else {
        return;
    };

    input
        .request_document_versions
        .lock()
        .expect("received request version lock poisoned")
        .insert(request_id.to_string(), version);
}

fn take_received_request_version(message: &Value, input: &ReceivedInputState) -> Option<i64> {
    message.get("method")?.as_str()?;
    document_uri_from_message(message)?;
    let request_id = message.get("id")?;

    input
        .request_document_versions
        .lock()
        .expect("received request version lock poisoned")
        .remove(&request_id.to_string())
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
            for message in scheduler.finish(server, *result) {
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
    client_inlay_hint_refresh: bool,
    client_semantic_tokens_refresh: bool,
    client_completion_label_details: bool,
    next_server_request_id: u64,
    trace_config: TraceConfig,
    trace_log_counts: TraceLogCounts,
    input_backlog_active: bool,
    workspace_warm: WorkspaceWarmState,
    scheduler_health: SchedulerHealthState,
    request_health: RequestHealthState,
    shutdown_requested: bool,
    should_exit: bool,
}

#[derive(Clone, Debug, Default)]
struct SchedulerHealthState {
    coordinator_queue_depth: usize,
    in_flight_jobs: usize,
}

#[derive(Clone, Debug, Default)]
struct RequestHealthState {
    recent_latencies: VecDeque<RequestLatencySample>,
    stale_results: usize,
}

#[derive(Clone, Debug)]
struct RequestLatencySample {
    method: String,
    elapsed_ms: u64,
}

#[derive(Clone, Debug)]
enum WorkspaceWarmWork {
    File {
        path: PathBuf,
        priority: tua_core::AnalysisWarmPriority,
        revision: u64,
    },
    Snapshot {
        revision: u64,
    },
}

impl WorkspaceWarmWork {
    fn revision(&self) -> u64 {
        match self {
            Self::File { revision, .. } | Self::Snapshot { revision } => *revision,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct WorkspaceWarmState {
    enabled: bool,
    high: VecDeque<PathBuf>,
    low: VecDeque<PathBuf>,
    queued: BTreeSet<PathBuf>,
    warmed: BTreeSet<PathBuf>,
    failed: BTreeSet<PathBuf>,
    files_discovered: usize,
    stale_results: usize,
    snapshot_ready_revision: Option<u64>,
    snapshot_failed_revision: Option<u64>,
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

    fn reset_workspace_warm_queue(&mut self) {
        let open = self
            .analysis
            .open_paths()
            .into_iter()
            .collect::<BTreeSet<_>>();
        let paths = self.analysis.file_paths();
        let mut state = WorkspaceWarmState {
            enabled: true,
            files_discovered: paths.len(),
            ..WorkspaceWarmState::default()
        };

        for path in paths {
            let priority = if open.contains(&path) {
                tua_core::AnalysisWarmPriority::High
            } else {
                tua_core::AnalysisWarmPriority::Low
            };

            state.queue(path, priority, false);
        }

        self.workspace_warm = state;
    }

    fn queue_workspace_warm_path(
        &mut self,
        path: PathBuf,
        priority: tua_core::AnalysisWarmPriority,
        invalidated: bool,
    ) {
        self.workspace_warm.enabled = true;
        self.workspace_warm.files_discovered = self.analysis.file_paths().len();
        self.workspace_warm.snapshot_ready_revision = None;
        self.workspace_warm.snapshot_failed_revision = None;
        self.workspace_warm.queue(path, priority, invalidated);
    }

    fn take_next_workspace_warm_work(&mut self) -> Option<WorkspaceWarmWork> {
        if !self.workspace_warm.enabled {
            return None;
        }

        let revision = self.analysis.revision();
        if let Some((path, priority)) = self.workspace_warm.pop_file() {
            return Some(WorkspaceWarmWork::File {
                path,
                priority,
                revision,
            });
        }

        (self.workspace_warm.snapshot_ready_revision != Some(revision)
            && self.workspace_warm.snapshot_failed_revision != Some(revision))
        .then_some(WorkspaceWarmWork::Snapshot { revision })
    }

    fn finish_workspace_warm(
        &mut self,
        work: &WorkspaceWarmWork,
        warmed_analysis: Option<&AnalysisHost>,
        succeeded: bool,
    ) -> Vec<Value> {
        if work.revision() != self.analysis.revision() {
            self.workspace_warm.stale_results += 1;
            if let WorkspaceWarmWork::File { path, priority, .. } = work {
                self.workspace_warm.queue(path.clone(), *priority, false);
            }
            return Vec::new();
        }

        match work {
            WorkspaceWarmWork::File { path, .. } => {
                let adopted = succeeded
                    && warmed_analysis
                        .is_some_and(|warmed| self.analysis.adopt_warmed_file(warmed, path));

                if !adopted {
                    self.workspace_warm.failed.insert(path.clone());
                    return Vec::new();
                }

                self.workspace_warm.warmed.insert(path.clone());
                for related in self.analysis.warm_related_paths(path) {
                    self.workspace_warm
                        .queue(related, tua_core::AnalysisWarmPriority::High, false);
                }
            }
            WorkspaceWarmWork::Snapshot { revision } if succeeded => {
                self.workspace_warm.snapshot_ready_revision = Some(*revision);
                return self.workspace_semantic_refresh_requests();
            }
            WorkspaceWarmWork::Snapshot { revision } => {
                self.workspace_warm.snapshot_failed_revision = Some(*revision);
            }
        }

        Vec::new()
    }

    fn workspace_semantic_refresh_requests(&mut self) -> Vec<Value> {
        let mut requests = Vec::new();

        if self.client_inlay_hint_refresh {
            requests.push(protocol::workspace_inlay_hint_refresh_request(
                self.next_request_id(),
            ));
        }
        if self.client_semantic_tokens_refresh {
            requests.push(protocol::workspace_semantic_tokens_refresh_request(
                self.next_request_id(),
            ));
        }

        requests
    }

    fn update_scheduler_health(
        &mut self,
        coordinator_queue_depth: usize,
        scheduler: &RequestScheduler,
    ) {
        self.scheduler_health = SchedulerHealthState {
            coordinator_queue_depth,
            in_flight_jobs: scheduler.in_flight_count(),
        };
    }

    fn record_request_latency(&mut self, method: &str, elapsed: Duration) {
        if self.request_health.recent_latencies.len() >= RECENT_REQUEST_LATENCY_LIMIT {
            self.request_health.recent_latencies.pop_front();
        }

        self.request_health
            .recent_latencies
            .push_back(RequestLatencySample {
                method: method.to_owned(),
                elapsed_ms: elapsed.as_millis().min(u128::from(u64::MAX)) as u64,
            });
    }

    fn record_stale_request_result(&mut self) {
        self.request_health.stale_results = self.request_health.stale_results.saturating_add(1);
    }
}

impl WorkspaceWarmState {
    fn queue(
        &mut self,
        path: PathBuf,
        priority: tua_core::AnalysisWarmPriority,
        invalidated: bool,
    ) {
        if invalidated {
            self.warmed.remove(&path);
            self.failed.remove(&path);
        } else if self.warmed.contains(&path) {
            return;
        }
        if self.failed.contains(&path) {
            return;
        }

        if self.queued.contains(&path) {
            if priority == tua_core::AnalysisWarmPriority::High {
                self.low.retain(|queued| queued != &path);
                if !self.high.contains(&path) {
                    self.high.push_back(path);
                }
            }
            return;
        }

        self.queued.insert(path.clone());
        match priority {
            tua_core::AnalysisWarmPriority::High => self.high.push_back(path),
            tua_core::AnalysisWarmPriority::Low => self.low.push_back(path),
        }
    }

    fn pop_file(&mut self) -> Option<(PathBuf, tua_core::AnalysisWarmPriority)> {
        let (path, priority) = self
            .high
            .pop_front()
            .map(|path| (path, tua_core::AnalysisWarmPriority::High))
            .or_else(|| {
                self.low
                    .pop_front()
                    .map(|path| (path, tua_core::AnalysisWarmPriority::Low))
            })?;

        self.queued.remove(&path);
        Some((path, priority))
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
        | "tua/setTrace"
        | "tua/health" => Some(0),
        "textDocument/completion"
        | "completionItem/resolve"
        | "tua/inlineCompletionContext"
        | "tua/validateInlineCompletion" => Some(1),
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
        "textDocument/hover" | "textDocument/documentHighlight" | "textDocument/inlayHint" => {
            Some(3)
        }
        "tua/explainType" | "tua/inspect" | "tua/signalGraph" => Some(3),
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
            | "textDocument/semanticTokens/full"
            | "textDocument/semanticTokens/full/delta"
            | "textDocument/semanticTokens/range"
            | "tua/assetInventory"
            | "tua/ecsGraph"
            | "tua/moduleGraph"
            | "tua/objectPoolGraph"
            | "tua/audioGraph"
            | "workspace/symbol"
    )
}

#[cfg(test)]
mod tests;
