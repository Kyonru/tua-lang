use std::collections::BTreeMap;
use std::io;
use std::panic::{self, AssertUnwindSafe};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::json;
use serde_json::Value;

use super::handlers::RequestTrace;
use super::protocol::error_response;
use super::{method_is_background_refresh, LspServer, PendingDiagnostics};

const REQUEST_CANCELLED: i64 = -32800;
const CONTENT_MODIFIED: i64 = -32801;
const INTERNAL_SLOW_DIAGNOSTICS: &str = "tua/internalSlowDiagnostics";

#[cfg(test)]
static TEST_SLOW_EXECUTIONS: AtomicUsize = AtomicUsize::new(0);

pub(super) enum ServerEvent {
    Input(io::Result<Option<Value>>),
    RequestFinished(WorkerResult),
}

pub(super) struct RequestScheduler {
    foreground_tx: Sender<WorkerJob>,
    background_tx: Sender<WorkerJob>,
    jobs: BTreeMap<u64, InFlightJob>,
    request_jobs: BTreeMap<String, u64>,
    coalesced_jobs: BTreeMap<String, u64>,
    next_job_id: u64,
}

pub(super) struct WorkerResult {
    job_id: u64,
    response: Value,
}

struct WorkerJob {
    job_id: u64,
    server: LspServer,
    message: Value,
    method: String,
    cancelled: Arc<AtomicBool>,
}

struct InFlightJob {
    uri: Option<String>,
    generation: Option<u64>,
    background: bool,
    cancelled: Arc<AtomicBool>,
    coalesce_key: Option<String>,
    waiters: Vec<RequestWaiter>,
    slow_diagnostics: Option<(String, PendingDiagnostics)>,
}

struct RequestWaiter {
    id: Value,
    trace: RequestTrace,
    cancelled: bool,
}

impl RequestScheduler {
    pub(super) fn new(event_tx: Sender<ServerEvent>) -> Self {
        let foreground_tx = spawn_worker_lane(2, event_tx.clone());
        let background_tx = spawn_worker_lane(1, event_tx);

        Self {
            foreground_tx,
            background_tx,
            jobs: BTreeMap::new(),
            request_jobs: BTreeMap::new(),
            coalesced_jobs: BTreeMap::new(),
            next_job_id: 0,
        }
    }

    pub(super) fn dispatch(&mut self, server: &mut LspServer, message: Value) -> Vec<Value> {
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
        let background = request_runs_in_background(&method);
        let coalesce_key =
            (background && uri.is_some()).then(|| coalesce_key(&method, &message, generation));
        let (mut outbound, trace) = server.begin_request_trace(&message, &method);

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
                        cancelled: false,
                    });
                    self.request_jobs.insert(request_key, job_id);

                    return outbound;
                }
            }
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
        };
        let in_flight = InFlightJob {
            uri,
            generation,
            background,
            cancelled,
            coalesce_key: coalesce_key.clone(),
            waiters: vec![RequestWaiter {
                id: id.clone(),
                trace,
                cancelled: false,
            }],
            slow_diagnostics: None,
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
        };
        self.jobs.insert(
            job_id,
            InFlightJob {
                uri: Some(uri.clone()),
                generation: Some(pending.generation),
                background: true,
                cancelled,
                coalesce_key: None,
                waiters: Vec::new(),
                slow_diagnostics: Some((uri.clone(), pending)),
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

        for waiter in &mut job.waiters {
            if waiter.id == *id && !waiter.cancelled {
                waiter.cancelled = true;
                break;
            }
        }
        if job.waiters.iter().all(|waiter| waiter.cancelled) {
            job.cancelled.store(true, Ordering::Release);
        }

        vec![error_response(
            id.clone(),
            REQUEST_CANCELLED,
            "request cancelled".to_owned(),
        )]
    }

    pub(super) fn cancel_stale_background(
        &mut self,
        uri: &str,
        generation: Option<u64>,
    ) -> Vec<Value> {
        let stale_job_ids = self
            .jobs
            .iter()
            .filter_map(|(job_id, job)| {
                (job.background && job.uri.as_deref() == Some(uri) && job.generation != generation)
                    .then_some(*job_id)
            })
            .collect::<Vec<_>>();
        let mut outbound = Vec::new();

        for job_id in stale_job_ids {
            let Some(job) = self.jobs.get_mut(&job_id) else {
                continue;
            };
            job.cancelled.store(true, Ordering::Release);

            for waiter in &mut job.waiters {
                if waiter.cancelled {
                    continue;
                }
                waiter.cancelled = true;
                self.request_jobs.remove(&request_id_key(&waiter.id));
                outbound.push(error_response(
                    waiter.id.clone(),
                    CONTENT_MODIFIED,
                    "document changed while request was running".to_owned(),
                ));
            }
        }

        outbound
    }

    pub(super) fn finish(&mut self, server: &mut LspServer, result: WorkerResult) -> Vec<Value> {
        let Some(job) = self.remove_job(result.job_id) else {
            return Vec::new();
        };
        let stale = job
            .uri
            .as_ref()
            .is_some_and(|uri| server.document_generations.get(uri).copied() != job.generation);
        let mut outbound = Vec::new();

        if let Some((uri, pending)) = job.slow_diagnostics {
            if stale {
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
            if stale {
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

fn spawn_worker_lane(worker_count: usize, event_tx: Sender<ServerEvent>) -> Sender<WorkerJob> {
    let (tx, rx) = mpsc::channel::<WorkerJob>();
    let rx = Arc::new(Mutex::new(rx));

    for _ in 0..worker_count {
        let rx = Arc::clone(&rx);
        let event_tx = event_tx.clone();

        thread::spawn(move || loop {
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
            } = job;
            let response = if cancelled.load(Ordering::Acquire) {
                error_response(
                    Value::Null,
                    REQUEST_CANCELLED,
                    "request cancelled".to_owned(),
                )
            } else {
                panic::catch_unwind(AssertUnwindSafe(|| {
                    execute_worker_request(&mut server, message, &method)
                }))
                .unwrap_or_else(|_| {
                    error_response(
                        Value::Null,
                        -32603,
                        "language server request worker panicked".to_owned(),
                    )
                })
            };

            let event = ServerEvent::RequestFinished(WorkerResult { job_id, response });
            let _ = event_tx.send(event);
        });
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
            | "tua/cacheStats"
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
