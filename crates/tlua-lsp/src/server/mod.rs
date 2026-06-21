use std::collections::HashMap;
use std::env;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tlua_core::{
    check_tlua_source, complete_tlua_source, definition_tlua_source, document_symbols_tlua_source,
    hover_tlua_source, inlay_hints_tlua_source, prepare_rename_tlua_source, project_description,
    references_tlua_source, rename_tlua_source, semantic_tokens_tlua_source,
    workspace_symbols_from_dir, Completion, CompletionKind, Definition, Diagnostic,
    DiagnosticSeverity, DocumentSymbol, DocumentSymbolKind, Hover, InlayHint, InlayHintKind,
    Reference, RenameEdit, RenameTarget, SemanticToken, SemanticTokenKind, SemanticTokenModifier,
    SourcePosition, SourceSpan, TextEdit, WorkspaceSymbol, WorkspaceSymbolKind,
};

pub fn server_name() -> &'static str {
    "tlua-lsp"
}

pub fn server_description() -> String {
    format!("{} language server", project_description())
}

pub fn run_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();

    run(stdin.lock(), stdout.lock())
}

pub(crate) fn run(reader: impl Read, writer: impl Write) -> io::Result<()> {
    let mut server = LspServer::default();
    let mut reader = BufReader::new(reader);
    let mut writer = writer;

    while let Some(message) = read_message(&mut reader)? {
        if let Some(outbound) = server.handle_message(message) {
            for message in outbound {
                write_message(&mut writer, &message)?;
            }
        }

        if server.should_exit {
            break;
        }
    }

    Ok(())
}

#[derive(Default)]
struct LspServer {
    documents: HashMap<String, String>,
    root_dir: Option<PathBuf>,
    shutdown_requested: bool,
    should_exit: bool,
}

impl LspServer {
    fn handle_message(&mut self, message: Value) -> Option<Vec<Value>> {
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
                self.root_dir = initialize_root_dir(&message);

                response(
                    id,
                    json!({
                        "capabilities": {
                            "textDocumentSync": 1,
                            "hoverProvider": true,
                            "completionProvider": {
                                "triggerCharacters": [":", "."]
                            },
                            "definitionProvider": true,
                            "documentSymbolProvider": true,
                            "referencesProvider": true,
                            "workspaceSymbolProvider": true,
                            "renameProvider": {
                                "prepareProvider": true
                            },
                            "inlayHintProvider": {
                                "resolveProvider": false
                            },
                            "semanticTokensProvider": {
                                "legend": {
                                    "tokenTypes": SEMANTIC_TOKEN_TYPES,
                                    "tokenModifiers": SEMANTIC_TOKEN_MODIFIERS
                                },
                                "full": true
                            }
                        },
                        "serverInfo": {
                            "name": server_name(),
                            "version": tlua_core::VERSION
                        }
                    }),
                )
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
        let params = message.get("params")?;
        let uri = params.get("textDocument")?.get("uri")?.as_str()?;
        let position = params.get("position")?;
        let line = position.get("line")?.as_u64()? as usize;
        let character = position.get("character")?.as_u64()? as usize;
        let text = self.documents.get(uri)?;
        let path = path_from_uri(uri);
        let hover = hover_tlua_source(
            path,
            text.clone(),
            SourcePosition {
                line: line + 1,
                column: character + 1,
            },
        )
        .ok()
        .flatten()?;

        Some(lsp_hover_from_core(hover))
    }

    fn completion_response(&self, message: &Value) -> Value {
        let Some(params) = message.get("params") else {
            return json!([]);
        };
        let Some(uri) = params
            .get("textDocument")
            .and_then(|document| document.get("uri"))
            .and_then(Value::as_str)
        else {
            return json!([]);
        };
        let Some(position) = params.get("position") else {
            return json!([]);
        };
        let Some(line) = position.get("line").and_then(Value::as_u64) else {
            return json!([]);
        };
        let Some(character) = position.get("character").and_then(Value::as_u64) else {
            return json!([]);
        };
        let Some(text) = self.documents.get(uri) else {
            return json!([]);
        };
        let path = path_from_uri(uri);
        let completions = complete_tlua_source(
            path,
            text.clone(),
            SourcePosition {
                line: line as usize + 1,
                column: character as usize + 1,
            },
        )
        .unwrap_or_default();

        Value::Array(
            completions
                .into_iter()
                .map(lsp_completion_from_core)
                .collect(),
        )
    }

    fn definition_response(&self, message: &Value) -> Option<Value> {
        let params = message.get("params")?;
        let uri = params.get("textDocument")?.get("uri")?.as_str()?;
        let position = params.get("position")?;
        let line = position.get("line")?.as_u64()? as usize;
        let character = position.get("character")?.as_u64()? as usize;
        let text = self.documents.get(uri)?;
        let path = path_from_uri(uri);
        let definition = definition_tlua_source(
            path,
            text.clone(),
            SourcePosition {
                line: line + 1,
                column: character + 1,
            },
        )
        .ok()
        .flatten()?;

        Some(lsp_location_from_core(uri, definition))
    }

    fn document_symbol_response(&self, message: &Value) -> Value {
        let Some(params) = message.get("params") else {
            return json!([]);
        };
        let Some(uri) = params
            .get("textDocument")
            .and_then(|document| document.get("uri"))
            .and_then(Value::as_str)
        else {
            return json!([]);
        };
        let Some(text) = self.documents.get(uri) else {
            return json!([]);
        };
        let path = path_from_uri(uri);
        let symbols = document_symbols_tlua_source(path, text.clone()).unwrap_or_default();

        Value::Array(
            symbols
                .into_iter()
                .map(lsp_document_symbol_from_core)
                .collect(),
        )
    }

    fn references_response(&self, message: &Value) -> Value {
        let Some(params) = message.get("params") else {
            return json!([]);
        };
        let Some(uri) = params
            .get("textDocument")
            .and_then(|document| document.get("uri"))
            .and_then(Value::as_str)
        else {
            return json!([]);
        };
        let Some(position) = params.get("position") else {
            return json!([]);
        };
        let Some(line) = position.get("line").and_then(Value::as_u64) else {
            return json!([]);
        };
        let Some(character) = position.get("character").and_then(Value::as_u64) else {
            return json!([]);
        };
        let Some(text) = self.documents.get(uri) else {
            return json!([]);
        };
        let include_declaration = params
            .get("context")
            .and_then(|context| context.get("includeDeclaration"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let path = path_from_uri(uri);
        let references = references_tlua_source(
            path,
            text.clone(),
            SourcePosition {
                line: line as usize + 1,
                column: character as usize + 1,
            },
            include_declaration,
        )
        .unwrap_or_default();

        Value::Array(
            references
                .into_iter()
                .map(|reference| lsp_reference_location(uri, reference))
                .collect(),
        )
    }

    fn prepare_rename_response(&self, message: &Value) -> Option<Value> {
        let params = message.get("params")?;
        let uri = params.get("textDocument")?.get("uri")?.as_str()?;
        let position = params.get("position")?;
        let line = position.get("line")?.as_u64()? as usize;
        let character = position.get("character")?.as_u64()? as usize;
        let text = self.documents.get(uri)?;
        let path = path_from_uri(uri);
        let target = prepare_rename_tlua_source(
            path,
            text.clone(),
            SourcePosition {
                line: line + 1,
                column: character + 1,
            },
        )
        .ok()
        .flatten()?;

        Some(lsp_prepare_rename_from_core(target))
    }

    fn rename_response(&self, message: &Value) -> Option<Value> {
        let params = message.get("params")?;
        let uri = params.get("textDocument")?.get("uri")?.as_str()?;
        let position = params.get("position")?;
        let line = position.get("line")?.as_u64()? as usize;
        let character = position.get("character")?.as_u64()? as usize;
        let new_name = params.get("newName")?.as_str()?;
        let text = self.documents.get(uri)?;
        let path = path_from_uri(uri);
        let edit = rename_tlua_source(
            path,
            text.clone(),
            SourcePosition {
                line: line + 1,
                column: character + 1,
            },
            new_name,
        )
        .ok()
        .flatten()?;

        Some(lsp_workspace_edit(uri, edit))
    }

    fn inlay_hint_response(&self, message: &Value) -> Value {
        let Some(params) = message.get("params") else {
            return json!([]);
        };
        let Some(uri) = params
            .get("textDocument")
            .and_then(|document| document.get("uri"))
            .and_then(Value::as_str)
        else {
            return json!([]);
        };
        let Some(text) = self.documents.get(uri) else {
            return json!([]);
        };
        let range = params.get("range").and_then(lsp_source_range);
        let path = path_from_uri(uri);
        let hints = inlay_hints_tlua_source(path, text.clone()).unwrap_or_default();

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
        let Some(params) = message.get("params") else {
            return json!({ "data": [] });
        };
        let Some(uri) = params
            .get("textDocument")
            .and_then(|document| document.get("uri"))
            .and_then(Value::as_str)
        else {
            return json!({ "data": [] });
        };
        let Some(text) = self.documents.get(uri) else {
            return json!({ "data": [] });
        };
        let path = path_from_uri(uri);
        let tokens = semantic_tokens_tlua_source(path, text.clone()).unwrap_or_default();

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
}

fn response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn error_response(id: Value, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn publish_diagnostics(uri: &str, diagnostics: Vec<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {
            "uri": uri,
            "diagnostics": diagnostics
        }
    })
}

fn lsp_diagnostic_from_core(diagnostic: Diagnostic) -> Value {
    let start_line = diagnostic.line.saturating_sub(1);
    let start_character = diagnostic.column.saturating_sub(1);

    json!({
        "range": {
            "start": {
                "line": start_line,
                "character": start_character
            },
            "end": {
                "line": start_line,
                "character": start_character + 1
            }
        },
        "severity": lsp_severity(diagnostic.severity),
        "code": diagnostic.code.map(|code| code.to_string()),
        "source": "tlua",
        "message": diagnostic.message
    })
}

fn lsp_severity(severity: DiagnosticSeverity) -> u8 {
    match severity {
        DiagnosticSeverity::Error => 1,
        DiagnosticSeverity::Warning => 2,
        DiagnosticSeverity::Info => 3,
        DiagnosticSeverity::Hint => 4,
    }
}

fn lsp_hover_from_core(hover: Hover) -> Value {
    let mut result = json!({
        "contents": {
            "kind": "markdown",
            "value": format!("```tlua\n{}\n```", hover.contents)
        }
    });

    if let Some(span) = hover.span {
        result["range"] = lsp_range(span);
    }

    result
}

fn lsp_completion_from_core(completion: Completion) -> Value {
    let mut item = json!({
        "label": completion.label,
        "kind": lsp_completion_kind(completion.kind)
    });

    if let Some(detail) = completion.detail {
        item["detail"] = json!(detail);
    }

    item
}

fn lsp_completion_kind(kind: CompletionKind) -> u8 {
    match kind {
        CompletionKind::TypeAlias => 25,
        CompletionKind::Field => 5,
    }
}

fn lsp_location_from_core(uri: &str, definition: Definition) -> Value {
    lsp_location(uri, definition.span)
}

fn lsp_reference_location(uri: &str, reference: Reference) -> Value {
    lsp_location(uri, reference.span)
}

fn lsp_prepare_rename_from_core(target: RenameTarget) -> Value {
    json!({
        "range": lsp_range(target.span),
        "placeholder": target.placeholder
    })
}

fn lsp_workspace_edit(uri: &str, edit: RenameEdit) -> Value {
    json!({
        "changes": {
            uri: edit.edits.into_iter().map(lsp_text_edit).collect::<Vec<_>>()
        }
    })
}

fn lsp_text_edit(edit: TextEdit) -> Value {
    json!({
        "range": lsp_range(edit.span),
        "newText": edit.new_text
    })
}

fn lsp_workspace_symbol_from_core(symbol: WorkspaceSymbol) -> Value {
    let mut item = json!({
        "name": symbol.name,
        "kind": lsp_workspace_symbol_kind(symbol.kind),
        "location": {
            "uri": uri_from_path(&symbol.path),
            "range": lsp_range(symbol.selection_range)
        }
    });

    if let Some(container_name) = symbol.container_name {
        item["containerName"] = json!(container_name);
    }

    item
}

fn lsp_workspace_symbol_kind(kind: WorkspaceSymbolKind) -> u8 {
    match kind {
        WorkspaceSymbolKind::Module => 2,
        WorkspaceSymbolKind::Function => 12,
        WorkspaceSymbolKind::TypeAlias => 23,
    }
}

const SEMANTIC_TOKEN_TYPES: [&str; 7] = [
    "typeAlias",
    "function",
    "variable",
    "parameter",
    "property",
    "builtinType",
    "unresolvedReference",
];

const SEMANTIC_TOKEN_MODIFIERS: [&str; 1] = ["declaration"];

fn lsp_semantic_tokens_from_core(tokens: Vec<SemanticToken>) -> Value {
    let mut data = Vec::with_capacity(tokens.len() * 5);
    let mut previous_line = 0usize;
    let mut previous_start = 0usize;

    for token in tokens {
        if token.span.start.line != token.span.end.line {
            continue;
        }

        let line = token.span.start.line.saturating_sub(1);
        let start = token.span.start.column.saturating_sub(1);
        let length = token
            .span
            .end
            .column
            .saturating_sub(token.span.start.column);
        let delta_line = line.saturating_sub(previous_line);
        let delta_start = if delta_line == 0 {
            start.saturating_sub(previous_start)
        } else {
            start
        };

        data.extend([
            delta_line,
            delta_start,
            length,
            semantic_token_type_index(token.kind),
            semantic_token_modifier_bits(&token.modifiers),
        ]);

        previous_line = line;
        previous_start = start;
    }

    json!({ "data": data })
}

fn semantic_token_type_index(kind: SemanticTokenKind) -> usize {
    match kind {
        SemanticTokenKind::TypeAlias => 0,
        SemanticTokenKind::Function => 1,
        SemanticTokenKind::Variable => 2,
        SemanticTokenKind::Parameter => 3,
        SemanticTokenKind::Property => 4,
        SemanticTokenKind::BuiltinType => 5,
        SemanticTokenKind::UnresolvedReference => 6,
    }
}

fn semantic_token_modifier_bits(modifiers: &[SemanticTokenModifier]) -> usize {
    modifiers.iter().fold(0usize, |bits, modifier| {
        bits | semantic_token_modifier_bit(*modifier)
    })
}

fn semantic_token_modifier_bit(modifier: SemanticTokenModifier) -> usize {
    match modifier {
        SemanticTokenModifier::Declaration => 1 << 0,
    }
}

fn lsp_inlay_hint_from_core(hint: InlayHint) -> Value {
    let mut item = json!({
        "position": lsp_position(hint.position),
        "label": hint.label,
        "kind": lsp_inlay_hint_kind(hint.kind)
    });

    if hint.kind == InlayHintKind::Parameter {
        item["paddingRight"] = json!(true);
    }

    item
}

fn lsp_inlay_hint_kind(kind: InlayHintKind) -> u8 {
    match kind {
        InlayHintKind::Type => 1,
        InlayHintKind::Parameter => 2,
    }
}

fn lsp_location(uri: &str, span: SourceSpan) -> Value {
    json!({
        "uri": uri,
        "range": lsp_range(span)
    })
}

fn lsp_document_symbol_from_core(symbol: DocumentSymbol) -> Value {
    let children = symbol
        .children
        .into_iter()
        .map(lsp_document_symbol_from_core)
        .collect::<Vec<_>>();
    let mut item = json!({
        "name": symbol.name,
        "kind": lsp_symbol_kind(symbol.kind),
        "range": lsp_range(symbol.range),
        "selectionRange": lsp_range(symbol.selection_range),
    });

    if let Some(detail) = symbol.detail {
        item["detail"] = json!(detail);
    }

    if !children.is_empty() {
        item["children"] = Value::Array(children);
    }

    item
}

fn lsp_symbol_kind(kind: DocumentSymbolKind) -> u8 {
    match kind {
        DocumentSymbolKind::Field => 8,
        DocumentSymbolKind::Function => 12,
        DocumentSymbolKind::Local | DocumentSymbolKind::Parameter => 13,
        DocumentSymbolKind::ModuleExports => 19,
        DocumentSymbolKind::TypeAlias => 23,
    }
}

fn lsp_range(span: SourceSpan) -> Value {
    json!({
        "start": {
            "line": span.start.line.saturating_sub(1),
            "character": span.start.column.saturating_sub(1)
        },
        "end": {
            "line": span.end.line.saturating_sub(1),
            "character": span.end.column.saturating_sub(1)
        }
    })
}

fn lsp_position(position: SourcePosition) -> Value {
    json!({
        "line": position.line.saturating_sub(1),
        "character": position.column.saturating_sub(1)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceRange {
    start: SourcePosition,
    end: SourcePosition,
}

impl SourceRange {
    fn contains(self, position: SourcePosition) -> bool {
        position_key(self.start) <= position_key(position)
            && position_key(position) <= position_key(self.end)
    }
}

fn lsp_source_range(range: &Value) -> Option<SourceRange> {
    Some(SourceRange {
        start: source_position_from_lsp(range.get("start")?)?,
        end: source_position_from_lsp(range.get("end")?)?,
    })
}

fn source_position_from_lsp(position: &Value) -> Option<SourcePosition> {
    Some(SourcePosition {
        line: position.get("line")?.as_u64()? as usize + 1,
        column: position.get("character")?.as_u64()? as usize + 1,
    })
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn initialize_root_dir(message: &Value) -> Option<PathBuf> {
    let params = message.get("params")?;

    params
        .get("rootUri")
        .and_then(Value::as_str)
        .map(path_from_uri)
        .or_else(|| {
            params
                .get("workspaceFolders")
                .and_then(Value::as_array)
                .and_then(|folders| folders.first())
                .and_then(|folder| folder.get("uri"))
                .and_then(Value::as_str)
                .map(path_from_uri)
        })
        .or_else(|| {
            params
                .get("rootPath")
                .and_then(Value::as_str)
                .map(PathBuf::from)
        })
}

fn uri_from_path(path: &Path) -> String {
    format!("file://{}", percent_encode_path(path))
}

fn path_from_uri(uri: &str) -> PathBuf {
    let Some(raw_path) = uri.strip_prefix("file://") else {
        return PathBuf::from(uri);
    };
    let raw_path = raw_path.strip_prefix("localhost").unwrap_or(raw_path);

    PathBuf::from(percent_decode(raw_path))
}

fn percent_encode_path(path: &Path) -> String {
    let mut encoded = String::new();

    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = &input[index + 1..index + 3];

            if let Ok(value) = u8::from_str_radix(hex, 16) {
                output.push(value);
                index += 3;
                continue;
            }
        }

        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&output).into_owned()
}

fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut content_length = None;
    let mut line = String::new();

    loop {
        line.clear();

        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);

        if trimmed.is_empty() {
            break;
        }

        if let Some((name, value)) = trimmed.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = Some(value.trim().parse::<usize>().map_err(|source| {
                    io::Error::new(io::ErrorKind::InvalidData, source.to_string())
                })?);
            }
        }
    }

    let Some(content_length) = content_length else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing Content-Length header",
        ));
    };
    let mut body = vec![0; content_length];

    reader.read_exact(&mut body)?;

    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|source| io::Error::new(io::ErrorKind::InvalidData, source.to_string()))
}

fn write_message(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)
        .map_err(|source| io::Error::new(io::ErrorKind::InvalidData, source.to_string()))?;

    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

#[cfg(test)]
mod tests;
