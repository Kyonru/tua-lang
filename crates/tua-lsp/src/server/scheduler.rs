use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::panic::{self, AssertUnwindSafe};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Instant;

use serde_json::json;
use serde_json::Value;

use super::analysis::AnalysisHost;
use super::handlers::{LocalInlineCompletionWork, RequestTrace};
use super::protocol::{error_response, response};
use super::{
    method_is_background_refresh, LspServer, PatternCorpusWork, PendingDiagnostics,
    SharedReceivedInputState, WorkspaceWarmWork,
};

const REQUEST_CANCELLED: i64 = -32800;
const CONTENT_MODIFIED: i64 = -32801;
const INTERNAL_SLOW_DIAGNOSTICS: &str = "tua/internalSlowDiagnostics";
const INTERNAL_WORKSPACE_WARM: &str = "tua/internalWorkspaceWarm";
const INTERNAL_PATTERN_CORPUS: &str = "tua/internalPatternCorpus";
const REQUEST_WORKER_STACK_BYTES: usize = 8 * 1024 * 1024;
pub(super) const MAX_IN_FLIGHT_JOBS: usize = 16;
pub(super) const MAX_PENDING_REQUESTS: usize = 256;
const MAX_NON_INTERACTIVE_JOBS: usize = MAX_IN_FLIGHT_JOBS * 3 / 4;
const MAX_INTERNAL_BACKGROUND_JOBS: usize = MAX_IN_FLIGHT_JOBS / 2;
const MAX_JOBS_WHILE_DISPATCHING_INTERNAL_BACKGROUND: usize = MAX_IN_FLIGHT_JOBS - 1;
const SPECULATIVE_PRIORITY_WINDOW: std::time::Duration = std::time::Duration::from_millis(10);

#[cfg(test)]
static TEST_SLOW_EXECUTIONS: AtomicUsize = AtomicUsize::new(0);

pub(super) enum ServerEvent {
    Input(io::Result<Option<Value>>),
    RequestFinished(Box<WorkerResult>),
    SpeculativeReady,
}

pub(super) struct RequestScheduler {
    foreground_tx: Sender<WorkerJob>,
    background_tx: Sender<WorkerJob>,
    speculative_lane: SpeculativeLane,
    jobs: BTreeMap<u64, InFlightJob>,
    request_jobs: BTreeMap<String, u64>,
    coalesced_jobs: BTreeMap<String, u64>,
    newest_wins_jobs: BTreeMap<String, u64>,
    speculative_priority_until: Option<Instant>,
    received_input: SharedReceivedInputState,
    next_job_id: u64,
}

pub(super) struct WorkerResult {
    job_id: u64,
    response: Value,
    warm_result: Option<WorkspaceWarmResult>,
    corpus_result: Option<PatternCorpusWorkerResult>,
}

impl WorkerResult {
    pub(super) fn is_background_adoption(&self) -> bool {
        self.warm_result.is_some() || self.corpus_result.is_some()
    }

    #[cfg(test)]
    pub(super) fn test_result(job_id: u64, workspace_warm: bool) -> Self {
        Self {
            job_id,
            response: json!({"jsonrpc": "2.0", "id": job_id, "result": null}),
            warm_result: workspace_warm.then(|| WorkspaceWarmResult {
                work: WorkspaceWarmWork::Snapshot { revision: 0 },
                analysis: None,
                succeeded: false,
            }),
            corpus_result: None,
        }
    }
}

pub(super) struct PatternCorpusWorkerResult {
    pub(super) generation: u64,
    pub(super) paths: Vec<std::path::PathBuf>,
    pub(super) entries: Vec<tua_core::PatternCorpusBatchEntry>,
    pub(super) remerge: Option<tua_core::PatternCorpusRemergeOutcome>,
    pub(super) unfinished_paths: Vec<std::path::PathBuf>,
    pub(super) cancelled: bool,
    pub(super) elapsed_ms: u64,
    pub(super) disposed: bool,
}

struct WorkspaceWarmResult {
    work: WorkspaceWarmWork,
    analysis: Option<AnalysisHost>,
    succeeded: bool,
}

struct WorkerJob {
    job_id: u64,
    queued_at: Option<Instant>,
    server: Option<LspServer>,
    local_inline: Option<LocalInlineCompletionWork>,
    message: Value,
    method: String,
    cancelled: Arc<AtomicBool>,
    warm_work: Option<WorkspaceWarmWork>,
    corpus_work: Option<PatternCorpusWork>,
}

struct InFlightJob {
    non_interactive: bool,
    uri: Option<String>,
    generation: Option<u64>,
    cancelled: Arc<AtomicBool>,
    coalesce_key: Option<String>,
    newest_wins_key: Option<String>,
    waiters: Vec<RequestWaiter>,
    slow_diagnostics: Option<(String, PendingDiagnostics)>,
    warm_work: Option<WorkspaceWarmWork>,
    corpus_work: Option<(u64, Vec<std::path::PathBuf>)>,
}

struct RequestWaiter {
    id: Value,
    trace: RequestTrace,
    document_version: Option<i64>,
    cancelled: bool,
}

struct SpeculativeLane {
    state: Arc<SpeculativeLaneState>,
    results: mpsc::Receiver<WorkerResult>,
}

struct SpeculativeLaneState {
    queue: Mutex<SpeculativeQueue>,
    ready: Condvar,
}

#[derive(Default)]
struct SpeculativeQueue {
    order: VecDeque<String>,
    jobs: BTreeMap<String, WorkerJob>,
    stopped: bool,
}

impl RequestScheduler {
    pub(super) fn new(
        event_tx: SyncSender<ServerEvent>,
        received_input: SharedReceivedInputState,
    ) -> Self {
        let foreground_tx = spawn_worker_lane("foreground", 2, event_tx.clone());
        let background_tx = spawn_worker_lane("background", 1, event_tx.clone());
        let speculative_lane = spawn_speculative_lane(event_tx);

        Self {
            foreground_tx,
            background_tx,
            speculative_lane,
            jobs: BTreeMap::new(),
            request_jobs: BTreeMap::new(),
            coalesced_jobs: BTreeMap::new(),
            newest_wins_jobs: BTreeMap::new(),
            speculative_priority_until: None,
            received_input,
            next_job_id: 0,
        }
    }

    pub(super) fn dispatch(
        &mut self,
        server: &mut LspServer,
        message: Value,
        received_document_version: Option<i64>,
    ) -> Vec<Value> {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Vec::new();
        };
        let method = method.to_owned();
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        let request_key = request_id_key(&id);
        let uri = document_uri(&message).map(str::to_owned);
        let generation = uri
            .as_ref()
            .and_then(|uri| server.document_generations.get(uri).copied());
        let document_version = received_document_version.or_else(|| {
            uri.as_ref()
                .and_then(|uri| server.document_versions.get(uri).copied())
        });
        let background = request_runs_in_background(&method);
        let speculative = method == "tua/localInlineCompletion";
        if speculative && uri.is_none() {
            return vec![error_response(
                id,
                -32602,
                "inline completion requires a text document URI".to_owned(),
            )];
        }
        let non_interactive = background || method == "textDocument/inlayHint" || speculative;
        let coalesce_key = ((background || method == "textDocument/inlayHint") && uri.is_some())
            .then(|| coalesce_key(&method, &message, generation));
        let newest_wins_key = speculative
            .then(|| uri.as_ref().map(|uri| format!("{method}:{uri}")))
            .flatten();
        let (mut outbound, trace) = server.begin_request_trace(&message, &method);

        if self.request_jobs.contains_key(&request_key) {
            outbound.push(error_response(
                id,
                -32600,
                "request id is already in flight".to_owned(),
            ));
            return outbound;
        }

        if speculative && !server.local_inline_completion_is_enabled() {
            let response = response(
                id,
                json!({
                    "documentVersion": document_version,
                    "items": []
                }),
            );
            outbound.extend(server.finish_request_trace(trace, &response));
            outbound.push(response);
            return outbound;
        }

        if let Some(key) = newest_wins_key.as_deref() {
            outbound.extend(self.cancel_superseded_request(key));
        }
        if self.request_jobs.len() >= MAX_PENDING_REQUESTS {
            outbound.push(error_response(
                id,
                -32000,
                "language server request capacity exceeded".to_owned(),
            ));
            return outbound;
        }

        if let Some(job_id) = coalesce_key
            .as_ref()
            .and_then(|key| self.coalesced_jobs.get(key))
            .copied()
        {
            if let Some(job) = self.jobs.get_mut(&job_id) {
                if !job.cancelled.load(Ordering::Acquire) {
                    job.waiters.push(RequestWaiter {
                        id,
                        trace,
                        document_version,
                        cancelled: false,
                    });
                    self.request_jobs.insert(request_key, job_id);

                    return outbound;
                }
            }
        }

        if self.jobs.len() >= MAX_IN_FLIGHT_JOBS
            || non_interactive && self.non_interactive_job_count() >= MAX_NON_INTERACTIVE_JOBS
        {
            outbound.push(error_response(
                id,
                -32000,
                "language server request capacity exceeded".to_owned(),
            ));
            return outbound;
        }

        self.next_job_id = self.next_job_id.saturating_add(1);
        let job_id = self.next_job_id;
        let cancelled = Arc::new(AtomicBool::new(false));
        let local_inline = speculative.then(|| {
            let _profile = tua_core::profile::span("inline.request.prepare");
            server.prepare_local_inline_completion(&message)
        });
        let worker = WorkerJob {
            job_id,
            server: (!speculative).then(|| server.clone()),
            local_inline,
            queued_at: tua_core::profile::enabled().then(Instant::now),
            message,
            method,
            cancelled: Arc::clone(&cancelled),
            warm_work: None,
            corpus_work: None,
        };
        let in_flight = InFlightJob {
            non_interactive,
            uri,
            generation,
            cancelled,
            coalesce_key: coalesce_key.clone(),
            newest_wins_key: newest_wins_key.clone(),
            waiters: vec![RequestWaiter {
                id: id.clone(),
                trace,
                document_version,
                cancelled: false,
            }],
            slow_diagnostics: None,
            warm_work: None,
            corpus_work: None,
        };

        self.request_jobs.insert(request_key, job_id);
        if let Some(key) = coalesce_key {
            self.coalesced_jobs.insert(key, job_id);
        }
        if let Some(key) = newest_wins_key.clone() {
            self.newest_wins_jobs.insert(key, job_id);
            self.speculative_priority_until = Some(Instant::now() + SPECULATIVE_PRIORITY_WINDOW);
        }
        self.jobs.insert(job_id, in_flight);

        let send_result = if speculative {
            let key = newest_wins_key.expect("inline completion should have a newest-wins key");
            self.speculative_lane.enqueue(key, worker).map(|replaced| {
                if let Some(replaced) = replaced {
                    debug_assert!(!self.jobs.contains_key(&replaced.job_id));
                }
            })
        } else if background {
            self.background_tx.send(worker).map_err(|_| ())
        } else {
            self.foreground_tx.send(worker).map_err(|_| ())
        };

        if send_result.is_err() {
            self.remove_job(job_id);
            self.request_jobs.remove(&request_id_key(&id));
            outbound.push(error_response(
                id,
                -32603,
                "language server request worker stopped".to_owned(),
            ));
        }

        outbound
    }

    pub(super) fn dispatch_due_slow_diagnostic(
        &mut self,
        server: &mut LspServer,
        now: std::time::Instant,
    ) -> bool {
        if self.jobs.len() >= MAX_JOBS_WHILE_DISPATCHING_INTERNAL_BACKGROUND
            || self.internal_background_job_count() >= MAX_INTERNAL_BACKGROUND_JOBS
        {
            return false;
        }
        let Some((uri, pending)) = server.take_next_due_slow_diagnostic(now) else {
            return false;
        };

        self.next_job_id = self.next_job_id.saturating_add(1);
        let job_id = self.next_job_id;
        let cancelled = Arc::new(AtomicBool::new(false));
        let message = json!({
            "jsonrpc": "2.0",
            "id": Value::Null,
            "method": INTERNAL_SLOW_DIAGNOSTICS,
            "params": {
                "textDocument": { "uri": uri }
            }
        });
        let worker = WorkerJob {
            job_id,
            server: Some(server.clone()),
            local_inline: None,
            queued_at: tua_core::profile::enabled().then(Instant::now),
            message,
            method: INTERNAL_SLOW_DIAGNOSTICS.to_owned(),
            cancelled: Arc::clone(&cancelled),
            warm_work: None,
            corpus_work: None,
        };
        self.jobs.insert(
            job_id,
            InFlightJob {
                non_interactive: true,
                uri: Some(uri.clone()),
                generation: Some(pending.generation),
                cancelled,
                coalesce_key: None,
                newest_wins_key: None,
                waiters: Vec::new(),
                slow_diagnostics: Some((uri.clone(), pending)),
                warm_work: None,
                corpus_work: None,
            },
        );

        if self.background_tx.send(worker).is_err() {
            self.remove_job(job_id);
            return false;
        }

        true
    }

    pub(super) fn dispatch_next_workspace_warm(&mut self, server: &mut LspServer) -> bool {
        if self.has_in_flight() {
            return false;
        }
        let Some(work) = server.take_next_workspace_warm_work() else {
            return false;
        };

        self.next_job_id = self.next_job_id.saturating_add(1);
        let job_id = self.next_job_id;
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker = WorkerJob {
            job_id,
            server: Some(server.clone()),
            local_inline: None,
            queued_at: tua_core::profile::enabled().then(Instant::now),
            message: Value::Null,
            method: INTERNAL_WORKSPACE_WARM.to_owned(),
            cancelled: Arc::clone(&cancelled),
            warm_work: Some(work.clone()),
            corpus_work: None,
        };

        self.jobs.insert(
            job_id,
            InFlightJob {
                non_interactive: true,
                uri: None,
                generation: None,
                cancelled,
                coalesce_key: None,
                newest_wins_key: None,
                waiters: Vec::new(),
                slow_diagnostics: None,
                warm_work: Some(work),
                corpus_work: None,
            },
        );

        if self.background_tx.send(worker).is_err() {
            self.remove_job(job_id);
            return false;
        }

        true
    }

    pub(super) fn dispatch_due_pattern_corpus(
        &mut self,
        server: &mut LspServer,
        now: Instant,
    ) -> bool {
        if self.has_in_flight() {
            return false;
        }
        let Some(work) = server.take_next_pattern_corpus_work(now) else {
            return false;
        };

        self.next_job_id = self.next_job_id.saturating_add(1);
        let job_id = self.next_job_id;
        let cancelled = Arc::new(AtomicBool::new(false));
        let generation = work.generation;
        let paths = work.paths.clone();
        let worker = WorkerJob {
            job_id,
            server: None,
            local_inline: None,
            queued_at: tua_core::profile::enabled().then(Instant::now),
            message: Value::Null,
            method: INTERNAL_PATTERN_CORPUS.to_owned(),
            cancelled: Arc::clone(&cancelled),
            warm_work: None,
            corpus_work: Some(work),
        };

        self.jobs.insert(
            job_id,
            InFlightJob {
                non_interactive: true,
                uri: None,
                generation: None,
                cancelled,
                coalesce_key: None,
                newest_wins_key: None,
                waiters: Vec::new(),
                slow_diagnostics: None,
                warm_work: None,
                corpus_work: Some((generation, paths)),
            },
        );

        if self.background_tx.send(worker).is_err() {
            self.remove_job(job_id);
            return false;
        }

        true
    }

    pub(super) fn cancel_stale_pattern_corpus(&mut self, generation: u64) {
        for job in self.jobs.values() {
            if job
                .corpus_work
                .as_ref()
                .is_some_and(|(job_generation, _)| *job_generation != generation)
            {
                job.cancelled.store(true, Ordering::Release);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn pattern_corpus_job_is_cancelled(&self, generation: u64) -> bool {
        self.jobs.values().any(|job| {
            job.corpus_work.as_ref().is_some_and(|(job_generation, _)| {
                *job_generation == generation && job.cancelled.load(Ordering::Acquire)
            })
        })
    }

    pub(super) fn cancel(&mut self, id: &Value) -> Vec<Value> {
        let key = request_id_key(id);
        let Some(job_id) = self.request_jobs.remove(&key) else {
            return Vec::new();
        };
        let Some(job) = self.jobs.get_mut(&job_id) else {
            return Vec::new();
        };

        if let Some(index) = job
            .waiters
            .iter()
            .position(|waiter| waiter.id == *id && !waiter.cancelled)
        {
            job.waiters.remove(index);
        }
        if job.waiters.is_empty() {
            job.cancelled.store(true, Ordering::Release);
        }

        vec![error_response(
            id.clone(),
            REQUEST_CANCELLED,
            "request cancelled".to_owned(),
        )]
    }

    pub(super) fn cancel_stale_document_requests(
        &mut self,
        uri: &str,
        generation: Option<u64>,
        document_version: Option<i64>,
    ) -> Vec<Value> {
        let document_job_ids = self
            .jobs
            .iter()
            .filter_map(|(job_id, job)| (job.uri.as_deref() == Some(uri)).then_some(*job_id))
            .collect::<Vec<_>>();
        let mut outbound = Vec::new();

        for job_id in document_job_ids {
            let Some(job) = self.jobs.get_mut(&job_id) else {
                continue;
            };
            let stale_generation = job.generation != generation;

            let mut waiter_index = 0;
            while waiter_index < job.waiters.len() {
                let waiter = &job.waiters[waiter_index];
                let stale_version = match (waiter.document_version, document_version) {
                    (Some(request), Some(current)) => request != current,
                    _ => false,
                };
                if waiter.cancelled || (!stale_generation && !stale_version) {
                    waiter_index += 1;
                    continue;
                }
                let waiter = job.waiters.remove(waiter_index);
                self.request_jobs.remove(&request_id_key(&waiter.id));
                outbound.push(error_response(
                    waiter.id,
                    CONTENT_MODIFIED,
                    "document changed while request was running".to_owned(),
                ));
            }

            if stale_generation || job.waiters.is_empty() {
                job.cancelled.store(true, Ordering::Release);
            }
        }

        outbound
    }

    pub(super) fn abandon_workspace_warm(&mut self) {
        let warm_jobs = self
            .jobs
            .iter()
            .filter_map(|(job_id, job)| {
                (job.warm_work.is_some() || job.corpus_work.is_some()).then_some(*job_id)
            })
            .collect::<Vec<_>>();

        for job_id in warm_jobs {
            if let Some(job) = self.jobs.get(&job_id) {
                job.cancelled.store(true, Ordering::Release);
            }
            self.remove_job(job_id);
        }
    }

    pub(super) fn finish(&mut self, server: &mut LspServer, result: WorkerResult) -> Vec<Value> {
        let speculative_active = self.has_speculative_in_flight();
        let Some(job) = self.remove_job(result.job_id) else {
            return Vec::new();
        };
        let finish_kind = if job.warm_work.is_some() {
            "workspaceWarm"
        } else if job.corpus_work.is_some() {
            "patternCorpus"
        } else if job.slow_diagnostics.is_some() {
            "slowDiagnostics"
        } else if job.newest_wins_key.is_some() {
            "inline"
        } else {
            "request"
        };
        let _finish_profile = (tua_core::profile::enabled()
            && speculative_active
            && finish_kind != "inline")
            .then(|| tua_core::profile::span(format!("inline.coordinator.finish {finish_kind}")));
        if let Some(warm_result) = result.warm_result {
            return server.finish_workspace_warm(
                &warm_result.work,
                warm_result.analysis.as_ref(),
                warm_result.succeeded,
            );
        }
        if let Some(corpus_result) = result.corpus_result {
            server.finish_pattern_corpus(corpus_result);
            return Vec::new();
        }
        debug_assert!(job.warm_work.is_none());
        debug_assert!(job.corpus_work.is_none());
        let stale_generation = job
            .uri
            .as_ref()
            .is_some_and(|uri| server.document_generations.get(uri).copied() != job.generation);
        let mut outbound = Vec::new();

        if let Some((uri, pending)) = job.slow_diagnostics {
            if stale_generation || self.received_document_version_is_stale(&uri, pending.version) {
                return outbound;
            }
            if result.response.get("error").is_some() {
                return outbound;
            }
            let diagnostics = result
                .response
                .get("result")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if let Some(publication) = server.finish_slow_diagnostics(&uri, pending, diagnostics) {
                outbound.push(publication);
            }

            return outbound;
        }

        for waiter in job.waiters {
            self.request_jobs.remove(&request_id_key(&waiter.id));
            if waiter.cancelled {
                continue;
            }
            let stale = stale_generation
                || job.uri.as_ref().is_some_and(|uri| {
                    self.received_document_version_is_stale(uri, waiter.document_version)
                });
            if stale {
                server.record_stale_request_result();
                outbound.push(error_response(
                    waiter.id,
                    CONTENT_MODIFIED,
                    "document changed while request was running".to_owned(),
                ));
                continue;
            }

            let mut response = result.response.clone();
            response["id"] = waiter.id;
            outbound.extend(server.finish_request_trace(waiter.trace, &response));
            outbound.push(response);
        }

        outbound
    }

    pub(super) fn has_in_flight(&self) -> bool {
        !self.jobs.is_empty()
    }

    pub(super) fn in_flight_count(&self) -> usize {
        self.jobs.len()
    }

    #[cfg(test)]
    pub(super) fn speculative_pending_count(&self) -> usize {
        self.speculative_lane.pending_count()
    }

    pub(super) fn take_speculative_results(&self) -> Vec<WorkerResult> {
        self.speculative_lane.results.try_iter().collect()
    }

    pub(super) fn speculative_has_priority(&self, now: Instant) -> bool {
        !self.newest_wins_jobs.is_empty()
            && self
                .speculative_priority_until
                .is_some_and(|deadline| now < deadline)
    }

    pub(super) fn has_speculative_in_flight(&self) -> bool {
        !self.newest_wins_jobs.is_empty()
    }

    fn internal_background_job_count(&self) -> usize {
        self.jobs
            .values()
            .filter(|job| job.slow_diagnostics.is_some() || job.warm_work.is_some())
            .count()
    }

    fn non_interactive_job_count(&self) -> usize {
        self.jobs.values().filter(|job| job.non_interactive).count()
    }

    fn received_document_version_is_stale(&self, uri: &str, document_version: Option<i64>) -> bool {
        let Some(document_version) = document_version else {
            return false;
        };
        let versions = self
            .received_input
            .document_versions
            .lock()
            .expect("received document version lock poisoned");

        matches!(versions.get(uri), Some(version) if *version != Some(document_version))
    }

    fn cancel_superseded_request(&mut self, key: &str) -> Vec<Value> {
        let Some(job_id) = self.newest_wins_jobs.remove(key) else {
            return Vec::new();
        };
        let Some(job) = self.remove_job(job_id) else {
            return Vec::new();
        };

        job.cancelled.store(true, Ordering::Release);
        job.waiters
            .into_iter()
            .filter(|waiter| !waiter.cancelled)
            .map(|waiter| {
                self.request_jobs.remove(&request_id_key(&waiter.id));
                error_response(
                    waiter.id,
                    REQUEST_CANCELLED,
                    "request superseded by a newer inline completion".to_owned(),
                )
            })
            .collect()
    }

    fn remove_job(&mut self, job_id: u64) -> Option<InFlightJob> {
        let job = self.jobs.remove(&job_id)?;
        if let Some(key) = &job.coalesce_key {
            if self.coalesced_jobs.get(key) == Some(&job_id) {
                self.coalesced_jobs.remove(key);
            }
        }
        if let Some(key) = &job.newest_wins_key {
            if self.newest_wins_jobs.get(key) == Some(&job_id) {
                self.newest_wins_jobs.remove(key);
            }
        }

        Some(job)
    }
}

fn spawn_worker_lane(
    lane: &str,
    worker_count: usize,
    event_tx: SyncSender<ServerEvent>,
) -> Sender<WorkerJob> {
    let (tx, rx) = mpsc::channel::<WorkerJob>();
    let rx = Arc::new(Mutex::new(rx));

    for worker_index in 0..worker_count {
        let rx = Arc::clone(&rx);
        let event_tx = event_tx.clone();

        thread::Builder::new()
            .name(format!("tua-lsp-{lane}-{worker_index}"))
            .stack_size(REQUEST_WORKER_STACK_BYTES)
            .spawn(move || loop {
                let job = {
                    let receiver = rx.lock().expect("request worker queue lock poisoned");
                    receiver.recv()
                };
                let Ok(job) = job else {
                    break;
                };
                let result = run_worker_job(job);
                let _ = event_tx.send(ServerEvent::RequestFinished(Box::new(result)));
            })
            .expect("language server request worker should start");
    }

    tx
}

impl SpeculativeLane {
    fn enqueue(&self, key: String, job: WorkerJob) -> Result<Option<WorkerJob>, ()> {
        let mut queue = self
            .state
            .queue
            .lock()
            .expect("speculative worker queue lock poisoned");
        if queue.stopped {
            return Err(());
        }

        let replaced = queue.jobs.insert(key.clone(), job);
        if replaced.is_none() {
            queue.order.push_back(key);
        }
        self.state.ready.notify_one();

        Ok(replaced)
    }

    #[cfg(test)]
    fn pending_count(&self) -> usize {
        self.state
            .queue
            .lock()
            .expect("speculative worker queue lock poisoned")
            .jobs
            .len()
    }
}

impl Drop for SpeculativeLane {
    fn drop(&mut self) {
        let mut queue = self
            .state
            .queue
            .lock()
            .expect("speculative worker queue lock poisoned");
        queue.stopped = true;
        self.state.ready.notify_all();
    }
}

fn spawn_speculative_lane(event_tx: SyncSender<ServerEvent>) -> SpeculativeLane {
    let state = Arc::new(SpeculativeLaneState {
        queue: Mutex::new(SpeculativeQueue::default()),
        ready: Condvar::new(),
    });
    let worker_state = Arc::clone(&state);
    let (result_tx, results) = mpsc::channel();

    thread::Builder::new()
        .name("tua-lsp-speculative-0".to_owned())
        .stack_size(REQUEST_WORKER_STACK_BYTES)
        .spawn(move || loop {
            let job = {
                let mut queue = worker_state
                    .queue
                    .lock()
                    .expect("speculative worker queue lock poisoned");
                while queue.order.is_empty() && !queue.stopped {
                    queue = worker_state
                        .ready
                        .wait(queue)
                        .expect("speculative worker queue lock poisoned");
                }
                if queue.stopped {
                    break;
                }
                let key = queue
                    .order
                    .pop_front()
                    .expect("non-empty speculative queue should have a key");
                queue
                    .jobs
                    .remove(&key)
                    .expect("speculative queue key should have a job")
            };

            if result_tx.send(run_worker_job(job)).is_err() {
                break;
            }
            let _ = event_tx.send(ServerEvent::SpeculativeReady);
        })
        .expect("speculative request worker should start");

    SpeculativeLane { state, results }
}

fn run_worker_job(job: WorkerJob) -> WorkerResult {
    let WorkerJob {
        job_id,
        queued_at,
        mut server,
        local_inline,
        message,
        method,
        cancelled,
        warm_work,
        corpus_work,
    } = job;
    if let Some(queued_at) = queued_at {
        tua_core::profile::record_duration(format!("lsp.queue {method}"), queued_at.elapsed());
    }
    let (response_value, warm_result, corpus_result) = if let Some(work) = corpus_work {
        let started = Instant::now();
        let PatternCorpusWork {
            generation,
            paths,
            operation,
        } = work;
        let mut entries = Vec::new();
        let mut remerge = None;
        let mut completed = 0usize;
        let mut disposed = false;
        let succeeded = panic::catch_unwind(AssertUnwindSafe(|| match operation {
            super::PatternCorpusOperation::Files {
                inputs,
                project_model,
                retrieval_limits,
            } => {
                entries.reserve(inputs.len());
                for input in inputs {
                    if cancelled.load(Ordering::Acquire) {
                        break;
                    }
                    let outcome = {
                        let _profile = tua_core::profile::span("inline.index.file");
                        input.build(&project_model, retrieval_limits)
                    };
                    entries.push(tua_core::PatternCorpusBatchEntry::File(outcome));
                    completed += 1;
                }
            }
            super::PatternCorpusOperation::Remerge(prepared) => {
                if !cancelled.load(Ordering::Acquire) {
                    let _profile = tua_core::profile::span("inline.index.remerge");
                    remerge = Some(prepared.build());
                }
            }
            super::PatternCorpusOperation::Dispose(disposals) => {
                for disposal in disposals {
                    match disposal {
                        super::PatternCorpusDisposal::Retired(retired) => drop(retired),
                        super::PatternCorpusDisposal::RejectedRemerge(rejected) => drop(rejected),
                    }
                }
                disposed = true;
            }
        }))
        .is_ok();
        let was_cancelled = cancelled.load(Ordering::Acquire) || !succeeded;
        let unfinished_paths = paths.iter().skip(completed).cloned().collect();
        let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

        (
            Value::Null,
            None,
            Some(PatternCorpusWorkerResult {
                generation,
                paths,
                entries,
                remerge,
                unfinished_paths,
                cancelled: was_cancelled,
                elapsed_ms,
                disposed,
            }),
        )
    } else if let Some(work) = warm_work {
        let worker_server = server
            .take()
            .expect("workspace warm job should carry a server clone");
        let succeeded = if cancelled.load(Ordering::Acquire) {
            false
        } else {
            panic::catch_unwind(AssertUnwindSafe(|| match &work {
                WorkspaceWarmWork::Files { files, .. } => {
                    let mut completed = true;
                    for (path, priority) in files {
                        if cancelled.load(Ordering::Acquire) {
                            completed = false;
                            break;
                        }
                        let _ = worker_server.analysis.warm_file(path, *priority);
                    }
                    completed
                }
                WorkspaceWarmWork::Snapshot { .. } => {
                    let _ = worker_server
                        .analysis
                        .snapshot(&worker_server.workspace_folders, &worker_server.settings);
                    true
                }
            }))
            .unwrap_or(false)
        };
        let analysis =
            matches!(work, WorkspaceWarmWork::Files { .. }).then_some(worker_server.analysis);

        (
            Value::Null,
            Some(WorkspaceWarmResult {
                work,
                analysis,
                succeeded,
            }),
            None,
        )
    } else if cancelled.load(Ordering::Acquire) {
        (
            error_response(
                Value::Null,
                REQUEST_CANCELLED,
                "request cancelled".to_owned(),
            ),
            None,
            None,
        )
    } else if let Some(work) = local_inline {
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        let response = panic::catch_unwind(AssertUnwindSafe(|| {
            #[cfg(test)]
            run_speculative_test_controls(&message);
            work.execute(&cancelled)
        }))
        .map(|result| response(id, result))
        .unwrap_or_else(|_| {
            error_response(
                Value::Null,
                -32603,
                "language server speculative request worker panicked".to_owned(),
            )
        });
        (response, None, None)
    } else {
        let server = server
            .as_mut()
            .expect("ordinary worker job should carry a server clone");
        let response = panic::catch_unwind(AssertUnwindSafe(|| {
            execute_worker_request(server, message, &method, &cancelled)
        }))
        .unwrap_or_else(|_| {
            error_response(
                Value::Null,
                -32603,
                "language server request worker panicked".to_owned(),
            )
        });

        (response, None, None)
    };

    WorkerResult {
        job_id,
        response: response_value,
        warm_result,
        corpus_result,
    }
}

fn execute_worker_request(
    server: &mut LspServer,
    message: Value,
    method: &str,
    cancelled: &AtomicBool,
) -> Value {
    if method == INTERNAL_SLOW_DIAGNOSTICS {
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        let diagnostics = document_uri(&message)
            .map(|uri| server.diagnostics_for(uri))
            .unwrap_or_default();

        return json!({ "jsonrpc": "2.0", "id": id, "result": diagnostics });
    }

    #[cfg(test)]
    if method == "tua/testSlowBackground" {
        let execution = TEST_SLOW_EXECUTIONS.fetch_add(1, Ordering::Relaxed);
        let delay_ms = message
            .get("params")
            .and_then(|params| params.get("delayMs"))
            .and_then(Value::as_u64)
            .unwrap_or(250);
        let id = message.get("id").cloned().unwrap_or(Value::Null);

        thread::sleep(std::time::Duration::from_millis(delay_ms));
        return json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "slow": true, "execution": execution }
        });
    }

    server.execute_request_with_cancellation(message, method, cancelled)
}

#[cfg(test)]
fn run_speculative_test_controls(message: &Value) {
    let params = message.get("params");
    if params
        .and_then(|params| params.get("testPanic"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        panic!("requested speculative worker test panic");
    }
    if let Some(delay_ms) = params
        .and_then(|params| params.get("testDelayMs"))
        .and_then(Value::as_u64)
    {
        thread::sleep(std::time::Duration::from_millis(delay_ms));
    }
}

fn request_runs_in_background(method: &str) -> bool {
    method_is_background_refresh(method)
        || method == "tua/signalGraph"
        || cfg!(test) && method == "tua/testSlowBackground"
}

pub(super) fn request_runs_on_coordinator(method: &str) -> bool {
    matches!(
        method,
        "initialize"
            | "shutdown"
            | "tua/builtinDocument"
            | "tua/projectLayout"
            | "tua/cacheStats"
            | "tua/health"
            | "tua/resetCompletionStats"
    )
}

fn coalesce_key(method: &str, message: &Value, generation: Option<u64>) -> String {
    let params = message.get("params").cloned().unwrap_or(Value::Null);

    format!("{method}:{generation:?}:{params}")
}

fn request_id_key(id: &Value) -> String {
    id.to_string()
}

fn document_uri(message: &Value) -> Option<&str> {
    message
        .get("params")?
        .get("textDocument")?
        .get("uri")?
        .as_str()
}
