use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tua_core::{load_config_from_dir, DiagnosticsConfig, SourcePosition};

use super::protocol::{
    completion_resolve_data, error_response, lsp_completion_from_core, lsp_diagnostic_from_core,
    lsp_document_highlight_from_core, lsp_document_link_from_core, lsp_document_symbol_from_core,
    lsp_hover_from_core, lsp_inlay_hint_from_core, lsp_location_from_core,
    lsp_prepare_rename_from_core, lsp_reference_location, lsp_resolved_completion_item,
    lsp_semantic_tokens_from_core, lsp_signature_help_from_core, lsp_source_range,
    lsp_workspace_edit, lsp_workspace_symbol_from_core, publish_diagnostics, response,
    source_position_from_lsp,
};
use super::uri::{path_from_uri, workspace_folders_from_change, workspace_folders_from_initialize};
use super::{capabilities, LspServer};

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
        let id = message.get("id").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => {
                self.workspace_folders = workspace_folders_from_initialize(&message);
                self.analysis.reindex_workspace(&self.workspace_folders);
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
            "textDocument/documentLink" => response(id, self.document_link_response(&message)),
            "textDocument/documentHighlight" => {
                response(id, self.document_highlight_response(&message))
            }
            "textDocument/documentSymbol" => response(id, self.document_symbol_response(&message)),
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
                response(id, self.semantic_tokens_response(&message))
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

        self.analysis.open_document(uri.clone(), text);

        Some(vec![publish_diagnostics(&uri, self.diagnostics_for(&uri))])
    }

    fn did_change_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let params = message.get("params")?;
        let uri = params.get("textDocument")?.get("uri")?.as_str()?.to_owned();
        let current = self.analysis.document_text(&uri)?;
        let text = apply_content_changes(current, params.get("contentChanges")?.as_array()?)?;

        self.analysis.update_document(uri.clone(), text);

        Some(vec![publish_diagnostics(&uri, self.diagnostics_for(&uri))])
    }

    fn did_save_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let params = message.get("params")?;
        let uri = document_uri(params)?.to_owned();

        if let Some(text) = params.get("text").and_then(Value::as_str) {
            self.analysis.update_document(uri.clone(), text.to_owned());
        }

        self.analysis
            .contains_document(&uri)
            .then(|| vec![publish_diagnostics(&uri, self.diagnostics_for(&uri))])
    }

    fn did_close_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let params = message.get("params")?;
        let uri = document_uri(params)?.to_owned();

        self.analysis.close_document(&uri);

        Some(vec![publish_diagnostics(&uri, Vec::new())])
    }

    fn did_change_watched_files_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let changes = watched_file_changes(message);
        let mut source_changes = Vec::new();
        let mut deleted_sources = Vec::new();
        let mut should_reindex = false;

        for change in changes {
            if is_config_uri(&change.uri) {
                should_reindex = true;
                continue;
            }

            if !is_source_uri(&change.uri) {
                continue;
            }

            match change.kind {
                FileChangeKind::Created | FileChangeKind::Changed => {
                    self.analysis.cache_disk_file(&change.uri);
                    source_changes.push(change.uri);
                }
                FileChangeKind::Deleted => {
                    self.analysis.remove_file(&change.uri);
                    deleted_sources.push(change.uri);
                }
            }
        }

        if should_reindex {
            self.analysis.reindex_workspace(&self.workspace_folders);
        }

        if should_reindex || !source_changes.is_empty() || !deleted_sources.is_empty() {
            self.analysis.refresh_open_documents();
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

        self.analysis.reindex_workspace(&self.workspace_folders);

        None
    }

    fn diagnostics_for(&self, uri: &str) -> Vec<Value> {
        if !self.analysis.contains_document(uri) {
            return Vec::new();
        }

        let path = path_from_uri(uri);
        let diagnostics_config = self.diagnostics_config_for_path(&path);
        let snapshot = self.analysis.snapshot(&self.workspace_folders);

        snapshot
            .diagnostics_for_uri(uri, &diagnostics_config)
            .into_iter()
            .map(lsp_diagnostic_from_core)
            .collect()
    }

    fn diagnostics_for_disk_file(&self, uri: &str) -> Vec<Value> {
        let path = path_from_uri(uri);
        let diagnostics_config = self.diagnostics_config_for_path(&path);
        let snapshot = self.analysis.snapshot(&self.workspace_folders);

        snapshot
            .diagnostics_for_uri(uri, &diagnostics_config)
            .into_iter()
            .map(lsp_diagnostic_from_core)
            .collect()
    }

    fn diagnostics_config_for_path(&self, path: &Path) -> DiagnosticsConfig {
        let mut roots = self
            .workspace_folders
            .iter()
            .filter(|root| path.starts_with(root))
            .collect::<Vec<_>>();

        if !roots.is_empty() {
            roots.sort_by_key(|root| root.components().count());

            return roots
                .into_iter()
                .rev()
                .find_map(|root| load_config_from_dir(root).ok())
                .map(|loaded| loaded.config.diagnostics.clone())
                .unwrap_or_default();
        }

        if let Some(parent) = path.parent() {
            for ancestor in parent.ancestors() {
                match load_config_from_dir(ancestor) {
                    Ok(loaded) if loaded.found => return loaded.config.diagnostics.clone(),
                    Ok(_) | Err(_) => {}
                }
            }
        }

        DiagnosticsConfig::default()
    }

    fn hover_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
        let hover = snapshot.hover(&request.path, position)?;

        Some(lsp_hover_from_core(hover))
    }

    fn completion_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let Some(position) = request.position() else {
            return json!([]);
        };
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
        let completions = snapshot.completion(&request.path, position);

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
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
        let completion = snapshot.resolve_completion(&path, data.position, &data.label, data.kind);

        lsp_resolved_completion_item(item, completion)
    }

    fn signature_help_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
        let help = snapshot.signature_help(&request.path, position)?;

        Some(lsp_signature_help_from_core(help))
    }

    fn definition_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
        let definition = snapshot.definition(&request.path, position)?;

        Some(lsp_location_from_core(definition))
    }

    fn document_link_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
        let links = snapshot.document_links(&request.path);

        Value::Array(links.into_iter().map(lsp_document_link_from_core).collect())
    }

    fn document_highlight_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let Some(position) = request.position() else {
            return json!([]);
        };
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
        let highlights = snapshot.document_highlights(&request.path, position);

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
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
        let symbols = snapshot.document_symbols(&request.path);

        Value::Array(
            symbols
                .into_iter()
                .map(lsp_document_symbol_from_core)
                .collect(),
        )
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
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
        let references = snapshot.references(&request.path, position, include_declaration);

        Value::Array(
            references
                .into_iter()
                .map(|reference| lsp_reference_location(request.uri, reference))
                .collect(),
        )
    }

    fn prepare_rename_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
        let target = snapshot.prepare_rename(&request.path, position)?;

        Some(lsp_prepare_rename_from_core(target))
    }

    fn rename_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let new_name = request.params.get("newName")?.as_str()?;
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
        let edit = snapshot.rename(&request.path, position, new_name)?;

        Some(lsp_workspace_edit(request.uri, edit))
    }

    fn inlay_hint_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let range = request.params.get("range").and_then(lsp_source_range);
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
        let hints = snapshot.inlay_hints(&request.path);

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

    fn semantic_tokens_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!({ "data": [] });
        };
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
        let tokens = snapshot.semantic_tokens(&request.path);

        lsp_semantic_tokens_from_core(tokens)
    }

    fn workspace_symbol_response(&self, message: &Value) -> Value {
        let Some(params) = message.get("params") else {
            return json!([]);
        };
        let query = params.get("query").and_then(Value::as_str).unwrap_or("");
        let snapshot = self.analysis.snapshot(&self.workspace_folders);
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

fn is_source_uri(uri: &str) -> bool {
    path_from_uri(uri)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "tua" || extension == "lua")
}

fn is_config_uri(uri: &str) -> bool {
    path_from_uri(uri)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "tua.toml")
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
                            { "globPattern": "**/tua.toml" }
                        ]
                    }
                }
            ]
        }
    })
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
