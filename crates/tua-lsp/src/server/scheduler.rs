use std::collections::BTreeMap;
use std::io;
use std::panic::{self, AssertUnwindSafe};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::json;
use serde_json::Value;

use super::analysis::AnalysisHost;
use super::handlers::RequestTrace;
use super::protocol::error_response;
use super::{
    method_is_background_refresh, LspServer, PendingDiagnostics, SharedReceivedInputState,
    WorkspaceWarmWork,
};

const REQUEST_CANCELLED: i64 = -32800;
const CONTENT_MODIFIED: i64 = -32801;
const INTERNAL_SLOW_DIAGNOSTICS: &str = "tua/internalSlowDiagnostics";
const INTERNAL_WORKSPACE_WARM: &str = "tua/internalWorkspaceWarm";
const REQUEST_WORKER_STACK_BYTES: usize = 8 * 1024 * 1024;
pub(super) const MAX_IN_FLIGHT_JOBS: usize = 16;
pub(super) const MAX_PENDING_REQUESTS: usize = 256;
const MAX_NON_INTERACTIVE_JOBS: usize = MAX_IN_FLIGHT_JOBS * 3 / 4;
const MAX_INTERNAL_BACKGROUND_JOBS: usize = MAX_IN_FLIGHT_JOBS / 2;

#[cfg(test)]
static TEST_SLOW_EXECUTIONS: AtomicUsize = AtomicUsize::new(0);

pub(super) enum ServerEvent {
    Input(io::Result<Option<Value>>),
    RequestFinished(Box<WorkerResult>),
}

pub(super) struct RequestScheduler {
    foreground_tx: Sender<WorkerJob>,
    background_tx: Sender<WorkerJob>,
    jobs: BTreeMap<u64, InFlightJob>,
    request_jobs: BTreeMap<String, u64>,
    coalesced_jobs: BTreeMap<String, u64>,
    received_input: SharedReceivedInputState,
    next_job_id: u64,
}

pub(super) struct WorkerResult {
    job_id: u64,
    response: Value,
    warm_result: Option<WorkspaceWarmResult>,
}

struct WorkspaceWarmResult {
    work: WorkspaceWarmWork,
    analysis: Option<AnalysisHost>,
    succeeded: bool,
}

struct WorkerJob {
    job_id: u64,
    server: LspServer,
    message: Value,
    method: String,
    cancelled: Arc<AtomicBool>,
    warm_work: Option<WorkspaceWarmWork>,
}

struct InFlightJob {
    non_interactive: bool,
    uri: Option<String>,
    generation: Option<u64>,
    cancelled: Arc<AtomicBool>,
    coalesce_key: Option<String>,
    waiters: Vec<RequestWaiter>,
    slow_diagnostics: Option<(String, PendingDiagnostics)>,
    warm_work: Option<WorkspaceWarmWork>,
}

struct RequestWaiter {
    id: Value,
    trace: RequestTrace,
    document_version: Option<i64>,
    cancelled: bool,
}

impl RequestScheduler {
    pub(super) fn new(
        event_tx: SyncSender<ServerEvent>,
        received_input: SharedReceivedInputState,
    ) -> Self {
        let foreground_tx = spawn_worker_lane("foreground", 2, event_tx.clone());
        let background_tx = spawn_worker_lane("background", 1, event_tx);

        Self {
            foreground_tx,
            background_tx,
            jobs: BTreeMap::new(),
            request_jobs: BTreeMap::new(),
            coalesced_jobs: BTreeMap::new(),
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
        let non_interactive = background || method == "textDocument/inlayHint";
        let coalesce_key = ((background || method == "textDocument/inlayHint") && uri.is_some())
            .then(|| coalesce_key(&method, &message, generation));
        let (mut outbound, trace) = server.begin_request_trace(&message, &method);

        if self.request_jobs.contains_key(&request_key) {
            outbound.push(error_response(
                id,
                -32600,
                "request id is already in flight".to_owned(),
            ));
            return outbound;
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
        let worker = WorkerJob {
            job_id,
            server: server.clone(),
            message,
            method,
            cancelled: Arc::clone(&cancelled),
            warm_work: None,
        };
        let in_flight = InFlightJob {
            non_interactive,
            uri,
            generation,
            cancelled,
            coalesce_key: coalesce_key.clone(),
            waiters: vec![RequestWaiter {
                id: id.clone(),
                trace,
                document_version,
                cancelled: false,
            }],
            slow_diagnostics: None,
            warm_work: None,
        };

        self.request_jobs.insert(request_key, job_id);
        if let Some(key) = coalesce_key {
            self.coalesced_jobs.insert(key, job_id);
        }
        self.jobs.insert(job_id, in_flight);

        let send_result = if background {
            self.background_tx.send(worker)
        } else {
            self.foreground_tx.send(worker)
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
        if self.jobs.len() >= MAX_IN_FLIGHT_JOBS
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
            server: server.clone(),
            message,
            method: INTERNAL_SLOW_DIAGNOSTICS.to_owned(),
            cancelled: Arc::clone(&cancelled),
            warm_work: None,
        };
        self.jobs.insert(
            job_id,
            InFlightJob {
                non_interactive: true,
                uri: Some(uri.clone()),
                generation: Some(pending.generation),
                cancelled,
                coalesce_key: None,
                waiters: Vec::new(),
                slow_diagnostics: Some((uri.clone(), pending)),
                warm_work: None,
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
            server: server.clone(),
            message: Value::Null,
            method: INTERNAL_WORKSPACE_WARM.to_owned(),
            cancelled: Arc::clone(&cancelled),
            warm_work: Some(work.clone()),
        };

        self.jobs.insert(
            job_id,
            InFlightJob {
                non_interactive: true,
                uri: None,
                generation: None,
                cancelled,
                coalesce_key: None,
                waiters: Vec::new(),
                slow_diagnostics: None,
                warm_work: Some(work),
            },
        );

        if self.background_tx.send(worker).is_err() {
            self.remove_job(job_id);
            return false;
        }

        true
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
            .filter_map(|(job_id, job)| job.warm_work.is_some().then_some(*job_id))
            .collect::<Vec<_>>();

        for job_id in warm_jobs {
            if let Some(job) = self.jobs.get(&job_id) {
                job.cancelled.store(true, Ordering::Release);
            }
            self.remove_job(job_id);
        }
    }

    pub(super) fn finish(&mut self, server: &mut LspServer, result: WorkerResult) -> Vec<Value> {
        let Some(job) = self.remove_job(result.job_id) else {
            return Vec::new();
        };
        if let Some(warm_result) = result.warm_result {
            return server.finish_workspace_warm(
                &warm_result.work,
                warm_result.analysis.as_ref(),
                warm_result.succeeded,
            );
        }
        debug_assert!(job.warm_work.is_none());
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

    fn remove_job(&mut self, job_id: u64) -> Option<InFlightJob> {
        let job = self.jobs.remove(&job_id)?;
        if let Some(key) = &job.coalesce_key {
            if self.coalesced_jobs.get(key) == Some(&job_id) {
                self.coalesced_jobs.remove(key);
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
                let WorkerJob {
                    job_id,
                    mut server,
                    message,
                    method,
                    cancelled,
                    warm_work,
                } = job;
                let (response, warm_result) = if let Some(work) = warm_work {
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
                                    let _ = server.analysis.warm_file(path, *priority);
                                }
                                completed
                            }
                            WorkspaceWarmWork::Snapshot { .. } => {
                                let _ = server
                                    .analysis
                                    .snapshot(&server.workspace_folders, &server.settings);
                                true
                            }
                        }))
                        .unwrap_or(false)
                    };
                    let analysis =
                        matches!(work, WorkspaceWarmWork::Files { .. }).then_some(server.analysis);

                    (
                        Value::Null,
                        Some(WorkspaceWarmResult {
                            work,
                            analysis,
                            succeeded,
                        }),
                    )
                } else if cancelled.load(Ordering::Acquire) {
                    (
                        error_response(
                            Value::Null,
                            REQUEST_CANCELLED,
                            "request cancelled".to_owned(),
                        ),
                        None,
                    )
                } else {
                    let response = panic::catch_unwind(AssertUnwindSafe(|| {
                        execute_worker_request(&mut server, message, &method)
                    }))
                    .unwrap_or_else(|_| {
                        error_response(
                            Value::Null,
                            -32603,
                            "language server request worker panicked".to_owned(),
                        )
                    });

                    (response, None)
                };

                let event = ServerEvent::RequestFinished(Box::new(WorkerResult {
                    job_id,
                    response,
                    warm_result,
                }));
                let _ = event_tx.send(event);
            })
            .expect("language server request worker should start");
    }

    tx
}

fn execute_worker_request(server: &mut LspServer, message: Value, method: &str) -> Value {
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

    server.execute_request(message, method)
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
