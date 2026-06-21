use std::env;
use std::path::PathBuf;

use serde_json::{json, Value};
use tlua_core::{
    check_tlua_source, complete_tlua_source, definition_tlua_source, document_symbols_tlua_source,
    hover_tlua_source, inlay_hints_tlua_source, prepare_rename_tlua_source, references_tlua_source,
    rename_tlua_source, semantic_tokens_tlua_source, workspace_symbols_from_dir, SourcePosition,
};

use super::protocol::{
    error_response, lsp_completion_from_core, lsp_diagnostic_from_core,
    lsp_document_symbol_from_core, lsp_hover_from_core, lsp_inlay_hint_from_core,
    lsp_location_from_core, lsp_prepare_rename_from_core, lsp_reference_location,
    lsp_semantic_tokens_from_core, lsp_source_range, lsp_workspace_edit,
    lsp_workspace_symbol_from_core, publish_diagnostics, response, source_position_from_lsp,
};
use super::uri::{path_from_uri, root_dir_from_initialize};
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
                self.root_dir = root_dir_from_initialize(&message);

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
            "textDocument/definition" => response(
                id,
                self.definition_response(&message).unwrap_or(Value::Null),
            ),
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
            "initialized" => None,
            "exit" => {
                self.should_exit = true;

                None
            }
            "textDocument/didOpen" => {
                let text_document = message.get("params")?.get("textDocument")?;
                let uri = text_document.get("uri")?.as_str()?.to_owned();
                let text = text_document.get("text")?.as_str()?.to_owned();

                self.documents.insert(uri.clone(), text);

                Some(vec![publish_diagnostics(&uri, self.diagnostics_for(&uri))])
            }
            "textDocument/didChange" => {
                let params = message.get("params")?;
                let uri = params.get("textDocument")?.get("uri")?.as_str()?.to_owned();
                let text = params
                    .get("contentChanges")?
                    .as_array()?
                    .last()?
                    .get("text")?
                    .as_str()?
                    .to_owned();

                self.documents.insert(uri.clone(), text);

                Some(vec![publish_diagnostics(&uri, self.diagnostics_for(&uri))])
            }
            _ => None,
        }
    }

    fn diagnostics_for(&self, uri: &str) -> Vec<Value> {
        let Some(text) = self.documents.get(uri) else {
            return Vec::new();
        };
        let path = path_from_uri(uri);

        match check_tlua_source(&path, text.clone()) {
            Ok(()) => Vec::new(),
            Err(diagnostics) => diagnostics
                .into_iter()
                .map(lsp_diagnostic_from_core)
                .collect(),
        }
    }

    fn hover_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let hover = hover_tlua_source(request.path, request.text.to_owned(), position)
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
        let completions = complete_tlua_source(request.path, request.text.to_owned(), position)
            .unwrap_or_default();

        Value::Array(
            completions
                .into_iter()
                .map(lsp_completion_from_core)
                .collect(),
        )
    }

    fn definition_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let definition = definition_tlua_source(request.path, request.text.to_owned(), position)
            .ok()
            .flatten()?;

        Some(lsp_location_from_core(request.uri, definition))
    }

    fn document_symbol_response(&self, message: &Value) -> Value {
        let Some(request) = self.document_request(message) else {
            return json!([]);
        };
        let symbols =
            document_symbols_tlua_source(request.path, request.text.to_owned()).unwrap_or_default();

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
        let references = references_tlua_source(
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
        let target = prepare_rename_tlua_source(request.path, request.text.to_owned(), position)
            .ok()
            .flatten()?;

        Some(lsp_prepare_rename_from_core(target))
    }

    fn rename_response(&self, message: &Value) -> Option<Value> {
        let request = self.document_request(message)?;
        let position = request.position()?;
        let new_name = request.params.get("newName")?.as_str()?;
        let edit = rename_tlua_source(request.path, request.text.to_owned(), position, new_name)
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
            inlay_hints_tlua_source(request.path, request.text.to_owned()).unwrap_or_default();

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
            semantic_tokens_tlua_source(request.path, request.text.to_owned()).unwrap_or_default();

        lsp_semantic_tokens_from_core(tokens)
    }

    fn workspace_symbol_response(&self, message: &Value) -> Value {
        let Some(params) = message.get("params") else {
            return json!([]);
        };
        let query = params.get("query").and_then(Value::as_str).unwrap_or("");
        let root_dir = self
            .root_dir
            .clone()
            .or_else(|| env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let symbols = workspace_symbols_from_dir(root_dir, query).unwrap_or_default();

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
