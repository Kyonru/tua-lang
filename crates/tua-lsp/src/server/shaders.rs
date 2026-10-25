//! Shader compilation has its own idle worker; interactive requests never wait for LOVE.
use super::analysis::AnalysisSnapshot;
use super::protocol::{lsp_diagnostic_from_core, publish_diagnostics};
use super::scheduler::ServerEvent;
use super::uri::{path_from_uri, uri_from_path};
use super::LspServer;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, SyncSender};
use std::time::{Duration, Instant};
use tua_core::shader_validation::{validation_diagnostics, ShaderValidator};

const DEBOUNCE: Duration = Duration::from_millis(500);

#[derive(Clone, Default)]
pub(super) struct ShaderState {
    enabled: bool,
    generation: u64,
    due: Option<Instant>,
    busy: bool,
    observed_revision: u64,
    diagnostics: BTreeMap<String, Vec<Value>>,
    last_note: Option<String>,
}

struct Job {
    generation: u64,
    snapshot: AnalysisSnapshot,
    overlays: BTreeMap<PathBuf, String>,
}

pub(super) struct ShaderResult {
    generation: u64,
    diagnostics: BTreeMap<String, Vec<Value>>,
    note: Option<String>,
}

pub(super) struct ShaderWorker {
    sender: Option<SyncSender<Job>>,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ShaderWorker {
    pub(super) fn new(
        mut validator: Box<dyn ShaderValidator>,
        events: SyncSender<ServerEvent>,
    ) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Job>(1);
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopped = cancelled.clone();
        let thread = std::thread::spawn(move || {
            let mut previous_inputs = Vec::new();
            let mut previous_outcomes = Vec::new();
            while let Ok(job) = receiver.recv() {
                let result = (|| {
                    let requests = job.snapshot.shader_requests(&job.overlays)?;
                    if requests.is_empty() {
                        validator.stop();
                    }
                    let inputs: Vec<_> = requests
                        .iter()
                        .map(|request| {
                            (
                                request.asset.path.clone(),
                                request.target,
                                request.source.clone(),
                            )
                        })
                        .collect();
                    let outcomes = if inputs == previous_inputs {
                        previous_outcomes.clone()
                    } else {
                        let outcomes = validator.validate(&requests)?;
                        if outcomes.len() != requests.len() {
                            return Err("incomplete shader validator results".to_owned());
                        }
                        previous_inputs = inputs;
                        previous_outcomes = outcomes.clone();
                        outcomes
                    };
                    let mut diagnostics = BTreeMap::<String, Vec<Value>>::new();
                    for diagnostic in job
                        .snapshot
                        .apply_shader_policy(validation_diagnostics(&requests, &outcomes))
                    {
                        diagnostics
                            .entry(uri_from_path(&diagnostic.file))
                            .or_default()
                            .push(lsp_diagnostic_from_core(diagnostic));
                    }
                    Ok(diagnostics)
                })();
                let (diagnostics, note) = match result {
                    Ok(diagnostics) => (diagnostics, None),
                    Err(note) => (BTreeMap::new(), Some(note)),
                };
                let mut event = ServerEvent::ShadersFinished(ShaderResult {
                    generation: job.generation,
                    diagnostics,
                    note,
                });
                loop {
                    if stopped.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }
                    match events.try_send(event) {
                        Ok(()) => break,
                        Err(mpsc::TrySendError::Disconnected(_)) => return,
                        Err(mpsc::TrySendError::Full(pending)) => {
                            event = pending;
                            std::thread::sleep(Duration::from_millis(5));
                        }
                    }
                }
            }
        });
        Self {
            sender: Some(sender),
            cancelled,
            thread: Some(thread),
        }
    }

    pub(super) fn dispatch_due(&self, server: &mut LspServer) {
        let now = Instant::now();
        if !server.shaders.enabled
            || server.shaders.busy
            || !server.shaders.due.is_some_and(|due| due <= now)
        {
            return;
        }
        // Cloning the warmed snapshot and small shader overlays does no analysis or disk I/O.
        let snapshot = server
            .analysis
            .snapshot(&server.workspace_folders, &server.settings);
        let overlays = server
            .analysis
            .open_uris()
            .into_iter()
            .filter_map(|uri| {
                let path = path_from_uri(&uri);
                let extension = path.extension()?.to_str()?;
                matches!(extension, "glsl" | "vert" | "frag" | "vs" | "fs")
                    .then(|| {
                        server
                            .analysis
                            .document_text(&uri)
                            .map(|source| (path, source.to_owned()))
                    })
                    .flatten()
            })
            .collect();
        let job = Job {
            generation: server.shaders.generation,
            snapshot,
            overlays,
        };
        if self
            .sender
            .as_ref()
            .is_some_and(|sender| sender.try_send(job).is_ok())
        {
            server.shaders.busy = true;
            server.shaders.due = None;
        }
    }
}

impl LspServer {
    pub(super) fn configure_shaders(&mut self, message: &Value) {
        // Clients must explicitly attest workspace trust before external tools run.
        self.shaders.enabled = message
            .pointer("/params/initializationOptions/workspaceTrusted")
            .and_then(Value::as_bool)
            == Some(true);
        self.queue_shaders();
    }

    pub(super) fn queue_shaders(&mut self) {
        if self.shaders.enabled {
            self.shaders.generation = self.shaders.generation.wrapping_add(1);
            self.shaders.due = Some(Instant::now() + DEBOUNCE);
            self.shaders.observed_revision = self.analysis.revision();
        }
    }

    pub(super) fn observe_shader_revision(&mut self) {
        if self.shaders.observed_revision != self.analysis.revision() {
            self.queue_shaders();
        }
    }

    pub(super) fn shader_due(&self) -> Option<Instant> {
        (!self.shaders.busy).then_some(self.shaders.due).flatten()
    }

    pub(super) fn shader_diagnostics(&self, uri: &str) -> impl Iterator<Item = Value> + '_ {
        self.shaders
            .diagnostics
            .get(uri)
            .into_iter()
            .flatten()
            .cloned()
    }

    pub(super) fn finish_shaders(&mut self, result: ShaderResult) -> Vec<Value> {
        self.shaders.busy = false;
        if result.generation != self.shaders.generation {
            return Vec::new();
        }
        let mut messages = Vec::new();
        if let Some(note) = &result.note {
            if self.shaders.last_note.as_ref() != Some(note) {
                messages.push(
                    json!({"jsonrpc":"2.0", "method":"window/logMessage", "params":{
                        "type": 2, "message": format!("Shader validation unavailable: {note}")
                    }}),
                );
            }
        }
        self.shaders.last_note = result.note;
        let old = std::mem::replace(&mut self.shaders.diagnostics, result.diagnostics);
        let paths: std::collections::BTreeSet<_> = old
            .keys()
            .chain(self.shaders.diagnostics.keys())
            .cloned()
            .collect();
        let mut changed = false;
        for uri in paths {
            if old.get(&uri) == self.shaders.diagnostics.get(&uri) {
                continue;
            }
            changed = true;
            // Pull clients own source-document diagnostics. Refresh their full
            // report instead of replacing it with a shader-only publication.
            let path = path_from_uri(&uri);
            let shader_file = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    matches!(extension, "glsl" | "vert" | "frag" | "vs" | "fs")
                });
            if self.client_pull_diagnostics && !shader_file {
                continue;
            }
            // Preserve existing language diagnostics and replace only validator-owned entries.
            let mut diagnostics = self
                .last_published_diagnostics
                .get(&uri)
                .cloned()
                .unwrap_or_default();
            diagnostics.retain(|diagnostic| {
                diagnostic.get("code").and_then(Value::as_str) != Some("TL2016")
            });
            diagnostics.extend(self.shader_diagnostics(&uri));
            self.last_published_diagnostics
                .insert(uri.clone(), diagnostics.clone());
            messages.push(publish_diagnostics(&uri, diagnostics));
        }
        if changed && self.client_pull_diagnostics && self.client_diagnostic_refresh {
            let id = self.next_request_id();
            messages.push(super::protocol::workspace_diagnostic_refresh_request(id));
        }
        messages
    }
}

impl Drop for ShaderWorker {
    fn drop(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_shader_result_is_discarded_without_replacing_current_diagnostics() {
        let mut server = LspServer::default();
        server.shaders.enabled = true;
        server.queue_shaders();
        let generation = server.shaders.generation;
        server.queue_shaders();
        let messages = server.finish_shaders(ShaderResult {
            generation,
            diagnostics: BTreeMap::from([(
                "file:///old.glsl".into(),
                vec![json!({"code":"TL2016"})],
            )]),
            note: None,
        });
        assert!(messages.is_empty());
        assert!(server.shaders.diagnostics.is_empty());
        assert!(server.shader_due().is_some());
    }
    #[test]
    fn pull_clients_refresh_source_reports_without_shader_only_replacements() {
        let mut server = LspServer {
            client_pull_diagnostics: true,
            client_diagnostic_refresh: true,
            ..Default::default()
        };
        let messages = server.finish_shaders(ShaderResult {
            generation: 0,
            diagnostics: BTreeMap::from([
                ("file:///main.tua".into(), vec![json!({"code":"TL2016"})]),
                ("file:///effect.glsl".into(), vec![json!({"code":"TL2016"})]),
            ]),
            note: None,
        });
        assert!(messages
            .iter()
            .any(|message| message["method"] == "workspace/diagnostic/refresh"));
        assert!(messages
            .iter()
            .any(|message| message["params"]["uri"] == "file:///effect.glsl"));
        assert!(!messages
            .iter()
            .any(|message| message["params"]["uri"] == "file:///main.tua"));
        assert_eq!(server.shader_diagnostics("file:///main.tua").count(), 1);
    }

    #[test]
    fn validator_publications_preserve_language_diagnostics() {
        let mut server = LspServer::default();
        let uri = "file:///main.tua";
        server
            .last_published_diagnostics
            .insert(uri.into(), vec![json!({"code":"TL1003"})]);
        let messages = server.finish_shaders(ShaderResult {
            generation: 0,
            diagnostics: BTreeMap::from([(uri.into(), vec![json!({"code":"TL2016"})])]),
            note: None,
        });
        assert_eq!(
            messages[0]["params"]["diagnostics"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let messages = server.finish_shaders(ShaderResult {
            generation: 0,
            diagnostics: BTreeMap::new(),
            note: None,
        });
        assert_eq!(
            messages[0]["params"]["diagnostics"],
            json!([{"code":"TL1003"}])
        );
    }
}
