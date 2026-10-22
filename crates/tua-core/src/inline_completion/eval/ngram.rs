//! Evaluator-only statistical completion experiment.
//!
//! The model deliberately stays behind `test-fixture`: Phase 3 must beat exact
//! retrieval before any of this can become a production dependency. Every
//! query excludes the target file, checks a deadline and cancellation flag,
//! and generates only concrete tokens or identifiers already bound by the
//! request context.

use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use full_moon::ast::LuaVersion;
use full_moon::tokenizer::{Lexer, LexerResult, TokenType};
use serde::Serialize;

use super::dataset::EvalProject;
use super::retrieval::EvalProjectRetrievalIndex;

const MAX_ORDER: usize = 6;
const MAX_OUTPUT_TOKENS: usize = 12;
const BEAM_WIDTH: usize = 3;
const MAX_SOURCE_BYTES: usize = 256 * 1024;
const MAX_CONTEXTS_PER_FILE: usize = 100_000;
const MAX_QUERY_MODEL_BYTES: usize = 16 * 1024 * 1024;
const MAX_FILE_MODEL_BYTES: usize = 256 * 1024;
const MAX_EVALUATOR_FILES: usize = 1_024;
const MAX_QUERY_FILES: usize = 512;
const MAX_STEP_CANDIDATES: usize = 256;
const CONTEXT_BYTES: usize = 2 * 1024;
const JM_PSEUDO_COUNT: f64 = 10.0;
const MAX_MEAN_SURPRISE: f64 = 3.0;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum RawToken {
    Symbol(String),
    UnclassifiedIdentifier(String),
    ConcreteIdentifier(String),
    SlotIdentifier(String),
    Number(String),
    QuotedString(String),
    UnknownString,
    Newline,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

#[derive(Debug)]
struct FileModel {
    path: PathBuf,
    language: &'static str,
    counts: BTreeMap<Vec<ModelToken>, BTreeMap<ModelToken, u32>>,
    approximate_bytes: usize,
    truncated: bool,
}

/// Construction measurements for one project model.
#[derive(Clone, Debug, Default, Serialize)]
pub struct NgramIndexReport {
    pub files_considered: usize,
    pub files_indexed: usize,
    pub files_skipped_by_limits: usize,
    pub files_truncated_by_limits: usize,
    /// Targets for which the Phase 2 index could not construct a specific
    /// leakage-safe holdout view. Queries for them fail closed.
    pub targets_without_holdout_view: usize,
    pub contexts: usize,
    /// Largest production-shaped per-target view. This is an explicit,
    /// conservative accounting estimate, not process RSS.
    pub approximate_bytes: usize,
    /// Evaluator-only storage for all per-file models and target views.
    pub evaluation_holdout_bytes: usize,
    pub build_ms: f64,
}

/// Immutable evaluator model. It retains per-file tables so a target file can
/// be omitted at query time without subtracting counts from a pruned model.
#[derive(Debug)]
pub struct EvalNgramIndex {
    files: Box<[FileModel]>,
    target_views: BTreeMap<PathBuf, QueryView>,
}

#[derive(Debug, Default)]
struct QueryView {
    file_indices: Box<[usize]>,
    approximate_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct Prediction {
    pub text: String,
    pub score: f64,
}

#[derive(Clone)]
struct Beam {
    context: Vec<RawToken>,
    output: Vec<RawToken>,
    negative_log_probability: f64,
}

impl EvalNgramIndex {
    pub fn build(
        project: &EvalProject,
        retrieval: &EvalProjectRetrievalIndex,
    ) -> (Self, NgramIndexReport) {
        let started = Instant::now();
        let mut report = NgramIndexReport {
            files_considered: project.files.len(),
            ..NgramIndexReport::default()
        };
        let mut files = Vec::new();
        let mut inputs = project.files.iter().collect::<Vec<_>>();
        inputs.sort_by(|left, right| left.path.cmp(&right.path));
        // Fail the experiment closed instead of selecting a path-ordered
        // subset that a held-out target could perturb. Normal evaluator sets
        // are well below this evaluator-only memory bound.
        if inputs.len() > MAX_EVALUATOR_FILES {
            report.files_skipped_by_limits = inputs.len();
            report.targets_without_holdout_view = inputs.len();
            report.build_ms = started.elapsed().as_secs_f64() * 1000.0;
            return (
                Self {
                    files: Box::new([]),
                    target_views: BTreeMap::new(),
                },
                report,
            );
        }
        for file in inputs {
            if file.source.len() > MAX_SOURCE_BYTES {
                report.files_skipped_by_limits += 1;
                continue;
            }
            let Some(model) = FileModel::build(
                file.path.clone(),
                file.language(),
                &file.source,
                MAX_CONTEXTS_PER_FILE,
                MAX_FILE_MODEL_BYTES,
            ) else {
                report.files_skipped_by_limits += 1;
                continue;
            };
            report.files_truncated_by_limits += usize::from(model.truncated);
            report.contexts += model.counts.len();
            files.push(model);
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let make_view = |target_path: &Path, language: &str| {
            let retained = retrieval.phase3_paths_for_target(target_path)?;
            Some(select_view(&files, &retained, target_path, language))
        };
        let target_views = project
            .files
            .iter()
            .filter_map(|target| {
                Some((
                    target.path.clone(),
                    make_view(&target.path, target.language())?,
                ))
            })
            .collect::<BTreeMap<_, _>>();
        let model_bytes = size_of::<Self>()
            + files
                .iter()
                .map(|file| file.approximate_bytes)
                .sum::<usize>();
        let view_bytes = target_views
            .iter()
            .map(|(path, view)| path.as_os_str().len() + view_storage_bytes(view))
            .sum::<usize>();
        report.files_indexed = files.len();
        report.targets_without_holdout_view = project.files.len() - target_views.len();
        report.approximate_bytes = target_views
            .values()
            .map(|view| view.approximate_bytes)
            .max()
            .unwrap_or_default();
        report.evaluation_holdout_bytes = model_bytes + view_bytes;
        report.build_ms = started.elapsed().as_secs_f64() * 1000.0;
        (
            Self {
                files: files.into_boxed_slice(),
                target_views,
            },
            report,
        )
    }

    /// Predict one source line using only other files in the project.
    pub fn predict(
        &self,
        target_path: &Path,
        language: &str,
        document: &str,
        offset: usize,
        budget: Duration,
        cancelled: &AtomicBool,
    ) -> Option<Prediction> {
        let deadline = Instant::now() + budget;
        if cancelled.load(Ordering::Relaxed) || offset == 0 || offset > document.len() {
            return None;
        }
        let prefix = &document[..offset];
        // Completing the middle of an identifier requires prefix-aware token
        // recovery. Abstaining is safer than inserting punctuation after a
        // partially typed name.
        if prefix
            .chars()
            .next_back()
            .is_some_and(is_identifier_character)
        {
            return None;
        }
        let start = context_start(prefix);
        let mut context = tokenize(&prefix[start..])?;
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
            if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                return None;
            }
            let mut next = Vec::new();
            for beam in std::mem::take(&mut beams) {
                let Some(distribution) =
                    self.distribution(target_path, language, &beam.context, deadline, cancelled)
                else {
                    if !beam.output.is_empty() {
                        completed.push(beam);
                    }
                    continue;
                };
                for (token, probability) in distribution.into_iter().take(BEAM_WIDTH) {
                    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                        return None;
                    }
                    if token == RawToken::Newline {
                        if !beam.output.is_empty() {
                            completed.push(beam.clone());
                        }
                        continue;
                    }
                    let mut candidate = beam.clone();
                    candidate.negative_log_probability -= probability.max(f64::MIN_POSITIVE).ln();
                    let token_count = candidate.output.len() + 1;
                    if candidate.negative_log_probability / token_count as f64 > MAX_MEAN_SURPRISE {
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
            if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                return None;
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
        if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
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
        if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return None;
        }
        let text = render(&best.context, &best.output);
        if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline || text.is_empty() {
            return None;
        }
        Some(Prediction {
            text,
            score: beam_score(&best),
        })
    }

    fn distribution(
        &self,
        target_path: &Path,
        language: &str,
        context: &[RawToken],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Option<Vec<(RawToken, f64)>> {
        let max_order = MAX_ORDER.min(context.len());
        let view = self.target_views.get(target_path)?;
        let mut by_order = Vec::<OrderDistribution>::with_capacity(max_order);
        for order in 1..=max_order {
            if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                return None;
            }
            let (key, bindings) = normalize(&context[context.len() - order..]);
            let mut distribution = OrderDistribution::default();
            for file_index in view.file_indices.iter().copied() {
                if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    return None;
                }
                let file = &self.files[file_index];
                if file.path == target_path || (language == "lua" && file.language == "tua") {
                    continue;
                }
                let weight = if file.path.parent() == target_path.parent() {
                    2.0
                } else {
                    1.0
                };
                if let Some(next) = file.counts.get(&key) {
                    for (token, count) in next {
                        if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                            return None;
                        }
                        let weighted = f64::from(*count) * weight;
                        distribution.total += weighted;
                        if let Some(raw) = resolve(token, &bindings) {
                            *distribution.resolved.entry(raw).or_default() += weighted;
                        }
                    }
                }
            }
            by_order.push(distribution);
        }
        let mut candidates = BTreeSet::new();
        for distribution in &by_order {
            for candidate in distribution.resolved.keys() {
                if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    return None;
                }
                if candidates.len() >= MAX_STEP_CANDIDATES && !candidates.contains(candidate) {
                    return None;
                }
                candidates.insert(candidate.clone());
            }
        }
        let mut scored = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                return None;
            }
            let mut probability = 0.0;
            for distribution in &by_order {
                if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    return None;
                }
                if distribution.total == 0.0 {
                    continue;
                }
                let lambda = distribution.total / (distribution.total + JM_PSEUDO_COUNT);
                let local = distribution
                    .resolved
                    .get(&candidate)
                    .copied()
                    .unwrap_or_default()
                    / distribution.total;
                probability = lambda * local + (1.0 - lambda) * probability;
            }
            if probability > 0.0 {
                scored.push((candidate, probability));
            }
        }
        if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return None;
        }
        scored.sort_by(|(left_token, left), (right_token, right)| {
            right
                .total_cmp(left)
                .then_with(|| left_token.cmp(right_token))
        });
        if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return None;
        }
        (!scored.is_empty()).then_some(scored)
    }
}

#[derive(Default)]
struct OrderDistribution {
    resolved: BTreeMap<RawToken, f64>,
    total: f64,
}

fn select_view(
    files: &[FileModel],
    retained_paths: &BTreeSet<PathBuf>,
    target_path: &Path,
    language: &str,
) -> QueryView {
    let mut eligible = files
        .iter()
        .enumerate()
        .filter(|(_, file)| {
            retained_paths.contains(&file.path)
                && file.path != target_path
                && !(language == "lua" && file.language == "tua")
        })
        .collect::<Vec<_>>();
    eligible.sort_by(|(_, left), (_, right)| {
        let left_tier = usize::from(left.path.parent() != target_path.parent());
        let right_tier = usize::from(right.path.parent() != target_path.parent());
        left_tier
            .cmp(&right_tier)
            .then_with(|| left.path.cmp(&right.path))
    });
    let mut bytes = size_of::<QueryView>();
    let mut indices = Vec::new();
    // Both caps are deliberately applied after target, near-duplicate, and
    // language exclusion. A target can therefore never consume its own view.
    for (index, file) in eligible {
        if indices.len() >= MAX_QUERY_FILES {
            break;
        }
        if bytes.saturating_add(file.approximate_bytes) > MAX_QUERY_MODEL_BYTES {
            continue;
        }
        bytes += file.approximate_bytes;
        indices.push(index);
    }
    QueryView {
        file_indices: indices.into_boxed_slice(),
        approximate_bytes: bytes,
    }
}

fn view_storage_bytes(view: &QueryView) -> usize {
    size_of::<QueryView>() + size_of_val(view.file_indices.as_ref())
}

impl FileModel {
    fn build(
        path: PathBuf,
        language: &'static str,
        source: &str,
        max_contexts: usize,
        max_bytes: usize,
    ) -> Option<Self> {
        let tokens = tokenize(source)?;
        let mut counts = BTreeMap::<Vec<ModelToken>, BTreeMap<ModelToken, u32>>::new();
        let mut approximate_bytes = size_of::<FileModel>() + path.as_os_str().len();
        let mut truncated = false;
        'tokens: for next_index in 1..tokens.len() {
            for order in 1..=MAX_ORDER.min(next_index) {
                let normalized = normalize(&tokens[next_index - order..=next_index]).0;
                let (key, next) = normalized.split_at(normalized.len() - 1);
                if !counts.contains_key(key) && counts.len() >= max_contexts {
                    truncated = true;
                    break 'tokens;
                }
                let existing = counts.get(key);
                let added_context = usize::from(existing.is_none())
                    * (size_of::<(Vec<ModelToken>, BTreeMap<ModelToken, u32>)>()
                        + 64
                        + key.iter().map(model_token_bytes).sum::<usize>());
                let added_value =
                    usize::from(existing.is_none_or(|values| !values.contains_key(&next[0])))
                        * (size_of::<(ModelToken, u32)>() + 32 + model_token_bytes(&next[0]));
                let added_bytes = added_context.saturating_add(added_value);
                if approximate_bytes.saturating_add(added_bytes) > max_bytes {
                    truncated = true;
                    break 'tokens;
                }
                let count = counts
                    .entry(key.to_vec())
                    .or_default()
                    .entry(next[0].clone())
                    .or_default();
                *count = count.saturating_add(1);
                approximate_bytes += added_bytes;
            }
        }
        if counts.is_empty() {
            return None;
        }
        Some(Self {
            path,
            language,
            counts,
            approximate_bytes,
            truncated,
        })
    }
}

fn normalize(tokens: &[RawToken]) -> (Vec<ModelToken>, BTreeMap<u8, RawToken>) {
    let mut names = BTreeMap::<String, u8>::new();
    let mut bindings = BTreeMap::new();
    let mut next_slot = 1_u8;
    let normalized = tokens
        .iter()
        .map(|token| match token {
            RawToken::Symbol(text) => ModelToken::Symbol(text.clone()),
            RawToken::UnclassifiedIdentifier(text) => ModelToken::ConcreteIdentifier(text.clone()),
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

fn tokenize(source: &str) -> Option<Vec<RawToken>> {
    let tokens = match Lexer::new(source, LuaVersion::luau()).collect() {
        LexerResult::Ok(tokens) | LexerResult::Recovered(tokens, _) => tokens,
        LexerResult::Fatal(_) => return None,
    };
    let mut result = Vec::new();
    for token in tokens {
        match token.token_type() {
            TokenType::Eof => {}
            TokenType::Whitespace { characters } => push_newlines(&mut result, characters),
            TokenType::SingleLineComment { .. } => {}
            TokenType::MultiLineComment { comment, .. } => push_newlines(&mut result, comment),
            TokenType::Shebang { .. } => {}
            TokenType::StringLiteral { literal, .. } => {
                result.push(if common_string(literal) {
                    RawToken::QuotedString(token.to_string())
                } else {
                    RawToken::UnknownString
                });
            }
            TokenType::InterpolatedString { .. } => {
                result.push(RawToken::UnknownString);
            }
            TokenType::Number { text } => {
                result.push(RawToken::Number(text.to_string()));
            }
            TokenType::Identifier { identifier } => {
                result.push(RawToken::UnclassifiedIdentifier(identifier.to_string()));
            }
            TokenType::Symbol { .. } => {
                let text = token.to_string();
                result.push(RawToken::Symbol(text));
            }
            _ => return None,
        }
    }
    Some(classify_bindings(result))
}

fn classify_bindings(tokens: Vec<RawToken>) -> Vec<RawToken> {
    // This intentionally remains a conservative recovered-token heuristic,
    // not a second scope engine. It recognizes preceding local declarations,
    // function parameters, and `for` binders symmetrically in training and
    // query text; other identifiers remain concrete. It does not model nested
    // scope exit or type-grammar nesting, so this experiment may keep a proven
    // local slotted longer than its real lexical lifetime. Production use
    // would need the shared scope map rather than expanding this heuristic.
    let mut bound = BTreeSet::<String>::new();
    let mut local_names = false;
    let mut local_function_name = false;
    let mut for_names = false;
    let mut awaiting_parameters = false;
    let mut parameter_depth = 0_usize;
    let mut parameter_name_expected = false;
    let mut result = Vec::with_capacity(tokens.len());

    for (index, token) in tokens.iter().enumerate() {
        let previous_symbol = result
            .iter()
            .rev()
            .find_map(|token| match token {
                RawToken::Newline => None,
                _ => Some(symbol_text(token)),
            })
            .flatten();
        let next_symbol = tokens[index + 1..]
            .iter()
            .find_map(|token| match token {
                RawToken::Newline => None,
                _ => Some(symbol_text(token)),
            })
            .flatten();
        match token {
            RawToken::Symbol(symbol) => {
                match symbol.as_str() {
                    "local" => local_names = true,
                    "for" => for_names = true,
                    "function" => {
                        if local_names {
                            local_function_name = true;
                        }
                        local_names = false;
                        awaiting_parameters = true;
                    }
                    "(" if awaiting_parameters => {
                        awaiting_parameters = false;
                        parameter_depth = 1;
                        parameter_name_expected = true;
                    }
                    "(" if parameter_depth > 0 => parameter_depth += 1,
                    ")" if parameter_depth > 0 => {
                        parameter_depth -= 1;
                        parameter_name_expected = false;
                    }
                    "," if parameter_depth == 1 => parameter_name_expected = true,
                    "=" => {
                        local_names = false;
                        for_names = false;
                    }
                    "in" | "do" => for_names = false,
                    _ => {}
                }
                result.push(token.clone());
            }
            RawToken::Newline => {
                local_names = false;
                result.push(token.clone());
            }
            RawToken::UnclassifiedIdentifier(name) => {
                let member = matches!(previous_symbol, Some("." | ":"));
                let table_key =
                    next_symbol == Some("=") && matches!(previous_symbol, Some("{" | ","));
                let declaration = local_function_name
                    || (local_names && !matches!(previous_symbol, Some(":")))
                    || for_names
                    || (parameter_depth == 1 && parameter_name_expected);
                if declaration {
                    bound.insert(name.clone());
                }
                let concrete = member || table_key || name == "self" || !bound.contains(name);
                result.push(if concrete {
                    RawToken::ConcreteIdentifier(name.clone())
                } else {
                    RawToken::SlotIdentifier(name.clone())
                });
                if local_function_name {
                    local_function_name = false;
                }
                if parameter_depth == 1 && parameter_name_expected {
                    parameter_name_expected = false;
                }
            }
            _ => result.push(token.clone()),
        }
    }
    result
}

fn symbol_text(token: &RawToken) -> Option<&str> {
    match token {
        RawToken::Symbol(symbol) => Some(symbol),
        _ => None,
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

fn context_start(prefix: &str) -> usize {
    let mut start = prefix.len().saturating_sub(CONTEXT_BYTES);
    while !prefix.is_char_boundary(start) {
        start += 1;
    }
    prefix[start..]
        .find('\n')
        .map_or(start, |newline| start + newline + 1)
}

fn beam_score(beam: &Beam) -> f64 {
    (-beam.negative_log_probability / beam.output.len().max(1) as f64).exp()
}

fn can_end(token: Option<&RawToken>) -> bool {
    !matches!(
        token,
        Some(RawToken::Symbol(symbol)) if matches!(symbol.as_str(), "." | ":" | "," | "+" | "-" | "*" | "/" | "=" | "(" | "[" | "{")
    )
}

fn render(context: &[RawToken], output: &[RawToken]) -> String {
    let prefix_len = context.len().saturating_sub(output.len());
    let prior = prefix_len
        .checked_sub(1)
        .and_then(|index| context.get(index));
    let before_prior = prefix_len
        .checked_sub(2)
        .and_then(|index| context.get(index));
    let mut text = String::new();
    let mut previous = prior;
    let mut before_previous = before_prior;
    for (index, token) in output.iter().enumerate() {
        if token == &RawToken::Newline {
            break;
        }
        if needs_space(before_previous, previous, token, output.get(index + 1)) {
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
        | RawToken::UnclassifiedIdentifier(text)
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
    next: Option<&RawToken>,
) -> bool {
    let current_text = token_text(current);
    if matches!(current_text, ")" | "]" | "}" | "," | ";" | "." | ":") {
        return false;
    }
    let Some(previous) = previous else {
        return false;
    };
    let previous_text = token_text(previous);
    if matches!(previous_text, "(" | "[" | "{" | ".") {
        return false;
    }
    if previous_text == ":" {
        // Method calls are `receiver:method(`. Type annotations use a space
        // after the colon. Looking one token ahead is enough to distinguish
        // the forms generated by this single-line experiment.
        return !matches!(next, Some(RawToken::Symbol(symbol)) if symbol == "(");
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
    if current_text == "{" {
        return !matches!(previous_text, "(" | "[" | "{" | ",");
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
            | RawToken::UnclassifiedIdentifier(_)
            | RawToken::SlotIdentifier(_)
            | RawToken::Number(_)
            | RawToken::QuotedString(_)
    ) || matches!(token, RawToken::Symbol(text) if is_word_symbol(text))
}

fn is_word_symbol(text: &str) -> bool {
    matches!(
        text,
        "and"
            | "break"
            | "class"
            | "continue"
            | "do"
            | "else"
            | "elseif"
            | "end"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "for"
            | "from"
            | "function"
            | "if"
            | "import"
            | "in"
            | "local"
            | "nil"
            | "not"
            | "or"
            | "repeat"
            | "return"
            | "then"
            | "true"
            | "type"
            | "until"
            | "while"
    )
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
    use super::super::retrieval::EvalProjectRetrievalIndex;
    use super::*;
    use crate::{AnalysisSourceKind, ProjectConfig};

    fn project(files: &[(&str, &str)]) -> EvalProject {
        EvalProject {
            name: "fixture".to_owned(),
            config: ProjectConfig::default(),
            source_roots: vec![],
            output_roots: vec![],
            asset_roots: vec![],
            files: files
                .iter()
                .map(|(path, source)| super::super::dataset::EvalFile {
                    path: PathBuf::from(path),
                    kind: AnalysisSourceKind::Tua,
                    source: (*source).to_owned(),
                })
                .collect(),
        }
    }

    fn build(project: &EvalProject) -> (EvalNgramIndex, NgramIndexReport) {
        let (retrieval, _) = EvalProjectRetrievalIndex::build(project);
        EvalNgramIndex::build(project, &retrieval)
    }

    #[test]
    fn target_file_is_never_training_evidence() {
        let fixture = project(&[
            ("main.tua", "value = unique()\n"),
            ("other.tua", "value = shared()\n"),
        ]);
        let (index, _) = build(&fixture);
        let cancelled = AtomicBool::new(false);
        let result = index.predict(
            Path::new("main.tua"),
            "tua",
            "value = ",
            8,
            Duration::from_millis(50),
            &cancelled,
        );
        assert!(result.is_none_or(|prediction| !prediction.text.contains("unique")));
    }

    #[test]
    fn slots_reuse_names_from_the_request_context() {
        let fixture = project(&[
            ("target.tua", "point = point + speed * \n"),
            ("train.tua", "position = position + velocity * 1\n"),
        ]);
        let (index, _) = build(&fixture);
        let cancelled = AtomicBool::new(false);
        let prediction = index
            .predict(
                Path::new("target.tua"),
                "tua",
                "point = point + speed * ",
                "point = point + speed * ".len(),
                Duration::from_millis(50),
                &cancelled,
            )
            .unwrap();
        assert!(!prediction.text.contains('$'));
    }

    #[test]
    fn unresolved_literals_are_not_generated() {
        let fixture = project(&[
            ("target.tua", "name = \n"),
            ("train.tua", "name = \"private-value\"\ncount = 77\n"),
        ]);
        let (index, _) = build(&fixture);
        let cancelled = AtomicBool::new(false);
        assert!(index
            .predict(
                Path::new("target.tua"),
                "tua",
                "name = ",
                7,
                Duration::from_millis(50),
                &cancelled,
            )
            .is_none());
    }

    #[test]
    fn cancellation_and_expired_budget_fail_closed() {
        let fixture = project(&[("target.tua", "value = \n"), ("train.tua", "value = 1\n")]);
        let (index, _) = build(&fixture);
        let cancelled = AtomicBool::new(true);
        assert!(index
            .predict(
                Path::new("target.tua"),
                "tua",
                "value = ",
                8,
                Duration::from_millis(50),
                &cancelled,
            )
            .is_none());
        cancelled.store(false, Ordering::Relaxed);
        assert!(index
            .predict(
                Path::new("target.tua"),
                "tua",
                "value = ",
                8,
                Duration::ZERO,
                &cancelled,
            )
            .is_none());
    }

    #[test]
    fn renderer_covers_calls_members_operators_and_tables() {
        let mut context = vec![RawToken::SlotIdentifier("value".to_owned())];
        let output = vec![
            RawToken::Symbol("=".to_owned()),
            RawToken::Symbol("-".to_owned()),
            RawToken::SlotIdentifier("item".to_owned()),
            RawToken::Symbol(".".to_owned()),
            RawToken::ConcreteIdentifier("x".to_owned()),
            RawToken::Symbol("+".to_owned()),
            RawToken::ConcreteIdentifier("math".to_owned()),
            RawToken::Symbol(".".to_owned()),
            RawToken::ConcreteIdentifier("max".to_owned()),
            RawToken::Symbol("(".to_owned()),
            RawToken::Number("0".to_owned()),
            RawToken::Symbol(",".to_owned()),
            RawToken::Number("1".to_owned()),
            RawToken::Symbol(")".to_owned()),
        ];
        context.extend(output.iter().cloned());
        assert_eq!(render(&context, &output), " = -item.x + math.max(0, 1)");

        let binary = vec![
            RawToken::Symbol("-".to_owned()),
            RawToken::SlotIdentifier("right".to_owned()),
        ];
        let mut binary_context = vec![RawToken::SlotIdentifier("left".to_owned())];
        binary_context.extend(binary.iter().cloned());
        assert_eq!(render(&binary_context, &binary), " - right");

        let table = vec![
            RawToken::Symbol("{".to_owned()),
            RawToken::SlotIdentifier("x".to_owned()),
            RawToken::Symbol("=".to_owned()),
            RawToken::Number("1".to_owned()),
            RawToken::Symbol("}".to_owned()),
        ];
        let mut table_context = vec![RawToken::Symbol("=".to_owned())];
        table_context.extend(table.iter().cloned());
        assert_eq!(render(&table_context, &table), " {x = 1}");
    }

    #[test]
    fn tokenization_keeps_minus_separate_and_drops_sensitive_literals() {
        let tokens = tokenize("local n = -1\nlocal key = 'secret'\n").unwrap();
        assert!(tokens.windows(2).any(|pair| matches!(pair, [RawToken::Symbol(symbol), RawToken::Number(number)] if symbol == "-" && number == "1")));
        assert!(tokens.contains(&RawToken::UnknownString));
    }

    #[test]
    fn each_order_renumbers_slots_from_its_own_first_appearance() {
        let a = RawToken::SlotIdentifier("alpha".to_owned());
        let b = RawToken::SlotIdentifier("beta".to_owned());
        assert_eq!(
            normalize(&[a, b.clone()]).0,
            vec![ModelToken::Slot(1), ModelToken::Slot(2)]
        );
        assert_eq!(normalize(&[b]).0, vec![ModelToken::Slot(1)]);
    }

    #[test]
    fn tokenizer_slots_only_lexically_bound_names() {
        let tokens = tokenize(
            "local local_value = global_value\n\
             local function update(parameter)\n\
               for index, item in ipairs(items) do\n\
                 object.field = { table_key = local_value }\n\
               end\n\
             end\n",
        )
        .unwrap();
        for local in ["local_value", "update", "parameter", "index", "item"] {
            assert!(
                tokens.contains(&RawToken::SlotIdentifier(local.to_owned())),
                "{local}"
            );
        }
        for concrete in [
            "global_value",
            "ipairs",
            "items",
            "object",
            "field",
            "table_key",
        ] {
            assert!(
                tokens.contains(&RawToken::ConcreteIdentifier(concrete.to_owned())),
                "{concrete}"
            );
        }
    }

    #[test]
    fn numeric_for_bounds_are_not_mistaken_for_binders() {
        let tokens =
            tokenize("for index = start_value, finish_value do\nprint(index)\nend\n").unwrap();
        assert!(tokens.contains(&RawToken::SlotIdentifier("index".to_owned())));
        for bound in ["start_value", "finish_value"] {
            assert!(
                tokens.contains(&RawToken::ConcreteIdentifier(bound.to_owned())),
                "{bound}"
            );
        }
    }

    #[test]
    fn unresolved_mass_is_abstention_not_renormalized_confidence() {
        let mut training = (0..99)
            .map(|index| format!("value = \"private-{index}\""))
            .collect::<Vec<_>>();
        training.push("value = 1".to_owned());
        let source = training.join("\n");
        let fixture = project(&[("target.tua", "value = \n"), ("train.tua", &source)]);
        let (index, _) = build(&fixture);
        let context = tokenize("value = ").unwrap();
        let cancelled = AtomicBool::new(false);
        let distribution = index
            .distribution(
                Path::new("target.tua"),
                "tua",
                &context,
                Instant::now() + Duration::from_secs(1),
                &cancelled,
            )
            .unwrap();
        let probability = distribution
            .iter()
            .find_map(|(token, probability)| {
                (token == &RawToken::Number("1".to_owned())).then_some(*probability)
            })
            .unwrap();
        assert!(probability > 0.0 && probability < 0.011, "{probability}");
    }

    #[test]
    fn renderer_spaces_keyword_statements_and_round_trips_tokens() {
        let output = vec![
            RawToken::Symbol("local".to_owned()),
            RawToken::SlotIdentifier("value".to_owned()),
            RawToken::Symbol("=".to_owned()),
            RawToken::Symbol("not".to_owned()),
            RawToken::ConcreteIdentifier("ready".to_owned()),
            RawToken::Symbol("and".to_owned()),
            RawToken::ConcreteIdentifier("fallback".to_owned()),
            RawToken::Symbol("or".to_owned()),
            RawToken::Symbol("nil".to_owned()),
        ];
        let rendered = render(&output, &output);
        assert_eq!(rendered, "local value = not ready and fallback or nil");
        assert_eq!(tokenize(&rendered).unwrap(), output);
        crate::parse_tua_source("renderer.tua", format!("{rendered}\nreturn value\n"))
            .expect("rendered keyword statement should parse");

        let returns = vec![
            RawToken::Symbol("return".to_owned()),
            RawToken::ConcreteIdentifier("value".to_owned()),
        ];
        assert_eq!(render(&returns, &returns), "return value");
    }

    #[test]
    fn target_and_near_copy_do_not_enter_the_target_view() {
        let common = (0..20)
            .map(|index| format!("value{index} = {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let target = format!("{common}\nresult = target_only()\n");
        let sibling = format!("{common}\nresult = target_copy()\n");
        let independent = "result = independent()\n";
        let fixture = project(&[
            ("target.tua", &target),
            ("copy.tua", &sibling),
            ("independent.tua", independent),
        ]);
        let (model, report) = build(&fixture);
        let view = &model.target_views[Path::new("target.tua")];
        let paths = view
            .file_indices
            .iter()
            .map(|index| model.files[*index].path.as_path())
            .collect::<Vec<_>>();
        assert!(!paths.contains(&Path::new("target.tua")));
        assert!(!paths.contains(&Path::new("copy.tua")));
        assert!(paths.contains(&Path::new("independent.tua")));
        assert!(view.file_indices.len() <= MAX_QUERY_FILES);
        assert!(report.approximate_bytes <= MAX_QUERY_MODEL_BYTES);
    }

    #[test]
    fn target_skipped_by_retrieval_limits_has_no_global_fallback_view() {
        let copy = std::iter::repeat_n("value = shared()", 14_000)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(copy.len() < MAX_SOURCE_BYTES);
        let target = format!("{copy}\n-- {}", "padding".repeat(5_000));
        assert!(target.len() > MAX_SOURCE_BYTES);
        let fixture = project(&[("target.tua", &target), ("near-copy.tua", &copy)]);
        let (model, _) = build(&fixture);
        assert!(!model.target_views.contains_key(Path::new("target.tua")));
        let cancelled = AtomicBool::new(false);
        assert!(model
            .predict(
                Path::new("target.tua"),
                "tua",
                "value = ",
                "value = ".len(),
                Duration::from_millis(50),
                &cancelled,
            )
            .is_none());
    }

    #[test]
    fn tokenizer_and_renderer_cover_newlines_comments_and_tua_operators() {
        let tokens = tokenize("value += 1 -- update\nnext:value()\n").unwrap();
        assert!(tokens.contains(&RawToken::Symbol("+=".to_owned())));
        assert!(tokens.contains(&RawToken::Newline));
        assert!(!tokens
            .iter()
            .any(|token| token_text(token).contains("update")));

        let output = vec![
            RawToken::Symbol("+=".to_owned()),
            RawToken::Number("1".to_owned()),
        ];
        let mut context = vec![RawToken::SlotIdentifier("value".to_owned())];
        context.extend(output.iter().cloned());
        let rendered = render(&context, &output);
        assert_eq!(rendered, " += 1");
        assert!(tokenize(&format!("value{rendered}"))
            .is_some_and(|round_trip| round_trip.ends_with(&output)));

        let string_output = vec![RawToken::QuotedString("'fill'".to_owned())];
        let mut string_context = vec![RawToken::Symbol("(".to_owned())];
        string_context.extend(string_output.iter().cloned());
        assert_eq!(render(&string_context, &string_output), "'fill'");
        assert!(tokenize("draw('fill')")
            .is_some_and(|tokens| tokens.contains(&RawToken::QuotedString("'fill'".to_owned()))));
    }

    #[test]
    fn retained_byte_cap_is_enforced() {
        let source = (0..4_000)
            .map(|index| format!("love.value{index} = love.value{index} + 1"))
            .collect::<Vec<_>>()
            .join("\n");
        let files = (0..20)
            .map(|index| (format!("src/{index}.tua"), source.clone()))
            .collect::<Vec<_>>();
        let fixture = EvalProject {
            name: "large".to_owned(),
            config: ProjectConfig::default(),
            source_roots: vec![],
            output_roots: vec![],
            asset_roots: vec![],
            files: files
                .into_iter()
                .map(|(path, source)| super::super::dataset::EvalFile {
                    path: PathBuf::from(path),
                    kind: AnalysisSourceKind::Tua,
                    source,
                })
                .collect(),
        };
        let (_, report) = build(&fixture);
        assert!(report.approximate_bytes <= MAX_QUERY_MODEL_BYTES);
        assert!(report.files_skipped_by_limits + report.files_truncated_by_limits > 0);
    }
}
