use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::{env, fs};

use serde_json::{json, Value};
use tua_core::{
    check_tua_source_with_diagnostics_config, complete_tua_source, definition_tua_source,
    document_highlights_tua_source, document_symbols_tua_source, hover_tua_source,
    inlay_hints_tua_source, load_config_from_dir, module_links_tua_source,
    prepare_rename_tua_source, references_tua_source, rename_tua_source,
    semantic_tokens_tua_source, signature_help_tua_source, workspace_symbols_from_dir,
    DiagnosticsConfig, SourcePosition,
};

use super::protocol::{
    error_response, lsp_completion_from_core, lsp_diagnostic_from_core,
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
    text: &'a str,
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

        self.documents.insert(uri.clone(), text);

        Some(vec![publish_diagnostics(&uri, self.diagnostics_for(&uri))])
    }

    fn did_change_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let params = message.get("params")?;
        let uri = params.get("textDocument")?.get("uri")?.as_str()?.to_owned();
        let current = self.documents.get(&uri)?;
        let text = apply_content_changes(current, params.get("contentChanges")?.as_array()?)?;

        self.documents.insert(uri.clone(), text);

        Some(vec![publish_diagnostics(&uri, self.diagnostics_for(&uri))])
    }

    fn did_save_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let params = message.get("params")?;
        let uri = document_uri(params)?.to_owned();

        if let Some(text) = params.get("text").and_then(Value::as_str) {
            self.documents.insert(uri.clone(), text.to_owned());
        }

        self.documents
            .contains_key(&uri)
            .then(|| vec![publish_diagnostics(&uri, self.diagnostics_for(&uri))])
    }

    fn did_close_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let params = message.get("params")?;
        let uri = document_uri(params)?.to_owned();

        self.documents.remove(&uri);

        Some(vec![publish_diagnostics(&uri, Vec::new())])
    }

    fn did_change_watched_files_notification(&mut self, message: &Value) -> Option<Vec<Value>> {
        let changes = watched_file_changes(message);
        let mut publications = Vec::new();
        let mut published = BTreeSet::new();

        for change in changes {
            match change.kind {
                FileChangeKind::Created | FileChangeKind::Changed => {
                    let diagnostics = if self.documents.contains_key(&change.uri) {
                        Some(self.diagnostics_for(&change.uri))
                    } else if is_tua_uri(&change.uri) {
                        Some(self.diagnostics_for_disk_file(&change.uri))
                    } else {
                        None
                    };

                    if let Some(diagnostics) = diagnostics {
                        push_diagnostics_publication(
                            &mut publications,
                            &mut published,
                            &change.uri,
                            diagnostics,
                        );
                    }
                }
                FileChangeKind::Deleted => {
                    self.documents.remove(&change.uri);
                    push_diagnostics_publication(
                        &mut publications,
                        &mut published,
                        &change.uri,
                        Vec::new(),
                    );
                }
            }
        }

        for uri in self.documents.keys() {
            push_diagnostics_publication(
                &mut publications,
                &mut published,
                uri,
                self.diagnostics_for(uri),
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

        None
    }

    fn diagnostics_for(&self, uri: &str) -> Vec<Value> {
        let Some(text) = self.documents.get(uri) else {
            return Vec::new();
        };
        let path = path_from_uri(uri);
        let diagnostics_config = self.diagnostics_config_for_path(&path);

        match check_tua_source_with_diagnostics_config(&path, text.clone(), &diagnostics_config) {
            Ok(()) => Vec::new(),
            Err(diagnostics) => diagnostics
                .into_iter()
                .map(lsp_diagnostic_from_core)
                .collect(),
        }
    }

    fn diagnostics_for_disk_file(&self, uri: &str) -> Vec<Value> {
        let path = path_from_uri(uri);
        let Ok(source) = fs::read_to_string(&path) else {
            return Vec::new();
        };
        let diagnostics_config = self.diagnostics_config_for_path(&path);

        match check_tua_source_with_diagnostics_config(&path, source, &diagnostics_config) {
            Ok(()) => Vec::new(),
            Err(diagnostics) => diagnostics
                .into_iter()
                .map(lsp_diagnostic_from_core)
                .collect(),
        }
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
                .map(|loaded| loaded.config.diagnostics)
                .unwrap_or_default();
        }

        if let Some(parent) = path.parent() {
            for ancestor in parent.ancestors() {
                match load_config_from_dir(ancestor) {
                    Ok(loaded) if loaded.found => return loaded.config.diagnostics,
                    Ok(_) | Err(_) => {}
                }
            }
        }

        DiagnosticsConfig::default()
    }

    fn hover_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let hover = hover_tua_source(request.path, request.text.to_owned(), position)
            .ok()
            .flatten()?;

        Some(lsp_hover_from_core(hover))
    }

    fn completion_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let Some(position) = request.position() else {
            return json!([]);
        };
        let completions = complete_tua_source(request.path, request.text.to_owned(), position)
            .unwrap_or_default();

        Value::Array(
            completions
                .into_iter()
                .map(|completion| lsp_completion_from_core(request.uri, completion))
                .collect(),
        )
    }

    fn completion_resolve_response(&self, message: Value) -> Value {
        message
            .get("params")
            .cloned()
            .map(lsp_resolved_completion_item)
            .unwrap_or(Value::Null)
    }

    fn signature_help_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let help = signature_help_tua_source(request.path, request.text.to_owned(), position)
            .ok()
            .flatten()?;

        Some(lsp_signature_help_from_core(help))
    }

    fn definition_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let definition = definition_tua_source(request.path, request.text.to_owned(), position)
            .ok()
            .flatten()?;

        Some(lsp_location_from_core(definition))
    }

    fn document_link_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let links = module_links_tua_source(request.path, request.text.to_owned());

        Value::Array(links.into_iter().map(lsp_document_link_from_core).collect())
    }

    fn document_highlight_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let Some(position) = request.position() else {
            return json!([]);
        };
        let highlights =
            document_highlights_tua_source(request.path, request.text.to_owned(), position)
                .unwrap_or_default();

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
        let symbols =
            document_symbols_tua_source(request.path, request.text.to_owned()).unwrap_or_default();

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
        let references = references_tua_source(
            request.path,
            request.text.to_owned(),
            position,
            include_declaration,
        )
        .unwrap_or_default();

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
        let target = prepare_rename_tua_source(request.path, request.text.to_owned(), position)
            .ok()
            .flatten()?;

        Some(lsp_prepare_rename_from_core(target))
    }

    fn rename_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let new_name = request.params.get("newName")?.as_str()?;
        let edit = rename_tua_source(request.path, request.text.to_owned(), position, new_name)
            .ok()
            .flatten()?;

        Some(lsp_workspace_edit(request.uri, edit))
    }

    fn inlay_hint_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let range = request.params.get("range").and_then(lsp_source_range);
        let hints =
            inlay_hints_tua_source(request.path, request.text.to_owned()).unwrap_or_default();

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
        let tokens =
            semantic_tokens_tua_source(request.path, request.text.to_owned()).unwrap_or_default();

        lsp_semantic_tokens_from_core(tokens)
    }

    fn workspace_symbol_response(&self, message: &Value) -> Value {
        let Some(params) = message.get("params") else {
            return json!([]);
        };
        let query = params.get("query").and_then(Value::as_str).unwrap_or("");
        let roots = if self.workspace_folders.is_empty() {
            vec![env::current_dir().unwrap_or_else(|_| PathBuf::from("."))]
        } else {
            self.workspace_folders.clone()
        };
        let symbols = roots
            .into_iter()
            .flat_map(|root| workspace_symbols_from_dir(root, query).unwrap_or_default())
            .collect::<Vec<_>>();

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
        let text = self.documents.get(uri)?;

        Some(DocumentRequest {
            params,
            uri,
            text,
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

fn is_tua_uri(uri: &str) -> bool {
    path_from_uri(uri)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "tua")
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
