use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};

use serde_json::{json, Value};
use tua_core::{
    format_tua_source, language_contract, AnalysisHost, AnalysisSourceKind, Assist, AssistKind,
    AssistRange, Diagnostic, DocumentSymbol, ModelCompletionValidation,
    ModelCompletionValidationRequest, ProjectWorkspace, Reference, RequireResolution,
    SourcePosition, SourceSpan, DEFAULT_MODEL_CONTEXT_BYTES,
};

use crate::error::CliError;
use crate::source::{discover_build_files, load_type_libraries, read_source_file, SourceKind};

const API_PROTOCOL_VERSION: &str = "1";
const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

pub fn run_api_command(workspace: impl AsRef<Path>) -> Result<(), CliError> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_api_stdio(stdin.lock(), stdout.lock(), workspace.as_ref())
}

pub fn run_mcp_command(workspace: impl AsRef<Path>) -> Result<(), CliError> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_mcp_stdio(stdin.lock(), stdout.lock(), workspace.as_ref())
}

fn run_api_stdio(reader: impl Read, writer: impl Write, workspace: &Path) -> Result<(), CliError> {
    let mut session = ApiSession::load(workspace)?;
    run_json_lines(reader, writer, |message| session.handle_api(message))
}

fn run_mcp_stdio(reader: impl Read, writer: impl Write, workspace: &Path) -> Result<(), CliError> {
    let mut session = ApiSession::load(workspace)?;
    session.source_context = true;
    run_json_lines(reader, writer, |message| session.handle_mcp(message))
}

fn run_json_lines(
    reader: impl Read,
    mut writer: impl Write,
    mut handle: impl FnMut(Value) -> ProtocolOutcome,
) -> Result<(), CliError> {
    let reader = BufReader::new(reader);
    for line in reader.lines() {
        let line =
            line.map_err(|error| CliError::usage(format!("failed to read stdio: {error}")))?;
        if line.trim().is_empty() {
            continue;
        }
        let outcome = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle(message),
            Err(error) => ProtocolOutcome::response(json_rpc_error(
                Value::Null,
                -32700,
                format!("parse error: {error}"),
            )),
        };
        if let Some(response) = outcome.response {
            serde_json::to_writer(&mut writer, &response).map_err(|error| {
                CliError::usage(format!("failed to write protocol response: {error}"))
            })?;
            writer.write_all(b"\n").map_err(|error| {
                CliError::usage(format!("failed to write protocol response: {error}"))
            })?;
            writer.flush().map_err(|error| {
                CliError::usage(format!("failed to flush protocol response: {error}"))
            })?;
        }
        if outcome.shutdown {
            break;
        }
    }
    Ok(())
}

struct ProtocolOutcome {
    response: Option<Value>,
    shutdown: bool,
}

impl ProtocolOutcome {
    fn response(response: Value) -> Self {
        Self {
            response: Some(response),
            shutdown: false,
        }
    }

    fn notification() -> Self {
        Self {
            response: None,
            shutdown: false,
        }
    }

    fn shutdown(response: Value) -> Self {
        Self {
            response: Some(response),
            shutdown: true,
        }
    }
}

struct ApiSession {
    workspace_root: PathBuf,
    workspace: ProjectWorkspace,
    host: AnalysisHost,
    indexed_paths: Vec<PathBuf>,
    overlays: BTreeMap<PathBuf, String>,
    source_context: bool,
    initialized: bool,
}

impl ApiSession {
    fn load(workspace_root: &Path) -> Result<Self, CliError> {
        let workspace_root = if workspace_root.is_absolute() {
            normalize_path(workspace_root)
        } else {
            let cwd = std::env::current_dir().map_err(|error| {
                CliError::usage(format!("failed to resolve API workspace: {error}"))
            })?;
            normalize_path(&cwd.join(workspace_root))
        };
        let (workspace, host, indexed_paths) = load_analysis(&workspace_root)?;
        Ok(Self {
            workspace_root,
            workspace,
            host,
            indexed_paths,
            overlays: BTreeMap::new(),
            source_context: false,
            initialized: false,
        })
    }

    fn refresh(&mut self) -> Result<(), CliError> {
        let (workspace, mut host, indexed_paths) = load_analysis(&self.workspace_root)?;
        for (path, source) in &self.overlays {
            host.open_document(path, source);
        }
        self.workspace = workspace;
        self.host = host;
        self.indexed_paths = indexed_paths;
        Ok(())
    }

    fn handle_api(&mut self, message: Value) -> ProtocolOutcome {
        let id = message.get("id").cloned();
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return ProtocolOutcome::response(json_rpc_error(
                id.unwrap_or(Value::Null),
                -32600,
                "invalid request",
            ));
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        if method == "tua.api/initialize" || method == "initialize" {
            self.initialized = true;
            self.source_context = params
                .get("capabilities")
                .and_then(|value| value.get("sourceContext"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            return ProtocolOutcome::response(json_rpc_result(
                id.unwrap_or(Value::Null),
                json!({
                    "protocolVersion": API_PROTOCOL_VERSION,
                    "serverInfo": {"name": "tua-api", "version": tua_core::VERSION},
                    "capabilities": {
                        "sourceContext": self.source_context,
                        "overlays": true,
                        "refresh": "explicit",
                        "readOnly": true
                    }
                }),
            ));
        }
        if !self.initialized {
            return ProtocolOutcome::response(json_rpc_error(
                id.unwrap_or(Value::Null),
                -32002,
                "tua.api/initialize must be called first",
            ));
        }
        if method == "shutdown" || method == "tua.api/shutdown" {
            return ProtocolOutcome::shutdown(json_rpc_result(
                id.unwrap_or(Value::Null),
                Value::Null,
            ));
        }

        let result = self.dispatch_api(method, &params);
        if id.is_none() {
            return ProtocolOutcome::notification();
        }
        match result {
            Ok(value) => ProtocolOutcome::response(json_rpc_result(id.unwrap(), value)),
            Err(error) => {
                ProtocolOutcome::response(json_rpc_error(id.unwrap(), error.code, error.message))
            }
        }
    }

    fn dispatch_api(&mut self, method: &str, params: &Value) -> Result<Value, ApiError> {
        let method = method.strip_prefix("tua.api/").unwrap_or(method);
        match method {
            "refresh" => {
                self.refresh()
                    .map_err(|error| ApiError::internal(error.format_stderr()))?;
                Ok(self.project_summary())
            }
            "setOverlay" => {
                let path = self.required_path(params)?;
                let source = required_string(params, "source")?.to_owned();
                self.host.open_document(&path, &source);
                self.overlays.insert(path.clone(), source);
                Ok(json!({"path": display_path(&path), "overlay": true}))
            }
            "clearOverlay" => {
                let path = self.required_path(params)?;
                self.overlays.remove(&path);
                self.refresh()
                    .map_err(|error| ApiError::internal(error.format_stderr()))?;
                Ok(json!({"path": display_path(&path), "overlay": false}))
            }
            "projectSummary" => Ok(self.project_summary()),
            "languageContract" => Ok(json!({
                "format": "text/plain",
                "content": language_contract()
            })),
            "diagnostics" => Ok(self.diagnostics(params)?),
            "semanticFacts" => Ok(self.semantic_facts(params)?),
            "moduleGraph" => Ok(self.module_graph()),
            "references" => Ok(self.references(params)?),
            "assetMetadata" => Ok(self.asset_metadata()),
            "completionContext" => Ok(self.completion_context(params)?),
            "validateCompletion" => Ok(self.validate_completion(params)?),
            "formatPreview" => Ok(self.format_preview(params)?),
            "codeActions" => Ok(self.code_actions(params)?),
            _ => Err(ApiError::method_not_found(method)),
        }
    }

    fn project_summary(&self) -> Value {
        json!({
            "workspace": display_path(&self.workspace_root),
            "analysisRevision": self.host.revision(),
            "target": self.workspace.config().target,
            "extension": self.workspace.config().extension,
            "sourceRoots": self.workspace.source_roots().iter().map(|path| display_path(path)).collect::<Vec<_>>(),
            "files": self.indexed_paths.iter().map(|path| display_path(path)).collect::<Vec<_>>(),
            "overlays": self.overlays.keys().map(|path| display_path(path)).collect::<Vec<_>>(),
            "readOnly": true
        })
    }

    fn diagnostics(&self, params: &Value) -> Result<Value, ApiError> {
        let snapshot = self.host.snapshot();
        let paths = self
            .optional_path(params)?
            .map_or_else(|| self.indexed_paths.clone(), |path| vec![path]);
        let diagnostics = paths
            .iter()
            .flat_map(|path| snapshot.diagnostics(path))
            .map(diagnostic_json)
            .collect::<Vec<_>>();
        Ok(json!({
            "analysisRevision": snapshot.revision(),
            "diagnostics": diagnostics
        }))
    }

    fn semantic_facts(&self, params: &Value) -> Result<Value, ApiError> {
        let path = self.required_path(params)?;
        let snapshot = self.host.snapshot();
        let symbols = snapshot
            .document_symbols(&path)
            .iter()
            .map(document_symbol_json)
            .collect::<Vec<_>>();
        let bindings = snapshot
            .type_environment(&path)
            .map(|environment| {
                environment
                    .locals()
                    .map(|(name, ty)| json!({"name": name, "type": ty.to_string()}))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(json!({
            "analysisRevision": snapshot.revision(),
            "path": display_path(&path),
            "symbols": symbols,
            "bindings": bindings
        }))
    }

    fn module_graph(&self) -> Value {
        let snapshot = self.host.snapshot();
        let modules = snapshot
            .module_graph()
            .modules()
            .map(|module| {
                let requires = module
                    .require_edges
                    .iter()
                    .map(|edge| {
                        let (status, path) = match &edge.resolution {
                            RequireResolution::Resolved { path } => {
                                ("resolved", Some(display_path(path)))
                            }
                            RequireResolution::Missing { .. } => ("missing", None),
                            RequireResolution::Invalid { .. } => ("invalid", None),
                            RequireResolution::Duplicate { selected, .. } => {
                                ("duplicate", Some(display_path(selected)))
                            }
                        };
                        json!({
                            "module": edge.module,
                            "status": status,
                            "path": path,
                            "range": span_json(edge.span)
                        })
                    })
                    .collect::<Vec<_>>();
                json!({
                    "path": display_path(&module.path),
                    "requires": requires,
                    "exports": module.exports.iter().map(|entry| entry.public_name.clone()).collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        json!({"analysisRevision": snapshot.revision(), "modules": modules})
    }

    fn references(&self, params: &Value) -> Result<Value, ApiError> {
        let path = self.required_path(params)?;
        let position = required_position(params)?;
        let include_declaration = params
            .get("includeDeclaration")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let snapshot = self.host.snapshot();
        let references = snapshot
            .references(&path, position, include_declaration)
            .into_iter()
            .map(reference_json)
            .collect::<Vec<_>>();
        Ok(json!({
            "analysisRevision": snapshot.revision(),
            "references": references
        }))
    }

    fn asset_metadata(&self) -> Value {
        let snapshot = self.host.snapshot();
        let inventory = snapshot.asset_inventory();
        let assets = inventory
            .assets
            .into_iter()
            .map(|asset| {
                json!({
                    "path": display_path(&asset.path),
                    "relativePath": asset.relative_path,
                    "category": asset.category.name(),
                    "extension": asset.extension,
                    "dimensions": asset.dimensions.map(|(width, height)| json!({"width": width, "height": height})),
                    "references": asset.references.into_iter().map(|reference| json!({
                        "path": display_path(&reference.path),
                        "range": span_json(reference.span)
                    })).collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        json!({
            "analysisRevision": snapshot.revision(),
            "assets": assets,
            "diagnostics": inventory.diagnostics.into_iter().map(diagnostic_json).collect::<Vec<_>>()
        })
    }

    fn completion_context(&self, params: &Value) -> Result<Value, ApiError> {
        self.require_source_context()?;
        let path = self.required_path(params)?;
        let position = required_position(params)?;
        let snapshot = self.host.snapshot();
        let context = snapshot
            .model_completion_context(&path, position, DEFAULT_MODEL_CONTEXT_BYTES)
            .ok_or_else(|| ApiError::invalid_params("no completion context at this position"))?;
        Ok(json!({
            "contextId": context.context_id,
            "analysisRevision": context.analysis_revision,
            "contextBytes": context.context_bytes,
            "sourceFingerprint": context.source_fingerprint,
            "position": position_json(context.position),
            "prompt": context.prompt,
            "suffix": context.suffix,
            "range": span_json(context.insertion_span)
        }))
    }

    fn validate_completion(&self, params: &Value) -> Result<Value, ApiError> {
        let path = self.required_path(params)?;
        let position = required_position(params)?;
        let validation = self.host.validate_model_completion(
            &path,
            ModelCompletionValidationRequest {
                context_id: required_string(params, "contextId")?,
                analysis_revision: required_u64(params, "analysisRevision")?,
                context_bytes: required_u64(params, "contextBytes")? as usize,
                source_fingerprint: required_string(params, "sourceFingerprint")?,
                position,
                candidate: required_string(params, "candidate")?,
            },
        );
        Ok(validation_json(validation))
    }

    fn format_preview(&self, params: &Value) -> Result<Value, ApiError> {
        self.require_source_context()?;
        let path = self.required_path(params)?;
        let snapshot = self.host.snapshot();
        let source = snapshot
            .source(&path)
            .ok_or_else(|| ApiError::invalid_params("path is not indexed"))?;
        match format_tua_source(&path, source, &snapshot.config().format.options()) {
            Ok(formatted) => Ok(json!({
                "path": display_path(&path),
                "changed": formatted != source,
                "formatted": formatted
            })),
            Err(diagnostics) => Ok(json!({
                "path": display_path(&path),
                "changed": false,
                "formatted": Value::Null,
                "diagnostics": diagnostics.into_iter().map(diagnostic_json).collect::<Vec<_>>()
            })),
        }
    }

    fn code_actions(&self, params: &Value) -> Result<Value, ApiError> {
        self.require_source_context()?;
        let path = self.required_path(params)?;
        let snapshot = self.host.snapshot();
        let range = optional_range(params).unwrap_or(AssistRange {
            start: SourcePosition { line: 1, column: 1 },
            end: SourcePosition {
                line: usize::MAX,
                column: usize::MAX,
            },
        });
        let diagnostics = snapshot.diagnostics(&path);
        let requested_kinds = [
            AssistKind::QuickFix,
            AssistKind::SourceOrganizeImports,
            AssistKind::SourceSortImports,
            AssistKind::SourceRemoveUnusedImports,
            AssistKind::SourceCreateLoveCallbacks,
            AssistKind::SourceFixAll,
        ];
        let actions = snapshot
            .code_actions(&path, range, &diagnostics, &requested_kinds)
            .into_iter()
            .map(assist_json)
            .collect::<Vec<_>>();
        Ok(json!({
            "analysisRevision": snapshot.revision(),
            "actions": actions,
            "applied": false
        }))
    }

    fn required_path(&self, params: &Value) -> Result<PathBuf, ApiError> {
        self.resolve_path(required_string(params, "path")?)
    }

    fn optional_path(&self, params: &Value) -> Result<Option<PathBuf>, ApiError> {
        params
            .get("path")
            .and_then(Value::as_str)
            .map(|path| self.resolve_path(path))
            .transpose()
    }

    fn resolve_path(&self, raw: &str) -> Result<PathBuf, ApiError> {
        let candidate = PathBuf::from(raw);
        let candidate = if candidate.is_absolute() {
            normalize_path(&candidate)
        } else {
            normalize_path(&self.workspace_root.join(candidate))
        };
        let root = normalize_path(&self.workspace_root);
        if !candidate.starts_with(&root) {
            return Err(ApiError::invalid_params(
                "path must remain inside the workspace",
            ));
        }
        Ok(candidate)
    }

    fn require_source_context(&self) -> Result<(), ApiError> {
        if self.source_context {
            Ok(())
        } else {
            Err(ApiError {
                code: -32010,
                message: "client must negotiate capabilities.sourceContext=true".to_owned(),
            })
        }
    }

    fn handle_mcp(&mut self, message: Value) -> ProtocolOutcome {
        let id = message.get("id").cloned();
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "initialize" => {
                self.initialized = true;
                ProtocolOutcome::response(json_rpc_result(
                    id.unwrap_or(Value::Null),
                    json!({
                        "protocolVersion": MCP_PROTOCOL_VERSION,
                        "serverInfo": {"name": "tua", "version": tua_core::VERSION},
                        "capabilities": {"tools": {"listChanged": false}}
                    }),
                ))
            }
            "notifications/initialized" => ProtocolOutcome::notification(),
            "ping" => {
                ProtocolOutcome::response(json_rpc_result(id.unwrap_or(Value::Null), json!({})))
            }
            "tools/list" if self.initialized => ProtocolOutcome::response(json_rpc_result(
                id.unwrap_or(Value::Null),
                json!({"tools": mcp_tools()}),
            )),
            "tools/call" if self.initialized => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let result = mcp_method(name)
                    .ok_or_else(|| ApiError::method_not_found(name))
                    .and_then(|method| self.dispatch_api(method, &arguments));
                let value = match result {
                    Ok(value) => json!({
                        "content": [{"type": "text", "text": compact_text(name, &value)}],
                        "structuredContent": value,
                        "isError": false
                    }),
                    Err(error) => json!({
                        "content": [{"type": "text", "text": error.message}],
                        "isError": true
                    }),
                };
                ProtocolOutcome::response(json_rpc_result(id.unwrap_or(Value::Null), value))
            }
            "shutdown" => {
                ProtocolOutcome::shutdown(json_rpc_result(id.unwrap_or(Value::Null), Value::Null))
            }
            _ if id.is_none() => ProtocolOutcome::notification(),
            _ => ProtocolOutcome::response(json_rpc_error(
                id.unwrap_or(Value::Null),
                -32601,
                format!("method not found: {method}"),
            )),
        }
    }
}

fn load_analysis(
    workspace_root: &Path,
) -> Result<(ProjectWorkspace, AnalysisHost, Vec<PathBuf>), CliError> {
    let loaded =
        tua_core::load_config_from_dir(workspace_root).map_err(CliError::from_config_error)?;
    let workspace = ProjectWorkspace::from_loaded(workspace_root, loaded);
    let mut host = AnalysisHost::with_config(workspace.config().clone());
    host.set_source_roots(workspace.source_roots().iter().cloned());
    host.set_output_roots(workspace.output_roots().iter().cloned());
    host.set_asset_roots(workspace.asset_roots());
    let library_diagnostics = load_type_libraries(&mut host, &workspace);
    for diagnostic in library_diagnostics {
        eprintln!("{}", diagnostic.format_cli());
    }

    let mut indexed = BTreeSet::new();
    for source_root in workspace.source_roots() {
        let files = discover_build_files(source_root, &workspace.config().extension, None)
            .map_err(|diagnostic| {
                CliError::from_diagnostics("failed to discover API workspace", vec![diagnostic])
            })?;
        for file in files {
            if workspace.is_ignored(&file.path) || workspace.is_type_library_path(&file.path) {
                continue;
            }
            let kind = match file.kind {
                SourceKind::Tua => Some(AnalysisSourceKind::Tua),
                SourceKind::Asset
                    if file.path.extension().and_then(|value| value.to_str()) == Some("lua") =>
                {
                    Some(AnalysisSourceKind::Lua)
                }
                SourceKind::Asset => None,
            };
            let Some(kind) = kind else {
                continue;
            };
            let source = read_source_file(&file.path).map_err(|diagnostic| {
                CliError::from_diagnostics("failed to read API workspace", vec![diagnostic])
            })?;
            host.set_file_with_kind(&file.path, source, kind);
            indexed.insert(file.path);
        }
    }
    Ok((workspace, host, indexed.into_iter().collect()))
}

struct ApiError {
    code: i64,
    message: String,
}

impl ApiError {
    fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
        }
    }

    fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("method not found: {method}"),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: message.into(),
        }
    }
}

fn required_string<'a>(params: &'a Value, field: &str) -> Result<&'a str, ApiError> {
    params
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::invalid_params(format!("`{field}` must be a string")))
}

fn required_u64(params: &Value, field: &str) -> Result<u64, ApiError> {
    params
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| ApiError::invalid_params(format!("`{field}` must be an unsigned integer")))
}

fn required_position(params: &Value) -> Result<SourcePosition, ApiError> {
    let position = params
        .get("position")
        .ok_or_else(|| ApiError::invalid_params("`position` is required"))?;
    let line = position
        .get("line")
        .and_then(Value::as_u64)
        .ok_or_else(|| ApiError::invalid_params("`position.line` must be an integer"))?;
    let character = position
        .get("character")
        .and_then(Value::as_u64)
        .ok_or_else(|| ApiError::invalid_params("`position.character` must be an integer"))?;
    Ok(SourcePosition {
        line: line as usize + 1,
        column: character as usize + 1,
    })
}

fn optional_range(params: &Value) -> Option<AssistRange> {
    let range = params.get("range")?;
    let start = required_position(&json!({"position": range.get("start")?})).ok()?;
    let end = required_position(&json!({"position": range.get("end")?})).ok()?;
    Some(AssistRange { start, end })
}

fn json_rpc_result(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn json_rpc_error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message.into()}
    })
}

fn position_json(position: SourcePosition) -> Value {
    json!({
        "line": position.line.saturating_sub(1),
        "character": position.column.saturating_sub(1)
    })
}

fn span_json(span: SourceSpan) -> Value {
    json!({"start": position_json(span.start), "end": position_json(span.end)})
}

fn diagnostic_json(diagnostic: Diagnostic) -> Value {
    json!({
        "path": display_path(&diagnostic.file),
        "range": {
            "start": {"line": diagnostic.line.saturating_sub(1), "character": diagnostic.column.saturating_sub(1)},
            "end": {"line": diagnostic.end_line.saturating_sub(1), "character": diagnostic.end_column.saturating_sub(1)}
        },
        "severity": diagnostic.severity.to_string(),
        "code": diagnostic.code.map(|code| code.to_string()),
        "message": diagnostic.message
    })
}

fn document_symbol_json(symbol: &DocumentSymbol) -> Value {
    json!({
        "name": symbol.name,
        "kind": format!("{:?}", symbol.kind),
        "detail": symbol.detail,
        "range": span_json(symbol.range),
        "selectionRange": span_json(symbol.selection_range),
        "children": symbol.children.iter().map(document_symbol_json).collect::<Vec<_>>()
    })
}

fn reference_json(reference: Reference) -> Value {
    json!({
        "path": display_path(&reference.path),
        "range": span_json(reference.span),
        "kind": format!("{:?}", reference.kind)
    })
}

fn validation_json(validation: ModelCompletionValidation) -> Value {
    json!({
        "status": validation.status.as_str(),
        "reason": validation.reason.map(|reason| reason.as_str()),
        "insertText": validation.normalized_insertion,
        "range": span_json(validation.insertion_span)
    })
}

fn assist_json(assist: Assist) -> Value {
    let changes = assist
        .edit
        .changes
        .into_iter()
        .map(|(path, edits)| {
            json!({
                "path": display_path(&path),
                "edits": edits.into_iter().map(|edit| json!({
                    "range": span_json(edit.span),
                    "newText": edit.new_text
                })).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    json!({
        "title": assist.title,
        "kind": format!("{:?}", assist.kind),
        "preview": {
            "changes": changes,
            "createFiles": assist.edit.create_files.into_iter().map(|file| json!({
                "path": display_path(&file.path),
                "ignoreIfExists": file.ignore_if_exists
            })).collect::<Vec<_>>()
        }
    })
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn mcp_method(name: &str) -> Option<&'static str> {
    Some(match name {
        "tua_project_summary" => "projectSummary",
        "tua_language_contract" => "languageContract",
        "tua_refresh" => "refresh",
        "tua_diagnostics" => "diagnostics",
        "tua_semantic_facts" => "semanticFacts",
        "tua_module_graph" => "moduleGraph",
        "tua_references" => "references",
        "tua_asset_metadata" => "assetMetadata",
        "tua_completion_context" => "completionContext",
        "tua_validate_completion" => "validateCompletion",
        "tua_format_preview" => "formatPreview",
        "tua_code_actions" => "codeActions",
        _ => return None,
    })
}

fn mcp_tools() -> Vec<Value> {
    [
        ("tua_project_summary", "Summarize the loaded Tua project.", json!({})),
        ("tua_language_contract", "Return the compact Tua language contract.", json!({})),
        ("tua_refresh", "Explicitly rescan the workspace from disk.", json!({})),
        ("tua_diagnostics", "Return compiler diagnostics.", path_schema(false)),
        ("tua_semantic_facts", "Return symbols and typed bindings for a file.", path_schema(true)),
        ("tua_module_graph", "Return project modules, imports, and exports.", json!({})),
        ("tua_references", "Find references at a zero-based source position.", positioned_schema()),
        ("tua_asset_metadata", "Return LOVE asset metadata and references.", json!({})),
        ("tua_completion_context", "Build bounded model-ready FIM context.", positioned_schema()),
        ("tua_validate_completion", "Validate a model completion without mutating the project.", json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "position": position_schema(),
                "contextId": {"type": "string"},
                "analysisRevision": {"type": "integer"},
                "contextBytes": {"type": "integer"},
                "sourceFingerprint": {"type": "string"},
                "candidate": {"type": "string"}
            },
            "required": ["path", "position", "contextId", "analysisRevision", "contextBytes", "sourceFingerprint", "candidate"],
            "additionalProperties": false
        })),
        ("tua_format_preview", "Preview formatting without writing files.", path_schema(true)),
        ("tua_code_actions", "Preview compiler-proven code actions without applying them.", path_schema(true)),
    ]
    .into_iter()
    .map(|(name, description, schema)| {
        let input_schema = if schema.as_object().is_some_and(|object| object.is_empty()) {
            json!({"type": "object", "properties": {}, "additionalProperties": false})
        } else {
            schema
        };
        json!({"name": name, "description": description, "inputSchema": input_schema})
    })
    .collect()
}

fn path_schema(required: bool) -> Value {
    json!({
        "type": "object",
        "properties": {"path": {"type": "string"}},
        "required": if required {json!(["path"])} else {json!([])},
        "additionalProperties": false
    })
}

fn positioned_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {"type": "string"},
            "position": position_schema(),
            "includeDeclaration": {"type": "boolean"}
        },
        "required": ["path", "position"],
        "additionalProperties": false
    })
}

fn position_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "line": {"type": "integer", "minimum": 0},
            "character": {"type": "integer", "minimum": 0}
        },
        "required": ["line", "character"],
        "additionalProperties": false
    })
}

fn compact_text(name: &str, value: &Value) -> String {
    match name {
        "tua_language_contract" => value
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        "tua_project_summary" => format!(
            "Tua project: {} ({} indexed files)",
            value
                .get("workspace")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            value
                .get("files")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0)
        ),
        _ => serde_json::to_string(value).unwrap_or_else(|_| "{}".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn mcp_exposes_the_pinned_tool_set() {
        let names = mcp_tools()
            .into_iter()
            .filter_map(|tool| tool.get("name")?.as_str().map(str::to_owned))
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 12);
        assert!(names.contains(&"tua_completion_context".to_owned()));
        assert!(names.contains(&"tua_validate_completion".to_owned()));
    }

    #[test]
    fn api_framing_returns_parse_errors_as_one_clean_json_line() {
        let mut output = Vec::new();
        run_json_lines("{\n".as_bytes(), &mut output, |_| {
            panic!("invalid JSON must not reach the handler")
        })
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text.lines().count(), 1);
        let response: Value = serde_json::from_str(text.trim()).unwrap();
        assert_eq!(response["error"]["code"], -32700);
    }

    #[test]
    fn api_negotiates_source_context_and_preserves_overlay_lifecycle() {
        let project = TempProject::new();
        project.write("tua.toml", "source = \"src\"\n");
        project.write("src/main.tua", "local value: number = ");

        let denied = run_api(
            &project,
            vec![
                json!({"jsonrpc":"2.0","id":1,"method":"tua.api/initialize","params":{"capabilities":{}}}),
                json!({"jsonrpc":"2.0","id":2,"method":"tua.api/completionContext","params":{
                    "path":"src/main.tua",
                    "position":{"line":0,"character":22}
                }}),
                json!({"jsonrpc":"2.0","id":3,"method":"shutdown"}),
            ],
        );
        assert_eq!(denied[1]["error"]["code"], -32010);

        let allowed = run_api(
            &project,
            vec![
                json!({"jsonrpc":"2.0","id":1,"method":"tua.api/initialize","params":{"capabilities":{"sourceContext":true}}}),
                json!({"jsonrpc":"2.0","id":2,"method":"tua.api/setOverlay","params":{
                    "path":"src/main.tua",
                    "source":"local value: number = 1\nlocal next = "
                }}),
                json!({"jsonrpc":"2.0","id":3,"method":"tua.api/completionContext","params":{
                    "path":"src/main.tua",
                    "position":{"line":1,"character":13}
                }}),
                json!({"jsonrpc":"2.0","id":4,"method":"tua.api/clearOverlay","params":{
                    "path":"src/main.tua"
                }}),
                json!({"jsonrpc":"2.0","id":5,"method":"tua.api/projectSummary","params":{}}),
                json!({"jsonrpc":"2.0","id":6,"method":"shutdown"}),
            ],
        );
        assert!(allowed[2]["result"]["prompt"]
            .as_str()
            .is_some_and(|prompt| prompt.ends_with("local next = ")));
        assert_eq!(
            allowed[4]["result"]["overlays"].as_array().map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn mcp_stdio_lists_tools_with_structured_schemas() {
        let project = TempProject::new();
        project.write("tua.toml", "source = \"src\"\n");
        project.write("src/main.tua", "return true\n");
        let input = [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities":{},
                "clientInfo":{"name":"test","version":"1"}
            }}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            json!({"jsonrpc":"2.0","id":3,"method":"shutdown"}),
        ]
        .into_iter()
        .map(|value| serde_json::to_string(&value).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
        let mut output = Vec::new();
        run_mcp_stdio(input.as_bytes(), &mut output, &project.root).unwrap();
        let responses = response_lines(&output);
        assert_eq!(responses.len(), 3);
        assert_eq!(
            responses[0]["result"]["protocolVersion"],
            MCP_PROTOCOL_VERSION
        );
        assert_eq!(
            responses[1]["result"]["tools"].as_array().map(Vec::len),
            Some(12)
        );
    }

    fn run_api(project: &TempProject, messages: Vec<Value>) -> Vec<Value> {
        let input = messages
            .into_iter()
            .map(|value| serde_json::to_string(&value).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        let mut output = Vec::new();
        run_api_stdio(input.as_bytes(), &mut output, &project.root).unwrap();
        response_lines(&output)
    }

    fn response_lines(output: &[u8]) -> Vec<Value> {
        String::from_utf8(output.to_vec())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    struct TempProject {
        root: PathBuf,
    }

    impl TempProject {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root =
                std::env::temp_dir().join(format!("tua-api-test-{}-{nonce}", std::process::id()));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn write(&self, relative: &str, content: &str) {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, content).unwrap();
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
