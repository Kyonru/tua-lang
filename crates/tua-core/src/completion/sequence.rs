use std::time::{Duration, Instant};

use crate::source_text::{byte_for_column, is_identifier_continue, line_at};
use crate::{Completion, SourcePosition};

const DEFAULT_TIMEOUT_MS: u64 = 4;
const DEFAULT_MAX_CANDIDATES: usize = 64;
const MAX_CONTEXT_CHARS: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionSequenceRankingConfig {
    pub enabled: bool,
    pub timeout_ms: u64,
    pub max_candidates: usize,
    pub provider: CompletionSequenceRankingProvider,
}

impl Default for CompletionSequenceRankingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            max_candidates: DEFAULT_MAX_CANDIDATES,
            provider: CompletionSequenceRankingProvider::Local,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionSequenceRankingProvider {
    Local,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionSequenceRankingInput {
    pub context: CompletionSequenceContextSnapshot,
    pub candidates: Vec<CompletionSequenceCandidate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionSequenceContextSnapshot {
    pub prefix: String,
    pub receiver_parts: Vec<String>,
    pub normalized_line_prefix: String,
    pub normalized_before_cursor: String,
}

impl CompletionSequenceContextSnapshot {
    pub(crate) fn from_source(
        source: &str,
        position: SourcePosition,
        prefix: &str,
        receiver_parts: &[String],
    ) -> Self {
        let line_prefix = line_at(source, position.line)
            .map(|line| {
                let byte = byte_for_column(line, position.column);

                normalize_context(&line[..byte.min(line.len())])
            })
            .unwrap_or_default();
        let before_cursor = source_before_position(source, position);
        let before_cursor = before_cursor
            .get(before_cursor.len().saturating_sub(MAX_CONTEXT_CHARS)..)
            .unwrap_or(before_cursor);

        Self {
            prefix: prefix.to_owned(),
            receiver_parts: receiver_parts.to_vec(),
            normalized_line_prefix: line_prefix,
            normalized_before_cursor: normalize_context(before_cursor),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionSequenceCandidate {
    pub index: usize,
    pub label: String,
    pub kind: String,
    pub detail: Option<String>,
    pub has_documentation: bool,
}

impl CompletionSequenceCandidate {
    pub(crate) fn from_completion(index: usize, completion: &Completion) -> Self {
        Self {
            index,
            label: completion.label.clone(),
            kind: completion.kind.as_str().to_owned(),
            detail: completion.detail.clone(),
            has_documentation: completion.documentation.is_some(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionSequenceScore {
    pub candidate_index: usize,
    pub score: i32,
    pub rationale_tokens: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompletionSequenceRankingError {
    Timeout,
    ProviderUnavailable,
}

#[derive(Clone, Copy, Debug)]
pub struct CompletionSequenceRankingBudget {
    deadline: Instant,
}

impl CompletionSequenceRankingBudget {
    fn new(timeout_ms: u64) -> Self {
        Self {
            deadline: Instant::now() + Duration::from_millis(timeout_ms),
        }
    }

    pub fn expired(self) -> bool {
        Instant::now() >= self.deadline
    }
}

pub trait CompletionSequenceRanker {
    fn score(
        &self,
        input: &CompletionSequenceRankingInput,
        budget: CompletionSequenceRankingBudget,
    ) -> Result<Vec<CompletionSequenceScore>, CompletionSequenceRankingError>;
}

#[derive(Clone, Debug, Default)]
struct LocalCompletionSequenceRanker;

impl CompletionSequenceRanker for LocalCompletionSequenceRanker {
    fn score(
        &self,
        input: &CompletionSequenceRankingInput,
        budget: CompletionSequenceRankingBudget,
    ) -> Result<Vec<CompletionSequenceScore>, CompletionSequenceRankingError> {
        let before_tokens = context_tokens(&input.context.normalized_before_cursor);
        let line_tokens = context_tokens(&input.context.normalized_line_prefix);
        let mut scores = Vec::new();

        for candidate in &input.candidates {
            if budget.expired() {
                return Err(CompletionSequenceRankingError::Timeout);
            }

            let context_count = exact_token_count(&before_tokens, &candidate.label);
            let line_count = exact_token_count(&line_tokens, &candidate.label);
            let score = context_count.saturating_mul(120) + line_count.saturating_mul(40);

            if score == 0 {
                continue;
            }

            let mut rationale_tokens = vec!["context-token-frequency".to_owned()];

            if line_count > 0 {
                rationale_tokens.push("current-line".to_owned());
            }

            scores.push(CompletionSequenceScore {
                candidate_index: candidate.index,
                score: i32::try_from(score).unwrap_or(i32::MAX),
                rationale_tokens,
            });
        }

        Ok(scores)
    }
}

pub(crate) fn score_completion_sequence(
    config: &CompletionSequenceRankingConfig,
    input: CompletionSequenceRankingInput,
) -> Vec<CompletionSequenceScore> {
    if !config.enabled || config.max_candidates == 0 || config.timeout_ms == 0 {
        return Vec::new();
    }

    let mut input = input;

    input.candidates.truncate(config.max_candidates);

    let budget = CompletionSequenceRankingBudget::new(config.timeout_ms);
    let result = match config.provider {
        CompletionSequenceRankingProvider::Local => {
            LocalCompletionSequenceRanker.score(&input, budget)
        }
    };

    result.unwrap_or_default()
}

fn source_before_position(source: &str, position: SourcePosition) -> &str {
    let mut offset = 0;

    for (index, line) in source.split_inclusive('\n').enumerate() {
        let line_number = index + 1;

        if line_number == position.line {
            let line = line.strip_suffix('\n').unwrap_or(line);
            let byte = byte_for_column(line.strip_suffix('\r').unwrap_or(line), position.column);

            return &source[..offset + byte.min(line.len())];
        }

        offset += line.len();
    }

    source
}

fn normalize_context(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut result = String::new();
    let mut index = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut line_comment = false;
    let mut needs_space = false;

    while index < bytes.len() {
        let byte = bytes[index];

        if byte == b'\n' {
            line_comment = false;
            push_space(&mut result, &mut needs_space);
            index += 1;
            continue;
        }

        if line_comment {
            index += 1;
            continue;
        }

        if let Some(quote_byte) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote_byte {
                quote = None;
                push_token(&mut result, &mut needs_space, "<string>");
            }

            index += 1;
            continue;
        }

        if byte == b'-' && bytes.get(index + 1) == Some(&b'-') {
            line_comment = true;
            index += 2;
            continue;
        }

        if matches!(byte, b'\'' | b'"') {
            quote = Some(byte);
            index += 1;
            continue;
        }

        if byte.is_ascii_digit() {
            while index < bytes.len() && (bytes[index].is_ascii_digit() || bytes[index] == b'.') {
                index += 1;
            }
            push_token(&mut result, &mut needs_space, "<number>");
            continue;
        }

        if byte == b'_' || byte.is_ascii_alphabetic() {
            let start = index;

            index += 1;
            while index < bytes.len() && is_identifier_continue(bytes[index]) {
                index += 1;
            }

            push_token(&mut result, &mut needs_space, &source[start..index]);
            continue;
        }

        if byte.is_ascii_whitespace() {
            push_space(&mut result, &mut needs_space);
            index += 1;
            continue;
        }

        if byte.is_ascii() {
            push_token(&mut result, &mut needs_space, &source[index..index + 1]);
        } else {
            push_space(&mut result, &mut needs_space);
        }

        index += source[index..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1);
    }

    result
}

fn context_tokens(context: &str) -> Vec<&str> {
    context
        .split(|character: char| !(character == '_' || character.is_ascii_alphanumeric()))
        .filter(|token| !token.is_empty())
        .collect()
}

fn exact_token_count(tokens: &[&str], label: &str) -> usize {
    tokens.iter().filter(|token| **token == label).count()
}

fn push_token(result: &mut String, needs_space: &mut bool, token: &str) {
    if *needs_space && !result.is_empty() {
        result.push(' ');
    }

    result.push_str(token);
    *needs_space = true;
}

fn push_space(result: &mut String, needs_space: &mut bool) {
    if !result.is_empty() {
        *needs_space = true;
    }
}
