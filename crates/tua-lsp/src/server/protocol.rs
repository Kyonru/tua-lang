use serde_json::{json, Value};
use tua_core::{
    Completion, CompletionKind, Definition, Diagnostic, DiagnosticSeverity, DocumentHighlight,
    DocumentHighlightKind, DocumentSymbol, DocumentSymbolKind, Hover, InlayHint, InlayHintKind,
    ModuleLink, Reference, RenameEdit, RenameTarget, SemanticToken, SemanticTokenKind,
    SemanticTokenModifier, SignatureHelp, SignatureInformation, SourcePosition, SourceSpan,
    TextEdit, WorkspaceSymbol, WorkspaceSymbolKind,
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

fn lsp_severity(severity: DiagnosticSeverity) -> u8 {
    match severity {
        DiagnosticSeverity::Error => 1,
        DiagnosticSeverity::Warning => 2,
        DiagnosticSeverity::Info => 3,
        DiagnosticSeverity::Hint => 4,
    }
}

pub(super) fn lsp_hover_from_core(hover: Hover) -> Value {
    let mut result = json!({
        "contents": {
            "kind": "markdown",
            "value": format!("```tua\n{}\n```", hover.contents)
        }
    });

    if let Some(span) = hover.span {
        result["range"] = lsp_range(span);
    }

    result
}

pub(super) fn lsp_completion_from_core(completion: Completion) -> Value {
    let mut item = json!({
        "label": completion.label,
        "kind": lsp_completion_kind(completion.kind)
    });

    if let Some(detail) = completion.detail {
        item["detail"] = json!(detail);
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
            .map(|parameter| json!({ "label": parameter.label }))
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

fn lsp_completion_kind(kind: CompletionKind) -> u8 {
    match kind {
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

pub(super) fn lsp_reference_location(uri: &str, reference: Reference) -> Value {
    lsp_location(uri, reference.span)
}

pub(super) fn lsp_prepare_rename_from_core(target: RenameTarget) -> Value {
    json!({
        "range": lsp_range(target.span),
        "placeholder": target.placeholder
    })
}

pub(super) fn lsp_workspace_edit(uri: &str, edit: RenameEdit) -> Value {
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
    start: SourcePosition,
    end: SourcePosition,
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
