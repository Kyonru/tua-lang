use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::PathBuf;

use serde_json::{json, Value};
use tlua_core::{
    check_tlua_source, complete_tlua_source, definition_tlua_source, document_symbols_tlua_source,
    hover_tlua_source, project_description, Completion, CompletionKind, Definition, Diagnostic,
    DiagnosticSeverity, DocumentSymbol, DocumentSymbolKind, Hover, SourcePosition, SourceSpan,
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
            "initialize" => response(
                id,
                json!({
                    "capabilities": {
                        "textDocumentSync": 1,
                        "hoverProvider": true,
                        "completionProvider": {
                            "triggerCharacters": [":", "."]
                        },
                        "definitionProvider": true,
                        "documentSymbolProvider": true
                    },
                    "serverInfo": {
                        "name": server_name(),
                        "version": tlua_core::VERSION
                    }
                }),
            ),
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
    json!({
        "uri": uri,
        "range": lsp_range(definition.span)
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

fn path_from_uri(uri: &str) -> PathBuf {
    let Some(raw_path) = uri.strip_prefix("file://") else {
        return PathBuf::from(uri);
    };
    let raw_path = raw_path.strip_prefix("localhost").unwrap_or(raw_path);

    PathBuf::from(percent_decode(raw_path))
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
