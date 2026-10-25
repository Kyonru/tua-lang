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
mod shaders;
mod transport;
mod uri;

#[cfg(feature = "fuzzing")]
pub(crate) fn fuzz_transport(input: &[u8]) {
    let _ = transport::read_message(&mut std::io::Cursor::new(input));
}

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
const PATTERN_CORPUS_REBUILD_DEBOUNCE: Duration = Duration::from_millis(300);
const PATTERN_CORPUS_BATCH_SIZE: usize = 16;
const RECENT_REQUEST_LATENCY_LIMIT: usize = 4096;
const EVENT_QUEUE_CAPACITY: usize = 16;

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
    run_with_shader_validator(reader, writer, None)
}

pub fn run_stdio_with_shader_validator(
    validator: Box<dyn tua_core::shader_validation::ShaderValidator>,
) -> io::Result<()> {
    run_with_shader_validator(io::stdin(), io::stdout().lock(), Some(validator))
}

fn run_with_shader_validator(
    reader: impl Read + Send + 'static,
    writer: impl Write,
    validator: Option<Box<dyn tua_core::shader_validation::ShaderValidator>>,
) -> io::Result<()> {
    let mut server = LspServer::default();
    let mut writer = writer;
    let (event_tx, event_rx) = mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let shader_worker =
        validator.map(|validator| shaders::ShaderWorker::new(validator, event_tx.clone()));
    let received_input = SharedReceivedInputState::default();
    let mut scheduler = RequestScheduler::new(event_tx.clone(), Arc::clone(&received_input));
    let mut pending_messages = VecDeque::new();
    let mut deferred_background_adoptions = VecDeque::new();
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
        if shader_worker.is_some() {
            server.observe_shader_revision();
        }
        drain_speculative_results(&mut server, &mut scheduler, &mut writer)?;

        if scheduler.speculative_has_priority(Instant::now())
            && (pending_messages.is_empty()
                || pending_messages
                    .iter()
                    .all(message_can_wait_for_speculative_result))
        {
            match event_rx.recv_timeout(Duration::from_millis(1)) {
                Ok(event) => {
                    if let Some(event) =
                        defer_background_adoption(event, &mut deferred_background_adoptions)
                    {
                        process_server_event(
                            event,
                            &mut server,
                            &mut scheduler,
                            &mut pending_messages,
                            &mut reader_closed,
                            &mut writer,
                        )?;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => reader_closed = true,
            }
            continue;
        }

        if let Some(result) = take_deferred_background_adoption(
            &mut deferred_background_adoptions,
            scheduler.speculative_has_priority(Instant::now()),
            pending_messages.is_empty(),
        ) {
            process_server_event(
                ServerEvent::RequestFinished(result),
                &mut server,
                &mut scheduler,
                &mut pending_messages,
                &mut reader_closed,
                &mut writer,
            )?;
            continue;
        }

        if pending_messages.is_empty() {
            // Give input already in the transport channel priority over
            // snapshot preparation for previously queued background work.
            match event_rx.try_recv() {
                Ok(event) => {
                    process_server_event(
                        event,
                        &mut server,
                        &mut scheduler,
                        &mut pending_messages,
                        &mut reader_closed,
                        &mut writer,
                    )?;
                    continue;
                }
                Err(TryRecvError::Disconnected) => reader_closed = true,
                Err(TryRecvError::Empty) => {}
            }
            if let Some(messages) = scheduler.dispatch_deferred(&mut server) {
                for message in messages {
                    write_message(&mut writer, &message)?;
                }
                continue;
            }
        }

        if pending_messages.is_empty()
            && !reader_closed
            && !server.shutdown_requested
            && !server.should_exit
        {
            if let Some(worker) = &shader_worker {
                worker.dispatch_due(&mut server);
            }
            let dispatched_warm = scheduler.dispatch_next_workspace_warm(&mut server);
            if !dispatched_warm {
                scheduler.dispatch_due_pattern_corpus(&mut server, Instant::now());
            }
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

            match server.next_background_work_due() {
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
                                if let Some(worker) = &shader_worker {
                                    worker.dispatch_due(&mut server);
                                }
                                let now = Instant::now();
                                scheduler.dispatch_due_slow_diagnostic(&mut server, now);
                                scheduler.dispatch_due_pattern_corpus(&mut server, now);
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

        let event_budget = EVENT_QUEUE_CAPACITY.saturating_sub(pending_messages.len());
        for _ in 0..event_budget {
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
        let _speculative_bypass_profile =
            (tua_core::profile::enabled() && scheduler.has_speculative_in_flight()).then(|| {
                let state = if scheduler.speculative_has_priority(Instant::now()) {
                    "priority"
                } else {
                    "expired"
                };
                tua_core::profile::span(format!("inline.coordinator.active {state} {method}"))
            });
        if shader_worker.is_some() {
            if method == "initialize" {
                server.configure_shaders(&message);
            }
            if matches!(
                method.as_str(),
                "initialized"
                    | "textDocument/didOpen"
                    | "textDocument/didChange"
                    | "textDocument/didSave"
                    | "textDocument/didClose"
                    | "workspace/didChangeWatchedFiles"
                    | "workspace/didChangeConfiguration"
                    | "workspace/didChangeWorkspaceFolders"
            ) {
                server.queue_shaders();
            }
        }
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
        scheduler.cancel_stale_pattern_corpus(server.pattern_corpus.generation);
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
    drain_speculative_results(server, scheduler, writer)?;

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
        ServerEvent::ShadersFinished(result) => {
            for message in server.finish_shaders(result) {
                write_message(writer, &message)?;
            }
        }
        ServerEvent::SpeculativeReady => {}
    }

    Ok(())
}

fn drain_speculative_results(
    server: &mut LspServer,
    scheduler: &mut RequestScheduler,
    writer: &mut impl Write,
) -> io::Result<()> {
    for result in scheduler.take_speculative_results() {
        for message in scheduler.finish(server, result) {
            write_message(writer, &message)?;
        }
    }

    Ok(())
}

fn defer_background_adoption(
    event: ServerEvent,
    deferred: &mut VecDeque<Box<scheduler::WorkerResult>>,
) -> Option<ServerEvent> {
    match event {
        ServerEvent::RequestFinished(result) if result.is_background_adoption() => {
            deferred.push_back(result);
            None
        }
        event => Some(event),
    }
}

fn take_deferred_background_adoption(
    deferred: &mut VecDeque<Box<scheduler::WorkerResult>>,
    speculative_has_priority: bool,
    coordinator_idle: bool,
) -> Option<Box<scheduler::WorkerResult>> {
    (!speculative_has_priority && coordinator_idle)
        .then(|| deferred.pop_front())
        .flatten()
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
    shaders: shaders::ShaderState,
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
    pattern_corpus: PatternCorpusBuildState,
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
    Files {
        files: Vec<(PathBuf, tua_core::AnalysisWarmPriority)>,
        revision: u64,
    },
    Snapshot {
        revision: u64,
    },
}

impl WorkspaceWarmWork {
    fn revision(&self) -> u64 {
        match self {
            Self::Files { revision, .. } | Self::Snapshot { revision } => *revision,
        }
    }
}

const WORKSPACE_WARM_BATCH_SIZE: usize = 32;

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

struct PatternCorpusWork {
    generation: u64,
    paths: Vec<PathBuf>,
    operation: PatternCorpusOperation,
}

enum PatternCorpusOperation {
    Files {
        inputs: Vec<tua_core::PatternCorpusBuildInput>,
        project_model: tua_core::ProjectModel,
        retrieval_limits: tua_core::RetrievalLimits,
    },
    Remerge(tua_core::PreparedPatternCorpusRemerge),
    Dispose(Vec<PatternCorpusDisposal>),
}

#[derive(Debug)]
enum PatternCorpusDisposal {
    Retired(tua_core::RetiredProjectPatternCorpus),
    RejectedRemerge(tua_core::PatternCorpusRemergeOutcome),
}

#[derive(Clone, Debug, Default)]
struct PatternCorpusBuildState {
    enabled: bool,
    generation: u64,
    pending: BTreeMap<PathBuf, Instant>,
    in_flight: BTreeSet<PathBuf>,
    files_discovered: usize,
    batches_completed: usize,
    files_built: usize,
    adopted: usize,
    tombstoned: usize,
    stale: usize,
    skipped: usize,
    over_budget: usize,
    file_limit: usize,
    generation_rejections: usize,
    lineage_rejections: usize,
    cancelled_batches: usize,
    remerges: usize,
    remerge_pending: bool,
    remerge_in_flight: bool,
    remerge_rejections: usize,
    last_build_ms: u64,
    disposals: Arc<Mutex<VecDeque<PatternCorpusDisposal>>>,
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

    fn reset_pattern_corpus_queue(&mut self) {
        let generation = self.pattern_corpus.generation.saturating_add(1).max(1);
        let enabled =
            self.settings.inline_project_enabled() || self.settings.inline_statistical_enabled();
        self.analysis
            .set_project_pattern_eligibility_generation(generation);

        let disposals = Arc::clone(&self.pattern_corpus.disposals);
        if !enabled {
            if let Some(retired) = self.analysis.clear_project_pattern_corpus() {
                disposals
                    .lock()
                    .expect("pattern corpus disposal queue lock poisoned")
                    .push_back(PatternCorpusDisposal::Retired(retired));
            }
        }
        let mut state = PatternCorpusBuildState {
            enabled,
            generation,
            disposals,
            ..PatternCorpusBuildState::default()
        };
        if enabled {
            let now = Instant::now();
            for path in self.analysis.file_paths() {
                if self.analysis.project_pattern_path_is_eligible(
                    &path,
                    &self.workspace_folders,
                    &self.settings,
                ) {
                    state.pending.insert(path, now);
                }
            }
            state.files_discovered = state.pending.len();
        }
        self.pattern_corpus = state;
    }

    fn queue_pattern_corpus_path(&mut self, path: PathBuf, now: Instant) {
        if !self.pattern_corpus.enabled {
            return;
        }
        if !self.analysis.project_pattern_path_is_eligible(
            &path,
            &self.workspace_folders,
            &self.settings,
        ) || self.analysis.project_pattern_source_bytes(&path).is_none()
        {
            self.tombstone_pattern_corpus_path(&path);
            return;
        }

        self.pattern_corpus
            .pending
            .insert(path, now + PATTERN_CORPUS_REBUILD_DEBOUNCE);
    }

    fn tombstone_pattern_corpus_path(&mut self, path: &std::path::Path) {
        if !self.pattern_corpus.enabled {
            return;
        }
        self.pattern_corpus.pending.remove(path);
        let adoption = self
            .analysis
            .tombstone_project_pattern_file(self.pattern_corpus.generation, path);
        self.record_pattern_corpus_adoption(&adoption);
    }

    fn next_pattern_corpus_due(&self) -> Option<Instant> {
        if !self
            .pattern_corpus
            .disposals
            .lock()
            .expect("pattern corpus disposal queue lock poisoned")
            .is_empty()
        {
            return Some(Instant::now());
        }
        self.pattern_corpus
            .pending
            .values()
            .copied()
            .min()
            .or_else(|| self.pattern_corpus.remerge_pending.then(Instant::now))
    }

    fn next_background_work_due(&self) -> Option<Instant> {
        self.next_slow_diagnostics_due()
            .into_iter()
            .chain(self.next_pattern_corpus_due())
            .chain(self.shader_due())
            .min()
    }

    fn take_next_pattern_corpus_work(&mut self, now: Instant) -> Option<PatternCorpusWork> {
        let disposals = {
            let mut queued = self
                .pattern_corpus
                .disposals
                .lock()
                .expect("pattern corpus disposal queue lock poisoned");
            queued.drain(..).collect::<Vec<_>>()
        };
        if !disposals.is_empty() {
            return Some(PatternCorpusWork {
                generation: self.pattern_corpus.generation,
                paths: Vec::new(),
                operation: PatternCorpusOperation::Dispose(disposals),
            });
        }
        if !self.pattern_corpus.enabled {
            return None;
        }
        let mut due = self
            .pattern_corpus
            .pending
            .iter()
            .filter(|(_, due)| **due <= now)
            .map(|(path, due)| (path.clone(), *due))
            .collect::<Vec<_>>();
        due.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)));
        due.truncate(PATTERN_CORPUS_BATCH_SIZE);
        if due.is_empty() {
            if self.pattern_corpus.remerge_pending && self.pattern_corpus.pending.is_empty() {
                self.pattern_corpus.remerge_pending = false;
                let prepared = self.analysis.prepare_project_pattern_remerge()?;
                self.pattern_corpus.remerge_in_flight = true;
                return Some(PatternCorpusWork {
                    generation: self.pattern_corpus.generation,
                    paths: Vec::new(),
                    operation: PatternCorpusOperation::Remerge(prepared),
                });
            }
            return None;
        }

        let project_model = self.analysis.project_model()?;
        let retrieval_limits = self.analysis.project_pattern_retrieval_limits();
        let mut paths = Vec::with_capacity(due.len());
        let mut inputs = Vec::with_capacity(due.len());
        for (path, _) in due {
            self.pattern_corpus.pending.remove(&path);
            if self
                .analysis
                .project_pattern_source_bytes(&path)
                .is_none_or(|bytes| bytes > retrieval_limits.max_source_bytes)
            {
                self.pattern_corpus.skipped = self.pattern_corpus.skipped.saturating_add(1);
                self.tombstone_pattern_corpus_path(&path);
                continue;
            }
            self.pattern_corpus.in_flight.insert(path.clone());
            if let Some(input) = self.analysis.prepare_project_pattern_file(&path) {
                paths.push(path);
                inputs.push(input);
            } else {
                self.pattern_corpus.in_flight.remove(&path);
                self.tombstone_pattern_corpus_path(&path);
            }
        }

        (!inputs.is_empty()).then_some(PatternCorpusWork {
            generation: self.pattern_corpus.generation,
            paths,
            operation: PatternCorpusOperation::Files {
                inputs,
                project_model,
                retrieval_limits,
            },
        })
    }

    fn finish_pattern_corpus(&mut self, result: scheduler::PatternCorpusWorkerResult) {
        if result.disposed {
            return;
        }
        self.pattern_corpus.remerge_in_flight = false;
        for path in &result.paths {
            self.pattern_corpus.in_flight.remove(path);
        }
        self.pattern_corpus.batches_completed =
            self.pattern_corpus.batches_completed.saturating_add(1);
        let files_built = result
            .entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry,
                    tua_core::PatternCorpusBatchEntry::File(
                        tua_core::PatternCorpusBuildOutcome::Built(_)
                    )
                )
            })
            .count();
        self.pattern_corpus.files_built =
            self.pattern_corpus.files_built.saturating_add(files_built);
        self.pattern_corpus.last_build_ms = result.elapsed_ms;
        if result.cancelled {
            self.pattern_corpus.cancelled_batches =
                self.pattern_corpus.cancelled_batches.saturating_add(1);
        }

        if let Some(remerge) = result.remerge {
            let _profile = tua_core::profile::span("inline.index.adopt_remerge");
            let disposal = match self.analysis.try_adopt_project_pattern_remerge(remerge) {
                Ok(retired) => {
                    self.pattern_corpus.remerges = self.pattern_corpus.remerges.saturating_add(1);
                    PatternCorpusDisposal::Retired(retired)
                }
                Err(rejected) => {
                    self.pattern_corpus.remerge_rejections =
                        self.pattern_corpus.remerge_rejections.saturating_add(1);
                    PatternCorpusDisposal::RejectedRemerge(rejected)
                }
            };
            self.pattern_corpus
                .disposals
                .lock()
                .expect("pattern corpus disposal queue lock poisoned")
                .push_back(disposal);
            return;
        }

        let _profile = tua_core::profile::span("inline.index.adopt_batch");
        let adoption = self
            .analysis
            .adopt_project_pattern_batch(result.generation, result.entries);
        self.record_pattern_corpus_adoption(&adoption);

        if result.generation == self.pattern_corpus.generation && self.pattern_corpus.enabled {
            let due = Instant::now() + PATTERN_CORPUS_REBUILD_DEBOUNCE;
            for path in result.unfinished_paths {
                self.pattern_corpus.pending.insert(path, due);
            }
            for entry in &adoption.entries {
                if matches!(
                    entry.status,
                    tua_core::CorpusEntryAdoptionStatus::Stale
                        | tua_core::CorpusEntryAdoptionStatus::WrongLineage
                        | tua_core::CorpusEntryAdoptionStatus::GenerationMismatch
                ) {
                    self.queue_pattern_corpus_path(entry.path.clone(), Instant::now());
                }
            }
        }
    }

    fn record_pattern_corpus_adoption(&mut self, adoption: &tua_core::CorpusAdoptionResult) {
        self.pattern_corpus.adopted = self.pattern_corpus.adopted.saturating_add(adoption.adopted);
        self.pattern_corpus.tombstoned = self
            .pattern_corpus
            .tombstoned
            .saturating_add(adoption.tombstoned);
        self.pattern_corpus.stale = self.pattern_corpus.stale.saturating_add(adoption.stale);
        self.pattern_corpus.skipped = self.pattern_corpus.skipped.saturating_add(adoption.skipped);
        self.pattern_corpus.over_budget = self
            .pattern_corpus
            .over_budget
            .saturating_add(adoption.over_budget);
        self.pattern_corpus.file_limit = self
            .pattern_corpus
            .file_limit
            .saturating_add(adoption.file_limit);
        if adoption.generation_rejected {
            self.pattern_corpus.generation_rejections =
                self.pattern_corpus.generation_rejections.saturating_add(1);
        }
        if adoption.lineage_rejected {
            self.pattern_corpus.lineage_rejections =
                self.pattern_corpus.lineage_rejections.saturating_add(1);
        }
        self.pattern_corpus.remerge_pending |= adoption.needs_remerge;
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
        if let Some(first) = self.workspace_warm.pop_file() {
            let mut files = vec![first];
            while files.len() < WORKSPACE_WARM_BATCH_SIZE {
                let Some(next) = self.workspace_warm.pop_file() else {
                    break;
                };
                files.push(next);
            }
            return Some(WorkspaceWarmWork::Files { files, revision });
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
            if let WorkspaceWarmWork::Files { files, .. } = work {
                for (path, priority) in files {
                    self.workspace_warm.queue(path.clone(), *priority, false);
                }
            }
            return Vec::new();
        }

        match work {
            WorkspaceWarmWork::Files { files, .. } => {
                for (path, _) in files {
                    let adopted = succeeded
                        && warmed_analysis
                            .is_some_and(|warmed| self.analysis.adopt_warmed_file(warmed, path));

                    if !adopted {
                        self.workspace_warm.failed.insert(path.clone());
                        continue;
                    }

                    self.workspace_warm.warmed.insert(path.clone());
                    for related in self.analysis.warm_related_paths(path) {
                        self.workspace_warm.queue(
                            related,
                            tua_core::AnalysisWarmPriority::High,
                            false,
                        );
                    }
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
        | "textDocument/onTypeFormatting"
        | "tua/projectLayout"
        | "tua/localInlineCompletion"
        | "textDocument/inlineCompletion" => Some(2),
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

fn message_can_wait_for_speculative_result(message: &Value) -> bool {
    if message.get("method").and_then(Value::as_str) == Some("$/cancelRequest") {
        return false;
    }
    match message_interactive_priority(message) {
        Some(priority) => priority > 2,
        None => message_is_yieldable_background(message),
    }
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
