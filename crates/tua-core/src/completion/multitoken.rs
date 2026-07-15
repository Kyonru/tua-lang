use std::path::Path;

use crate::source_text::{byte_for_column, is_identifier_continue, is_identifier_start, line_at};
use crate::{parse_tua_source, Completion, CompletionKind, SourcePosition};

const DEFAULT_MAX_SUGGESTIONS: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionMultiTokenConfig {
    pub enabled: bool,
    pub advanced: bool,
    pub allow_model_integration: bool,
    pub max_suggestions: usize,
}

impl Default for CompletionMultiTokenConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            advanced: false,
            allow_model_integration: false,
            max_suggestions: DEFAULT_MAX_SUGGESTIONS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionMultiTokenInput {
    pub context: CompletionMultiTokenContext,
    pub primary_candidate_count: usize,
    pub advanced: bool,
    pub allow_model_integration: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionMultiTokenContext {
    pub kind: CompletionMultiTokenContextKind,
    pub line_prefix: String,
    pub subject: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionMultiTokenContextKind {
    AssignmentRhs,
    ExpressionTail,
    MethodChainEnding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionMultiTokenSuggestion {
    pub label: String,
    pub insert_text: String,
    pub rationale_tokens: Vec<String>,
}

pub(crate) fn complete_multi_token_source(
    path: &Path,
    source: &str,
    position: SourcePosition,
    primary_candidate_count: usize,
    config: &CompletionMultiTokenConfig,
) -> Vec<Completion> {
    if !config.enabled || config.max_suggestions == 0 {
        return Vec::new();
    }

    let Some(context) = multi_token_context(source, position) else {
        return Vec::new();
    };

    if !config.advanced && !local_confidence_is_low(primary_candidate_count) {
        return Vec::new();
    }

    let input = CompletionMultiTokenInput {
        context,
        primary_candidate_count,
        advanced: config.advanced,
        allow_model_integration: config.allow_model_integration,
    };

    local_multi_token_suggestions(&input)
        .into_iter()
        .filter_map(|suggestion| validated_completion(path, source, position, suggestion))
        .take(config.max_suggestions)
        .collect()
}

fn local_confidence_is_low(primary_candidate_count: usize) -> bool {
    primary_candidate_count <= 1
}

fn multi_token_context(
    source: &str,
    position: SourcePosition,
) -> Option<CompletionMultiTokenContext> {
    let line = line_at(source, position.line)?;
    let byte = byte_for_column(line, position.column).min(line.len());
    let line_prefix = line[..byte].to_owned();
    let trimmed = line_prefix.trim_end();

    if trimmed.ends_with('=') {
        let subject = assignment_subject(trimmed);

        return Some(CompletionMultiTokenContext {
            kind: CompletionMultiTokenContextKind::AssignmentRhs,
            line_prefix,
            subject,
        });
    }

    if trimmed.ends_with(')') {
        return Some(CompletionMultiTokenContext {
            kind: CompletionMultiTokenContextKind::MethodChainEnding,
            line_prefix,
            subject: None,
        });
    }

    if let Some(subject) = return_expression_subject(trimmed) {
        return Some(CompletionMultiTokenContext {
            kind: CompletionMultiTokenContextKind::ExpressionTail,
            line_prefix,
            subject: Some(subject),
        });
    }

    None
}

fn local_multi_token_suggestions(
    input: &CompletionMultiTokenInput,
) -> Vec<CompletionMultiTokenSuggestion> {
    match input.context.kind {
        CompletionMultiTokenContextKind::AssignmentRhs => assignment_suggestions(input),
        CompletionMultiTokenContextKind::ExpressionTail => expression_tail_suggestions(input),
        CompletionMultiTokenContextKind::MethodChainEnding => method_chain_suggestions(input),
    }
}

fn assignment_suggestions(
    input: &CompletionMultiTokenInput,
) -> Vec<CompletionMultiTokenSuggestion> {
    let Some(subject) = input.context.subject.as_deref() else {
        return Vec::new();
    };
    let lower = subject.to_ascii_lowercase();

    if lower.contains("velocity") || lower.contains("position") || lower == "pos" {
        return vec![CompletionMultiTokenSuggestion {
            label: "vec2(0, 0)".to_owned(),
            insert_text: "vec2(0, 0)".to_owned(),
            rationale_tokens: vec!["assignment-rhs".to_owned(), "vector-name".to_owned()],
        }];
    }

    Vec::new()
}

fn expression_tail_suggestions(
    input: &CompletionMultiTokenInput,
) -> Vec<CompletionMultiTokenSuggestion> {
    let Some(subject) = input.context.subject.as_deref() else {
        return Vec::new();
    };

    if subject == "nil" || subject == "false" || subject == "true" {
        return Vec::new();
    }

    vec![CompletionMultiTokenSuggestion {
        label: "or nil".to_owned(),
        insert_text: " or nil".to_owned(),
        rationale_tokens: vec!["expression-tail".to_owned(), "fallback".to_owned()],
    }]
}

fn method_chain_suggestions(
    input: &CompletionMultiTokenInput,
) -> Vec<CompletionMultiTokenSuggestion> {
    let lower = input.context.line_prefix.to_ascii_lowercase();

    if !(lower.contains("sound")
        || lower.contains("source")
        || lower.contains("audio")
        || lower.contains("music"))
    {
        return Vec::new();
    }

    vec![CompletionMultiTokenSuggestion {
        label: ":play()".to_owned(),
        insert_text: ":play()".to_owned(),
        rationale_tokens: vec!["method-chain".to_owned(), "audio-name".to_owned()],
    }]
}

fn validated_completion(
    path: &Path,
    source: &str,
    position: SourcePosition,
    suggestion: CompletionMultiTokenSuggestion,
) -> Option<Completion> {
    let insert_text = trim_to_valid_insert_text(path, source, position, &suggestion.insert_text)?;
    let documentation = format!(
        "Validated multi-token suggestion. Rationale: {}.",
        suggestion.rationale_tokens.join(", ")
    );

    Some(Completion {
        label: suggestion.label,
        kind: CompletionKind::Snippet,
        detail: Some("multi-token suggestion".to_owned()),
        documentation: Some(documentation),
        insert_text: Some(insert_text),
        filter_text: None,
        is_snippet: false,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    })
}

fn trim_to_valid_insert_text(
    path: &Path,
    source: &str,
    position: SourcePosition,
    insert_text: &str,
) -> Option<String> {
    let trimmed = insert_text.trim_end();
    let candidates = if trimmed == insert_text {
        vec![insert_text]
    } else {
        vec![insert_text, trimmed]
    };

    candidates.into_iter().find_map(|candidate| {
        let edited = insert_text_at_position(source, position, candidate)?;

        parse_tua_source(path, edited)
            .is_ok()
            .then(|| candidate.to_owned())
    })
}

fn insert_text_at_position(
    source: &str,
    position: SourcePosition,
    insert_text: &str,
) -> Option<String> {
    let offset = byte_offset_for_position(source, position)?;
    let mut edited = String::with_capacity(source.len() + insert_text.len());

    edited.push_str(&source[..offset]);
    edited.push_str(insert_text);
    edited.push_str(&source[offset..]);

    Some(edited)
}

fn byte_offset_for_position(source: &str, position: SourcePosition) -> Option<usize> {
    let mut offset = 0;

    for (index, line) in source.split_inclusive('\n').enumerate() {
        let line_number = index + 1;
        let without_newline = line.strip_suffix('\n').unwrap_or(line);
        let without_return = without_newline
            .strip_suffix('\r')
            .unwrap_or(without_newline);

        if line_number == position.line {
            return Some(
                offset + byte_for_column(without_return, position.column).min(without_return.len()),
            );
        }

        offset += line.len();
    }

    (position.line == source.lines().count() + 1 && position.column == 1).then_some(source.len())
}

fn assignment_subject(trimmed_line_prefix: &str) -> Option<String> {
    let before_equals = trimmed_line_prefix.strip_suffix('=')?.trim_end();
    let name = identifier_before(before_equals, before_equals.len())?;

    Some(name)
}

fn return_expression_subject(trimmed_line_prefix: &str) -> Option<String> {
    let expression = trimmed_line_prefix.strip_prefix("return ")?;
    let subject = identifier_before(expression, expression.len())?;

    (!subject.is_empty()).then_some(subject)
}

fn identifier_before(source: &str, end: usize) -> Option<String> {
    let bytes = source.as_bytes();

    if end == 0 || !is_identifier_continue(*bytes.get(end - 1)?) {
        return None;
    }

    let mut start = end - 1;

    while start > 0 && is_identifier_continue(bytes[start - 1]) {
        start -= 1;
    }

    is_identifier_start(bytes[start]).then(|| source[start..end].to_owned())
}
