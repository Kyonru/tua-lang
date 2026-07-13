use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;

use serde_json::{json, Value};
use tua_core::{SourcePosition, SourceSpan};

use super::configuration::WorkspaceSettings;
use super::protocol::{
    call_hierarchy_identity_from_lsp, completion_resolve_data, core_diagnostic_from_lsp,
    error_response, lsp_call_hierarchy_incoming_call, lsp_call_hierarchy_item,
    lsp_call_hierarchy_outgoing_call, lsp_code_action_from_core, lsp_completion_from_core,
    lsp_diagnostic_from_core, lsp_document_highlight_from_core, lsp_document_link_from_core,
    lsp_document_symbol_from_core, lsp_folding_range_from_core, lsp_full_document_text_edit,
    lsp_hover_from_core, lsp_inlay_hint_from_core, lsp_location_from_core,
    lsp_prepare_rename_from_core, lsp_reference_location, lsp_resolved_completion_item,
    lsp_selection_range_from_core, lsp_semantic_tokens_from_core, lsp_signature_help_from_core,
    lsp_source_range, lsp_workspace_edit, lsp_workspace_symbol_from_core, publish_diagnostics,
    response, source_position_from_lsp, SourceRange,
};
use super::uri::{path_from_uri, workspace_folders_from_change, workspace_folders_from_initialize};
use super::{capabilities, LspServer, PendingDiagnostics, SLOW_DIAGNOSTICS_DEBOUNCE};

struct DocumentRequest<'a> {
    params: &'a Value,
    uri: &'a str,
    path: PathBuf,
}

impl<'a> DocumentRequest<'a> {
    fn position(&self) -> Option<SourcePosition> {
        self.params
            .get("position")
            .and_then(source_position_from_lsp)
    }
}

impl LspServer {
    pub(super) fn handle_message(&mut self, message: Value) -> Option<Vec<Value>> {
        let method = message.get("method").and_then(Value::as_str)?.to_owned();

        if message.get("id").is_some() {
            return Some(vec![self.handle_request(message, &method)]);
        }

        self.handle_notification(&message, &method)
    }

    fn handle_request(&mut self, message: Value, method: &str) -> Value {
        let _profile = tua_core::profile::span(format!("lsp.request {method}"));
        let id = message.get("id").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => {
                self.workspace_folders = workspace_folders_from_initialize(&message);
                self.analysis
                    .reindex_workspace(&self.workspace_folders, &self.settings);
                self.client_watched_files_dynamic_registration =
                    client_supports_watched_files_dynamic_registration(&message);

                response(id, capabilities::initialize_result())
            }
            "shutdown" => {
                self.shutdown_requested = true;

                response(id, Value::Null)
            }
            "textDocument/hover" => {
                response(id, self.hover_response(&message).unwrap_or(Value::Null))
            }
            "textDocument/completion" => response(id, self.completion_response(&message)),
            "completionItem/resolve" => response(id, self.completion_resolve_response(message)),
            "textDocument/signatureHelp" => response(
                id,
                self.signature_help_response(&message)
                    .unwrap_or(Value::Null),
            ),
            "textDocument/definition" => response(
                id,
                self.definition_response(&message).unwrap_or(Value::Null),
            ),
            "tua/builtinDocument" => response(
                id,
                self.builtin_document_response(&message)
                    .unwrap_or(Value::Null),
            ),
            "tua/cacheStats" => response(id, self.cache_stats_response()),
            "textDocument/prepareCallHierarchy" => {
                response(id, self.prepare_call_hierarchy_response(&message))
            }
            "callHierarchy/incomingCalls" => {
                response(id, self.call_hierarchy_incoming_response(&message))
            }
            "callHierarchy/outgoingCalls" => {
                response(id, self.call_hierarchy_outgoing_response(&message))
            }
            "textDocument/documentLink" => response(id, self.document_link_response(&message)),
            "textDocument/documentHighlight" => {
                response(id, self.document_highlight_response(&message))
            }
            "textDocument/documentSymbol" => response(id, self.document_symbol_response(&message)),
            "textDocument/selectionRange" => response(id, self.selection_range_response(&message)),
            "textDocument/foldingRange" => response(id, self.folding_range_response(&message)),
            "textDocument/codeAction" => response(id, self.code_action_response(&message)),
            "textDocument/formatting" => response(
                id,
                self.formatting_response(&message).unwrap_or(Value::Null),
            ),
            "textDocument/references" => response(id, self.references_response(&message)),
            "textDocument/prepareRename" => response(
                id,
                self.prepare_rename_response(&message)
                    .unwrap_or(Value::Null),
            ),
            "textDocument/rename" => {
                response(id, self.rename_response(&message).unwrap_or(Value::Null))
            }
            "textDocument/inlayHint" => response(id, self.inlay_hint_response(&message)),
            "textDocument/semanticTokens/full" => {
                response(id, self.semantic_tokens_response(&message, None))
            }
            "textDocument/semanticTokens/range" => {
                let range = message
                    .get("params")
                    .and_then(|params| params.get("range"))
                    .and_then(lsp_source_range);

                response(id, self.semantic_tokens_response(&message, range))
            }
            "workspace/symbol" => response(id, self.workspace_symbol_response(&message)),
            _ => error_response(id, -32601, format!("method not found: {method}")),
        }
    }

    fn handle_notification(&mut self, message: &Value, method: &str) -> Option<Vec<Value>> {
        match method {
            "initialized" => self.initialized_notifications(),
            "exit" => {
                self.should_exit = true;

                None
            }
            "textDocument/didOpen" => self.did_open_notification(message),
            "textDocument/didChange" => self.did_change_notification(message),
            "textDocument/didSave" => self.did_save_notification(message),
            "textDocument/didClose" => self.did_close_notification(message),
            "workspace/didChangeWatchedFiles" => {
                self.did_change_watched_files_notification(message)
            }
            "workspace/didChangeWorkspaceFolders" => {
                self.did_change_workspace_folders_notification(message)
            }
            "workspace/didChangeConfiguration" => {
                self.did_change_configuration_notification(message)
            }
            _ => None,
        }
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

        self.analysis
            .open_document(uri.clone(), text, &self.workspace_folders, &self.settings);
        let generation = self.bump_document_generation(&uri, version);
        self.queue_slow_diagnostics(uri.clone(), generation, version, Instant::now());

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
        let generation = self.bump_document_generation(&uri, version);
        self.queue_slow_diagnostics(uri.clone(), generation, version, Instant::now());

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

            self.analysis.update_document(
                uri.clone(),
                text.to_owned(),
                &self.workspace_folders,
                &self.settings,
            );
            self.bump_document_generation(&uri, version);
        }

        self.pending_slow_diagnostics.remove(&uri);

        self.analysis.contains_document(&uri).then(|| {
            let diagnostics = self.diagnostics_for(&uri);

            vec![self.publish_tracked_diagnostics(&uri, diagnostics)]
        })
    }

    fn did_close_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let params = message.get("params")?;
        let uri = document_uri(params)?.to_owned();

        self.analysis.close_document(&uri);
        self.pending_slow_diagnostics.remove(&uri);
        self.last_published_diagnostics.remove(&uri);
        self.document_generations.remove(&uri);
        self.document_versions.remove(&uri);

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

        if should_reindex
            || asset_changed
            || !source_changes.is_empty()
            || !deleted_sources.is_empty()
        {
            self.pending_slow_diagnostics.clear();
            self.analysis
                .refresh_open_documents(&self.workspace_folders, &self.settings);
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

        None
    }

    fn did_change_configuration_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        self.settings = WorkspaceSettings::from_did_change_configuration(message);
        self.analysis
            .reindex_workspace(&self.workspace_folders, &self.settings);
        self.analysis
            .refresh_open_documents(&self.workspace_folders, &self.settings);
        self.pending_slow_diagnostics.clear();

        Some(self.publish_all_open_diagnostics())
    }

    fn publish_all_open_diagnostics(&self) -> Vec<Value> {
        self.analysis
            .open_uris()
            .into_iter()
            .map(|uri| publish_diagnostics(&uri, self.diagnostics_for(&uri)))
            .collect()
    }

    fn diagnostics_for(&self, uri: &str) -> Vec<Value> {
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
        self.pending_slow_diagnostics.insert(
            uri,
            PendingDiagnostics {
                generation,
                version,
                due_at: now + SLOW_DIAGNOSTICS_DEBOUNCE,
            },
        );
    }

    pub(super) fn next_slow_diagnostics_due(&self) -> Option<Instant> {
        self.pending_slow_diagnostics
            .values()
            .map(|pending| pending.due_at)
            .min()
    }

    #[cfg(test)]
    pub(super) fn drain_due_slow_diagnostics(&mut self, now: Instant) -> Vec<Value> {
        let due = self
            .pending_slow_diagnostics
            .iter()
            .filter_map(|(uri, pending)| (pending.due_at <= now).then(|| uri.clone()))
            .collect::<Vec<_>>();

        self.drain_slow_diagnostics_for(due)
    }

    pub(super) fn drain_next_due_slow_diagnostic(&mut self, now: Instant) -> Option<Value> {
        let uri = self
            .pending_slow_diagnostics
            .iter()
            .filter(|(_, pending)| pending.due_at <= now)
            .min_by_key(|(_, pending)| pending.due_at)
            .map(|(uri, _)| uri.clone())?;

        self.drain_slow_diagnostics_for(vec![uri])
            .into_iter()
            .next()
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

    fn hover_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
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
        let snapshot = {
            let _profile = tua_core::profile::span("lsp.completion.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let completions = {
            let _profile = tua_core::profile::span("lsp.completion.query");

            snapshot.completion(&request.path, position)
        };

        Value::Array(
            completions
                .into_iter()
                .map(|completion| lsp_completion_from_core(request.uri, position, completion))
                .collect(),
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

    fn builtin_document_response(&self, message: &Value) -> Option<Value> {
        let uri = message.get("params")?.get("uri")?.as_str()?;
        let source = tua_core::builtin_definition_document(uri)?;

        Some(Value::String(source))
    }

    fn cache_stats_response(&self) -> Value {
        let stats = self.analysis.cache_stats();

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
            "invalidations": stats.invalidations,
            "dependentInvalidations": stats.dependent_invalidations
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
        let snapshot = self
            .analysis
            .snapshot(&self.workspace_folders, &self.settings);
        let actions = snapshot.code_actions(
            &request.path,
            tua_core::CodeActionRange {
                start: range.start,
                end: range.end,
            },
            &diagnostics,
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
        let snapshot = {
            let _profile = tua_core::profile::span("lsp.inlay_hints.snapshot");

            self.analysis
                .snapshot(&self.workspace_folders, &self.settings)
        };
        let hints = {
            let _profile = tua_core::profile::span("lsp.inlay_hints.query");

            range
                .map(|range| {
                    snapshot.inlay_hints_in_range(
                        &request.path,
                        SourceSpan {
                            start: range.start,
                            end: range.end,
                        },
                    )
                })
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

    fn semantic_tokens_response(&self, message: &Value, range: Option<SourceRange>) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!({ "data": [] });
        };
        let mut tokens = self.analysis.semantic_tokens(&request.path);
        if let Some(range) = range {
            tokens.retain(|token| source_span_intersects_range(token.span, range));
        }

        lsp_semantic_tokens_from_core(tokens)
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

    fn format_options_for_path(&self, path: &PathBuf) -> tua_core::FormatOptions {
        self.analysis.format_options_for_path(path, &self.settings)
    }
}

fn document_uri(params: &Value) -> Option<&str> {
    params
        .get("textDocument")
        .and_then(|document| document.get("uri"))
        .and_then(Value::as_str)
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

fn is_config_uri(uri: &str) -> bool {
    path_from_uri(uri)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "tua.toml")
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
