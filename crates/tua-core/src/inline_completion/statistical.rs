//! Conservative project-local statistical completion.
//!
//! Tables are immutable, versioned by the surrounding project corpus, and
//! contain no source buffers. Identifier slots come only from the shared
//! semantic scope map; when current-source scope facts are unavailable the
//! production query abstains instead of guessing bindings.

use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use full_moon::ast::LuaVersion;
use full_moon::tokenizer::{Lexer, LexerResult, TokenType};

use crate::{ScopeDeclarationKind, ScopeMap, ScopeResolution, SourceSpan};

use super::InlineCompletionBudget;

const MAX_ORDER: usize = 6;
const MAX_OUTPUT_TOKENS: usize = 12;
const BEAM_WIDTH: usize = 3;
const MAX_STEP_CANDIDATES: usize = 128;
const MIN_SUPPORTING_FILES: usize = 2;
const JM_PSEUDO_COUNT: f64 = 10.0;
const MAX_MEAN_SURPRISE: f64 = 3.0;
pub(crate) const MAX_SOURCE_BYTES: usize = 128 * 1024;
pub(crate) const MAX_FILE_BYTES: usize = 128 * 1024;
pub(crate) const MAX_QUERY_FILES: usize = 64;
pub(crate) const MAX_QUERY_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MIN_SCORE: f64 = 0.85;
pub(crate) const MAX_SCOPE_FACT_FILES: usize = 128;
pub(crate) const MAX_SCOPE_FACT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum RawToken {
    Symbol(String),
    ConcreteIdentifier(String),
    SlotIdentifier(String),
    Number(String),
    QuotedString(String),
    UnknownString,
    Newline,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ModelToken {
    Symbol(String),
    ConcreteIdentifier(String),
    Slot(u8),
    Number(String),
    QuotedString(String),
    UnknownNumber,
    UnknownString,
    Newline,
}

#[derive(Clone, Debug)]
pub(crate) struct StatisticalFileTable {
    counts: BTreeMap<Vec<ModelToken>, BTreeMap<ModelToken, u32>>,
    approximate_bytes: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct StatisticalScopeFacts {
    local_spans: Arc<BTreeSet<(usize, usize, usize, usize)>>,
    valid_through_offset: usize,
}

impl StatisticalScopeFacts {
    pub(crate) fn from_scope(scope: &ScopeMap) -> Self {
        Self {
            local_spans: Arc::new(local_identifier_spans(scope)),
            valid_through_offset: usize::MAX,
        }
    }

    pub(crate) fn restricted_to_common_prefix(mut self, bytes: usize) -> Self {
        self.valid_through_offset = bytes;
        self
    }

    pub(crate) fn approximate_bytes(&self) -> usize {
        size_of::<Self>()
            + self.local_spans.len()
                * (size_of::<(usize, usize, usize, usize)>() + 3 * size_of::<usize>())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StatisticalPrediction {
    pub text: String,
    pub score: f64,
}

pub(crate) struct StatisticalEvidence<'a> {
    pub table: &'a StatisticalFileTable,
    pub weight: f64,
}

#[derive(Clone)]
struct Beam {
    context: Vec<RawToken>,
    output: Vec<RawToken>,
    negative_log_probability: f64,
}

impl StatisticalFileTable {
    pub(crate) fn build(source: &str, scope: &ScopeMap) -> Option<Self> {
        let tokens = tokenize(source, &StatisticalScopeFacts::from_scope(scope), 0)?;
        let mut counts = BTreeMap::<Vec<ModelToken>, BTreeMap<ModelToken, u32>>::new();
        let mut approximate_bytes = size_of::<Self>();
        'tokens: for next_index in 1..tokens.len() {
            for order in 1..=MAX_ORDER.min(next_index) {
                let normalized = normalize(&tokens[next_index - order..=next_index]).0;
                let (key, next) = normalized.split_at(normalized.len() - 1);
                let existing = counts.get(key);
                let added_context = usize::from(existing.is_none())
                    * (size_of::<(Vec<ModelToken>, BTreeMap<ModelToken, u32>)>()
                        + 64
                        + key.iter().map(model_token_bytes).sum::<usize>());
                let added_value =
                    usize::from(existing.is_none_or(|values| !values.contains_key(&next[0])))
                        * (size_of::<(ModelToken, u32)>() + 32 + model_token_bytes(&next[0]));
                let added = added_context.saturating_add(added_value);
                if approximate_bytes.saturating_add(added) > MAX_FILE_BYTES {
                    break 'tokens;
                }
                *counts
                    .entry(key.to_vec())
                    .or_default()
                    .entry(next[0].clone())
                    .or_default() += 1;
                approximate_bytes += added;
            }
        }
        (!counts.is_empty()).then_some(Self {
            counts,
            approximate_bytes,
        })
    }

    pub(crate) fn approximate_bytes(&self) -> usize {
        self.approximate_bytes
    }
}

pub(crate) fn predict(
    evidence: &[StatisticalEvidence<'_>],
    document: &str,
    offset: usize,
    scope: &StatisticalScopeFacts,
    budget: InlineCompletionBudget,
    cancelled: &AtomicBool,
) -> Option<StatisticalPrediction> {
    if evidence.is_empty()
        || offset == 0
        || offset > document.len()
        || offset > scope.valid_through_offset
        || cancelled.load(Ordering::Relaxed)
        || budget.is_expired()
    {
        return None;
    }
    let prefix = &document[..offset];
    if prefix
        .chars()
        .next_back()
        .is_some_and(is_identifier_character)
    {
        return None;
    }
    let line_start = prefix.rfind('\n').map_or(0, |newline| newline + 1);
    let base_line = prefix[..line_start]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count();
    let mut context = tokenize(&prefix[line_start..], scope, base_line)?;
    while context.last() == Some(&RawToken::Newline) {
        context.pop();
    }
    if context.is_empty() {
        return None;
    }

    let initial = Beam {
        context,
        output: Vec::new(),
        negative_log_probability: 0.0,
    };
    let mut beams = vec![initial.clone()];
    let mut completed = Vec::new();
    for _ in 0..MAX_OUTPUT_TOKENS {
        if cancelled.load(Ordering::Relaxed) || budget.is_expired() {
            return None;
        }
        let mut next = Vec::new();
        for beam in std::mem::take(&mut beams) {
            let Some(distribution) = distribution(evidence, &beam.context, budget, cancelled)
            else {
                if !beam.output.is_empty() {
                    completed.push(beam);
                }
                continue;
            };
            for (token, probability) in distribution.into_iter().take(BEAM_WIDTH) {
                if token == RawToken::Newline {
                    if !beam.output.is_empty() {
                        completed.push(beam.clone());
                    }
                    continue;
                }
                let mut candidate = beam.clone();
                candidate.negative_log_probability -= probability.max(f64::MIN_POSITIVE).ln();
                if candidate.negative_log_probability / (candidate.output.len() + 1) as f64
                    > MAX_MEAN_SURPRISE
                {
                    continue;
                }
                candidate.context.push(token.clone());
                candidate.output.push(token);
                next.push(candidate);
            }
        }
        if next.is_empty() {
            break;
        }
        next.sort_by(|left, right| {
            beam_score(right)
                .total_cmp(&beam_score(left))
                .then_with(|| left.output.cmp(&right.output))
        });
        next.truncate(BEAM_WIDTH);
        completed.extend(
            next.iter()
                .filter(|beam| can_end(beam.output.last()))
                .cloned(),
        );
        beams = next;
    }
    completed.extend(beams);
    if cancelled.load(Ordering::Relaxed) || budget.is_expired() {
        return None;
    }
    let best = completed
        .into_iter()
        .filter(|beam| !beam.output.is_empty())
        .max_by(|left, right| {
            beam_score(left)
                .total_cmp(&beam_score(right))
                .then_with(|| right.output.len().cmp(&left.output.len()))
        })?;
    let score = beam_score(&best);
    if score < MIN_SCORE {
        return None;
    }
    let mut text = render(&best.context, &best.output);
    if prefix.chars().next_back().is_some_and(char::is_whitespace) {
        text = text.trim_start().to_owned();
    }
    (!text.is_empty()).then_some(StatisticalPrediction { text, score })
}

fn distribution(
    evidence: &[StatisticalEvidence<'_>],
    context: &[RawToken],
    budget: InlineCompletionBudget,
    cancelled: &AtomicBool,
) -> Option<Vec<(RawToken, f64)>> {
    let mut by_order = Vec::<(BTreeMap<RawToken, f64>, BTreeMap<RawToken, usize>, f64)>::new();
    for order in 1..=MAX_ORDER.min(context.len()) {
        if cancelled.load(Ordering::Relaxed) || budget.is_expired() {
            return None;
        }
        let (key, bindings) = normalize(&context[context.len() - order..]);
        let mut resolved = BTreeMap::new();
        let mut supporters = BTreeMap::new();
        let mut total = 0.0;
        for (source_index, source) in evidence.iter().enumerate() {
            if source_index.is_multiple_of(8)
                && (cancelled.load(Ordering::Relaxed) || budget.is_expired())
            {
                return None;
            }
            let Some(next) = source.table.counts.get(&key) else {
                continue;
            };
            let mut source_supported = BTreeSet::new();
            for (token_index, (token, count)) in next.iter().enumerate() {
                if token_index.is_multiple_of(32)
                    && (cancelled.load(Ordering::Relaxed) || budget.is_expired())
                {
                    return None;
                }
                let weighted = f64::from(*count) * source.weight;
                total += weighted;
                if let Some(token) = resolve(token, &bindings) {
                    source_supported.insert(token.clone());
                    *resolved.entry(token).or_default() += weighted;
                }
            }
            for token in source_supported {
                *supporters.entry(token).or_default() += 1;
            }
        }
        by_order.push((resolved, supporters, total));
    }
    let mut candidates = BTreeSet::new();
    for (resolved, supporters, _) in &by_order {
        for token in resolved.keys().filter(|token| {
            supporters.get(*token).copied().unwrap_or_default() >= MIN_SUPPORTING_FILES
        }) {
            if candidates.len() >= MAX_STEP_CANDIDATES && !candidates.contains(token) {
                return None;
            }
            candidates.insert(token.clone());
        }
    }
    let mut scored = Vec::new();
    for candidate in candidates {
        let mut probability = 0.0;
        for (resolved, _, total) in &by_order {
            if *total == 0.0 {
                continue;
            }
            let lambda = *total / (*total + JM_PSEUDO_COUNT);
            let local = resolved.get(&candidate).copied().unwrap_or_default() / *total;
            probability = lambda * local + (1.0 - lambda) * probability;
        }
        if probability > 0.0 {
            scored.push((candidate, probability));
        }
    }
    scored.sort_by(|(left_token, left), (right_token, right)| {
        right
            .total_cmp(left)
            .then_with(|| left_token.cmp(right_token))
    });
    (!scored.is_empty()).then_some(scored)
}

fn tokenize(
    source: &str,
    scope: &StatisticalScopeFacts,
    base_line: usize,
) -> Option<Vec<RawToken>> {
    let tokens = match Lexer::new(source, LuaVersion::luau()).collect() {
        LexerResult::Ok(tokens) | LexerResult::Recovered(tokens, _) => tokens,
        LexerResult::Fatal(_) => return None,
    };
    let mut result = Vec::new();
    for token in tokens {
        match token.token_type() {
            TokenType::Eof => {}
            TokenType::Whitespace { characters } => push_newlines(&mut result, characters),
            TokenType::SingleLineComment { .. } | TokenType::Shebang { .. } => {}
            TokenType::MultiLineComment { comment, .. } => push_newlines(&mut result, comment),
            TokenType::StringLiteral { literal, .. } => result.push(if common_string(literal) {
                RawToken::QuotedString(token.to_string())
            } else {
                RawToken::UnknownString
            }),
            TokenType::InterpolatedString { .. } => result.push(RawToken::UnknownString),
            TokenType::Number { text } => result.push(RawToken::Number(text.to_string())),
            TokenType::Identifier { identifier } => {
                let mut span = SourceSpan::from_range(token.start_position(), token.end_position());
                span.start.line += base_line;
                span.end.line += base_line;
                let member = matches!(result.last(), Some(RawToken::Symbol(symbol)) if symbol == "." || symbol == ":");
                result.push(if !member && scope.local_spans.contains(&span_key(span)) {
                    RawToken::SlotIdentifier(identifier.to_string())
                } else {
                    RawToken::ConcreteIdentifier(identifier.to_string())
                });
            }
            TokenType::Symbol { .. } => result.push(RawToken::Symbol(token.to_string())),
            _ => return None,
        }
    }
    Some(result)
}

fn local_identifier_spans(scope: &ScopeMap) -> BTreeSet<(usize, usize, usize, usize)> {
    let local_symbol = |id| {
        scope.declaration(id).is_some_and(|declaration| {
            matches!(
                declaration.kind,
                ScopeDeclarationKind::Local
                    | ScopeDeclarationKind::Parameter
                    | ScopeDeclarationKind::Function
                    | ScopeDeclarationKind::ModuleAlias { .. }
            )
        })
    };
    scope
        .declarations()
        .iter()
        .filter(|declaration| local_symbol(declaration.id))
        .map(|declaration| span_key(declaration.name_span))
        .chain(scope.references().iter().filter_map(|reference| {
            let ScopeResolution::Symbol(id) = reference.resolution else {
                return None;
            };
            local_symbol(id).then_some(span_key(reference.span))
        }))
        .collect()
}

fn span_key(span: SourceSpan) -> (usize, usize, usize, usize) {
    (
        span.start.line,
        span.start.column,
        span.end.line,
        span.end.column,
    )
}

fn normalize(tokens: &[RawToken]) -> (Vec<ModelToken>, BTreeMap<u8, RawToken>) {
    let mut names = BTreeMap::<String, u8>::new();
    let mut bindings = BTreeMap::new();
    let mut next_slot = 1_u8;
    let normalized = tokens
        .iter()
        .map(|token| match token {
            RawToken::Symbol(text) => ModelToken::Symbol(text.clone()),
            RawToken::ConcreteIdentifier(text) => ModelToken::ConcreteIdentifier(text.clone()),
            RawToken::SlotIdentifier(text) => {
                let slot = *names.entry(text.clone()).or_insert_with(|| {
                    let slot = next_slot;
                    next_slot = next_slot.saturating_add(1);
                    slot
                });
                bindings.entry(slot).or_insert_with(|| token.clone());
                ModelToken::Slot(slot)
            }
            RawToken::Number(text) if common_number(text) => ModelToken::Number(text.clone()),
            RawToken::Number(_) => ModelToken::UnknownNumber,
            RawToken::QuotedString(text) => ModelToken::QuotedString(text.clone()),
            RawToken::UnknownString => ModelToken::UnknownString,
            RawToken::Newline => ModelToken::Newline,
        })
        .collect();
    (normalized, bindings)
}

fn resolve(token: &ModelToken, bindings: &BTreeMap<u8, RawToken>) -> Option<RawToken> {
    match token {
        ModelToken::Symbol(text) => Some(RawToken::Symbol(text.clone())),
        ModelToken::ConcreteIdentifier(text) => Some(RawToken::ConcreteIdentifier(text.clone())),
        ModelToken::Slot(slot) => bindings.get(slot).cloned(),
        ModelToken::Number(text) => Some(RawToken::Number(text.clone())),
        ModelToken::QuotedString(text) => Some(RawToken::QuotedString(text.clone())),
        ModelToken::Newline => Some(RawToken::Newline),
        ModelToken::UnknownNumber | ModelToken::UnknownString => None,
    }
}

fn push_newlines(tokens: &mut Vec<RawToken>, text: &str) {
    if text.contains('\n') && tokens.last() != Some(&RawToken::Newline) {
        tokens.push(RawToken::Newline);
    }
}

fn common_number(text: &str) -> bool {
    matches!(text, "0" | "1" | "2" | "255" | "0.5")
}

fn common_string(text: &str) -> bool {
    matches!(text, "fill" | "line")
}

fn beam_score(beam: &Beam) -> f64 {
    (-beam.negative_log_probability / beam.output.len().max(1) as f64).exp()
}

fn can_end(token: Option<&RawToken>) -> bool {
    !matches!(token, Some(RawToken::Symbol(symbol)) if matches!(symbol.as_str(), "." | ":" | "," | "+" | "-" | "*" | "/" | "=" | "(" | "[" | "{"))
}

fn render(context: &[RawToken], output: &[RawToken]) -> String {
    let prefix_len = context.len().saturating_sub(output.len());
    let mut text = String::new();
    let mut previous = prefix_len
        .checked_sub(1)
        .and_then(|index| context.get(index));
    let mut before_previous = prefix_len
        .checked_sub(2)
        .and_then(|index| context.get(index));
    for token in output {
        if token == &RawToken::Newline {
            break;
        }
        if needs_space(before_previous, previous, token) {
            text.push(' ');
        }
        text.push_str(token_text(token));
        before_previous = previous;
        previous = Some(token);
    }
    text
}

fn token_text(token: &RawToken) -> &str {
    match token {
        RawToken::Symbol(text)
        | RawToken::ConcreteIdentifier(text)
        | RawToken::SlotIdentifier(text)
        | RawToken::Number(text)
        | RawToken::QuotedString(text) => text,
        RawToken::UnknownString => "",
        RawToken::Newline => "\n",
    }
}

fn needs_space(
    before_previous: Option<&RawToken>,
    previous: Option<&RawToken>,
    current: &RawToken,
) -> bool {
    let current_text = token_text(current);
    if matches!(current_text, ")" | "]" | "}" | "," | ";" | "." | ":") {
        return false;
    }
    let Some(previous) = previous else {
        return false;
    };
    let previous_text = token_text(previous);
    if matches!(previous_text, "(" | "[" | "{" | "." | ":") {
        return false;
    }
    if current_text == "(" {
        return matches!(previous, RawToken::Symbol(_));
    }
    if current_text == "-" && unary_position(Some(previous)) {
        return !matches!(previous_text, "(" | "[" | "{" | ",");
    }
    if previous_text == "-" {
        return !unary_position(before_previous);
    }
    is_word(previous) && is_word(current)
        || is_binary_operator(previous_text)
        || is_binary_operator(current_text)
        || previous_text == ","
}

fn unary_position(previous: Option<&RawToken>) -> bool {
    previous.is_none_or(|token| {
        matches!(token, RawToken::Symbol(text) if matches!(text.as_str(), "(" | "[" | "{" | "," | "=" | "+" | "-" | "*" | "/"))
    })
}

fn is_word(token: &RawToken) -> bool {
    matches!(
        token,
        RawToken::ConcreteIdentifier(_)
            | RawToken::SlotIdentifier(_)
            | RawToken::Number(_)
            | RawToken::QuotedString(_)
    ) || matches!(token, RawToken::Symbol(text) if matches!(text.as_str(), "and" | "do" | "else" | "elseif" | "end" | "false" | "for" | "function" | "if" | "in" | "local" | "nil" | "not" | "or" | "repeat" | "return" | "then" | "true" | "until" | "while"))
}

fn is_binary_operator(text: &str) -> bool {
    matches!(
        text,
        "+" | "-"
            | "*"
            | "/"
            | "//"
            | "%"
            | "^"
            | "="
            | "=="
            | "~="
            | "<"
            | ">"
            | "<="
            | ">="
            | ".."
            | "+="
            | "-="
            | "*="
            | "/="
            | "->"
            | "and"
            | "or"
    )
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

fn model_token_bytes(token: &ModelToken) -> usize {
    match token {
        ModelToken::Symbol(text)
        | ModelToken::ConcreteIdentifier(text)
        | ModelToken::Number(text)
        | ModelToken::QuotedString(text) => size_of::<ModelToken>() + text.capacity(),
        _ => size_of::<ModelToken>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_tua_source, scope_map_parsed_source};
    use std::path::Path;

    fn table(name: &str) -> StatisticalFileTable {
        let source = format!(
            "local {name} = 0\n{}",
            format!("{name} = {name} + 1\n").repeat(40)
        );
        let parsed = parse_tua_source(Path::new("train.tua"), source).unwrap();
        let scope = scope_map_parsed_source(&parsed);
        StatisticalFileTable::build(&parsed.source, &scope).unwrap()
    }

    fn query_scope() -> StatisticalScopeFacts {
        let parsed = parse_tua_source(
            Path::new("target.tua"),
            "local point = 0\npoint = point + 1\n",
        )
        .unwrap();
        StatisticalScopeFacts::from_scope(&scope_map_parsed_source(&parsed))
    }

    #[test]
    fn requires_two_distinct_files_even_when_one_file_repeats() {
        let first = table("position");
        let scope = query_scope();
        let document = "local point = 0\npoint = point + ";
        assert!(predict(
            &[StatisticalEvidence {
                table: &first,
                weight: 1.0,
            }],
            document,
            document.len(),
            &scope,
            InlineCompletionBudget::unlimited(),
            &AtomicBool::new(false),
        )
        .is_none());

        let second = table("velocity");
        let prediction = predict(
            &[
                StatisticalEvidence {
                    table: &first,
                    weight: 1.0,
                },
                StatisticalEvidence {
                    table: &second,
                    weight: 1.0,
                },
            ],
            document,
            document.len(),
            &scope,
            InlineCompletionBudget::unlimited(),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(prediction.text, "1");
        assert!(prediction.score >= MIN_SCORE);
    }

    #[test]
    fn cancellation_fails_closed() {
        let first = table("position");
        let second = table("velocity");
        let cancelled = AtomicBool::new(true);
        let document = "local point = 0\npoint = point + ";
        assert!(predict(
            &[
                StatisticalEvidence {
                    table: &first,
                    weight: 1.0,
                },
                StatisticalEvidence {
                    table: &second,
                    weight: 1.0,
                },
            ],
            document,
            document.len(),
            &query_scope(),
            InlineCompletionBudget::unlimited(),
            &cancelled,
        )
        .is_none());
    }

    #[test]
    fn stale_scope_facts_abstain_past_the_unchanged_prefix() {
        let first = table("position");
        let second = table("velocity");
        let document = "local point = 0\npoint = point + ";
        let scope = query_scope().restricted_to_common_prefix(document.len() - 1);
        assert!(predict(
            &[
                StatisticalEvidence {
                    table: &first,
                    weight: 1.0,
                },
                StatisticalEvidence {
                    table: &second,
                    weight: 1.0,
                },
            ],
            document,
            document.len(),
            &scope,
            InlineCompletionBudget::unlimited(),
            &AtomicBool::new(false),
        )
        .is_none());
    }

    #[test]
    fn renderer_keeps_method_colons_tight_and_balanced() {
        let context = vec![
            RawToken::ConcreteIdentifier("ship".to_owned()),
            RawToken::Symbol(":".to_owned()),
            RawToken::ConcreteIdentifier("update".to_owned()),
            RawToken::Symbol("(".to_owned()),
            RawToken::Number("1".to_owned()),
            RawToken::Symbol(")".to_owned()),
        ];
        let output = &context[2..];
        assert_eq!(render(&context, output), "update(1)");
    }
}
