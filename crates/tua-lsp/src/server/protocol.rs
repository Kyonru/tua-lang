use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{json, Value};
use tua_core::{
    CodeAction, CodeActionEdit, CodeActionKind, Completion, CompletionKind, Definition, Diagnostic,
    DiagnosticCode, DiagnosticSeverity, DocumentHighlight, DocumentHighlightKind, DocumentSymbol,
    DocumentSymbolKind, FoldingRange, FoldingRangeKind, Hover, InlayHint, InlayHintKind,
    ModuleLink, Reference, RenameEdit, RenameTarget, SelectionRange, SemanticToken,
    SemanticTokenKind, SemanticTokenModifier, SignatureHelp, SignatureInformation, SourcePosition,
    SourceSpan, TextEdit, WorkspaceSymbol, WorkspaceSymbolKind,
};

use super::analysis::{
    CallHierarchyIdentity, CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyItemKind,
    CallHierarchyOutgoingCall,
};
use super::uri::uri_from_path;

pub(super) fn response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

pub(super) fn error_response(id: Value, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

pub(super) fn publish_diagnostics(uri: &str, diagnostics: Vec<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {
            "uri": uri,
            "diagnostics": diagnostics
        }
    })
}

pub(super) fn document_diagnostic_report(diagnostics: Vec<Value>) -> Value {
    json!({
        "kind": "full",
        "items": diagnostics
    })
}

pub(super) fn workspace_diagnostic_refresh_request(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "workspace/diagnostic/refresh",
        "params": null
    })
}

pub(super) fn lsp_diagnostic_from_core(diagnostic: Diagnostic) -> Value {
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
        "source": "tua",
        "message": diagnostic.message
    })
}

pub(super) fn core_diagnostic_from_lsp(
    path: &std::path::Path,
    diagnostic: &Value,
) -> Option<Diagnostic> {
    let start = diagnostic.get("range")?.get("start")?;
    let position = source_position_from_lsp(start)?;
    let code = diagnostic
        .get("code")
        .and_then(Value::as_str)
        .and_then(parse_diagnostic_code);
    let severity = diagnostic
        .get("severity")
        .and_then(Value::as_u64)
        .map(lsp_diagnostic_severity)
        .unwrap_or(DiagnosticSeverity::Warning);
    let message = diagnostic.get("message")?.as_str()?.to_owned();
    let mut output = Diagnostic::new(path, position.line, position.column, severity, message);

    if let Some(code) = code {
        output = output.with_code(code);
    }

    Some(output)
}

fn parse_diagnostic_code(code: &str) -> Option<DiagnosticCode> {
    let number = code.strip_prefix("TL")?.parse::<u16>().ok()?;

    DiagnosticCode::new(number)
}

fn lsp_diagnostic_severity(severity: u64) -> DiagnosticSeverity {
    match severity {
        1 => DiagnosticSeverity::Error,
        2 => DiagnosticSeverity::Warning,
        3 => DiagnosticSeverity::Info,
        4 => DiagnosticSeverity::Hint,
        _ => DiagnosticSeverity::Warning,
    }
}

fn lsp_severity(severity: DiagnosticSeverity) -> u8 {
    match severity {
        DiagnosticSeverity::Error => 1,
        DiagnosticSeverity::Warning => 2,
        DiagnosticSeverity::Info => 3,
        DiagnosticSeverity::Hint => 4,
    }
}

pub(super) fn lsp_hover_from_core(hover: Hover) -> Value {
    let mut markdown = format!("```tua\n{}\n```", hover.contents);

    if let Some(documentation) = &hover.documentation {
        markdown.push_str("\n\n");
        markdown.push_str(documentation);
    }

    if !hover.context.is_empty() {
        markdown.push_str("\n\n---\n");

        for item in &hover.context {
            markdown.push_str("\n- ");
            markdown.push_str(item);
        }
    }

    let mut result = json!({
        "contents": {
            "kind": "markdown",
            "value": markdown
        }
    });

    if let Some(span) = hover.span {
        result["range"] = lsp_range(span);
    }

    result
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompletionResolveData {
    pub(super) uri: String,
    pub(super) position: SourcePosition,
    pub(super) label: String,
    pub(super) kind: Option<CompletionKind>,
    pub(super) module_path: Option<PathBuf>,
    pub(super) source_module: Option<String>,
}

pub(super) fn lsp_completion_from_core(
    uri: &str,
    position: SourcePosition,
    completion: Completion,
) -> Value {
    let kind = lsp_completion_kind(completion.kind);
    let label = completion.label;
    let insert_text = completion.insert_text;
    let filter_text = completion.filter_text;
    let is_snippet = completion.is_snippet;
    let detail = completion.detail;
    let core_kind = completion.kind.as_str();
    let module_path = completion
        .module_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let source_module = completion.source_module.clone();
    let mut item = json!({
        "label": label.clone(),
        "kind": kind,
        "data": {
            "uri": uri,
            "label": label,
            "kind": core_kind,
            "position": lsp_position(position)
        }
    });

    if let Some(insert_text) = insert_text {
        item["insertText"] = json!(insert_text);
    }

    if let Some(filter_text) = filter_text {
        item["filterText"] = json!(filter_text);
    }

    if is_snippet {
        item["insertTextFormat"] = json!(2);
    }

    if let Some(detail) = detail {
        item["labelDetails"] = json!({
            "description": detail
        });
    }

    if let Some(module_path) = module_path {
        item["data"]["modulePath"] = json!(module_path);
    }

    if let Some(source_module) = source_module {
        item["data"]["sourceModule"] = json!(source_module);
    }

    item
}

pub(super) fn completion_resolve_data(item: &Value) -> Option<CompletionResolveData> {
    let data = item.get("data")?;
    let uri = data.get("uri")?.as_str()?.to_owned();
    let label = data.get("label")?.as_str()?.to_owned();
    let position = source_position_from_lsp(data.get("position")?);
    let kind = data
        .get("kind")
        .and_then(Value::as_str)
        .and_then(CompletionKind::parse_kind);
    let module_path = data
        .get("modulePath")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let source_module = data
        .get("sourceModule")
        .and_then(Value::as_str)
        .map(str::to_owned);

    Some(CompletionResolveData {
        uri,
        position: position?,
        label,
        kind,
        module_path,
        source_module,
    })
}

pub(super) fn lsp_resolved_completion_item(
    mut item: Value,
    completion: Option<Completion>,
) -> Value {
    let Some(completion) = completion else {
        return item;
    };

    if let Some(detail) = completion.detail {
        item["detail"] = json!(detail);
    }

    if let Some(documentation) = completion.documentation {
        item["documentation"] = json!({
            "kind": "markdown",
            "value": documentation
        });
    }

    if item.get("insertText").is_none() {
        if let Some(insert_text) = completion.insert_text {
            item["insertText"] = json!(insert_text);
        }
    }

    if item.get("filterText").is_none() {
        if let Some(filter_text) = completion.filter_text {
            item["filterText"] = json!(filter_text);
        }
    }

    if completion.is_snippet {
        item["insertTextFormat"] = json!(2);
    }

    if !completion.additional_text_edits.is_empty() {
        item["additionalTextEdits"] = json!(completion
            .additional_text_edits
            .into_iter()
            .map(lsp_text_edit)
            .collect::<Vec<_>>());
    }

    let type_label = item
        .get("detail")
        .and_then(Value::as_str)
        .map(str::to_owned);

    if let Some(data) = item.get_mut("data").and_then(Value::as_object_mut) {
        data.insert("symbolKind".to_owned(), json!(completion.kind.as_str()));

        if let Some(type_label) = type_label {
            data.insert("typeLabel".to_owned(), json!(type_label));
        }

        if let Some(module_path) = completion.module_path {
            data.insert(
                "modulePath".to_owned(),
                json!(module_path.to_string_lossy().to_string()),
            );
        }

        if let Some(source_module) = completion.source_module {
            data.insert("sourceModule".to_owned(), json!(source_module));
        }
    }

    item
}

pub(super) fn lsp_signature_help_from_core(help: SignatureHelp) -> Value {
    json!({
        "signatures": help
            .signatures
            .into_iter()
            .map(lsp_signature_information_from_core)
            .collect::<Vec<_>>(),
        "activeSignature": help.active_signature,
        "activeParameter": help.active_parameter
    })
}

fn lsp_signature_information_from_core(signature: SignatureInformation) -> Value {
    let mut item = json!({
        "label": signature.label,
        "parameters": signature
            .parameters
            .into_iter()
            .map(lsp_parameter_information_from_core)
            .collect::<Vec<_>>()
    });

    if let Some(documentation) = signature.documentation {
        item["documentation"] = json!({
            "kind": "markdown",
            "value": documentation
        });
    }

    item
}

fn lsp_parameter_information_from_core(parameter: tua_core::ParameterInformation) -> Value {
    let mut item = json!({
        "label": parameter.label
    });

    if let Some(documentation) = parameter.documentation {
        item["documentation"] = json!({
            "kind": "markdown",
            "value": documentation
        });
    }

    item
}

fn lsp_completion_kind(kind: CompletionKind) -> u8 {
    match kind {
        CompletionKind::Snippet => 15,
        CompletionKind::Function => 3,
        CompletionKind::TypeAlias => 25,
        CompletionKind::Field => 5,
        CompletionKind::Keyword => 14,
        CompletionKind::Module => 9,
        CompletionKind::Variable => 6,
        CompletionKind::Value => 12,
    }
}

pub(super) fn lsp_location_from_core(definition: Definition) -> Value {
    lsp_location(&uri_from_path(&definition.path), definition.span)
}

pub(super) fn lsp_call_hierarchy_item(item: CallHierarchyItem) -> Value {
    json!({
        "name": item.name,
        "kind": lsp_call_hierarchy_kind(item.kind),
        "uri": uri_from_path(&item.path),
        "range": lsp_range(item.range),
        "selectionRange": lsp_range(item.selection_range),
        "data": {
            "path": item.path.to_string_lossy().to_string(),
            "selectionRange": lsp_range(item.selection_range)
        }
    })
}

pub(super) fn call_hierarchy_identity_from_lsp(item: &Value) -> Option<CallHierarchyIdentity> {
    let data = item.get("data")?;
    let range = lsp_source_range(data.get("selectionRange")?)?;

    Some(CallHierarchyIdentity {
        path: PathBuf::from(data.get("path")?.as_str()?),
        selection_range: SourceSpan {
            start: range.start,
            end: range.end,
        },
    })
}

pub(super) fn lsp_call_hierarchy_incoming_call(call: CallHierarchyIncomingCall) -> Value {
    json!({
        "from": lsp_call_hierarchy_item(call.from),
        "fromRanges": call
            .from_ranges
            .into_iter()
            .map(lsp_range)
            .collect::<Vec<_>>()
    })
}

pub(super) fn lsp_call_hierarchy_outgoing_call(call: CallHierarchyOutgoingCall) -> Value {
    json!({
        "to": lsp_call_hierarchy_item(call.to),
        "fromRanges": call
            .from_ranges
            .into_iter()
            .map(lsp_range)
            .collect::<Vec<_>>()
    })
}

fn lsp_call_hierarchy_kind(kind: CallHierarchyItemKind) -> u8 {
    match kind {
        CallHierarchyItemKind::Function => 12,
        CallHierarchyItemKind::Method => 6,
    }
}

pub(super) fn lsp_document_link_from_core(link: ModuleLink) -> Value {
    json!({
        "range": lsp_range(link.span),
        "target": uri_from_path(&link.target)
    })
}

pub(super) fn lsp_document_highlight_from_core(highlight: DocumentHighlight) -> Value {
    json!({
        "range": lsp_range(highlight.span),
        "kind": lsp_document_highlight_kind(highlight.kind)
    })
}

fn lsp_document_highlight_kind(kind: DocumentHighlightKind) -> u8 {
    match kind {
        DocumentHighlightKind::Text => 1,
    }
}

pub(super) fn lsp_reference_location(reference: Reference) -> Value {
    lsp_location(&uri_from_path(&reference.path), reference.span)
}

pub(super) fn lsp_prepare_rename_from_core(target: RenameTarget) -> Value {
    json!({
        "range": lsp_range(target.span),
        "placeholder": target.placeholder
    })
}

pub(super) fn lsp_code_action_from_core(action: CodeAction) -> Value {
    json!({
        "title": action.title,
        "kind": lsp_code_action_kind(action.kind),
        "edit": lsp_code_action_edit(action.edit)
    })
}

fn lsp_code_action_kind(kind: CodeActionKind) -> &'static str {
    match kind {
        CodeActionKind::QuickFix => "quickfix",
    }
}

fn lsp_code_action_edit(edit: CodeActionEdit) -> Value {
    let raw_changes = edit.changes;
    let changes = lsp_changes(raw_changes.clone());

    if edit.create_files.is_empty() {
        return json!({
            "changes": changes
        });
    }

    let mut document_changes = edit
        .create_files
        .into_iter()
        .map(|file| {
            json!({
                "kind": "create",
                "uri": uri_from_path(&file.path),
                "options": {
                    "ignoreIfExists": file.ignore_if_exists
                }
            })
        })
        .collect::<Vec<_>>();

    for (uri, edits) in changes_for_document_changes(raw_changes) {
        document_changes.push(json!({
            "textDocument": {
                "uri": uri,
                "version": null
            },
            "edits": edits
        }));
    }

    json!({
        "documentChanges": document_changes
    })
}

pub(super) fn lsp_workspace_edit(edit: RenameEdit) -> Value {
    json!({
        "changes": lsp_changes(group_text_edits(edit.edits))
    })
}

fn group_text_edits(edits: Vec<TextEdit>) -> BTreeMap<std::path::PathBuf, Vec<TextEdit>> {
    let mut changes = BTreeMap::new();

    for edit in edits {
        changes
            .entry(edit.path.clone())
            .or_insert_with(Vec::new)
            .push(edit);
    }

    changes
}

fn lsp_changes(
    changes: BTreeMap<std::path::PathBuf, Vec<TextEdit>>,
) -> BTreeMap<String, Vec<Value>> {
    changes
        .into_iter()
        .map(|(path, edits)| {
            (
                uri_from_path(&path),
                edits.into_iter().map(lsp_text_edit).collect(),
            )
        })
        .collect()
}

fn changes_for_document_changes(
    changes: BTreeMap<std::path::PathBuf, Vec<TextEdit>>,
) -> Vec<(String, Vec<Value>)> {
    lsp_changes(changes).into_iter().collect()
}

pub(super) fn lsp_text_edit(edit: TextEdit) -> Value {
    json!({
        "range": lsp_range(edit.span),
        "newText": edit.new_text
    })
}

pub(super) fn lsp_full_document_text_edit(source: &str, new_text: String) -> Value {
    let (end_line, end_character) = lsp_document_end_position(source);

    json!({
        "range": {
            "start": {
                "line": 0,
                "character": 0
            },
            "end": {
                "line": end_line,
                "character": end_character
            }
        },
        "newText": new_text
    })
}

fn lsp_document_end_position(source: &str) -> (usize, usize) {
    let mut line = 0usize;
    let mut character = 0usize;

    for ch in source.chars() {
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }

    (line, character)
}

pub(super) fn lsp_workspace_symbol_from_core(symbol: WorkspaceSymbol) -> Value {
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
        WorkspaceSymbolKind::Local => 13,
        WorkspaceSymbolKind::Parameter => 13,
        WorkspaceSymbolKind::Field => 8,
        WorkspaceSymbolKind::Export => 8,
    }
}

pub(super) fn lsp_semantic_tokens_from_core(tokens: Vec<SemanticToken>) -> Value {
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
        SemanticTokenKind::Global => 7,
        SemanticTokenKind::Module => 8,
        SemanticTokenKind::Method => 9,
        SemanticTokenKind::AnnotationTag => 10,
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
        SemanticTokenModifier::Readonly => 1 << 1,
        SemanticTokenModifier::Deprecated => 1 << 2,
    }
}

pub(super) fn lsp_inlay_hint_from_core(hint: InlayHint) -> Value {
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

pub(super) fn lsp_document_symbol_from_core(symbol: DocumentSymbol) -> Value {
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

pub(super) fn lsp_selection_range_from_core(range: SelectionRange) -> Value {
    let mut item = json!({
        "range": lsp_range(range.range)
    });

    if let Some(parent) = range.parent {
        item["parent"] = lsp_selection_range_from_core(*parent);
    }

    item
}

pub(super) fn lsp_folding_range_from_core(range: FoldingRange) -> Value {
    json!({
        "startLine": range.range.start.line.saturating_sub(1),
        "startCharacter": range.range.start.column.saturating_sub(1),
        "endLine": range.range.end.line.saturating_sub(1),
        "endCharacter": range.range.end.column.saturating_sub(1),
        "kind": lsp_folding_range_kind(range.kind)
    })
}

fn lsp_folding_range_kind(kind: FoldingRangeKind) -> &'static str {
    match kind {
        FoldingRangeKind::Comment => "comment",
        FoldingRangeKind::Function | FoldingRangeKind::Table | FoldingRangeKind::TypeAlias => {
            "region"
        }
    }
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
pub(super) struct SourceRange {
    pub(super) start: SourcePosition,
    pub(super) end: SourcePosition,
}

impl SourceRange {
    pub(super) fn contains(self, position: SourcePosition) -> bool {
        position_key(self.start) <= position_key(position)
            && position_key(position) <= position_key(self.end)
    }
}

pub(super) fn lsp_source_range(range: &Value) -> Option<SourceRange> {
    Some(SourceRange {
        start: source_position_from_lsp(range.get("start")?)?,
        end: source_position_from_lsp(range.get("end")?)?,
    })
}

pub(super) fn source_position_from_lsp(position: &Value) -> Option<SourcePosition> {
    Some(SourcePosition {
        line: position.get("line")?.as_u64()? as usize + 1,
        column: position.get("character")?.as_u64()? as usize + 1,
    })
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}
