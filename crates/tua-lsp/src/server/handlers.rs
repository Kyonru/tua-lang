use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tua_core::{
    AnalysisCacheStats, AssistKind, InlineCompletionBudget, InlineCompletionOptions,
    ModelCompletionRejectionReason, ModelCompletionValidationRequest, PreparedInlineCompletion,
    ProjectModel, RenameEdit, SourcePosition, SourceSpan, DEFAULT_MODEL_CONTEXT_BYTES,
    MAX_MODEL_CONTEXT_BYTES,
};

use super::configuration::WorkspaceSettings;
use super::protocol::{
    call_hierarchy_identity_from_lsp, code_lens_resolve_data, completion_resolve_data,
    core_diagnostic_from_lsp, error_response, lsp_asset_inventory_from_core,
    lsp_call_hierarchy_incoming_call, lsp_call_hierarchy_item, lsp_call_hierarchy_outgoing_call,
    lsp_code_action_from_core, lsp_code_lens_from_core, lsp_completion_from_core,
    lsp_diagnostic_from_core, lsp_document_highlight_from_core, lsp_document_link_from_core,
    lsp_document_symbol_from_core, lsp_ecs_graph_from_core, lsp_folding_range_from_core,
    lsp_full_document_text_edit, lsp_hover_from_core, lsp_inlay_hint_from_core,
    lsp_inline_completion_from_core, lsp_inspector_items_from_core, lsp_location_from_core,
    lsp_module_dependency_graph_from_core, lsp_object_pool_graph_from_core,
    lsp_prepare_rename_from_core, lsp_range, lsp_reference_location, lsp_resolved_completion_item,
    lsp_resolved_reference_code_lens, lsp_selection_range_from_core,
    lsp_semantic_token_data_from_core, lsp_semantic_token_delta, lsp_semantic_tokens,
    lsp_semantic_tokens_from_core, lsp_signal_graph_from_core, lsp_signature_help_from_core,
    lsp_source_range, lsp_text_edit, lsp_type_explanation_from_core, lsp_workspace_edit,
    lsp_workspace_symbol_from_core, publish_diagnostics, response, source_position_from_lsp,
    workspace_diagnostic_refresh_request, SourceRange,
};
use super::request_registry::{NotificationKind, RequestKind};
use super::uri::{
    path_from_uri, uri_from_path, workspace_folders_from_change, workspace_folders_from_initialize,
};
use super::{
    capabilities, LogVerbosity, LspServer, PendingDiagnostics, RequestLatencySample,
    ServerTraceLevel, TraceConfig, TraceLogCounts, SLOW_DIAGNOSTICS_DEBOUNCE,
};

const TRACE_LOG_SUMMARY_THRESHOLD: usize = 8;
const ACTIVE_EDIT_BACKGROUND_REQUEST_COOLDOWN: Duration = Duration::from_millis(1500);
const LOCAL_INLINE_COMPLETION_BUDGET: Duration = Duration::from_millis(10);

struct DocumentRequest<'a> {
    params: &'a Value,
    uri: &'a str,
    path: PathBuf,
}

pub(super) struct RequestTrace {
    method: String,
    completion_context: Option<String>,
    definition_context: Option<String>,
    hover_context: Option<String>,
    background_skip_context: Option<String>,
    started: Option<Instant>,
}

pub(super) struct LocalInlineCompletionWork {
    prepared: Option<PreparedInlineCompletion>,
    position: Option<SourcePosition>,
    lsp_position: Option<Value>,
    options: InlineCompletionOptions,
    selected_completion: Option<Value>,
    document_version: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileRename {
    old_path: PathBuf,
    new_path: PathBuf,
}

impl<'a> DocumentRequest<'a> {
    fn position(&self) -> Option<SourcePosition> {
        self.params
            .get("position")
            .and_then(source_position_from_lsp)
    }
}

impl LocalInlineCompletionWork {
    fn empty(document_version: Option<i64>) -> Self {
        Self {
            prepared: None,
            position: None,
            lsp_position: None,
            options: InlineCompletionOptions::default(),
            selected_completion: None,
            document_version,
        }
    }

    pub(super) fn execute(self, cancelled: &AtomicBool) -> Value {
        let _worker_profile = tua_core::profile::span("lsp.request tua/localInlineCompletion");
        let (Some(prepared), Some(position), Some(lsp_position)) =
            (self.prepared, self.position, self.lsp_position)
        else {
            return empty_local_inline_completion(self.document_version);
        };
        let source = prepared.source();
        let budget = InlineCompletionBudget::new(LOCAL_INLINE_COMPLETION_BUDGET);
        let candidates = {
            let _profile = tua_core::profile::span("inline.request.core");
            prepared.complete(position, &self.options, &budget, cancelled)
        };
        let items = candidates
            .into_iter()
            .filter_map(|mut candidate| {
                let range = if let Some(selected) = self.selected_completion.as_ref() {
                    if candidate.range.start != position || candidate.range.end != position {
                        return None;
                    }
                    let selected_text = selected.get("text")?.as_str()?;
                    let selected_range = selected.get("range")?;
                    let selected_start = selected_range.get("start")?;
                    let selected_end = selected_range.get("end")?;
                    if !selected_completion_range_ends_at_cursor(
                        selected_start,
                        selected_end,
                        &lsp_position,
                    ) {
                        return None;
                    }
                    let start = exact_byte_offset_from_lsp_position(source, selected_start)?;
                    let cursor = exact_byte_offset_from_lsp_position(source, &lsp_position)?;
                    let end = exact_byte_offset_from_lsp_position(source, selected_end)?;
                    if start > cursor || end != cursor {
                        return None;
                    }
                    let replacement =
                        format!("{}{}", &source[start..cursor], candidate.insert_text);
                    if replacement == selected_text || !replacement.starts_with(selected_text) {
                        return None;
                    }
                    candidate.insert_text = replacement;
                    selected_range.clone()
                } else {
                    lsp_range_for_source(source, candidate.range)
                };

                Some(lsp_inline_completion_from_core(candidate, range))
            })
            .collect::<Vec<_>>();

        json!({
            "documentVersion": self.document_version,
            "items": items
        })
    }
}

impl LspServer {
    pub(super) fn handle_message(&mut self, message: Value) -> Option<Vec<Value>> {
        let method = message.get("method").and_then(Value::as_str)?.to_owned();

        if message.get("id").is_some() {
            let (mut outbound, trace) = self.begin_request_trace(&message, &method);
            let response = self.execute_request(message, &method);

            outbound.extend(self.finish_request_trace(trace, &response));
            outbound.push(response);

            return Some(outbound);
        }

        let mut outbound = self.trace_log_messages(&message, &method, false);
        let detail_trace = self.notification_debug_message(&message, &method);
        let notifications = self.handle_notification(&message, &method);

        extend_optional(&mut outbound, detail_trace);
        if let Some(notifications) = notifications {
            outbound.extend(notifications);
        }

        (!outbound.is_empty()).then_some(outbound)
    }

    pub(super) fn begin_request_trace(
        &mut self,
        message: &Value,
        method: &str,
    ) -> (Vec<Value>, RequestTrace) {
        let completion_context = self.completion_debug_context(message, method);
        let definition_context = self.definition_debug_context(message, method);
        let hover_context = self.hover_debug_context(message, method);
        let background_skip_context = self.background_request_skip_debug_context(message, method);
        let started = Some(Instant::now());

        (
            self.trace_log_messages(message, method, true),
            RequestTrace {
                method: method.to_owned(),
                completion_context,
                definition_context,
                hover_context,
                background_skip_context,
                started,
            },
        )
    }

    pub(super) fn finish_request_trace(
        &mut self,
        trace: RequestTrace,
        response: &Value,
    ) -> Vec<Value> {
        let mut outbound = Vec::new();
        if let Some(started) = trace.started {
            self.record_request_latency(&trace.method, started.elapsed());
        }
        let completion_trace = trace.completion_context.and_then(|context| {
            trace.started.map(|started| {
                log_message_notification(
                    4,
                    format!(
                        "tua completion response: {context} items={} {} elapsedMs={}",
                        completion_response_item_count(response),
                        completion_response_label_sample(response),
                        started.elapsed().as_millis()
                    ),
                )
            })
        });
        let definition_trace = trace.definition_context.and_then(|context| {
            trace.started.map(|started| {
                log_message_notification(
                    4,
                    format!(
                        "tua definition response: {context} {} elapsedMs={}",
                        definition_response_target(response),
                        started.elapsed().as_millis()
                    ),
                )
            })
        });
        let hover_trace = trace.hover_context.and_then(|context| {
            trace.started.map(|started| {
                log_message_notification(
                    4,
                    format!(
                        "tua hover response: {context} {} elapsedMs={}",
                        hover_response_summary(response),
                        started.elapsed().as_millis()
                    ),
                )
            })
        });

        extend_optional(&mut outbound, completion_trace);
        extend_optional(&mut outbound, definition_trace);
        extend_optional(&mut outbound, hover_trace);
        extend_optional(
            &mut outbound,
            trace.background_skip_context.map(|context| {
                log_message_notification(
                    4,
                    format!("tua background request fast response during input backlog: {context}"),
                )
            }),
        );

        outbound
    }

    pub(super) fn execute_request(&mut self, message: Value, method: &str) -> Value {
        self.execute_request_with_cancellation(message, method, &AtomicBool::new(false))
    }

    pub(super) fn execute_request_with_cancellation(
        &mut self,
        message: Value,
        method: &str,
        cancelled: &AtomicBool,
    ) -> Value {
        let _profile = tua_core::profile::span(format!("lsp.request {method}"));
        let id = message.get("id").cloned().unwrap_or(Value::Null);

        if let Some(result) = self.background_request_backlog_result(&message, method) {
            return response(id, result);
        }

        let Some(request) = RequestKind::from_method(method) else {
            return error_response(id, -32601, format!("method not found: {method}"));
        };

        match request {
            RequestKind::Initialize => {
                self.trace_config = trace_config_from_initialize(&message);
                self.workspace_folders = workspace_folders_from_initialize(&message);
                self.analysis
                    .reindex_workspace(&self.workspace_folders, &self.settings);
                self.reset_workspace_warm_queue();
                self.reset_pattern_corpus_queue();
                self.client_watched_files_dynamic_registration =
                    client_supports_watched_files_dynamic_registration(&message);
                self.client_pull_diagnostics = client_supports_pull_diagnostics(&message);
                self.client_diagnostic_refresh = client_supports_diagnostic_refresh(&message);
                self.client_inlay_hint_refresh =
                    client_supports_workspace_refresh(&message, "inlayHint");
                self.client_semantic_tokens_refresh =
                    client_supports_workspace_refresh(&message, "semanticTokens");
                self.client_completion_label_details =
                    client_supports_completion_label_details(&message);

                response(
                    id,
                    capabilities::initialize_result(
                        self.client_pull_diagnostics,
                        self.client_completion_label_details,
                    ),
                )
            }
            RequestKind::Shutdown => {
                self.shutdown_requested = true;

                response(id, Value::Null)
            }
            RequestKind::Hover => {
                response(id, self.hover_response(&message).unwrap_or(Value::Null))
            }
            RequestKind::Completion => response(id, self.completion_response(&message)),
            RequestKind::CompletionResolve => {
                response(id, self.completion_resolve_response(message))
            }
            RequestKind::SignatureHelp => response(
                id,
                self.signature_help_response(&message)
                    .unwrap_or(Value::Null),
            ),
            RequestKind::Definition => response(
                id,
                self.definition_response(&message).unwrap_or(Value::Null),
            ),
            RequestKind::TypeDefinition => response(
                id,
                self.type_definition_response(&message)
                    .unwrap_or(Value::Null),
            ),
            RequestKind::Implementation => response(id, self.implementation_response(&message)),
            RequestKind::Diagnostic => response(id, self.document_diagnostic_response(&message)),
            RequestKind::BuiltinDocument => response(
                id,
                self.builtin_document_response(&message)
                    .unwrap_or(Value::Null),
            ),
            RequestKind::ProjectLayout => response(id, self.project_layout_response(&message)),
            RequestKind::CacheStats => response(id, self.cache_stats_response()),
            RequestKind::Health => response(id, self.health_response()),
            RequestKind::ResetCompletionStats => {
                response(id, self.reset_completion_stats_response())
            }
            RequestKind::LocalInlineCompletion => response(
                id,
                self.local_inline_completion_response(&message, cancelled),
            ),
            RequestKind::InlineCompletionContext => {
                response(id, self.inline_completion_context_response(&message))
            }
            RequestKind::ValidateInlineCompletion => {
                response(id, self.validate_inline_completion_response(&message))
            }
            RequestKind::Inspect => response(id, self.inspector_response(&message)),
            RequestKind::ExplainType => response(id, self.type_explanation_response(&message)),
            RequestKind::AssetInventory => response(id, self.asset_inventory_response()),
            RequestKind::ModuleGraph => response(id, self.module_dependency_graph_response()),
            RequestKind::EcsGraph => response(id, self.ecs_graph_response()),
            RequestKind::ObjectPoolGraph => response(id, self.object_pool_graph_response()),
            RequestKind::AudioGraph => response(id, self.audio_graph_response()),
            RequestKind::SignalGraph => response(id, self.signal_graph_response()),
            RequestKind::PrepareCallHierarchy => {
                response(id, self.prepare_call_hierarchy_response(&message))
            }
            RequestKind::IncomingCalls => {
                response(id, self.call_hierarchy_incoming_response(&message))
            }
            RequestKind::OutgoingCalls => {
                response(id, self.call_hierarchy_outgoing_response(&message))
            }
            RequestKind::DocumentLink => response(id, self.document_link_response(&message)),
            RequestKind::DocumentHighlight => {
                response(id, self.document_highlight_response(&message))
            }
            RequestKind::DocumentSymbol => response(id, self.document_symbol_response(&message)),
            RequestKind::SelectionRange => response(id, self.selection_range_response(&message)),
            RequestKind::FoldingRange => response(id, self.folding_range_response(&message)),
            RequestKind::CodeAction => response(id, self.code_action_response(&message)),
            RequestKind::CodeLens => response(id, self.code_lens_response(&message)),
            RequestKind::CodeLensResolve => response(id, self.code_lens_resolve_response(message)),
            RequestKind::Formatting => response(
                id,
                self.formatting_response(&message).unwrap_or(Value::Null),
            ),
            RequestKind::RangeFormatting => response(
                id,
                self.range_formatting_response(&message)
                    .unwrap_or(Value::Null),
            ),
            RequestKind::OnTypeFormatting => response(
                id,
                self.on_type_formatting_response(&message)
                    .unwrap_or(Value::Null),
            ),
            RequestKind::References => response(id, self.references_response(&message)),
            RequestKind::PrepareRename => response(
                id,
                self.prepare_rename_response(&message)
                    .unwrap_or(Value::Null),
            ),
            RequestKind::Rename => {
                response(id, self.rename_response(&message).unwrap_or(Value::Null))
            }
            RequestKind::InlayHint => response(id, self.inlay_hint_response(&message)),
            RequestKind::SemanticTokensFull => {
                response(id, self.semantic_tokens_full_response(&message))
            }
            RequestKind::SemanticTokensDelta => {
                response(id, self.semantic_tokens_delta_response(&message))
            }
            RequestKind::SemanticTokensRange => {
                let range = message
                    .get("params")
                    .and_then(|params| params.get("range"))
                    .and_then(lsp_source_range);

                response(id, self.semantic_tokens_range_response(&message, range))
            }
            RequestKind::WorkspaceSymbol => response(id, self.workspace_symbol_response(&message)),
            RequestKind::WillRenameFiles => response(id, self.will_rename_files_response(&message)),
        }
    }

    fn handle_notification(&mut self, message: &Value, method: &str) -> Option<Vec<Value>> {
        match NotificationKind::from_method(method)? {
            NotificationKind::Initialized => self.initialized_notifications(),
            NotificationKind::Exit => {
                self.should_exit = true;

                None
            }
            NotificationKind::DidOpen => self.did_open_notification(message),
            NotificationKind::DidChange => self.did_change_notification(message),
            NotificationKind::DidSave => self.did_save_notification(message),
            NotificationKind::DidClose => self.did_close_notification(message),
            NotificationKind::DidChangeWatchedFiles => {
                self.did_change_watched_files_notification(message)
            }
            NotificationKind::DidChangeWorkspaceFolders => {
                self.did_change_workspace_folders_notification(message)
            }
            NotificationKind::DidChangeConfiguration => {
                self.did_change_configuration_notification(message)
            }
            NotificationKind::SetTrace => self.set_trace_notification(message),
        }
    }

    fn set_trace_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        self.trace_config = trace_config_from_params(message.get("params"));

        Some(vec![log_message_notification(
            4,
            format!(
                "tua trace updated: server={}, logVerbosity={}",
                self.trace_config.server.as_str(),
                self.trace_config.log_verbosity.as_str()
            ),
        )])
    }

    fn trace_log_messages(&mut self, message: &Value, method: &str, request: bool) -> Vec<Value> {
        if !self.trace_config.enabled() {
            self.trace_log_counts.clear();
            return Vec::new();
        }

        if trace_log_should_count(method, request) {
            self.trace_log_counts.increment(request, method);

            if self.trace_log_counts.total >= TRACE_LOG_SUMMARY_THRESHOLD {
                return self.drain_trace_log_summary();
            }

            return Vec::new();
        }

        let mut messages = self.drain_trace_log_summary();

        messages.push(self.trace_log_message(message, method, request));

        messages
    }

    fn trace_log_message(&self, message: &Value, method: &str, request: bool) -> Value {
        let direction = if request { "request" } else { "notification" };
        let mut parts = vec![format!("tua lsp {direction}: {method}")];

        if self.trace_config.verbose() {
            if let Some(id) = message.get("id") {
                parts.push(format!("id={id}"));
            }
        }

        log_message_notification(4, parts.join(" "))
    }

    fn drain_trace_log_summary(&mut self) -> Vec<Value> {
        if self.trace_log_counts.is_empty() {
            return Vec::new();
        }

        let summary = trace_log_counts_summary(&self.trace_log_counts);

        self.trace_log_counts.clear();

        vec![log_message_notification(4, summary)]
    }

    fn notification_debug_message(&self, message: &Value, method: &str) -> Option<Value> {
        if self.trace_config.log_verbosity < LogVerbosity::Debug {
            return None;
        }

        let params = message.get("params")?;

        match method {
            "textDocument/didOpen" => {
                let document = params.get("textDocument")?;
                let uri = document.get("uri")?.as_str()?;
                let version = document
                    .get("version")
                    .and_then(Value::as_i64)
                    .map_or_else(|| "<none>".to_owned(), |version| version.to_string());
                let language_id = document
                    .get("languageId")
                    .and_then(Value::as_str)
                    .unwrap_or("<unknown>");
                let text_len = document
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::len)
                    .unwrap_or_default();

                Some(log_message_notification(
                    4,
                    format!(
                        "tua document opened: uri={uri} languageId={language_id} version={version} textBytes={text_len}"
                    ),
                ))
            }
            "textDocument/didChange" => {
                let document = params.get("textDocument")?;
                let uri = document.get("uri")?.as_str()?;
                let version = document
                    .get("version")
                    .and_then(Value::as_i64)
                    .map_or_else(|| "<none>".to_owned(), |version| version.to_string());
                let changes = params
                    .get("contentChanges")
                    .and_then(Value::as_array)
                    .map(|changes| {
                        changes
                            .iter()
                            .map(content_change_debug_label)
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();

                Some(log_message_notification(
                    4,
                    format!(
                        "tua document changed: uri={uri} version={version} changes=[{changes}]"
                    ),
                ))
            }
            _ => None,
        }
    }

    fn completion_debug_context(&self, message: &Value, method: &str) -> Option<String> {
        if method != "textDocument/completion"
            || self.trace_config.log_verbosity < LogVerbosity::Debug
        {
            return None;
        }

        let params = message.get("params")?;
        let uri = document_uri(params)?;
        let position = params.get("position").and_then(source_position_from_lsp)?;
        let position_label = format!("{}:{}", position.line, position.column);
        let request_id = message
            .get("id")
            .map(Value::to_string)
            .unwrap_or_else(|| "<none>".to_owned());
        let document_version = self
            .document_versions
            .get(uri)
            .map_or_else(|| "<unknown>".to_owned(), ToString::to_string);
        let source_context = self
            .analysis
            .document_text(uri)
            .map(|source| completion_source_context(source, position))
            .unwrap_or_else(|| "prefix=<unknown> before=<unknown> line=<unknown>".to_owned());
        let trigger_kind = params
            .get("context")
            .and_then(|context| context.get("triggerKind"))
            .and_then(Value::as_u64)
            .map_or_else(|| "<none>".to_owned(), |kind| kind.to_string());
        let trigger_character = params
            .get("context")
            .and_then(|context| context.get("triggerCharacter"))
            .and_then(Value::as_str)
            .map(debug_text_preview)
            .unwrap_or_else(|| "<none>".to_owned());

        Some(format!(
            "id={request_id} uri={uri} docVersion={document_version} position={position_label} {source_context} triggerKind={trigger_kind} triggerCharacter={trigger_character}"
        ))
    }

    fn definition_debug_context(&self, message: &Value, method: &str) -> Option<String> {
        if method != "textDocument/definition"
            || self.trace_config.log_verbosity < LogVerbosity::Debug
        {
            return None;
        }

        let params = message.get("params")?;
        let uri = document_uri(params)?;
        let position = params
            .get("position")
            .and_then(source_position_from_lsp)
            .map(|position| format!("{}:{}", position.line, position.column))
            .unwrap_or_else(|| "<none>".to_owned());
        let request_id = message
            .get("id")
            .map(Value::to_string)
            .unwrap_or_else(|| "<none>".to_owned());

        Some(format!("id={request_id} uri={uri} position={position}"))
    }

    fn hover_debug_context(&self, message: &Value, method: &str) -> Option<String> {
        if method != "textDocument/hover" || self.trace_config.log_verbosity < LogVerbosity::Debug {
            return None;
        }

        let params = message.get("params")?;
        let uri = document_uri(params)?;
        let position = params.get("position").and_then(source_position_from_lsp)?;
        let position_label = format!("{}:{}", position.line, position.column);
        let request_id = message
            .get("id")
            .map(Value::to_string)
            .unwrap_or_else(|| "<none>".to_owned());
        let document_version = self
            .document_versions
            .get(uri)
            .map_or_else(|| "<unknown>".to_owned(), ToString::to_string);
        let source_context = self
            .analysis
            .document_text(uri)
            .map(|source| completion_source_context(source, position))
            .unwrap_or_else(|| "prefix=<unknown> before=<unknown> line=<unknown>".to_owned());

        Some(format!(
            "id={request_id} uri={uri} docVersion={document_version} position={position_label} {source_context}"
        ))
    }

    fn initialized_notifications(&mut self) -> Option<Vec<Value>> {
        if !self.client_watched_files_dynamic_registration {
            return None;
        }

        Some(vec![register_watched_files_request(self.next_request_id())])
    }

    fn did_open_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let text_document = message.get("params")?.get("textDocument")?;
        let uri = text_document.get("uri")?.as_str()?.to_owned();
        let text = text_document.get("text")?.as_str()?.to_owned();
        let version = text_document.get("version").and_then(Value::as_i64);

        self.recent_document_changes.remove(&uri);
        let project_inputs_changed =
            self.analysis
                .open_document(uri.clone(), text, &self.workspace_folders, &self.settings);
        self.queue_workspace_warm_path(
            path_from_uri(&uri),
            tua_core::AnalysisWarmPriority::High,
            true,
        );
        if project_inputs_changed {
            self.reset_pattern_corpus_queue();
        } else {
            self.queue_pattern_corpus_path(path_from_uri(&uri), Instant::now());
        }
        self.clear_semantic_token_results(&uri);
        self.pending_pull_fast_diagnostics.remove(&uri);
        let generation = self.bump_document_generation(&uri, version);
        self.queue_slow_diagnostics(uri.clone(), generation, version, Instant::now());

        if self.client_pull_diagnostics {
            return None;
        }

        let diagnostics = self.fast_diagnostics_for(&uri);

        Some(vec![self.publish_tracked_diagnostics(&uri, diagnostics)])
    }

    fn did_change_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let params = message.get("params")?;
        let text_document = params.get("textDocument")?;
        let uri = text_document.get("uri")?.as_str()?.to_owned();
        let version = text_document.get("version").and_then(Value::as_i64);
        let current = self.analysis.document_text(&uri)?;
        let text = apply_content_changes(current, params.get("contentChanges")?.as_array()?)?;

        self.analysis
            .update_document(uri.clone(), text, &self.workspace_folders, &self.settings);
        self.queue_workspace_warm_path(
            path_from_uri(&uri),
            tua_core::AnalysisWarmPriority::High,
            true,
        );
        let generation = self.bump_document_generation(&uri, version);
        let now = Instant::now();
        self.queue_pattern_corpus_path(path_from_uri(&uri), now);
        self.recent_document_changes.insert(uri.clone(), now);
        self.queue_slow_diagnostics(uri.clone(), generation, version, now);
        self.queue_pull_fast_diagnostics(uri.clone(), generation, version, now);

        if self.client_pull_diagnostics {
            return None;
        }

        let diagnostics = self.fast_diagnostics_for(&uri);

        Some(vec![self.publish_tracked_diagnostics(&uri, diagnostics)])
    }

    fn did_save_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let params = message.get("params")?;
        let uri = document_uri(params)?.to_owned();

        if let Some(text) = params.get("text").and_then(Value::as_str) {
            let version = params
                .get("textDocument")
                .and_then(|document| document.get("version"))
                .and_then(Value::as_i64);

            self.analysis.save_document(
                uri.clone(),
                text.to_owned(),
                &self.workspace_folders,
                &self.settings,
            );
            self.queue_pattern_corpus_path(path_from_uri(&uri), Instant::now());
            self.bump_document_generation(&uri, version);
        }

        self.pending_slow_diagnostics.remove(&uri);
        self.pending_pull_fast_diagnostics.remove(&uri);

        if self.client_pull_diagnostics {
            return None;
        }

        self.analysis.contains_document(&uri).then(|| {
            let diagnostics = self.diagnostics_for(&uri);

            vec![self.publish_tracked_diagnostics(&uri, diagnostics)]
        })
    }

    fn did_close_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let params = message.get("params")?;
        let uri = document_uri(params)?.to_owned();

        let path = path_from_uri(&uri);
        self.analysis.close_document(&uri);
        self.queue_pattern_corpus_path(path, Instant::now());
        self.pending_slow_diagnostics.remove(&uri);
        self.pending_pull_fast_diagnostics.remove(&uri);
        self.last_published_diagnostics.remove(&uri);
        self.document_generations.remove(&uri);
        self.document_versions.remove(&uri);
        self.recent_document_changes.remove(&uri);
        self.clear_semantic_token_results(&uri);

        if self.client_pull_diagnostics {
            return None;
        }

        Some(vec![publish_diagnostics(&uri, Vec::new())])
    }

    fn did_change_watched_files_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let changes = watched_file_changes(message);
        let mut source_changes = Vec::new();
        let mut deleted_sources = Vec::new();
        let mut should_reindex = false;
        let mut asset_changed = false;

        for change in changes {
            if is_config_uri(&change.uri) {
                should_reindex = true;
                continue;
            }

            if !self
                .analysis
                .is_source_uri(&change.uri, &self.workspace_folders, &self.settings)
            {
                if self
                    .analysis
                    .is_asset_uri(&change.uri, &self.workspace_folders, &self.settings)
                {
                    asset_changed = true;
                }
                continue;
            }

            match change.kind {
                FileChangeKind::Created | FileChangeKind::Changed => {
                    self.analysis.cache_disk_file(
                        &change.uri,
                        &self.workspace_folders,
                        &self.settings,
                    );
                    source_changes.push(change.uri);
                }
                FileChangeKind::Deleted => {
                    self.analysis.remove_file(&change.uri);
                    deleted_sources.push(change.uri);
                }
            }
        }

        if should_reindex {
            self.analysis
                .reindex_workspace(&self.workspace_folders, &self.settings);
        }

        let diagnostics_may_have_changed = should_reindex
            || asset_changed
            || !source_changes.is_empty()
            || !deleted_sources.is_empty();

        if diagnostics_may_have_changed {
            self.pending_slow_diagnostics.clear();
            self.pending_pull_fast_diagnostics.clear();
            self.analysis
                .refresh_open_documents(&self.workspace_folders, &self.settings);
            self.reset_workspace_warm_queue();
        }

        if should_reindex {
            self.reset_pattern_corpus_queue();
        } else {
            let now = Instant::now();
            for uri in &source_changes {
                self.queue_pattern_corpus_path(path_from_uri(uri), now);
            }
            for uri in &deleted_sources {
                self.tombstone_pattern_corpus_path(&path_from_uri(uri));
            }
        }

        if self.client_pull_diagnostics {
            return diagnostics_may_have_changed
                .then(|| self.diagnostic_refresh_notifications())
                .flatten();
        }

        let mut publications = Vec::new();
        let mut published = BTreeSet::new();

        for uri in source_changes {
            let diagnostics = if self.analysis.contains_document(&uri) {
                self.diagnostics_for(&uri)
            } else {
                self.diagnostics_for_disk_file(&uri)
            };

            push_diagnostics_publication(&mut publications, &mut published, &uri, diagnostics);
        }

        for uri in deleted_sources {
            push_diagnostics_publication(&mut publications, &mut published, &uri, Vec::new());
        }

        for uri in self.analysis.open_uris() {
            push_diagnostics_publication(
                &mut publications,
                &mut published,
                &uri,
                self.diagnostics_for(&uri),
            );
        }

        (!publications.is_empty()).then_some(publications)
    }

    fn did_change_workspace_folders_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        for removed in workspace_folders_from_change(message, "removed") {
            self.workspace_folders.retain(|folder| folder != &removed);
        }

        for added in workspace_folders_from_change(message, "added") {
            if !self.workspace_folders.iter().any(|folder| folder == &added) {
                self.workspace_folders.push(added);
            }
        }

        self.analysis
            .reindex_workspace(&self.workspace_folders, &self.settings);
        self.reset_workspace_warm_queue();
        self.reset_pattern_corpus_queue();
        self.pending_pull_fast_diagnostics.clear();

        self.diagnostic_refresh_notifications()
    }

    fn did_change_configuration_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        self.settings = WorkspaceSettings::from_did_change_configuration(message);
        self.analysis
            .reindex_workspace(&self.workspace_folders, &self.settings);
        self.analysis
            .refresh_open_documents(&self.workspace_folders, &self.settings);
        self.reset_workspace_warm_queue();
        self.reset_pattern_corpus_queue();
        self.pending_slow_diagnostics.clear();
        self.pending_pull_fast_diagnostics.clear();

        if self.client_pull_diagnostics {
            return self.diagnostic_refresh_notifications();
        }

        Some(self.publish_all_open_diagnostics())
    }

    fn diagnostic_refresh_notifications(&mut self) -> Option<Vec<Value>> {
        (self.client_pull_diagnostics && self.client_diagnostic_refresh)
            .then(|| vec![workspace_diagnostic_refresh_request(self.next_request_id())])
    }

    fn publish_all_open_diagnostics(&self) -> Vec<Value> {
        self.analysis
            .open_uris()
            .into_iter()
            .map(|uri| publish_diagnostics(&uri, self.diagnostics_for(&uri)))
            .collect()
    }

    pub(super) fn diagnostics_for(&self, uri: &str) -> Vec<Value> {
        if !self.analysis.contains_document(uri) {
            return Vec::new();
        }

        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);

        snapshot
            .diagnostics_for_uri(uri)
            .into_iter()
            .map(lsp_diagnostic_from_core)
            .collect()
    }

    fn fast_diagnostics_for(&self, uri: &str) -> Vec<Value> {
        self.analysis
            .fast_diagnostics_for_uri(uri)
            .into_iter()
            .map(lsp_diagnostic_from_core)
            .collect()
    }

    fn queue_slow_diagnostics(
        &mut self,
        uri: String,
        generation: u64,
        version: Option<i64>,
        now: Instant,
    ) {
        if self.client_pull_diagnostics {
            return;
        }

        self.pending_slow_diagnostics.insert(
            uri,
            PendingDiagnostics {
                generation,
                version,
                due_at: now + SLOW_DIAGNOSTICS_DEBOUNCE,
            },
        );
    }

    fn queue_pull_fast_diagnostics(
        &mut self,
        uri: String,
        generation: u64,
        version: Option<i64>,
        now: Instant,
    ) {
        if !self.client_pull_diagnostics || !self.client_diagnostic_refresh {
            return;
        }

        self.pending_pull_fast_diagnostics.insert(
            uri,
            PendingDiagnostics {
                generation,
                version,
                due_at: now + SLOW_DIAGNOSTICS_DEBOUNCE,
            },
        );
    }

    pub(super) fn next_slow_diagnostics_due(&self) -> Option<Instant> {
        self.pending_slow_diagnostics_due()
            .into_iter()
            .chain(self.pending_pull_fast_diagnostics_due())
            .min()
    }

    fn pending_slow_diagnostics_due(&self) -> Option<Instant> {
        self.pending_slow_diagnostics
            .values()
            .map(|pending| pending.due_at)
            .min()
    }

    fn pending_pull_fast_diagnostics_due(&self) -> Option<Instant> {
        if !self.client_pull_diagnostics || !self.client_diagnostic_refresh {
            return None;
        }

        self.pending_pull_fast_diagnostics
            .values()
            .map(|pending| pending.due_at)
            .min()
    }

    #[cfg(test)]
    pub(super) fn drain_due_slow_diagnostics(&mut self, now: Instant) -> Vec<Value> {
        let due = self
            .pending_slow_diagnostics
            .iter()
            .filter(|(_, pending)| pending.due_at <= now)
            .map(|(uri, _)| uri.clone())
            .collect::<Vec<_>>();

        let mut messages = self.drain_slow_diagnostics_for(due);
        if let Some(refresh) = self.drain_due_pull_diagnostic_refresh(now) {
            messages.push(refresh);
        }

        messages
    }

    pub(super) fn take_next_due_slow_diagnostic(
        &mut self,
        now: Instant,
    ) -> Option<(String, PendingDiagnostics)> {
        let uri = self
            .pending_slow_diagnostics
            .iter()
            .filter(|(_, pending)| pending.due_at <= now)
            .min_by_key(|(_, pending)| pending.due_at)
            .map(|(uri, _)| uri.clone())?;
        let pending = self.pending_slow_diagnostics.remove(&uri)?;

        Some((uri, pending))
    }

    pub(super) fn finish_slow_diagnostics(
        &mut self,
        uri: &str,
        pending: PendingDiagnostics,
        diagnostics: Vec<Value>,
    ) -> Option<Value> {
        if self.document_generations.get(uri).copied() != Some(pending.generation) {
            return None;
        }
        if pending.version.is_some() && self.document_versions.get(uri).copied() != pending.version
        {
            return None;
        }
        if !self.analysis.contains_document(uri) {
            return None;
        }

        self.publish_diagnostics_if_changed(uri, diagnostics)
    }

    pub(super) fn drain_all_pending_slow_diagnostics(&mut self) -> Vec<Value> {
        let pending = self
            .pending_slow_diagnostics
            .keys()
            .cloned()
            .collect::<Vec<_>>();

        self.drain_slow_diagnostics_for(pending)
    }

    fn drain_slow_diagnostics_for(&mut self, pending: Vec<String>) -> Vec<Value> {
        let mut publications = Vec::new();

        for uri in pending {
            let Some(job) = self.pending_slow_diagnostics.remove(&uri) else {
                continue;
            };
            if self.document_generations.get(&uri).copied() != Some(job.generation) {
                continue;
            }
            if job.version.is_some() && self.document_versions.get(&uri).copied() != job.version {
                continue;
            }
            if !self.analysis.contains_document(&uri) {
                continue;
            }

            let diagnostics = self.diagnostics_for(&uri);
            if let Some(publication) = self.publish_diagnostics_if_changed(&uri, diagnostics) {
                publications.push(publication);
            }
        }

        publications
    }

    pub(super) fn drain_due_pull_diagnostic_refresh(&mut self, now: Instant) -> Option<Value> {
        if !self.client_pull_diagnostics || !self.client_diagnostic_refresh {
            return None;
        }

        let due = self
            .pending_pull_fast_diagnostics
            .iter()
            .filter(|(_, pending)| pending.due_at <= now)
            .map(|(uri, _)| uri.clone())
            .collect::<Vec<_>>();

        if due.is_empty() {
            return None;
        }

        for uri in due {
            self.pending_pull_fast_diagnostics.remove(&uri);
        }

        Some(workspace_diagnostic_refresh_request(self.next_request_id()))
    }

    fn bump_document_generation(&mut self, uri: &str, version: Option<i64>) -> u64 {
        let generation = self
            .document_generations
            .get(uri)
            .copied()
            .unwrap_or_default()
            + 1;

        self.document_generations.insert(uri.to_owned(), generation);
        if let Some(version) = version {
            self.document_versions.insert(uri.to_owned(), version);
        }

        generation
    }

    fn publish_tracked_diagnostics(&mut self, uri: &str, diagnostics: Vec<Value>) -> Value {
        self.last_published_diagnostics
            .insert(uri.to_owned(), diagnostics.clone());

        publish_diagnostics(uri, diagnostics)
    }

    fn publish_diagnostics_if_changed(
        &mut self,
        uri: &str,
        diagnostics: Vec<Value>,
    ) -> Option<Value> {
        if self
            .last_published_diagnostics
            .get(uri)
            .is_some_and(|previous| previous == &diagnostics)
        {
            return None;
        }

        Some(self.publish_tracked_diagnostics(uri, diagnostics))
    }

    fn diagnostics_for_disk_file(&self, uri: &str) -> Vec<Value> {
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);

        snapshot
            .diagnostics_for_uri(uri)
            .into_iter()
            .map(lsp_diagnostic_from_core)
            .collect()
    }

    fn document_diagnostic_response(&self, message: &Value) -> Value {
        let now = Instant::now();
        let diagnostics = message
            .get("params")
            .and_then(document_uri)
            .map(|uri| self.diagnostics_for_uri_for_pull_request(uri, now))
            .unwrap_or_default();

        super::protocol::document_diagnostic_report(diagnostics)
    }

    fn diagnostics_for_uri_for_pull_request(&self, uri: &str, now: Instant) -> Vec<Value> {
        if self.should_use_pull_fast_diagnostics(uri, now) {
            return self.fast_diagnostics_for(uri);
        }

        self.diagnostics_for_uri(uri)
    }

    fn should_use_pull_fast_diagnostics(&self, uri: &str, now: Instant) -> bool {
        if !self.client_pull_diagnostics || !self.analysis.contains_document(uri) {
            return false;
        }

        let Some(pending) = self.pending_pull_fast_diagnostics.get(uri) else {
            return false;
        };

        if pending.due_at <= now {
            return false;
        }
        if self.document_generations.get(uri).copied() != Some(pending.generation) {
            return false;
        }
        if pending.version.is_some() && self.document_versions.get(uri).copied() != pending.version
        {
            return false;
        }

        true
    }

    fn diagnostics_for_uri(&self, uri: &str) -> Vec<Value> {
        if self.analysis.contains_document(uri) {
            self.diagnostics_for(uri)
        } else {
            self.diagnostics_for_disk_file(uri)
        }
    }

    fn background_request_backlog_result(&self, message: &Value, method: &str) -> Option<Value> {
        let uri = self.active_edit_background_request_uri(message, method)?;

        Some(match method {
            "textDocument/diagnostic" => {
                super::protocol::document_diagnostic_report(self.fast_diagnostics_for(uri))
            }
            "textDocument/semanticTokens/full"
            | "textDocument/semanticTokens/full/delta"
            | "textDocument/semanticTokens/range" => json!({ "data": [] }),
            _ => json!([]),
        })
    }

    fn background_request_skip_debug_context(
        &self,
        message: &Value,
        method: &str,
    ) -> Option<String> {
        if self.trace_config.log_verbosity < LogVerbosity::Debug {
            return None;
        }

        let uri = self.active_edit_background_request_uri(message, method)?;
        let changed_at = self.recent_document_changes.get(uri)?;
        let elapsed = changed_at.elapsed();
        let remaining = ACTIVE_EDIT_BACKGROUND_REQUEST_COOLDOWN.saturating_sub(elapsed);

        Some(format!(
            "method={method} uri={uri} sinceChangeMs={} cooldownRemainingMs={}",
            elapsed.as_millis(),
            remaining.as_millis()
        ))
    }

    fn active_edit_background_request_uri<'a>(
        &self,
        message: &'a Value,
        method: &str,
    ) -> Option<&'a str> {
        if !self.input_backlog_active || !edit_background_request_should_yield(method) {
            return None;
        }

        let uri = message.get("params").and_then(document_uri)?;
        let changed_at = self.recent_document_changes.get(uri)?;

        (changed_at.elapsed() < ACTIVE_EDIT_BACKGROUND_REQUEST_COOLDOWN).then_some(uri)
    }

    fn hover_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        if is_config_uri(request.uri) {
            let source = self.analysis.document_text(request.uri)?;

            return super::config::hover_response(source, position);
        }

        if let Some(hover) = self.analysis.fast_hover_for_uri(request.uri, position) {
            return Some(lsp_hover_from_core(hover));
        }

        let snapshot = {
            let _profile = tua_core::profile::span("lsp.hover.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let hover = {
            let _profile = tua_core::profile::span("lsp.hover.query");

            snapshot.hover(&request.path, position)?
        };

        Some(lsp_hover_from_core(hover))
    }

    fn completion_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let Some(position) = request.position() else {
            return json!([]);
        };
        if is_config_uri(request.uri) {
            let Some(source) = self.analysis.document_text(request.uri) else {
                return json!([]);
            };

            return super::config::completion_response(source, position);
        }

        if let Some(completions) = self
            .analysis
            .fast_field_completion_for_uri(request.uri, position)
        {
            return lsp_completion_array(
                request.uri,
                position,
                completions,
                self.client_completion_label_details,
            );
        }
        if completion_request_is_invoked(message) {
            if let Some((position, completions)) = self
                .analysis
                .fast_identifier_completion_for_uri(request.uri, position)
            {
                return lsp_completion_array(
                    request.uri,
                    position,
                    completions,
                    self.client_completion_label_details,
                );
            }
        }

        let snapshot = {
            let _profile = tua_core::profile::span("lsp.completion.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let mut completions = {
            let _profile = tua_core::profile::span("lsp.completion.query");

            snapshot.completion(&request.path, position)
        };
        let primary_candidate_count = completions.len();

        completions.extend(snapshot.multi_token_completion(
            &request.path,
            position,
            primary_candidate_count,
        ));

        lsp_completion_array(
            request.uri,
            position,
            completions,
            self.client_completion_label_details,
        )
    }

    fn completion_resolve_response(&self, message: Value) -> Value {
        let Some(item) = message.get("params").cloned() else {
            return Value::Null;
        };
        let Some(data) = completion_resolve_data(&item) else {
            return item;
        };

        let path = path_from_uri(&data.uri);
        let snapshot = {
            let _profile = tua_core::profile::span("lsp.completion.resolve.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let completion = {
            let _profile = tua_core::profile::span("lsp.completion.resolve.query");

            snapshot.resolve_completion(
                &path,
                data.position,
                &data.label,
                data.kind,
                data.module_path.as_deref(),
                data.source_module.as_deref(),
            )
        };

        lsp_resolved_completion_item(item, completion)
    }

    fn signature_help_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let snapshot = {
            let _profile = tua_core::profile::span("lsp.signature_help.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let help = {
            let _profile = tua_core::profile::span("lsp.signature_help.query");

            snapshot.signature_help(&request.path, position)?
        };

        Some(lsp_signature_help_from_core(help))
    }

    fn definition_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let snapshot = {
            let _profile = tua_core::profile::span("lsp.definition.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let definition = {
            let _profile = tua_core::profile::span("lsp.definition.query");

            snapshot.definition(&request.path, position)?
        };

        Some(lsp_location_from_core(definition))
    }

    fn type_definition_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let snapshot = {
            let _profile = tua_core::profile::span("lsp.type_definition.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let definition = {
            let _profile = tua_core::profile::span("lsp.type_definition.query");

            snapshot.type_definition(&request.path, position)?
        };

        Some(lsp_location_from_core(definition))
    }

    fn implementation_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let Some(position) = request.position() else {
            return json!([]);
        };
        let snapshot = {
            let _profile = tua_core::profile::span("lsp.implementation.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let definitions = {
            let _profile = tua_core::profile::span("lsp.implementation.query");

            snapshot.implementation(&request.path, position)
        };

        Value::Array(
            definitions
                .into_iter()
                .map(lsp_location_from_core)
                .collect(),
        )
    }

    fn builtin_document_response(&self, message: &Value) -> Option<Value> {
        let uri = message.get("params")?.get("uri")?.as_str()?;
        let source = tua_core::builtin_definition_document(uri)?;

        Some(Value::String(source))
    }

    fn project_layout_response(&self, message: &Value) -> Value {
        let Some(uri) = message
            .get("params")
            .and_then(|params| params.get("uri"))
            .and_then(Value::as_str)
        else {
            return Value::Null;
        };
        let path = path_from_uri(uri);
        let Some(workspace_root) = self
            .workspace_folders
            .iter()
            .filter(|root| path.starts_with(root))
            .max_by_key(|root| root.components().count())
        else {
            return Value::Null;
        };
        let Ok(model) = ProjectModel::load_for_path(&path) else {
            return Value::Null;
        };
        let Some(project) = model.primary_workspace() else {
            return Value::Null;
        };
        let generated_path = project
            .lua_output_paths_for_tua_source(&path)
            .and_then(|paths| {
                if path == project.configured_main_source_path() {
                    paths.last().cloned()
                } else {
                    paths.first().cloned()
                }
            });

        json!({
            "workspaceUri": uri_from_path(workspace_root),
            "configUri": project.config_found().then(|| uri_from_path(project.config_path())),
            "projectUri": uri_from_path(project.root()),
            "sourceUri": uri_from_path(project.primary_source_root()),
            "outputUri": uri_from_path(project.primary_output_root()),
            "mainSource": project.configured_main().to_string_lossy(),
            "generatedUri": generated_path.as_deref().map(uri_from_path),
            "moduleName": project.module_name_for_path(&path),
        })
    }

    fn cache_stats_response(&self) -> Value {
        let stats = self.analysis.cache_stats();
        let corpus = self.analysis.project_pattern_corpus_stats();
        let corpus_disposals = self
            .pattern_corpus
            .disposals
            .lock()
            .expect("pattern corpus disposal queue lock poisoned")
            .len();

        json!({
            "parse": {
                "hits": stats.parse_hits,
                "misses": stats.parse_misses
            },
            "hir": {
                "hits": stats.hir_hits,
                "misses": stats.hir_misses
            },
            "scope": {
                "hits": stats.scope_hits,
                "misses": stats.scope_misses
            },
            "typeEnvironment": {
                "hits": stats.type_environment_hits,
                "misses": stats.type_environment_misses
            },
            "diagnostics": {
                "hits": stats.diagnostics_hits,
                "misses": stats.diagnostics_misses
            },
            "moduleExports": {
                "hits": stats.module_export_hits,
                "misses": stats.module_export_misses
            },
            "requireEdges": {
                "hits": stats.require_edge_hits,
                "misses": stats.require_edge_misses
            },
            "symbolIndex": {
                "hits": stats.symbol_index_hits,
                "misses": stats.symbol_index_misses
            },
            "usageIndex": {
                "hits": stats.usage_index_hits,
                "misses": stats.usage_index_misses
            },
            "completionContext": {
                "hits": stats.completion_context_hits,
                "misses": stats.completion_context_misses
            },
            "inlineCompletion": {
                "requests": stats.inline_completion_requests,
                "skippedOversized": stats.inline_skipped_oversized,
                "cachePeek": hit_miss(stats.inline_cache_peek_hits, stats.inline_cache_peek_misses),
                "candidateAttempts": stats.inline_validation_attempts,
                "candidateParses": stats.inline_candidate_parses,
                "baselineParses": stats.inline_baseline_parses,
                "candidateParseDistribution": inline_parse_distribution(&stats),
                "returnedParseDistribution": inline_returned_parse_distribution(&stats),
                "editedParseSufficient": stats.inline_edited_parse_sufficient,
                "candidatesReturned": stats.inline_candidates_returned,
                "parsesPerRequest": ratio(
                    stats.inline_candidate_parses + stats.inline_baseline_parses,
                    stats.inline_completion_requests
                ),
                "editedParseSufficiencyRate": ratio(
                    stats.inline_edited_parse_sufficient,
                    stats.inline_candidate_parses
                )
            },
            "workspaceWarm": {
                "discovered": self.workspace_warm.files_discovered,
                "warmed": self.workspace_warm.warmed.len(),
                "failed": self.workspace_warm.failed.len(),
                "queuedHigh": self.workspace_warm.high.len(),
                "queuedLow": self.workspace_warm.low.len(),
                "staleResults": self.workspace_warm.stale_results,
                "snapshotReadyRevision": self.workspace_warm.snapshot_ready_revision,
                "snapshotFailedRevision": self.workspace_warm.snapshot_failed_revision
            },
            "patternCorpus": {
                "enabled": self.pattern_corpus.enabled,
                "generation": self.pattern_corpus.generation,
                "discovered": self.pattern_corpus.files_discovered,
                "queued": self.pattern_corpus.pending.len(),
                "inFlight": self.pattern_corpus.in_flight.len()
                    + usize::from(self.pattern_corpus.remerge_in_flight),
                "batchesCompleted": self.pattern_corpus.batches_completed,
                "filesBuilt": self.pattern_corpus.files_built,
                "adopted": self.pattern_corpus.adopted,
                "tombstoned": self.pattern_corpus.tombstoned,
                "stale": self.pattern_corpus.stale,
                "skipped": self.pattern_corpus.skipped,
                "overBudget": self.pattern_corpus.over_budget,
                "fileLimit": self.pattern_corpus.file_limit,
                "generationRejections": self.pattern_corpus.generation_rejections,
                "lineageRejections": self.pattern_corpus.lineage_rejections,
                "cancelledBatches": self.pattern_corpus.cancelled_batches,
                "remerges": self.pattern_corpus.remerges,
                "remergePending": self.pattern_corpus.remerge_pending,
                "remergeInFlight": self.pattern_corpus.remerge_in_flight,
                "remergeRejections": self.pattern_corpus.remerge_rejections,
                "disposalsQueued": corpus_disposals,
                "lastBuildMs": self.pattern_corpus.last_build_ms,
                "files": corpus.files,
                "baseFiles": corpus.base_files,
                "overlayFiles": corpus.overlay_files,
                "tombstones": corpus.tombstones,
                "approximateBytes": corpus.approximate_bytes
            },
            "invalidations": stats.invalidations,
            "dependentInvalidations": stats.dependent_invalidations
        })
    }

    fn health_response(&self) -> Value {
        let stats = self.analysis.cache_stats();
        let cache_hits = cache_hits(&stats);
        let cache_misses = cache_misses(&stats);
        let warm_discovered = self.workspace_warm.files_discovered;
        let warm_warmed = self.workspace_warm.warmed.len();
        let warm_failed = self.workspace_warm.failed.len();
        let warm_queued_high = self.workspace_warm.high.len();
        let warm_queued_low = self.workspace_warm.low.len();
        let warm_queued = warm_queued_high + warm_queued_low;
        let warm_coverage = percentage(warm_warmed, warm_discovered);
        let corpus = self.analysis.project_pattern_corpus_stats();
        let corpus_queued = self.pattern_corpus.pending.len();
        let corpus_in_flight = self.pattern_corpus.in_flight.len()
            + usize::from(self.pattern_corpus.remerge_in_flight);
        let corpus_disposals = self
            .pattern_corpus
            .disposals
            .lock()
            .expect("pattern corpus disposal queue lock poisoned")
            .len();
        let latency = latency_summary(&self.request_health.recent_latencies);
        let queue_total = self.scheduler_health.coordinator_queue_depth
            + self.scheduler_health.in_flight_jobs
            + warm_queued
            + corpus_queued
            + corpus_disposals;

        json!({
            "revision": self.analysis.revision(),
            "documents": {
                "open": self.document_versions.len(),
                "recentChanges": self.recent_document_changes.len()
            },
            "workspaceWarm": {
                "enabled": self.workspace_warm.enabled,
                "discovered": warm_discovered,
                "warmed": warm_warmed,
                "failed": warm_failed,
                "queuedHigh": warm_queued_high,
                "queuedLow": warm_queued_low,
                "queued": warm_queued,
                "coveragePercent": warm_coverage,
                "snapshotReadyRevision": self.workspace_warm.snapshot_ready_revision,
                "snapshotFailedRevision": self.workspace_warm.snapshot_failed_revision,
                "staleResults": self.workspace_warm.stale_results
            },
            "queue": {
                "coordinator": self.scheduler_health.coordinator_queue_depth,
                "inFlight": self.scheduler_health.in_flight_jobs,
                "workspaceWarmHigh": warm_queued_high,
                "workspaceWarmLow": warm_queued_low,
                "patternCorpus": corpus_queued,
                "total": queue_total
            },
            "patternCorpus": {
                "enabled": self.pattern_corpus.enabled,
                "generation": self.pattern_corpus.generation,
                "queued": corpus_queued,
                "inFlight": corpus_in_flight,
                "files": corpus.files,
                "baseFiles": corpus.base_files,
                "overlayFiles": corpus.overlay_files,
                "tombstones": corpus.tombstones,
                "approximateBytes": corpus.approximate_bytes,
                "stale": self.pattern_corpus.stale,
                "generationRejections": self.pattern_corpus.generation_rejections,
                "lineageRejections": self.pattern_corpus.lineage_rejections,
                "cancelledBatches": self.pattern_corpus.cancelled_batches,
                "remergePending": self.pattern_corpus.remerge_pending,
                "remergeInFlight": self.pattern_corpus.remerge_in_flight,
                "remergeRejections": self.pattern_corpus.remerge_rejections,
                "disposalsQueued": corpus_disposals,
                "lastBuildMs": self.pattern_corpus.last_build_ms
            },
            "cache": {
                "hits": cache_hits,
                "misses": cache_misses,
                "hitRate": percentage(cache_hits, cache_hits + cache_misses),
                "parse": hit_miss(stats.parse_hits, stats.parse_misses),
                "hir": hit_miss(stats.hir_hits, stats.hir_misses),
                "scope": hit_miss(stats.scope_hits, stats.scope_misses),
                "typeEnvironment": hit_miss(
                    stats.type_environment_hits,
                    stats.type_environment_misses
                ),
                "diagnostics": hit_miss(stats.diagnostics_hits, stats.diagnostics_misses),
                "moduleExports": hit_miss(stats.module_export_hits, stats.module_export_misses),
                "requireEdges": hit_miss(stats.require_edge_hits, stats.require_edge_misses),
                "symbolIndex": hit_miss(stats.symbol_index_hits, stats.symbol_index_misses),
                "usageIndex": hit_miss(stats.usage_index_hits, stats.usage_index_misses),
                "completionContext": hit_miss(
                    stats.completion_context_hits,
                    stats.completion_context_misses
                ),
                "inlineCurrentFile": hit_miss(
                    stats.inline_cache_peek_hits,
                    stats.inline_cache_peek_misses
                )
            },
            "inlineCompletion": {
                "requests": stats.inline_completion_requests,
                "skippedOversized": stats.inline_skipped_oversized,
                "candidateAttempts": stats.inline_validation_attempts,
                "candidateParses": stats.inline_candidate_parses,
                "baselineParses": stats.inline_baseline_parses,
                "candidateParseDistribution": inline_parse_distribution(&stats),
                "returnedParseDistribution": inline_returned_parse_distribution(&stats),
                "editedParseSufficient": stats.inline_edited_parse_sufficient,
                "candidatesReturned": stats.inline_candidates_returned,
                "parsesPerRequest": ratio(
                    stats.inline_candidate_parses + stats.inline_baseline_parses,
                    stats.inline_completion_requests
                ),
                "editedParseSufficiencyRate": ratio(
                    stats.inline_edited_parse_sufficient,
                    stats.inline_candidate_parses
                )
            },
            "staleResults": {
                "requests": self.request_health.stale_results,
                "workspaceWarm": self.workspace_warm.stale_results,
                "patternCorpus": self.pattern_corpus.stale + self.pattern_corpus.generation_rejections,
                "total": self.request_health.stale_results
                    + self.workspace_warm.stale_results
                    + self.pattern_corpus.stale
                    + self.pattern_corpus.generation_rejections
            },
            "latency": latency
        })
    }

    fn reset_completion_stats_response(&mut self) -> Value {
        let removed_files = self
            .analysis
            .reset_completion_adaptive_stats(&self.workspace_folders, &self.settings);

        json!({
            "removedFiles": removed_files
        })
    }

    fn signal_graph_response(&self) -> Value {
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);

        lsp_signal_graph_from_core(snapshot.signal_graph())
    }

    fn asset_inventory_response(&self) -> Value {
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);

        json!({
            "analysisRevision": snapshot.revision(),
            "inventory": lsp_asset_inventory_from_core(snapshot.asset_inventory())
        })
    }

    fn module_dependency_graph_response(&self) -> Value {
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);

        json!({
            "analysisRevision": snapshot.revision(),
            "graph": lsp_module_dependency_graph_from_core(snapshot.module_dependency_graph())
        })
    }

    fn ecs_graph_response(&self) -> Value {
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);

        json!({
            "analysisRevision": snapshot.revision(),
            "graph": lsp_ecs_graph_from_core(snapshot.ecs_graph())
        })
    }

    fn object_pool_graph_response(&self) -> Value {
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);

        json!({
            "analysisRevision": snapshot.revision(),
            "graph": lsp_object_pool_graph_from_core(snapshot.object_pool_graph())
        })
    }

    fn audio_graph_response(&self) -> Value {
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);
        json!({
            "analysisRevision": snapshot.revision(),
            "graph": crate::server::protocol::lsp_audio_graph_from_core(snapshot.audio_graph())
        })
    }

    fn inspector_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!({
                "analysisRevision": Value::Null,
                "documentVersion": Value::Null,
                "items": []
            });
        };
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);

        json!({
            "analysisRevision": snapshot.revision(),
            "documentVersion": self.document_versions.get(request.uri).copied(),
            "items": lsp_inspector_items_from_core(
                snapshot.inspect(&request.path, request.position())
            )
        })
    }

    fn type_explanation_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!({
                "analysisRevision": Value::Null,
                "documentVersion": Value::Null,
                "explanation": Value::Null
            });
        };
        let Some(position) = request.position() else {
            return json!({
                "analysisRevision": Value::Null,
                "documentVersion": self.document_versions.get(request.uri).copied(),
                "explanation": Value::Null
            });
        };
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);

        json!({
            "analysisRevision": snapshot.revision(),
            "documentVersion": self.document_versions.get(request.uri).copied(),
            "explanation": snapshot
                .explain_type(&request.path, position)
                .map(lsp_type_explanation_from_core)
        })
    }

    fn inline_completion_context_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return Value::Null;
        };
        let Some(position) = request.params.get("position").and_then(|lsp_position| {
            self.analysis
                .document_text(request.uri)
                .and_then(|source| model_completion_position(source, lsp_position))
        }) else {
            return Value::Null;
        };
        let current_version = self.document_versions.get(request.uri).copied();
        if requested_document_version(request.params)
            .is_some_and(|version| current_version.is_some_and(|current| current != version))
        {
            return Value::Null;
        }

        let budget = request
            .params
            .get("contextBytes")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(DEFAULT_MODEL_CONTEXT_BYTES)
            .min(MAX_MODEL_CONTEXT_BYTES);
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);
        let Some(context) = snapshot.model_completion_context(&request.path, position, budget)
        else {
            return Value::Null;
        };

        json!({
            "contextId": context.context_id,
            "analysisRevision": context.analysis_revision,
            "contextBytes": context.context_bytes,
            "documentVersion": current_version,
            "sourceFingerprint": context.source_fingerprint,
            "position": {
                "line": context.position.line.saturating_sub(1),
                "character": context.position.column.saturating_sub(1)
            },
            "prompt": context.prompt,
            "suffix": context.suffix,
            "completionScope": context.completion_scope.as_str(),
            "maxCompletionBytes": context.max_completion_bytes,
            "maxCompletionLines": context.max_completion_lines,
            "range": inline_request_range(request.params)
        })
    }

    pub(super) fn prepare_local_inline_completion(
        &self,
        message: &Value,
    ) -> LocalInlineCompletionWork {
        let Some(request) = self.document_request(message) else {
            return LocalInlineCompletionWork::empty(None);
        };
        let current_version = self.document_versions.get(request.uri).copied();
        if !self.settings.inline_completion_enabled()
            || requested_document_version(request.params) != current_version
        {
            return LocalInlineCompletionWork::empty(current_version);
        }
        let include_project = self.settings.inline_project_enabled();
        let Some(prepared) = self
            .analysis
            .prepare_local_inline_completion(&request.path, include_project)
        else {
            return LocalInlineCompletionWork::empty(current_version);
        };
        let Some(position) = request
            .params
            .get("position")
            .and_then(|position| model_completion_position(prepared.source(), position))
        else {
            return LocalInlineCompletionWork::empty(current_version);
        };
        let options = InlineCompletionOptions {
            semantic: self.settings.inline_semantic_enabled(),
            current_file: self.settings.inline_current_file_enabled(),
            project: include_project,
        };

        LocalInlineCompletionWork {
            prepared: Some(prepared),
            position: Some(position),
            lsp_position: request.params.get("position").cloned(),
            options,
            selected_completion: request.params.get("selectedCompletion").cloned(),
            document_version: current_version,
        }
    }

    pub(super) fn local_inline_completion_is_enabled(&self) -> bool {
        self.settings.inline_completion_enabled()
    }

    fn local_inline_completion_response(&self, message: &Value, cancelled: &AtomicBool) -> Value {
        self.prepare_local_inline_completion(message)
            .execute(cancelled)
    }

    fn validate_inline_completion_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return rejected_inline_completion(ModelCompletionRejectionReason::Stale, None);
        };
        let Some(position) = request.params.get("position").and_then(|lsp_position| {
            self.analysis
                .document_text(request.uri)
                .and_then(|source| model_completion_position(source, lsp_position))
        }) else {
            return rejected_inline_completion(ModelCompletionRejectionReason::Stale, None);
        };
        let current_version = self.document_versions.get(request.uri).copied();
        if requested_document_version(request.params) != current_version {
            return rejected_inline_completion(
                ModelCompletionRejectionReason::Stale,
                Some(SourceSpan {
                    start: position,
                    end: position,
                }),
            );
        }
        let Some(context_id) = request.params.get("contextId").and_then(Value::as_str) else {
            return rejected_inline_completion(ModelCompletionRejectionReason::Stale, None);
        };
        let Some(analysis_revision) = request
            .params
            .get("analysisRevision")
            .and_then(Value::as_u64)
        else {
            return rejected_inline_completion(ModelCompletionRejectionReason::Stale, None);
        };
        let Some(context_bytes) = request.params.get("contextBytes").and_then(Value::as_u64) else {
            return rejected_inline_completion(ModelCompletionRejectionReason::Stale, None);
        };
        let Some(source_fingerprint) = request
            .params
            .get("sourceFingerprint")
            .and_then(Value::as_str)
        else {
            return rejected_inline_completion(ModelCompletionRejectionReason::Stale, None);
        };
        let Some(candidate) = request.params.get("candidate").and_then(Value::as_str) else {
            return rejected_inline_completion(ModelCompletionRejectionReason::Empty, None);
        };

        let validation = self.analysis.validate_model_completion(
            &request.path,
            ModelCompletionValidationRequest {
                context_id,
                analysis_revision,
                context_bytes: (context_bytes as usize).min(MAX_MODEL_CONTEXT_BYTES),
                source_fingerprint,
                position,
                candidate,
            },
        );
        json!({
            "status": validation.status.as_str(),
            "reason": validation.reason.map(|reason| reason.as_str()),
            "insertText": validation.normalized_insertion,
            "range": inline_request_range(request.params),
            "documentVersion": current_version
        })
    }

    fn prepare_call_hierarchy_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let Some(position) = request.position() else {
            return json!([]);
        };
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);
        let items = snapshot.prepare_call_hierarchy(&request.path, position);

        Value::Array(items.into_iter().map(lsp_call_hierarchy_item).collect())
    }

    fn call_hierarchy_incoming_response(&self, message: &Value) -> Value {
        let Some(item) = message.get("params").and_then(|params| params.get("item")) else {
            return json!([]);
        };
        let Some(identity) = call_hierarchy_identity_from_lsp(item) else {
            return json!([]);
        };
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);
        let calls = snapshot.incoming_calls(&identity);

        Value::Array(
            calls
                .into_iter()
                .map(lsp_call_hierarchy_incoming_call)
                .collect(),
        )
    }

    fn call_hierarchy_outgoing_response(&self, message: &Value) -> Value {
        let Some(item) = message.get("params").and_then(|params| params.get("item")) else {
            return json!([]);
        };
        let Some(identity) = call_hierarchy_identity_from_lsp(item) else {
            return json!([]);
        };
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);
        let calls = snapshot.outgoing_calls(&identity);

        Value::Array(
            calls
                .into_iter()
                .map(lsp_call_hierarchy_outgoing_call)
                .collect(),
        )
    }

    fn document_link_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let links = self.analysis.document_links(&request.path);

        Value::Array(links.into_iter().map(lsp_document_link_from_core).collect())
    }

    fn document_highlight_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let Some(position) = request.position() else {
            return json!([]);
        };
        let highlights = self.analysis.document_highlights(&request.path, position);

        Value::Array(
            highlights
                .into_iter()
                .map(lsp_document_highlight_from_core)
                .collect(),
        )
    }

    fn document_symbol_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let symbols = self.analysis.document_symbols(&request.path);

        Value::Array(
            symbols
                .into_iter()
                .map(lsp_document_symbol_from_core)
                .collect(),
        )
    }

    fn selection_range_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let positions = request
            .params
            .get("positions")
            .and_then(Value::as_array)
            .map(|positions| {
                positions
                    .iter()
                    .filter_map(source_position_from_lsp)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let ranges = self.analysis.selection_ranges(&request.path, &positions);

        Value::Array(
            ranges
                .into_iter()
                .flatten()
                .map(lsp_selection_range_from_core)
                .collect(),
        )
    }

    fn folding_range_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let ranges = self.analysis.folding_ranges(&request.path);

        Value::Array(
            ranges
                .into_iter()
                .map(lsp_folding_range_from_core)
                .collect(),
        )
    }

    fn code_action_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let Some(range) = request.params.get("range").and_then(lsp_source_range) else {
            return json!([]);
        };
        let diagnostics = request
            .params
            .get("context")
            .and_then(|context| context.get("diagnostics"))
            .and_then(Value::as_array)
            .map(|diagnostics| {
                diagnostics
                    .iter()
                    .filter_map(|diagnostic| core_diagnostic_from_lsp(&request.path, diagnostic))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let requested_kinds = code_action_kinds_from_request(message);
        if is_config_uri(request.uri) {
            return super::config::code_action_response(
                &request.path,
                diagnostics,
                &requested_kinds,
            );
        }

        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);
        let diagnostics = if requested_kinds.contains(&AssistKind::SourceFixAll) {
            snapshot.diagnostics_for_uri(request.uri)
        } else {
            diagnostics
        };
        let actions = snapshot.code_actions(
            &request.path,
            tua_core::CodeActionRange {
                start: range.start,
                end: range.end,
            },
            &diagnostics,
            &requested_kinds,
        );

        Value::Array(actions.into_iter().map(lsp_code_action_from_core).collect())
    }

    fn formatting_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let source = self.analysis.document_text(request.uri)?;
        let options = self.format_options_for_path(&request.path);
        let formatted = tua_core::format_tua_source(&request.path, source, &options).ok()?;

        if formatted == source {
            Some(json!([]))
        } else {
            Some(json!([lsp_full_document_text_edit(source, formatted)]))
        }
    }

    fn range_formatting_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let range = request.params.get("range").and_then(lsp_source_range)?;
        let source = self.analysis.document_text(request.uri)?;
        let options = self.format_options_for_path(&request.path);
        let edits = tua_core::format_tua_source_range(
            &request.path,
            source,
            &options,
            SourceSpan {
                start: range.start,
                end: range.end,
            },
        )
        .ok()?;

        Some(Value::Array(edits.into_iter().map(lsp_text_edit).collect()))
    }

    fn on_type_formatting_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let trigger = request.params.get("ch").and_then(Value::as_str)?;
        let source = self.analysis.document_text(request.uri)?;
        let options = self.format_options_for_path(&request.path);
        let edits =
            tua_core::format_tua_source_on_type(&request.path, source, &options, position, trigger)
                .ok()?;

        Some(Value::Array(edits.into_iter().map(lsp_text_edit).collect()))
    }

    fn references_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let Some(position) = request.position() else {
            return json!([]);
        };
        let include_declaration = request
            .params
            .get("context")
            .and_then(|context| context.get("includeDeclaration"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let snapshot = {
            let _profile = tua_core::profile::span("lsp.references.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let references = {
            let _profile = tua_core::profile::span("lsp.references.query");

            snapshot.references(&request.path, position, include_declaration)
        };

        Value::Array(references.into_iter().map(lsp_reference_location).collect())
    }

    fn code_lens_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let snapshot = {
            let _profile = tua_core::profile::span("lsp.code_lens.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let lenses = {
            let _profile = tua_core::profile::span("lsp.code_lens.query");

            snapshot.code_lenses(&request.path)
        };

        Value::Array(lenses.into_iter().map(lsp_code_lens_from_core).collect())
    }

    fn code_lens_resolve_response(&self, message: Value) -> Value {
        let Some(lens) = message.get("params").cloned() else {
            return Value::Null;
        };
        let Some(data) = code_lens_resolve_data(&lens) else {
            return lens;
        };
        let snapshot = {
            let _profile = tua_core::profile::span("lsp.code_lens.resolve.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let references = {
            let _profile = tua_core::profile::span("lsp.code_lens.resolve.references");

            snapshot.references(&data.target.path, data.target.position, false)
        };

        lsp_resolved_reference_code_lens(lens, &data.target, references)
    }

    fn prepare_rename_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let snapshot = {
            let _profile = tua_core::profile::span("lsp.prepare_rename.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let target = {
            let _profile = tua_core::profile::span("lsp.prepare_rename.query");

            snapshot.prepare_rename(&request.path, position)?
        };

        Some(lsp_prepare_rename_from_core(target))
    }

    fn rename_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let new_name = request.params.get("newName")?.as_str()?;
        let snapshot = {
            let _profile = tua_core::profile::span("lsp.rename.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let edit = {
            let _profile = tua_core::profile::span("lsp.rename.query");

            snapshot.rename(&request.path, position, new_name)?
        };

        Some(lsp_workspace_edit(edit))
    }

    fn inlay_hint_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let range = request.params.get("range").and_then(lsp_source_range);
        let core_range = range.map(|range| SourceSpan {
            start: range.start,
            end: range.end,
        });
        let cached_snapshot = self
            .analysis
            .cached_snapshot(&self.workspace_folders, &self.settings);
        let hints = if let Some(snapshot) = cached_snapshot {
            let _profile = tua_core::profile::span("lsp.inlay_hints.cached_snapshot");

            core_range
                .map(|range| snapshot.inlay_hints_in_range(&request.path, range))
                .unwrap_or_else(|| snapshot.inlay_hints(&request.path))
        } else if self
            .recent_document_changes
            .get(request.uri)
            .is_some_and(|changed_at| {
                changed_at.elapsed() < ACTIVE_EDIT_BACKGROUND_REQUEST_COOLDOWN
            })
        {
            let _profile = tua_core::profile::span("lsp.inlay_hints.current_file");

            self.analysis
                .fast_inlay_hints_for_uri(request.uri, core_range)
                .unwrap_or_default()
        } else {
            let _profile = tua_core::profile::span("lsp.inlay_hints.snapshot");
            let snapshot = self
                .analysis
                .snapshot(&self.workspace_folders, &self.settings);

            core_range
                .map(|range| snapshot.inlay_hints_in_range(&request.path, range))
                .unwrap_or_else(|| snapshot.inlay_hints(&request.path))
        };

        Value::Array(
            hints
                .into_iter()
                .filter(|hint| {
                    range
                        .map(|range| range.contains(hint.position))
                        .unwrap_or(true)
                })
                .map(lsp_inlay_hint_from_core)
                .collect(),
        )
    }

    fn semantic_tokens_full_response(&mut self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!({ "data": [] });
        };
        let uri = request.uri.to_owned();
        let path = request.path.clone();

        if !self.analysis.contains_document(&uri) {
            return json!({ "data": [] });
        }

        let data = self.semantic_token_data(&path, None);
        let result_id = self.store_semantic_token_result(&uri, &data);

        lsp_semantic_tokens(data, Some(&result_id))
    }

    fn semantic_tokens_delta_response(&mut self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!({ "data": [] });
        };
        let uri = request.uri.to_owned();
        let path = request.path.clone();

        if !self.analysis.contains_document(&uri) {
            return json!({ "data": [] });
        }

        let previous_result_id = request
            .params
            .get("previousResultId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let previous_data = previous_result_id.as_ref().and_then(|result_id| {
            self.semantic_tokens
                .lock()
                .expect("semantic tokens lock poisoned")
                .results
                .get(result_id)
                .filter(|entry| entry.uri == uri)
                .map(|entry| entry.data.clone())
        });
        let data = self.semantic_token_data(&path, None);
        let result_id = self.store_semantic_token_result(&uri, &data);

        match previous_data {
            Some(previous_data) => lsp_semantic_token_delta(&previous_data, data, &result_id),
            None => lsp_semantic_tokens(data, Some(&result_id)),
        }
    }

    fn semantic_tokens_range_response(&self, message: &Value, range: Option<SourceRange>) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!({ "data": [] });
        };

        lsp_semantic_tokens_from_core(self.semantic_tokens_for_range(&request.path, range))
    }

    fn semantic_token_data(&self, path: &Path, range: Option<SourceRange>) -> Vec<usize> {
        lsp_semantic_token_data_from_core(self.semantic_tokens_for_range(path, range))
    }

    fn semantic_tokens_for_range(
        &self,
        path: &Path,
        range: Option<SourceRange>,
    ) -> Vec<tua_core::SemanticToken> {
        let mut tokens = self.analysis.semantic_tokens(path);

        if let Some(range) = range {
            tokens.retain(|token| source_span_intersects_range(token.span, range));
        }

        tokens
    }

    fn store_semantic_token_result(&self, uri: &str, data: &[usize]) -> String {
        let mut semantic_tokens = self
            .semantic_tokens
            .lock()
            .expect("semantic tokens lock poisoned");
        semantic_tokens.next_result_id = semantic_tokens.next_result_id.saturating_add(1);
        let sequence = semantic_tokens.next_result_id;
        let document_version = self.document_versions.get(uri).copied();
        let analysis_revision = self.analysis.revision();
        let version = document_version
            .map(|version| version.to_string())
            .unwrap_or_else(|| "none".to_owned());
        let result_id = format!("{analysis_revision}:{version}:{sequence}");

        semantic_tokens.results.insert(
            result_id.clone(),
            super::SemanticTokensCacheEntry {
                uri: uri.to_owned(),
                data: data.to_vec(),
                document_version,
                analysis_revision,
                sequence,
            },
        );
        Self::prune_semantic_token_results(&mut semantic_tokens, uri);

        result_id
    }

    fn prune_semantic_token_results(semantic_tokens: &mut super::SemanticTokensState, uri: &str) {
        const MAX_SEMANTIC_TOKEN_RESULTS_PER_DOCUMENT: usize = 8;

        let mut result_ids = semantic_tokens
            .results
            .iter()
            .filter(|(_, entry)| entry.uri == uri)
            .map(|(result_id, entry)| (entry.sequence, result_id.clone()))
            .collect::<Vec<_>>();

        result_ids.sort();

        let remove_count = result_ids
            .len()
            .saturating_sub(MAX_SEMANTIC_TOKEN_RESULTS_PER_DOCUMENT);

        for (_, result_id) in result_ids.into_iter().take(remove_count) {
            semantic_tokens.results.remove(&result_id);
        }
    }

    fn clear_semantic_token_results(&self, uri: &str) {
        self.semantic_tokens
            .lock()
            .expect("semantic tokens lock poisoned")
            .results
            .retain(|_, entry| entry.uri != uri);
    }

    fn workspace_symbol_response(&self, message: &Value) -> Value {
        let Some(params) = message.get("params") else {
            return json!([]);
        };
        let query = params.get("query").and_then(Value::as_str).unwrap_or("");
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);
        let symbols = snapshot.workspace_symbols(query);

        Value::Array(
            symbols
                .into_iter()
                .map(lsp_workspace_symbol_from_core)
                .collect(),
        )
    }

    fn will_rename_files_response(&self, message: &Value) -> Value {
        let file_renames = file_renames_from_request(message);

        if file_renames.is_empty() {
            return Value::Null;
        }

        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);
        let mut edits = Vec::new();

        for rename in file_renames {
            edits.extend(snapshot.file_rename_edits(&rename.old_path, &rename.new_path));
        }

        if edits.is_empty() {
            return Value::Null;
        }

        lsp_workspace_edit(RenameEdit { edits })
    }

    fn document_request<'a>(&'a self, message: &'a Value) -> Option<DocumentRequest<'a>> {
        let params = message.get("params")?;
        let uri = document_uri(params)?;
        self.analysis.document_text(uri)?;

        Some(DocumentRequest {
            params,
            uri,
            path: path_from_uri(uri),
        })
    }

    fn format_options_for_path(&self, path: &Path) -> tua_core::FormatOptions {
        self.analysis.format_options_for_path(path, &self.settings)
    }
}

fn document_uri(params: &Value) -> Option<&str> {
    params
        .get("textDocument")
        .and_then(|document| document.get("uri"))
        .and_then(Value::as_str)
}

fn requested_document_version(params: &Value) -> Option<i64> {
    params.get("documentVersion").and_then(Value::as_i64)
}

fn selected_completion_range_ends_at_cursor(start: &Value, end: &Value, cursor: &Value) -> bool {
    let Some(start_line) = start.get("line").and_then(Value::as_u64) else {
        return false;
    };
    let Some(start_character) = start.get("character").and_then(Value::as_u64) else {
        return false;
    };
    let Some(end_line) = end.get("line").and_then(Value::as_u64) else {
        return false;
    };
    let Some(end_character) = end.get("character").and_then(Value::as_u64) else {
        return false;
    };
    let Some(cursor_line) = cursor.get("line").and_then(Value::as_u64) else {
        return false;
    };
    let Some(cursor_character) = cursor.get("character").and_then(Value::as_u64) else {
        return false;
    };

    start_line == cursor_line
        && end_line == cursor_line
        && start_character <= cursor_character
        && end_character == cursor_character
}

fn empty_local_inline_completion(document_version: Option<i64>) -> Value {
    json!({
        "documentVersion": document_version,
        "items": []
    })
}

fn lsp_range_for_source(source: &str, span: SourceSpan) -> Value {
    json!({
        "start": lsp_position_for_source(source, span.start),
        "end": lsp_position_for_source(source, span.end)
    })
}

fn lsp_position_for_source(source: &str, position: SourcePosition) -> Value {
    let line_index = position.line.saturating_sub(1);
    let scalar_column = position.column.saturating_sub(1);
    let line = source.lines().nth(line_index).unwrap_or("");
    let character = line
        .chars()
        .take(scalar_column)
        .map(char::len_utf16)
        .sum::<usize>();

    json!({
        "line": line_index,
        "character": character
    })
}

fn model_completion_position(source: &str, lsp_position: &Value) -> Option<SourcePosition> {
    let line = lsp_position.get("line")?.as_u64()? as usize;
    let offset = byte_offset_from_lsp_position(source, lsp_position)?;
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    Some(SourcePosition {
        line: line + 1,
        column: source[line_start..offset].chars().count() + 1,
    })
}

fn inline_request_range(params: &Value) -> Value {
    let position = params.get("position").cloned().unwrap_or(Value::Null);
    json!({"start": position, "end": position})
}

fn rejected_inline_completion(
    reason: ModelCompletionRejectionReason,
    span: Option<SourceSpan>,
) -> Value {
    json!({
        "status": "rejected",
        "reason": reason.as_str(),
        "insertText": Value::Null,
        "range": span.map(lsp_range),
        "documentVersion": Value::Null
    })
}

fn completion_request_is_invoked(message: &Value) -> bool {
    message
        .get("params")
        .and_then(|params| params.get("context"))
        .and_then(|context| context.get("triggerKind"))
        .and_then(Value::as_u64)
        == Some(1)
}

fn lsp_completion_array(
    uri: &str,
    position: SourcePosition,
    completions: Vec<tua_core::Completion>,
    client_completion_label_details: bool,
) -> Value {
    Value::Array(
        completions
            .into_iter()
            .enumerate()
            .map(|(index, completion)| {
                let mut item = lsp_completion_from_core(
                    uri,
                    position,
                    completion,
                    client_completion_label_details,
                );
                let label = item
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or_default();

                item["sortText"] = json!(format!("{index:04}_{label}"));
                item
            })
            .collect(),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WatchedFileChange {
    uri: String,
    kind: FileChangeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FileChangeKind {
    Created,
    Changed,
    Deleted,
}

fn watched_file_changes(message: &Value) -> Vec<WatchedFileChange> {
    message
        .get("params")
        .and_then(|params| params.get("changes"))
        .and_then(Value::as_array)
        .map(|changes| {
            changes
                .iter()
                .filter_map(|change| {
                    Some(WatchedFileChange {
                        uri: change.get("uri")?.as_str()?.to_owned(),
                        kind: file_change_kind(change.get("type")?.as_u64()?)?,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn file_renames_from_request(message: &Value) -> Vec<FileRename> {
    message
        .get("params")
        .and_then(|params| params.get("files"))
        .and_then(Value::as_array)
        .map(|files| {
            files
                .iter()
                .filter_map(|file| {
                    Some(FileRename {
                        old_path: path_from_uri(file.get("oldUri")?.as_str()?),
                        new_path: path_from_uri(file.get("newUri")?.as_str()?),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn code_action_kinds_from_request(message: &Value) -> Vec<AssistKind> {
    let Some(only) = message
        .get("params")
        .and_then(|params| params.get("context"))
        .and_then(|context| context.get("only"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };

    let mut kinds = BTreeSet::new();

    for kind in only.iter().filter_map(Value::as_str) {
        match kind {
            "quickfix" => {
                kinds.insert(AssistKind::QuickFix);
            }
            "source" => {
                kinds.insert(AssistKind::SourceOrganizeImports);
                kinds.insert(AssistKind::SourceSortImports);
                kinds.insert(AssistKind::SourceRemoveUnusedImports);
                kinds.insert(AssistKind::SourceCreateLoveCallbacks);
                kinds.insert(AssistKind::SourceFixAll);
            }
            "source.organizeImports" => {
                kinds.insert(AssistKind::SourceOrganizeImports);
            }
            "source.sortImports" => {
                kinds.insert(AssistKind::SourceSortImports);
            }
            "source.removeUnusedImports" => {
                kinds.insert(AssistKind::SourceRemoveUnusedImports);
            }
            "source.tua.createMissingLoveCallbacks" => {
                kinds.insert(AssistKind::SourceCreateLoveCallbacks);
            }
            "source.fixAll" | "source.fixAll.tua" => {
                kinds.insert(AssistKind::SourceFixAll);
            }
            _ => {}
        }
    }

    kinds.into_iter().collect()
}

fn file_change_kind(value: u64) -> Option<FileChangeKind> {
    match value {
        1 => Some(FileChangeKind::Created),
        2 => Some(FileChangeKind::Changed),
        3 => Some(FileChangeKind::Deleted),
        _ => None,
    }
}

fn push_diagnostics_publication(
    publications: &mut Vec<Value>,
    published: &mut BTreeSet<String>,
    uri: &str,
    diagnostics: Vec<Value>,
) {
    if published.insert(uri.to_owned()) {
        publications.push(publish_diagnostics(uri, diagnostics));
    }
}

fn client_supports_watched_files_dynamic_registration(message: &Value) -> bool {
    message
        .get("params")
        .and_then(|params| params.get("capabilities"))
        .and_then(|capabilities| capabilities.get("workspace"))
        .and_then(|workspace| workspace.get("didChangeWatchedFiles"))
        .and_then(|watched| watched.get("dynamicRegistration"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn client_supports_pull_diagnostics(message: &Value) -> bool {
    message
        .get("params")
        .and_then(|params| params.get("capabilities"))
        .and_then(|capabilities| capabilities.get("textDocument"))
        .and_then(|text_document| text_document.get("diagnostic"))
        .is_some()
}

fn client_supports_completion_label_details(message: &Value) -> bool {
    message
        .get("params")
        .and_then(|params| params.get("capabilities"))
        .and_then(|capabilities| capabilities.get("textDocument"))
        .and_then(|text_document| text_document.get("completion"))
        .and_then(|completion| completion.get("completionItem"))
        .and_then(|completion_item| completion_item.get("labelDetailsSupport"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn client_supports_diagnostic_refresh(message: &Value) -> bool {
    message
        .get("params")
        .and_then(|params| params.get("capabilities"))
        .and_then(|capabilities| capabilities.get("workspace"))
        .and_then(|workspace| workspace.get("diagnostics"))
        .and_then(|diagnostics| diagnostics.get("refreshSupport"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn client_supports_workspace_refresh(message: &Value, capability: &str) -> bool {
    message
        .get("params")
        .and_then(|params| params.get("capabilities"))
        .and_then(|capabilities| capabilities.get("workspace"))
        .and_then(|workspace| workspace.get(capability))
        .and_then(|capability| capability.get("refreshSupport"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn is_config_uri(uri: &str) -> bool {
    path_from_uri(uri)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "tua.toml")
}

fn cache_hits(stats: &AnalysisCacheStats) -> usize {
    stats.parse_hits
        + stats.hir_hits
        + stats.scope_hits
        + stats.type_environment_hits
        + stats.diagnostics_hits
        + stats.module_export_hits
        + stats.require_edge_hits
        + stats.symbol_index_hits
        + stats.usage_index_hits
        + stats.completion_context_hits
}

fn cache_misses(stats: &AnalysisCacheStats) -> usize {
    stats.parse_misses
        + stats.hir_misses
        + stats.scope_misses
        + stats.type_environment_misses
        + stats.diagnostics_misses
        + stats.module_export_misses
        + stats.require_edge_misses
        + stats.symbol_index_misses
        + stats.usage_index_misses
        + stats.completion_context_misses
}

fn hit_miss(hits: usize, misses: usize) -> Value {
    json!({
        "hits": hits,
        "misses": misses,
        "hitRate": percentage(hits, hits + misses)
    })
}

fn percentage(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        return 0.0;
    }

    let percent = (numerator as f64 / denominator as f64) * 100.0;

    (percent * 10.0).round() / 10.0
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn inline_parse_distribution(stats: &AnalysisCacheStats) -> Value {
    json!({
        "zero": stats
            .inline_validation_attempts
            .saturating_sub(stats.inline_candidate_parses),
        "one": stats
            .inline_candidate_parses
            .saturating_sub(stats.inline_baseline_parses),
        "two": stats.inline_baseline_parses,
        "parsesPerAttempt": ratio(
            stats.inline_candidate_parses + stats.inline_baseline_parses,
            stats.inline_validation_attempts
        )
    })
}

fn inline_returned_parse_distribution(stats: &AnalysisCacheStats) -> Value {
    json!({
        "zero": stats.inline_returned_after_zero_parses,
        "one": stats.inline_returned_after_one_parse,
        "two": stats.inline_returned_after_two_parses,
        "atMostOneRate": ratio(
            stats.inline_returned_after_zero_parses + stats.inline_returned_after_one_parse,
            stats.inline_candidates_returned
        )
    })
}

fn latency_summary(samples: &VecDeque<RequestLatencySample>) -> Value {
    let mut latencies = samples
        .iter()
        .map(|sample| sample.elapsed_ms)
        .collect::<Vec<_>>();
    latencies.sort_unstable();

    let mut by_method = BTreeMap::<String, Vec<u64>>::new();
    for sample in samples {
        by_method
            .entry(sample.method.clone())
            .or_default()
            .push(sample.elapsed_ms);
    }

    let methods = by_method
        .into_iter()
        .map(|(method, mut method_latencies)| {
            method_latencies.sort_unstable();
            json!({
                "method": method,
                "count": method_latencies.len(),
                "p95Ms": percentile(&method_latencies, 95),
                "maxMs": method_latencies.last().copied().unwrap_or(0)
            })
        })
        .collect::<Vec<_>>();

    json!({
        "recentCount": latencies.len(),
        "p50Ms": percentile(&latencies, 50),
        "p95Ms": percentile(&latencies, 95),
        "p99Ms": percentile(&latencies, 99),
        "maxMs": latencies.last().copied().unwrap_or(0),
        "methods": methods
    })
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }

    let rank = percentile.min(100) * sorted.len();
    let index = rank.div_ceil(100).saturating_sub(1);

    sorted[index.min(sorted.len() - 1)]
}

fn register_watched_files_request(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "client/registerCapability",
        "params": {
            "registrations": [
                {
                    "id": "tua-workspace-file-watchers",
                    "method": "workspace/didChangeWatchedFiles",
                    "registerOptions": {
                        "watchers": [
                            { "globPattern": "**/*.tua" },
                            { "globPattern": "**/*.lua" },
                            { "globPattern": "**/tua.toml" },
                            { "globPattern": "**/*.bmp" },
                            { "globPattern": "**/*.exr" },
                            { "globPattern": "**/*.hdr" },
                            { "globPattern": "**/*.jpeg" },
                            { "globPattern": "**/*.jpg" },
                            { "globPattern": "**/*.pic" },
                            { "globPattern": "**/*.png" },
                            { "globPattern": "**/*.tga" },
                            { "globPattern": "**/*.flac" },
                            { "globPattern": "**/*.mp3" },
                            { "globPattern": "**/*.ogg" },
                            { "globPattern": "**/*.wav" },
                            { "globPattern": "**/*.otf" },
                            { "globPattern": "**/*.ttf" },
                            { "globPattern": "**/*.frag" },
                            { "globPattern": "**/*.fs" },
                            { "globPattern": "**/*.glsl" },
                            { "globPattern": "**/*.vert" },
                            { "globPattern": "**/*.vs" }
                        ]
                    }
                }
            ]
        }
    })
}

fn extend_optional(values: &mut Vec<Value>, value: Option<Value>) {
    if let Some(value) = value {
        values.push(value);
    }
}

fn trace_log_should_count(method: &str, request: bool) -> bool {
    if request {
        return super::method_is_background_refresh(method);
    }

    matches!(method, "$/cancelRequest")
}

fn edit_background_request_should_yield(method: &str) -> bool {
    super::method_is_background_refresh(method)
}

impl TraceLogCounts {
    fn clear(&mut self) {
        self.requests.clear();
        self.notifications.clear();
        self.total = 0;
    }

    fn is_empty(&self) -> bool {
        self.total == 0
    }

    fn increment(&mut self, request: bool, method: &str) {
        let counts = if request {
            &mut self.requests
        } else {
            &mut self.notifications
        };

        *counts.entry(method.to_owned()).or_insert(0) += 1;
        self.total += 1;
    }
}

fn trace_log_counts_summary(counts: &TraceLogCounts) -> String {
    let mut parts = vec![format!("total={}", counts.total)];

    if !counts.requests.is_empty() {
        parts.push(format!(
            "requests {}",
            trace_log_count_entries(&counts.requests)
        ));
    }
    if !counts.notifications.is_empty() {
        parts.push(format!(
            "notifications {}",
            trace_log_count_entries(&counts.notifications)
        ));
    }

    format!("tua lsp trace summary: {}", parts.join("; "))
}

fn trace_log_count_entries(counts: &std::collections::BTreeMap<String, usize>) -> String {
    counts
        .iter()
        .map(|(method, count)| format!("{method}={count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn log_message_notification(message_type: u8, message: impl Into<String>) -> Value {
    let message = format!("[lsp {}] {}", lsp_log_timestamp(), message.into());

    json!({
        "jsonrpc": "2.0",
        "method": "window/logMessage",
        "params": {
            "type": message_type,
            "message": message
        }
    })
}

fn lsp_log_timestamp() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = duration.as_secs() as i64;
    let milliseconds = duration.subsec_millis();
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_unix_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = seconds_of_day % 3_600 / 60;
    let second = seconds_of_day % 60;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{milliseconds:03}Z")
}

fn civil_from_unix_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);

    (year, month, day)
}

fn completion_response_item_count(response: &Value) -> usize {
    completion_response_items(response).map_or(0, Vec::len)
}

fn hover_response_summary(response: &Value) -> String {
    let Some(result) = response.get("result") else {
        return "result=<missing>".to_owned();
    };
    if result.is_null() {
        return "result=null".to_owned();
    }

    let contents = result
        .get("contents")
        .and_then(|contents| {
            contents
                .get("value")
                .and_then(Value::as_str)
                .or_else(|| contents.as_str())
        })
        .map(debug_text_preview)
        .unwrap_or_else(|| "<unknown>".to_owned());

    format!("result=hover contents={contents}")
}

fn completion_response_label_sample(response: &Value) -> String {
    const MAX_LABELS: usize = 32;

    let labels = completion_response_items(response)
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(completion_item_debug_label)
        .collect::<Vec<_>>();
    let visible = labels
        .iter()
        .take(MAX_LABELS)
        .map(|label| debug_text_preview(label.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = labels.len().saturating_sub(MAX_LABELS);

    if remaining == 0 {
        format!("labels=[{visible}]")
    } else {
        format!("labels=[{visible}] labelsMore={remaining}")
    }
}

fn completion_response_items(response: &Value) -> Option<&Vec<Value>> {
    let result = response.get("result")?;

    result
        .as_array()
        .or_else(|| result.get("items").and_then(Value::as_array))
}

fn completion_item_debug_label(item: &Value) -> Option<String> {
    let label = item.get("label").and_then(Value::as_str)?;
    let filter_text = item.get("filterText").and_then(Value::as_str);

    Some(match filter_text {
        Some(filter_text) if filter_text != label => format!("{label}|filter={filter_text}"),
        _ => label.to_owned(),
    })
}

fn completion_source_context(source: &str, position: SourcePosition) -> String {
    let Some(line) = source_line_at(source, position.line) else {
        return "prefix=<missing-line> before=<missing-line> line=<missing-line>".to_owned();
    };
    let cursor = byte_for_column(line, position.column).min(line.len());
    let before = &line[..cursor];
    let prefix = identifier_prefix_from_before(before);
    let effective_position = effective_identifier_completion_position(source, position);
    let effective_context = (effective_position != position)
        .then(|| effective_completion_source_context(source, effective_position))
        .flatten()
        .unwrap_or_default();

    format!(
        "prefix={} before={} line={}{}",
        debug_text_preview(prefix),
        debug_text_preview(before),
        debug_text_preview(line),
        effective_context
    )
}

fn effective_completion_source_context(source: &str, position: SourcePosition) -> Option<String> {
    let line = source_line_at(source, position.line)?;
    let cursor = byte_for_column(line, position.column).min(line.len());
    let before = &line[..cursor];
    let prefix = identifier_prefix_from_before(before);

    Some(format!(
        " effectivePosition={}:{} effectivePrefix={} effectiveBefore={}",
        position.line,
        position.column,
        debug_text_preview(prefix),
        debug_text_preview(before)
    ))
}

fn effective_identifier_completion_position(
    source: &str,
    position: SourcePosition,
) -> SourcePosition {
    let Some(line) = source_line_at(source, position.line) else {
        return position;
    };
    let cursor = byte_for_column(line, position.column).min(line.len());
    let bytes = line.as_bytes();

    if cursor >= bytes.len() || !is_identifier_continue_byte(bytes[cursor]) {
        return position;
    }

    let mut end = cursor;

    while end < bytes.len() && is_identifier_continue_byte(bytes[end]) {
        end += 1;
    }

    SourcePosition {
        line: position.line,
        column: column_for_byte(line, end),
    }
}

fn source_line_at(source: &str, line: usize) -> Option<&str> {
    source
        .split('\n')
        .nth(line.checked_sub(1)?)
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

fn identifier_prefix_from_before(before: &str) -> &str {
    let start = before
        .char_indices()
        .rev()
        .find_map(|(index, character)| {
            (!is_identifier_continue_char(character)).then_some(index + character.len_utf8())
        })
        .unwrap_or_default();

    &before[start..]
}

fn is_identifier_continue_char(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn byte_for_column(line: &str, column: usize) -> usize {
    let target = column.saturating_sub(1);

    line.char_indices()
        .nth(target)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

fn column_for_byte(line: &str, byte: usize) -> usize {
    line[..byte.min(line.len())].chars().count() + 1
}

fn is_identifier_continue_byte(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

fn definition_response_target(response: &Value) -> String {
    let Some(result) = response.get("result") else {
        return "result=<missing>".to_owned();
    };
    if result.is_null() {
        return "result=<null>".to_owned();
    }

    let uri = result
        .get("uri")
        .and_then(Value::as_str)
        .unwrap_or("<unknown>");
    let line = result
        .get("range")
        .and_then(|range| range.get("start"))
        .and_then(|start| start.get("line"))
        .and_then(Value::as_u64)
        .map(|line| line + 1)
        .unwrap_or_default();
    let column = result
        .get("range")
        .and_then(|range| range.get("start"))
        .and_then(|start| start.get("character"))
        .and_then(Value::as_u64)
        .map(|column| column + 1)
        .unwrap_or_default();

    format!("result={uri}:{line}:{column}")
}

fn content_change_debug_label(change: &Value) -> String {
    let text = change
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let range = change
        .get("range")
        .and_then(|range| {
            let start = range.get("start")?;
            let end = range.get("end")?;
            let start_line = start.get("line")?.as_u64()?;
            let start_character = start.get("character")?.as_u64()?;
            let end_line = end.get("line")?.as_u64()?;
            let end_character = end.get("character")?.as_u64()?;

            Some(format!(
                "{start_line}:{start_character}-{end_line}:{end_character}"
            ))
        })
        .unwrap_or_else(|| "full".to_owned());

    format!(
        "range={range} text={} bytes={}",
        debug_text_preview(text),
        text.len()
    )
}

fn debug_text_preview(text: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 24;

    let mut preview = text.chars().take(MAX_PREVIEW_CHARS).collect::<String>();
    let truncated = text.chars().nth(MAX_PREVIEW_CHARS).is_some();

    if truncated {
        preview.push_str("...");
    }

    format!("{preview:?}")
}

fn trace_config_from_initialize(message: &Value) -> TraceConfig {
    trace_config_from_params(
        message
            .get("params")
            .and_then(|params| params.get("initializationOptions")),
    )
}

fn trace_config_from_params(params: Option<&Value>) -> TraceConfig {
    let server = params
        .and_then(|params| params.get("trace"))
        .and_then(|trace| trace.get("server"))
        .and_then(Value::as_str)
        .and_then(ServerTraceLevel::parse)
        .unwrap_or_default();
    let log_verbosity = params
        .and_then(|params| params.get("logVerbosity"))
        .and_then(Value::as_str)
        .and_then(LogVerbosity::parse)
        .unwrap_or_default();

    TraceConfig {
        server,
        log_verbosity,
    }
}

impl TraceConfig {
    fn enabled(self) -> bool {
        self.server != ServerTraceLevel::Off || self.log_verbosity >= LogVerbosity::Info
    }

    fn verbose(self) -> bool {
        self.server == ServerTraceLevel::Verbose || self.log_verbosity >= LogVerbosity::Debug
    }
}

impl ServerTraceLevel {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "messages" => Some(Self::Messages),
            "verbose" => Some(Self::Verbose),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Messages => "messages",
            Self::Verbose => "verbose",
        }
    }
}

impl LogVerbosity {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "error" => Some(Self::Error),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            "trace" => Some(Self::Trace),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Error => "error",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

fn source_span_intersects_range(span: SourceSpan, range: SourceRange) -> bool {
    position_key(span.start) < position_key(range.end)
        && position_key(range.start) < position_key(span.end)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn apply_content_changes(current: &str, changes: &[Value]) -> Option<String> {
    let mut text = current.to_owned();

    for change in changes {
        let replacement = change.get("text")?.as_str()?;

        if change.get("range").is_none_or(Value::is_null) {
            text = replacement.to_owned();
            continue;
        }

        apply_incremental_change(&mut text, change.get("range")?, replacement)?;
    }

    Some(text)
}

fn apply_incremental_change(text: &mut String, range: &Value, replacement: &str) -> Option<()> {
    let start = byte_offset_from_lsp_position(text, range.get("start")?)?;
    let end = byte_offset_from_lsp_position(text, range.get("end")?)?;

    if start > end || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return None;
    }

    text.replace_range(start..end, replacement);

    Some(())
}

fn byte_offset_from_lsp_position(source: &str, position: &Value) -> Option<usize> {
    let target_line = position.get("line")?.as_u64()? as usize;
    let target_character = position.get("character")?.as_u64()? as usize;
    let mut offset = 0;

    if source.is_empty() {
        return (target_line == 0).then_some(0);
    }

    for (line, segment) in source.split_inclusive('\n').enumerate() {
        let text = line_without_ending(segment);

        if line == target_line {
            return Some(offset + byte_for_lsp_character(text, target_character));
        }

        offset += segment.len();
    }

    (target_line == source.lines().count()).then_some(source.len())
}

fn exact_byte_offset_from_lsp_position(source: &str, position: &Value) -> Option<usize> {
    let target_line = position.get("line")?.as_u64()? as usize;
    let target_character = position.get("character")?.as_u64()? as usize;
    let offset = byte_offset_from_lsp_position(source, position)?;
    let actual_line = source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count();
    if actual_line != target_line {
        return None;
    }
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let actual_character = source[line_start..offset].encode_utf16().count();

    (actual_character == target_character).then_some(offset)
}

fn line_without_ending(line: &str) -> &str {
    line.strip_suffix('\n')
        .unwrap_or(line)
        .strip_suffix('\r')
        .unwrap_or_else(|| line.strip_suffix('\n').unwrap_or(line))
}

fn byte_for_lsp_character(line: &str, target_character: usize) -> usize {
    let mut character = 0;

    for (index, value) in line.char_indices() {
        if character >= target_character {
            return index;
        }

        character += value.len_utf16();

        if character > target_character {
            return index;
        }
    }

    line.len()
}
