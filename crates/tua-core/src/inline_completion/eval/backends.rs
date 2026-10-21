//! The candidate sources the evaluator compares. These are measurement
//! prototypes of the Phase 1 sources, following its rules, so Phase 0 can
//! decide source order and thresholds before anything ships.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::super::eligibility::{strip_trailing_comment, word_before, CursorContext};

/// One inline-completion request, as the editor would make it.
pub struct Request<'a> {
    /// The document as it is when the request fires.
    pub document: &'a str,
    /// Byte offset of the cursor in `document`.
    pub offset: usize,
    /// Zero-based line of the cursor, excluded from current-file recall.
    pub line_index: usize,
    pub context: CursorContext,
    /// Callbacks declared in the current file, from a cache that already holds
    /// parsed facts for this exact source. `None` means it does not.
    pub declared_callbacks: Option<&'a BTreeSet<String>>,
    pub include_ecs: bool,
}

/// A suggestion: text inserted at the cursor, continuing what is typed.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Candidate {
    pub text: String,
    pub is_snippet: bool,
    pub source: &'static str,
    /// Comparable only within one source.
    pub score: f64,
}

pub trait Backend {
    fn name(&self) -> &'static str;
    fn suggest(&self, request: &Request<'_>) -> Option<Candidate>;
}

pub struct Noop;

impl Backend for Noop {
    fn name(&self) -> &'static str {
        "noop"
    }

    fn suggest(&self, _request: &Request<'_>) -> Option<Candidate> {
        None
    }
}

/// Statement snippets at the start of a line, and LÖVE callbacks after
/// `function love.`, each only when exactly one entry matches what is typed.
pub struct Semantic;

impl Backend for Semantic {
    fn name(&self) -> &'static str {
        "semantic"
    }

    fn suggest(&self, request: &Request<'_>) -> Option<Candidate> {
        love_callback(request).or_else(|| statement_snippet(request, 1))
    }
}

fn statement_snippet(request: &Request<'_>, min_prefix_chars: usize) -> Option<Candidate> {
    if request.context != CursorContext::StatementPrefix {
        return None;
    }
    let typed = word_before(request.document, request.offset);
    if typed.chars().count() < min_prefix_chars {
        return None;
    }
    let matches = crate::completion::syntax_snippet_completions(typed, request.include_ecs)
        .into_iter()
        .filter_map(|completion| {
            let text = completion.insert_text?;
            // Ghost text can only continue what is typed, so the snippet must
            // start with the typed word. Aliases such as `lfn` don't.
            text.strip_prefix(typed)
                .filter(|rest| !rest.is_empty())
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();

    let [text] = matches.as_slice() else {
        return None;
    };
    Some(Candidate {
        text: text.clone(),
        is_snippet: true,
        source: "snippet",
        score: 1.0,
    })
}

fn love_callback(request: &Request<'_>) -> Option<Candidate> {
    let line_start = request.document[..request.offset]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let prefix = request.document[line_start..request.offset].trim_start();
    let typed = prefix.strip_prefix("function love.")?;
    if !typed
        .chars()
        .all(super::super::eligibility::is_identifier_char)
    {
        return None;
    }
    // Without cached parsed facts, an already declared callback can't be ruled
    // out, so nothing is offered.
    let declared = request.declared_callbacks?;
    let matches = crate::love_callback::items()
        .into_iter()
        .map(|item| item.label)
        .filter(|label| label.starts_with(typed) && !declared.contains(*label))
        .collect::<Vec<_>>();

    let [label] = matches.as_slice() else {
        return None;
    };
    let declaration = crate::love_callback::declaration_suffix(label, "")?;
    Some(Candidate {
        text: declaration[typed.len()..].to_owned(),
        is_snippet: false,
        source: "love-callback",
        score: 1.0,
    })
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct RecallOptions {
    /// Non-whitespace characters required before the cursor on its line.
    pub min_prefix_chars: usize,
    /// How many times more often the most common continuation must occur than
    /// the next one, when several continuations match.
    pub dominance_ratio: f64,
    /// Documents larger than this are not scanned.
    pub max_document_bytes: usize,
}

impl Default for RecallOptions {
    fn default() -> Self {
        Self {
            min_prefix_chars: 6,
            dominance_ratio: 2.0,
            max_document_bytes: 256 * 1024,
        }
    }
}

/// Current-file recall: other lines of the same document whose normalized
/// text starts with the normalized line prefix, continued with their original
/// text up to the end of the line.
pub struct CurrentFileRecall {
    pub options: RecallOptions,
}

impl Backend for CurrentFileRecall {
    fn name(&self) -> &'static str {
        "current-file"
    }

    fn suggest(&self, request: &Request<'_>) -> Option<Candidate> {
        let continuations = self.continuations(request)?;
        let (best, best_count) = *continuations.first()?;
        let runner_up = continuations.get(1).map_or(0, |(_, count)| *count);
        if runner_up > 0 && (best_count as f64) < self.options.dominance_ratio * runner_up as f64 {
            return None;
        }
        let total = continuations.iter().map(|(_, count)| count).sum::<usize>();

        Some(Candidate {
            text: best.to_owned(),
            is_snippet: false,
            source: "current-file",
            score: best_count as f64 / total as f64,
        })
    }
}

/// A recall variant measured against the unique-continuation rule: it offers
/// only what every matching continuation agrees on, cut back to a whole
/// token, so diverging arguments or values are left for the user to type.
pub struct CurrentFileCommonPrefix {
    pub recall: CurrentFileRecall,
}

impl Backend for CurrentFileCommonPrefix {
    fn name(&self) -> &'static str {
        "current-file-prefix"
    }

    fn suggest(&self, request: &Request<'_>) -> Option<Candidate> {
        let continuations = self.recall.continuations(request)?;
        let (first, _) = *continuations.first()?;
        let mut common = first.len();
        for (continuation, _) in &continuations[1..] {
            common = first
                .char_indices()
                .zip(continuation.chars())
                .find(|((_, left), right)| left != right)
                .map_or(common.min(continuation.len()), |((index, _), _)| {
                    index.min(common)
                });
        }
        let mut prefix = &first[..common];
        // Don't stop inside a word that some continuation goes on with.
        let continues_word = continuations.iter().any(|(continuation, _)| {
            continuation[prefix.len()..]
                .chars()
                .next()
                .is_some_and(super::super::eligibility::is_identifier_char)
        });
        if continues_word {
            prefix = prefix.trim_end_matches(super::super::eligibility::is_identifier_char);
        }
        let prefix = prefix.trim_end();
        if prefix.is_empty() {
            return None;
        }
        let support = continuations.iter().map(|(_, count)| count).sum::<usize>();

        Some(Candidate {
            text: prefix.to_owned(),
            is_snippet: false,
            source: "current-file-prefix",
            score: support as f64,
        })
    }
}

impl CurrentFileRecall {
    /// Matching continuations with their counts, most common first.
    fn continuations<'r>(&self, request: &Request<'r>) -> Option<Vec<(&'r str, usize)>> {
        if request.document.len() > self.options.max_document_bytes {
            return None;
        }
        let line_start = request.document[..request.offset]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let prefix = &request.document[line_start..request.offset];
        let (normalized_prefix, _) = normalize(prefix.trim_start());
        let significant = normalized_prefix
            .chars()
            .filter(|c| !c.is_whitespace())
            .count();
        if significant < self.options.min_prefix_chars {
            return None;
        }
        // A trailing space in the prefix must match a space in the line, so
        // `local x ` doesn't match `local xs`.
        let mut continuations = BTreeMap::<&str, usize>::new();
        for (index, line) in request.document.split('\n').enumerate() {
            if index == request.line_index {
                continue;
            }
            let line = line.strip_suffix('\r').unwrap_or(line);
            let Some(continuation) = continuation_after(line, &normalized_prefix) else {
                continue;
            };
            let continuation = strip_trailing_comment(continuation);
            if !continuation.is_empty() {
                *continuations.entry(continuation).or_default() += 1;
            }
        }

        let mut ranked = continuations.into_iter().collect::<Vec<_>>();
        ranked.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));
        Some(ranked)
    }
}

/// Collapses each whitespace run to one space, returning the normalized text
/// and, for every normalized byte, the original byte offset it came from.
fn normalize(text: &str) -> (String, Vec<usize>) {
    let mut normalized = String::with_capacity(text.len());
    let mut origins = Vec::with_capacity(text.len());
    let mut in_space = false;
    for (offset, character) in text.char_indices() {
        if character.is_whitespace() {
            if !in_space {
                normalized.push(' ');
                origins.push(offset);
            }
            in_space = true;
        } else {
            in_space = false;
            let start = normalized.len();
            normalized.push(character);
            origins.extend(std::iter::repeat_n(offset, normalized.len() - start));
        }
    }
    (normalized, origins)
}

/// The original text of `line` after its part that normalizes to `prefix`.
fn continuation_after<'line>(line: &'line str, prefix: &str) -> Option<&'line str> {
    let indent = line.len() - line.trim_start().len();
    let body = &line[indent..];
    let (normalized, origins) = normalize(body);
    if !normalized.starts_with(prefix) || normalized.len() == prefix.len() {
        return None;
    }
    // The continuation starts at the original byte of the first normalized
    // character after the prefix. A prefix ending inside a whitespace run
    // resumes after that run.
    let mut start = origins[prefix.len()];
    if prefix.ends_with(' ') {
        start += body[start..].len() - body[start..].trim_start().len();
    }
    // A prefix ending mid-word must not match a longer word unless the line
    // continues that same word, which `starts_with` already guarantees.
    Some(&body[start..])
}

/// Which current-file recall rule the Phase 1 candidate uses.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecallVariant {
    /// The whole continuation of a unique or dominant matching line.
    #[default]
    Unique,
    /// Only the part every matching continuation agrees on.
    CommonPrefix,
}

/// The configuration of the candidate Phase 1 source order, which a
/// thresholds file can pin so a gate run evaluates exactly what was approved.
#[derive(Clone, Debug, Serialize)]
pub struct Phase1Config {
    pub recall_variant: RecallVariant,
    /// When set, suggestions are offered only in these cursor contexts.
    pub contexts: Option<BTreeSet<CursorContext>>,
    pub recall: RecallOptions,
    pub love_callbacks: bool,
    pub statement_snippets: bool,
    pub snippet_min_prefix_chars: usize,
}

impl Default for Phase1Config {
    fn default() -> Self {
        Self {
            recall_variant: RecallVariant::default(),
            contexts: None,
            recall: RecallOptions::default(),
            love_callbacks: true,
            statement_snippets: true,
            snippet_min_prefix_chars: 1,
        }
    }
}

/// The candidate Phase 1 order: semantic sources win in their exact contexts,
/// and current-file recall answers everywhere else. The default is the
/// plan's starting hypothesis.
pub struct Phase1 {
    pub config: Phase1Config,
}

impl Backend for Phase1 {
    fn name(&self) -> &'static str {
        "phase1"
    }

    fn suggest(&self, request: &Request<'_>) -> Option<Candidate> {
        if self
            .config
            .contexts
            .as_ref()
            .is_some_and(|contexts| !contexts.contains(&request.context))
        {
            return None;
        }
        let recall = CurrentFileRecall {
            options: self.config.recall,
        };
        self.config
            .love_callbacks
            .then(|| love_callback(request))
            .flatten()
            .or_else(|| {
                self.config
                    .statement_snippets
                    .then(|| statement_snippet(request, self.config.snippet_min_prefix_chars))
                    .flatten()
            })
            .or_else(|| match self.config.recall_variant {
                RecallVariant::Unique => recall.suggest(request),
                RecallVariant::CommonPrefix => CurrentFileCommonPrefix { recall }.suggest(request),
            })
    }
}
