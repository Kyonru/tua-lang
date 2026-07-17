use super::*;
use serde_json::{json, Value};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tua_core::{CompletionAdaptiveStats, Diagnostic, DiagnosticSeverity, Fixture, SourcePosition};

mod code_actions;
mod completion;
mod diagnostics;
mod document_features;
mod formatting;
mod hover;
mod inlay_hints;
mod inspector;
mod navigation;
mod recovery;
mod scheduling;
mod semantic_tokens;
mod server_protocol;
mod signature_help;
mod workspace;

fn run_messages(messages: Vec<Value>) -> Vec<Value> {
    let input = encode_messages(messages);
    let mut output = Vec::new();

    run(Cursor::new(input), &mut output).expect("server should run");

    read_output_messages(&output)
}

fn run_chunked_messages(chunks: Vec<(Duration, Vec<Value>)>) -> Vec<Value> {
    let chunks = chunks
        .into_iter()
        .map(|(delay, messages)| (delay, encode_messages(messages)))
        .collect();
    let mut output = Vec::new();

    run(DelayedChunks::new(chunks), &mut output).expect("server should run");

    read_output_messages(&output)
}

fn encode_messages(messages: Vec<Value>) -> Vec<u8> {
    let mut input = Vec::new();

    for message in messages {
        let body = serde_json::to_vec(&message).expect("message should serialize");

        input.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
        input.extend_from_slice(&body);
    }

    input
}

struct DelayedChunks {
    chunks: std::collections::VecDeque<(Duration, Vec<u8>)>,
    current: Cursor<Vec<u8>>,
}

impl DelayedChunks {
    fn new(chunks: Vec<(Duration, Vec<u8>)>) -> Self {
        Self {
            chunks: chunks.into(),
            current: Cursor::new(Vec::new()),
        }
    }
}

impl Read for DelayedChunks {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let read = self.current.read(buffer)?;
            if read > 0 {
                return Ok(read);
            }
            let Some((delay, chunk)) = self.chunks.pop_front() else {
                return Ok(0);
            };

            std::thread::sleep(delay);
            self.current = Cursor::new(chunk);
        }
    }
}

struct TestServer {
    server: LspServer,
    output: Vec<Value>,
}

impl TestServer {
    fn new() -> Self {
        Self {
            server: LspServer::default(),
            output: Vec::new(),
        }
    }

    fn send(&mut self, message: Value) {
        self.send_without_drain(message);

        self.output
            .extend(self.server.drain_all_pending_slow_diagnostics());
    }

    fn send_without_drain(&mut self, message: Value) {
        if let Some(messages) = self.server.handle_message(message) {
            self.output.extend(messages);
        }
    }

    fn drain_due_diagnostics(&mut self, now: Instant) {
        self.output
            .extend(self.server.drain_due_slow_diagnostics(now));
    }

    fn next_slow_diagnostics_due(&self) -> Option<Instant> {
        self.server.next_slow_diagnostics_due()
    }

    fn output(&self) -> &[Value] {
        &self.output
    }
}

fn did_open(uri: &str, source: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": uri,
                "languageId": "tua",
                "version": 1,
                "text": source
            }
        }
    })
}

fn did_close(uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didClose",
        "params": {
            "textDocument": {
                "uri": uri
            }
        }
    })
}

fn slow_background_request(id: u64, uri: Option<&str>, delay_ms: u64) -> Value {
    let mut params = json!({ "delayMs": delay_ms });
    if let Some(uri) = uri {
        params["textDocument"] = json!({ "uri": uri });
    }

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tua/testSlowBackground",
        "params": params
    })
}

fn response_index_with_id(messages: &[Value], id: u64) -> usize {
    messages
        .iter()
        .position(|message| message.get("id").and_then(Value::as_u64) == Some(id))
        .unwrap_or_else(|| panic!("response {id} should exist: {messages:#?}"))
}

fn did_change(uri: &str, source: &str, version: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": {
                "uri": uri,
                "version": version
            },
            "contentChanges": [
                {
                    "text": source
                }
            ]
        }
    })
}

fn initialize_with_root(id: u64, root_uri: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "rootUri": root_uri
        }
    })
}

fn initialize_with_completion_label_details(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "capabilities": completion_label_details_capabilities()
        }
    })
}

fn initialize_with_root_and_completion_label_details(id: u64, root_uri: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "rootUri": root_uri,
            "capabilities": completion_label_details_capabilities()
        }
    })
}

fn completion_label_details_capabilities() -> Value {
    json!({
        "textDocument": {
            "completion": {
                "completionItem": {
                    "labelDetailsSupport": true
                }
            }
        }
    })
}

fn initialize_with_pull_diagnostics(id: u64, root_uri: String, refresh_support: bool) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "diagnostic": {
                        "dynamicRegistration": false,
                        "relatedDocumentSupport": true
                    }
                },
                "workspace": {
                    "diagnostics": {
                        "refreshSupport": refresh_support
                    }
                }
            }
        }
    })
}

fn initialize_with_workspace_folders<const N: usize>(id: u64, roots: [String; N]) -> Value {
    let folders = roots
        .into_iter()
        .enumerate()
        .map(|(index, uri)| {
            json!({
                "uri": uri,
                "name": format!("workspace-{index}")
            })
        })
        .collect::<Vec<_>>();

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "workspaceFolders": folders
        }
    })
}

fn initialize_with_trace_options(id: u64, initialization_options: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "initializationOptions": initialization_options
        }
    })
}

fn set_trace_notification(settings: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "tua/setTrace",
        "params": settings
    })
}

fn workspace_folders_changed<const A: usize, const R: usize>(
    added: [String; A],
    removed: [String; R],
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "workspace/didChangeWorkspaceFolders",
        "params": {
            "event": {
                "added": workspace_folder_items(added),
                "removed": workspace_folder_items(removed)
            }
        }
    })
}

fn watched_files_changed<const N: usize>(changes: [(&str, u8); N]) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "workspace/didChangeWatchedFiles",
        "params": {
            "changes": changes
                .into_iter()
                .map(|(uri, kind)| {
                    json!({
                        "uri": uri,
                        "type": kind
                    })
                })
                .collect::<Vec<_>>()
        }
    })
}

fn will_rename_files_request<const N: usize>(id: u64, files: [(String, String); N]) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "workspace/willRenameFiles",
        "params": {
            "files": files
                .into_iter()
                .map(|(old_uri, new_uri)| {
                    json!({
                        "oldUri": old_uri,
                        "newUri": new_uri
                    })
                })
                .collect::<Vec<_>>()
        }
    })
}

fn did_change_configuration(settings: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "workspace/didChangeConfiguration",
        "params": {
            "settings": {
                "tua": settings
            }
        }
    })
}

fn workspace_folder_items<const N: usize>(roots: [String; N]) -> Vec<Value> {
    roots
        .into_iter()
        .enumerate()
        .map(|(index, uri)| {
            json!({
                "uri": uri,
                "name": format!("workspace-{index}")
            })
        })
        .collect()
}

fn hover_request(id: u64, uri: &str, source: &str, needle: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/hover",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_of(source, needle)
        }
    })
}

fn completion_request(id: u64, uri: &str, source: &str, needle: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/completion",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_after(source, needle)
        }
    })
}

fn invoked_completion_request(id: u64, uri: &str, source: &str, needle: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/completion",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_after(source, needle),
            "context": {
                "triggerKind": 1
            }
        }
    })
}

fn trigger_character_completion_request(
    id: u64,
    uri: &str,
    source: &str,
    needle: &str,
    trigger: &str,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/completion",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_after(source, needle),
            "context": {
                "triggerKind": 2,
                "triggerCharacter": trigger
            }
        }
    })
}

fn completion_resolve_request(id: u64, item: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "completionItem/resolve",
        "params": item
    })
}

fn formatting_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/formatting",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "options": {
                "tabSize": 2,
                "insertSpaces": true
            }
        }
    })
}

fn range_formatting_request(id: u64, uri: &str, start: Value, end: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/rangeFormatting",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "range": {
                "start": start,
                "end": end
            },
            "options": {
                "tabSize": 2,
                "insertSpaces": true
            }
        }
    })
}

fn on_type_formatting_request(id: u64, uri: &str, position: Value, trigger: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/onTypeFormatting",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": position,
            "ch": trigger,
            "options": {
                "tabSize": 2,
                "insertSpaces": true
            }
        }
    })
}

fn signature_help_request(id: u64, uri: &str, source: &str, needle: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/signatureHelp",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_after(source, needle)
        }
    })
}

fn signature_help_trigger_request(
    id: u64,
    uri: &str,
    source: &str,
    needle: &str,
    trigger: &str,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/signatureHelp",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_after(source, needle),
            "context": {
                "triggerKind": 2,
                "triggerCharacter": trigger,
                "isRetrigger": false
            }
        }
    })
}

fn signature_help_retrigger_request(
    id: u64,
    uri: &str,
    source: &str,
    needle: &str,
    trigger: &str,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/signatureHelp",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_after(source, needle),
            "context": {
                "triggerKind": 2,
                "triggerCharacter": trigger,
                "isRetrigger": true
            }
        }
    })
}

fn definition_request(id: u64, uri: &str, source: &str, needle: &str, offset: usize) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/definition",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset)
        }
    })
}

fn definition_request_at(id: u64, uri: &str, position: SourcePosition) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/definition",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position(position)
        }
    })
}

fn type_definition_request_at(id: u64, uri: &str, position: SourcePosition) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/typeDefinition",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position(position)
        }
    })
}

fn implementation_request_at(id: u64, uri: &str, position: SourcePosition) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/implementation",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position(position)
        }
    })
}

fn builtin_document_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tua/builtinDocument",
        "params": {
            "uri": uri
        }
    })
}

fn cache_stats_request(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tua/cacheStats",
        "params": null
    })
}

fn reset_completion_stats_request(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tua/resetCompletionStats",
        "params": null
    })
}

fn signal_graph_request(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tua/signalGraph",
        "params": null
    })
}

fn inspector_request(id: u64, uri: &str, position: Option<Value>) -> Value {
    let mut params = json!({
        "textDocument": {
            "uri": uri
        }
    });

    if let Some(position) = position {
        params["position"] = position;
    }

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tua/inspect",
        "params": params
    })
}

fn text_document_diagnostic_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/diagnostic",
        "params": {
            "textDocument": {
                "uri": uri
            }
        }
    })
}

fn prepare_call_hierarchy_request(
    id: u64,
    uri: &str,
    source: &str,
    needle: &str,
    offset: usize,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/prepareCallHierarchy",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset)
        }
    })
}

fn call_hierarchy_incoming_request(id: u64, item: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "callHierarchy/incomingCalls",
        "params": {
            "item": item
        }
    })
}

fn call_hierarchy_outgoing_request(id: u64, item: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "callHierarchy/outgoingCalls",
        "params": {
            "item": item
        }
    })
}

fn document_symbol_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/documentSymbol",
        "params": {
            "textDocument": {
                "uri": uri
            }
        }
    })
}

fn selection_range_request(id: u64, uri: &str, source: &str, needle: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/selectionRange",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "positions": [lsp_position_of(source, needle)]
        }
    })
}

fn folding_range_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/foldingRange",
        "params": {
            "textDocument": {
                "uri": uri
            }
        }
    })
}

fn document_link_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/documentLink",
        "params": {
            "textDocument": {
                "uri": uri
            }
        }
    })
}

fn document_highlight_request(
    id: u64,
    uri: &str,
    source: &str,
    needle: &str,
    offset: usize,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/documentHighlight",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset)
        }
    })
}

fn code_action_request(
    id: u64,
    uri: &str,
    start_line: usize,
    start_character: usize,
    end_line: usize,
    end_character: usize,
    diagnostics: Vec<Value>,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/codeAction",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "range": {
                "start": {
                    "line": start_line,
                    "character": start_character
                },
                "end": {
                    "line": end_line,
                    "character": end_character
                }
            },
            "context": {
                "diagnostics": diagnostics
            }
        }
    })
}

fn code_action_request_with_only(
    id: u64,
    uri: &str,
    only: &[&str],
    diagnostics: Vec<Value>,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/codeAction",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "range": {
                "start": {
                    "line": 0,
                    "character": 0
                },
                "end": {
                    "line": 999,
                    "character": 0
                }
            },
            "context": {
                "diagnostics": diagnostics,
                "only": only
            }
        }
    })
}

fn code_lens_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/codeLens",
        "params": {
            "textDocument": {
                "uri": uri
            }
        }
    })
}

fn code_lens_resolve_request(id: u64, lens: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "codeLens/resolve",
        "params": lens
    })
}

fn references_request(
    id: u64,
    uri: &str,
    source: &str,
    needle: &str,
    offset: usize,
    include_declaration: bool,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/references",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset),
            "context": {
                "includeDeclaration": include_declaration
            }
        }
    })
}

fn prepare_rename_request(id: u64, uri: &str, source: &str, needle: &str, offset: usize) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/prepareRename",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset)
        }
    })
}

fn rename_request(
    id: u64,
    uri: &str,
    source: &str,
    needle: &str,
    offset: usize,
    new_name: &str,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/rename",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset),
            "newName": new_name
        }
    })
}

fn inlay_hint_request(
    id: u64,
    uri: &str,
    start_line: usize,
    start_character: usize,
    end_line: usize,
    end_character: usize,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/inlayHint",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "range": {
                "start": {
                    "line": start_line,
                    "character": start_character
                },
                "end": {
                    "line": end_line,
                    "character": end_character
                }
            }
        }
    })
}

fn semantic_tokens_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/semanticTokens/full",
        "params": {
            "textDocument": {
                "uri": uri
            }
        }
    })
}

fn semantic_tokens_delta_request(id: u64, uri: &str, previous_result_id: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/semanticTokens/full/delta",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "previousResultId": previous_result_id
        }
    })
}

fn semantic_tokens_range_request(id: u64, uri: &str, start: Value, end: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/semanticTokens/range",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "range": {
                "start": start,
                "end": end
            }
        }
    })
}

fn workspace_symbol_request(id: u64, query: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "workspace/symbol",
        "params": {
            "query": query
        }
    })
}

fn lsp_position_of(source: &str, needle: &str) -> Value {
    let index = source.find(needle).expect("needle should exist in source");
    let before = &source[..index];
    let line = before.bytes().filter(|byte| *byte == b'\n').count();
    let character = before
        .rsplit_once('\n')
        .map(|(_, line)| line.len())
        .unwrap_or(before.len());

    json!({
        "line": line,
        "character": character
    })
}

fn lsp_position_with_offset(source: &str, needle: &str, offset: usize) -> Value {
    let mut position = lsp_position_of(source, needle);
    position["character"] = json!(
        position["character"]
            .as_u64()
            .expect("character should be numeric")
            + offset as u64
    );

    position
}

fn lsp_position_after(source: &str, needle: &str) -> Value {
    let mut position = lsp_position_of(source, needle);
    position["character"] = json!(
        position["character"]
            .as_u64()
            .expect("character should be numeric")
            + needle.chars().count() as u64
    );

    position
}

fn lsp_position(position: SourcePosition) -> Value {
    json!({
        "line": position.line.saturating_sub(1),
        "character": position.column.saturating_sub(1)
    })
}

fn read_output_messages(mut output: &[u8]) -> Vec<Value> {
    let mut messages = Vec::new();

    while !output.is_empty() {
        let header_end = output
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("output should contain a header terminator");
        let header = std::str::from_utf8(&output[..header_end]).expect("header should be UTF-8");
        let content_length = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;

                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("valid content length"))
            })
            .expect("content length should exist");
        let body_start = header_end + 4;
        let body_end = body_start + content_length;
        let message =
            serde_json::from_slice(&output[body_start..body_end]).expect("valid JSON response");

        messages.push(message);
        output = &output[body_end..];
    }

    messages
}

fn published_diagnostics(output: &[Value], uri: &str) -> Vec<Value> {
    output
        .iter()
        .find(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == uri
        })
        .and_then(|message| message["params"]["diagnostics"].as_array())
        .cloned()
        .expect("diagnostics publication should exist")
}

fn diagnostics_publications<'a>(output: &'a [Value], uri: &str) -> Vec<&'a Value> {
    output
        .iter()
        .filter(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == uri
        })
        .collect()
}

fn last_published_diagnostics(output: &[Value], uri: &str) -> Vec<Value> {
    diagnostics_publications(output, uri)
        .last()
        .and_then(|message| message["params"]["diagnostics"].as_array())
        .cloned()
        .expect("diagnostics publication should exist")
}

fn document_diagnostic_items(output: &[Value], id: u64) -> Vec<Value> {
    let response = response_with_id(output, id);

    assert_eq!(response["result"]["kind"], "full");

    response["result"]["items"]
        .as_array()
        .expect("document diagnostic items should be an array")
        .clone()
}

fn diagnostic_refresh_requests(output: &[Value]) -> Vec<&Value> {
    output
        .iter()
        .filter(|message| message["method"] == "workspace/diagnostic/refresh")
        .collect()
}

fn diagnostic_codes(diagnostics: &[Value]) -> Vec<String> {
    diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic["code"].as_str().map(str::to_owned))
        .collect()
}

fn assert_hover_contains(output: &[Value], id: u64, expected: &str) {
    let contents = hover_contents(output, id);

    assert!(
        contents.contains(expected),
        "expected hover contents to contain {expected:?}, got {contents:?}"
    );
}

fn hover_contents(output: &[Value], id: u64) -> &str {
    let response = response_with_id(output, id);

    response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be a string")
}

fn assert_completion_labels<const N: usize>(output: &[Value], id: u64, expected: [&str; N]) {
    let labels = completion_labels(output, id);

    assert_eq!(labels, expected);
}

fn completion_labels(output: &[Value], id: u64) -> Vec<&str> {
    response_with_id(output, id)["result"]
        .as_array()
        .expect("completion result should be an array")
        .iter()
        .map(|item| {
            item["label"]
                .as_str()
                .expect("completion label should be a string")
        })
        .collect()
}

fn completion_with_label(output: &[Value], id: u64, label: &str) -> Value {
    response_with_id(output, id)["result"]
        .as_array()
        .expect("completion result should be an array")
        .iter()
        .find(|item| item["label"] == label)
        .unwrap_or_else(|| panic!("completion {label:?} should exist"))
        .clone()
}

fn assert_signature_label(output: &[Value], id: u64, expected: &str) {
    let response = response_with_id(output, id);

    assert_eq!(response["result"]["signatures"][0]["label"], expected);
}

fn assert_definition_text(output: &[Value], id: u64, uri: &str, source: &str, expected: &str) {
    let response = response_with_id(output, id);

    assert_eq!(
        response["result"]["uri"], uri,
        "location URI mismatch for response id {id}"
    );

    let range = &response["result"]["range"];
    let line = range["start"]["line"]
        .as_u64()
        .expect("definition start line should be numeric") as usize;
    let start = range["start"]["character"]
        .as_u64()
        .expect("definition start character should be numeric") as usize;
    let end = range["end"]["character"]
        .as_u64()
        .expect("definition end character should be numeric") as usize;
    let text = source
        .split('\n')
        .nth(line)
        .expect("definition line should exist");

    assert_eq!(&text[start..end], expected);
}

fn assert_first_location_text(output: &[Value], id: u64, uri: &str, source: &str, expected: &str) {
    let response = response_with_id(output, id);
    let locations = response["result"]
        .as_array()
        .expect("implementation result should be an array");
    let location = locations
        .first()
        .expect("implementation result should contain a location");

    assert_eq!(location["uri"], uri);
    assert_eq!(range_text(source, &location["range"]), expected);
}

fn assert_empty_locations(output: &[Value], id: u64) {
    let response = response_with_id(output, id);
    let locations = response["result"]
        .as_array()
        .expect("implementation result should be an array");

    assert!(
        locations.is_empty(),
        "expected empty locations, got {locations:#?}"
    );
}

fn first_call_hierarchy_item(output: &[Value], id: u64) -> Value {
    response_with_id(output, id)["result"]
        .as_array()
        .expect("call hierarchy prepare result should be an array")
        .first()
        .expect("call hierarchy prepare should return an item")
        .clone()
}

fn call_hierarchy_from_names(calls: &[Value]) -> Vec<&str> {
    calls
        .iter()
        .map(|call| {
            call["from"]["name"]
                .as_str()
                .expect("incoming caller name should be a string")
        })
        .collect()
}

fn call_hierarchy_to_names(calls: &[Value]) -> Vec<&str> {
    calls
        .iter()
        .map(|call| {
            call["to"]["name"]
                .as_str()
                .expect("outgoing callee name should be a string")
        })
        .collect()
}

fn assert_call_hierarchy_range_text(
    calls: &[Value],
    caller_name: &str,
    source: &str,
    expected: &str,
) {
    let call = calls
        .iter()
        .find(|call| call["from"]["name"] == caller_name)
        .unwrap_or_else(|| panic!("incoming call from {caller_name:?} should exist"));
    let range = call["fromRanges"]
        .as_array()
        .expect("incoming call ranges should be an array")
        .first()
        .expect("incoming call should have a range");

    assert_eq!(range_text(source, range), expected);
}

fn assert_call_hierarchy_outgoing_range_text(
    calls: &[Value],
    callee_name: &str,
    source: &str,
    expected: &str,
) {
    let call = calls
        .iter()
        .find(|call| call["to"]["name"] == callee_name)
        .unwrap_or_else(|| panic!("outgoing call to {callee_name:?} should exist"));
    let range = call["fromRanges"]
        .as_array()
        .expect("outgoing call ranges should be an array")
        .first()
        .expect("outgoing call should have a range");

    assert_eq!(range_text(source, range), expected);
}

fn assert_lsp_document_symbol_ranges_contain_selection(symbol: &Value) {
    assert!(
        lsp_range_contains(&symbol["range"], &symbol["selectionRange"]),
        "{} selectionRange {:?} should be contained in range {:?}",
        symbol["name"],
        symbol["selectionRange"],
        symbol["range"]
    );

    if let Some(children) = symbol["children"].as_array() {
        for child in children {
            assert_lsp_document_symbol_ranges_contain_selection(child);
        }
    }
}

fn lsp_range_contains(outer: &Value, inner: &Value) -> bool {
    lsp_position_key(&outer["start"]) <= lsp_position_key(&inner["start"])
        && lsp_position_key(&inner["end"]) <= lsp_position_key(&outer["end"])
}

fn lsp_position_key(position: &Value) -> (u64, u64) {
    (
        position["line"].as_u64().expect("line should be numeric"),
        position["character"]
            .as_u64()
            .expect("character should be numeric"),
    )
}

fn assert_reference_texts<const N: usize>(
    output: &[Value],
    id: u64,
    source: &str,
    expected: [&str; N],
) {
    let response = response_with_id(output, id);
    let texts = response["result"]
        .as_array()
        .expect("references result should be an array")
        .iter()
        .map(|location| range_text(source, &location["range"]))
        .collect::<Vec<_>>();

    assert_eq!(texts, expected);
}

fn assert_reference_texts_for_uri<const N: usize>(
    output: &[Value],
    id: u64,
    uri: &str,
    source: &str,
    expected: [&str; N],
) {
    let response = response_with_id(output, id);
    let texts = response["result"]
        .as_array()
        .expect("references result should be an array")
        .iter()
        .filter(|location| location["uri"] == uri)
        .map(|location| range_text(source, &location["range"]))
        .collect::<Vec<_>>();

    assert_eq!(texts, expected);
}

fn assert_document_highlight_texts<const N: usize>(
    output: &[Value],
    id: u64,
    source: &str,
    expected: [&str; N],
) {
    let response = response_with_id(output, id);
    let highlights = response["result"]
        .as_array()
        .expect("document highlight result should be an array");
    let texts = highlights
        .iter()
        .map(|highlight| range_text(source, &highlight["range"]))
        .collect::<Vec<_>>();

    assert_eq!(texts, expected);
    assert!(highlights.iter().all(|highlight| highlight["kind"] == 1));
}

fn code_action_result(output: &[Value], id: u64, index: usize) -> &Value {
    &response_with_id(output, id)["result"]
        .as_array()
        .expect("code actions should be an array")[index]
}

fn workspace_edit_changes_for_uri<'a>(output: &'a [Value], id: u64, uri: &str) -> &'a [Value] {
    workspace_edit_changes_for_uri_optional(output, id, uri)
        .unwrap_or_else(|| panic!("workspace edit should contain changes for {uri}"))
}

fn workspace_edit_changes_for_uri_optional<'a>(
    output: &'a [Value],
    id: u64,
    uri: &str,
) -> Option<&'a [Value]> {
    response_with_id(output, id)["result"]["changes"][uri]
        .as_array()
        .map(Vec::as_slice)
}

fn code_lens_result(output: &[Value], id: u64) -> &[Value] {
    response_with_id(output, id)["result"]
        .as_array()
        .expect("code lenses should be an array")
}

fn code_lens_with_name(output: &[Value], id: u64, name: &str) -> Value {
    code_lens_result(output, id)
        .iter()
        .find(|lens| lens["data"]["name"] == name)
        .unwrap_or_else(|| panic!("code lens for {name:?} should exist"))
        .clone()
}

fn code_lens_names(lenses: &[Value]) -> Vec<&str> {
    lenses
        .iter()
        .map(|lens| {
            lens["data"]["name"]
                .as_str()
                .expect("code lens name should be a string")
        })
        .collect()
}

fn assert_rename_edit_texts<const N: usize>(
    output: &[Value],
    id: u64,
    uri: &str,
    source: &str,
    expected_originals: [&str; N],
    expected_new_text: &str,
) {
    let response = response_with_id(output, id);
    let edits = response["result"]["changes"][uri]
        .as_array()
        .expect("rename edits should be an array");
    let originals = edits
        .iter()
        .map(|edit| range_text(source, &edit["range"]))
        .collect::<Vec<_>>();
    let new_texts = edits
        .iter()
        .map(|edit| {
            edit["newText"]
                .as_str()
                .expect("rename edit text should be a string")
        })
        .collect::<Vec<_>>();

    assert_eq!(originals, expected_originals);
    assert!(new_texts
        .iter()
        .all(|new_text| *new_text == expected_new_text));
}

fn inlay_hint_labels(hints: &[Value]) -> Vec<&str> {
    hints
        .iter()
        .map(|hint| {
            hint["label"]
                .as_str()
                .expect("inlay hint label should be a string")
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedSemanticToken<'a> {
    text: &'a str,
    token_type: usize,
    declaration: bool,
    readonly: bool,
    deprecated: bool,
}

fn decoded_semantic_tokens<'a>(source: &'a str, response: &Value) -> Vec<DecodedSemanticToken<'a>> {
    let data = response["result"]["data"]
        .as_array()
        .expect("semantic token data should be an array");
    let mut line = 0usize;
    let mut character = 0usize;
    let mut tokens = Vec::new();

    for chunk in data.chunks(5) {
        let delta_line = chunk[0].as_u64().expect("delta line should be numeric") as usize;
        let delta_start = chunk[1].as_u64().expect("delta start should be numeric") as usize;
        let length = chunk[2].as_u64().expect("length should be numeric") as usize;
        let token_type = chunk[3].as_u64().expect("type should be numeric") as usize;
        let modifiers = chunk[4].as_u64().expect("modifiers should be numeric") as usize;

        if delta_line == 0 {
            character += delta_start;
        } else {
            line += delta_line;
            character = delta_start;
        }

        let text = source
            .split('\n')
            .nth(line)
            .and_then(|line_text| line_text.get(character..character + length))
            .expect("semantic token range should exist");

        tokens.push(DecodedSemanticToken {
            text,
            token_type,
            declaration: modifiers & 1 == 1,
            readonly: modifiers & 2 == 2,
            deprecated: modifiers & 4 == 4,
        });
    }

    tokens
}

fn semantic_tokens_result_id(result: &Value) -> String {
    result["resultId"]
        .as_str()
        .expect("semantic token result id should be present")
        .to_owned()
}

fn semantic_token_data(result: &Value) -> Vec<Value> {
    result["data"]
        .as_array()
        .expect("semantic token data should be an array")
        .clone()
}

fn semantic_token_edits(result: &Value) -> &[Value] {
    result["edits"]
        .as_array()
        .expect("semantic token delta edits should be an array")
}

fn apply_semantic_token_edits(mut data: Vec<Value>, edits: &[Value]) -> Vec<Value> {
    for edit in edits {
        let start = edit["start"]
            .as_u64()
            .expect("semantic token edit start should be numeric") as usize;
        let delete_count = edit["deleteCount"]
            .as_u64()
            .expect("semantic token edit deleteCount should be numeric")
            as usize;
        let inserted = edit
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let end = start + delete_count;

        data.splice(start..end, inserted);
    }

    data
}

fn assert_semantic_token(
    tokens: &[DecodedSemanticToken<'_>],
    text: &str,
    token_type: usize,
    declaration: bool,
) {
    assert!(
        tokens.iter().any(|token| {
            token.text == text
                && token.token_type == token_type
                && token.declaration == declaration
        }),
        "expected semantic token `{text}` type {token_type} declaration {declaration}, got {tokens:?}"
    );
}

fn range_text<'a>(source: &'a str, range: &Value) -> &'a str {
    let line = range["start"]["line"]
        .as_u64()
        .expect("range start line should be numeric") as usize;
    let start = range["start"]["character"]
        .as_u64()
        .expect("range start character should be numeric") as usize;
    let end = range["end"]["character"]
        .as_u64()
        .expect("range end character should be numeric") as usize;
    let text = source
        .split('\n')
        .nth(line)
        .expect("reference line should exist");

    &text[start..end]
}

fn selection_range_texts<'a>(source: &'a str, range: &'a Value) -> Vec<&'a str> {
    let mut texts = Vec::new();
    let mut current = Some(range);

    while let Some(range) = current {
        texts.push(lsp_range_text(source, &range["range"]));
        current = range.get("parent");
    }

    texts
}

fn lsp_range_text<'a>(source: &'a str, range: &Value) -> &'a str {
    let start_line = range["start"]["line"]
        .as_u64()
        .expect("range start line should be numeric") as usize;
    let start_character = range["start"]["character"]
        .as_u64()
        .expect("range start character should be numeric") as usize;
    let end_line = range["end"]["line"]
        .as_u64()
        .expect("range end line should be numeric") as usize;
    let end_character = range["end"]["character"]
        .as_u64()
        .expect("range end character should be numeric") as usize;

    if start_line == end_line {
        let text = source
            .split('\n')
            .nth(start_line)
            .expect("range line should exist");

        return &text[start_character..end_character];
    }

    let start_index = lsp_byte_index(source, start_line, start_character);
    let end_index = lsp_byte_index(source, end_line, end_character);

    &source[start_index..end_index]
}

fn lsp_byte_index(source: &str, target_line: usize, target_character: usize) -> usize {
    let mut line = 0;
    let mut character = 0;

    for (index, value) in source.char_indices() {
        if line == target_line && character == target_character {
            return index;
        }

        if value == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }

    source.len()
}

fn symbol_names(symbols: &[Value]) -> Vec<&str> {
    symbols
        .iter()
        .map(|symbol| {
            symbol["name"]
                .as_str()
                .expect("symbol name should be a string")
        })
        .collect()
}

fn workspace_symbol_names(symbols: &[Value]) -> Vec<&str> {
    symbols
        .iter()
        .map(|symbol| {
            symbol["name"]
                .as_str()
                .expect("workspace symbol name should be a string")
        })
        .collect()
}

fn response_with_id(output: &[Value], id: u64) -> &Value {
    output
        .iter()
        .find(|message| message["id"] == id)
        .expect("response should exist")
}

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tua-lsp-{name}-{}-{unique}", process::id()));

        fs::create_dir_all(&path).expect("temp project should be created");

        Self { path }
    }

    fn uri(&self) -> String {
        path_uri(&self.path)
    }

    fn uri_for(&self, relative: &str) -> String {
        path_uri(&self.path.join(relative))
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory should be created");
        }

        fs::write(path, contents).expect("fixture should be written");
    }

    fn remove(&self, relative: &str) {
        fs::remove_file(self.path.join(relative)).expect("fixture should be removed");
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn path_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}
