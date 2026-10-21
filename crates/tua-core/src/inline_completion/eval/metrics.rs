//! Scoring a suggestion against what the author actually wrote, and
//! aggregating those scores.
//!
//! Definitions, for a suggestion `s` at a position whose true continuation of
//! the line is `t`:
//!
//! - The *offered text* is the part of `s` a user keeps without editing: the
//!   first line of `s`, and for a snippet only the literal text before its
//!   first placeholder. VS Code inserts the whole snippet, but placeholder
//!   defaults are selected for the user to overwrite.
//! - *Exact*: the first line of `s`, with snippet placeholders filled by their
//!   defaults, equals `t`.
//! - *Template match*: for a snippet, `t` fits its first line with every
//!   placeholder standing for some non-empty text, so
//!   `local ${1:name} = ${0:nil}` matches `local speed = 300`. For other
//!   suggestions it equals *exact*.
//! - *Correct prefix*: the offered text contains a non-whitespace character
//!   and `t` starts with it, so accepting it is never wrong.
//! - *Characters saved*: the length of the offered text when `t` starts with
//!   it, otherwise 0. Accepting a wrong suggestion saves nothing.
//! - *Incorrect characters*: offered characters past the longest common prefix
//!   with `t`, which a user would have to delete.
//! - *First-token* precision compares the first token of the offered text and
//!   `t`; *first-three-token* precision requires the offered text to have at
//!   least three tokens, all matching.

use std::collections::BTreeMap;

use serde::Serialize;

/// How one suggestion compares with the truth.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct Outcome {
    pub exact: bool,
    pub template_match: bool,
    pub correct_prefix: bool,
    pub first_token: bool,
    pub first_three_tokens: bool,
    pub chars_saved: usize,
    pub incorrect_chars: usize,
}

pub fn score(suggestion: &str, is_snippet: bool, truth: &str) -> Outcome {
    let first_line = suggestion.split('\n').next().unwrap_or_default();
    let (offered, rendered, template_match) = if is_snippet {
        let segments = snippet_segments(first_line);
        (
            snippet_literal_prefix(first_line),
            render(&segments),
            !truth.is_empty() && matches_template(&segments, truth),
        )
    } else {
        let exact = !truth.is_empty() && first_line == truth;
        (first_line.to_owned(), first_line.to_owned(), exact)
    };

    let common = common_prefix_chars(&offered, truth);
    let offered_chars = offered.chars().count();
    let offered_tokens = tokens(&offered);
    let truth_tokens = tokens(truth);
    let first_token = !offered_tokens.is_empty() && offered_tokens.first() == truth_tokens.first();
    let first_three_tokens = offered_tokens.len() >= 3
        && truth_tokens.len() >= 3
        && offered_tokens[..3] == truth_tokens[..3];
    let is_prefix = common == offered_chars;

    Outcome {
        exact: !truth.is_empty() && rendered == truth,
        template_match,
        correct_prefix: is_prefix && !offered.trim().is_empty(),
        first_token,
        first_three_tokens,
        chars_saved: if is_prefix { offered_chars } else { 0 },
        incorrect_chars: offered_chars - common,
    }
}

/// Lua/Tua-shaped tokens: words, numbers, quoted strings, and punctuation,
/// with multi-character operators kept whole. Whitespace is dropped.
pub fn tokens(text: &str) -> Vec<&str> {
    const OPERATORS: &[&str] = &[
        "...", "..", "==", "~=", "<=", ">=", "::", "->", "+=", "-=", "*=", "/=", "%=",
    ];
    let mut result = Vec::new();
    let mut rest = text;

    while let Some(character) = rest.chars().next() {
        if character.is_whitespace() {
            rest = &rest[character.len_utf8()..];
            continue;
        }
        let length = if character == '_' || character.is_alphanumeric() {
            rest.find(|next: char| {
                !(next == '_'
                    || next.is_alphanumeric()
                    || next == '.' && character.is_ascii_digit())
            })
            .unwrap_or(rest.len())
        } else if character == '"' || character == '\'' {
            let mut escaped = false;
            rest[1..]
                .char_indices()
                .find(|(_, next)| {
                    let closes = !escaped && *next == character;
                    escaped = !escaped && *next == '\\';
                    closes
                })
                .map_or(rest.len(), |(index, _)| index + 2)
        } else {
            OPERATORS
                .iter()
                .find(|operator| rest.starts_with(**operator))
                .map_or(character.len_utf8(), |operator| operator.len())
        };
        result.push(&rest[..length]);
        rest = &rest[length..];
    }

    result
}

/// Snippet text before its first `$1`, `${1:...}`, or `$0` placeholder.
pub fn snippet_literal_prefix(snippet: &str) -> String {
    let end = placeholder_start(snippet, 0).unwrap_or(snippet.len());
    unescape_snippet(&snippet[..end])
}

#[derive(Clone, Debug, PartialEq)]
enum Segment {
    Literal(String),
    /// A placeholder, with its default text (empty for `$1` or `${0}`).
    Placeholder(String),
}

fn snippet_segments(snippet: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut index = 0;
    while let Some(start) = placeholder_start(snippet, index) {
        let literal = unescape_snippet(&snippet[index..start]);
        if !literal.is_empty() {
            segments.push(Segment::Literal(literal));
        }
        let rest = &snippet[start + 1..];
        if let Some(body) = rest.strip_prefix('{') {
            let close = body.find('}').unwrap_or(body.len());
            let default = body[..close]
                .split_once(':')
                .map_or("", |(_, default)| default);
            segments.push(Segment::Placeholder(unescape_snippet(default)));
            index = start + 2 + close + 1;
        } else {
            segments.push(Segment::Placeholder(String::new()));
            index = start
                + 1
                + rest
                    .find(|next: char| !next.is_ascii_digit())
                    .unwrap_or(rest.len());
        }
    }
    let literal = unescape_snippet(&snippet[index.min(snippet.len())..]);
    if !literal.is_empty() {
        segments.push(Segment::Literal(literal));
    }
    segments
}

/// The snippet line as VS Code first inserts it, defaults filled in.
fn render(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|segment| match segment {
            Segment::Literal(text) | Segment::Placeholder(text) => text.as_str(),
        })
        .collect()
}

/// Whether `text` fits the segments, each placeholder matching one or more
/// characters.
fn matches_template(segments: &[Segment], text: &str) -> bool {
    match segments.split_first() {
        None => text.is_empty(),
        Some((Segment::Literal(literal), rest)) => text
            .strip_prefix(literal.as_str())
            .is_some_and(|remaining| matches_template(rest, remaining)),
        Some((Segment::Placeholder(_), rest)) => text
            .char_indices()
            .skip(1)
            .map(|(index, _)| index)
            .chain(std::iter::once(text.len()))
            .filter(|end| *end > 0)
            .any(|end| matches_template(rest, &text[end..])),
    }
}

fn placeholder_start(snippet: &str, from: usize) -> Option<usize> {
    let bytes = snippet.as_bytes();
    let mut index = from;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'$' if bytes
                .get(index + 1)
                .is_some_and(|next| *next == b'{' || next.is_ascii_digit()) =>
            {
                return Some(index)
            }
            _ => index += 1,
        }
    }
    None
}

fn unescape_snippet(text: &str) -> String {
    text.replace("\\$", "$")
        .replace("\\}", "}")
        .replace("\\\\", "\\")
}

fn common_prefix_chars(left: &str, right: &str) -> usize {
    left.chars()
        .zip(right.chars())
        .take_while(|(left, right)| left == right)
        .count()
}

/// Totals for one group of evaluated positions.
///
/// Rates ending in `_per_eligible` divide by `eligible`; precisions divide by
/// `suggested`. For a per-source breakdown, `eligible` is every eligible
/// request the backend saw, so the rates stay comparable with the overall
/// summary.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Summary {
    pub eligible: usize,
    pub suggested: usize,
    pub exact: usize,
    pub template_match: usize,
    pub correct_prefix: usize,
    pub first_token: usize,
    pub first_three_tokens: usize,
    pub chars_saved: usize,
    pub incorrect_chars: usize,
    pub suggestions_per_eligible: f64,
    pub exact_precision: f64,
    pub template_match_precision: f64,
    pub correct_prefix_precision: f64,
    pub first_token_precision: f64,
    pub first_three_token_precision: f64,
    pub chars_saved_per_eligible: f64,
    pub incorrect_chars_per_eligible: f64,
    /// Incorrect characters offered for every character saved, with at least
    /// one saved character in the denominator.
    pub wrong_to_saved_ratio: f64,
    pub latency_us: Percentiles,
    #[serde(skip)]
    latencies_ns: Vec<u64>,
}

impl Summary {
    pub fn add(&mut self, outcome: Option<Outcome>, latency_ns: u64) {
        self.eligible += 1;
        self.latencies_ns.push(latency_ns);
        if let Some(outcome) = outcome {
            self.add_suggestion(outcome);
        }
    }

    /// Counts a suggestion without counting an eligible request, for
    /// breakdowns whose denominator is set later with [`Summary::rebase`].
    pub fn add_suggestion(&mut self, outcome: Outcome) {
        self.suggested += 1;
        self.exact += usize::from(outcome.exact);
        self.template_match += usize::from(outcome.template_match);
        self.correct_prefix += usize::from(outcome.correct_prefix);
        self.first_token += usize::from(outcome.first_token);
        self.first_three_tokens += usize::from(outcome.first_three_tokens);
        self.chars_saved += outcome.chars_saved;
        self.incorrect_chars += outcome.incorrect_chars;
    }

    /// Sets the number of eligible requests behind this summary.
    pub fn rebase(mut self, eligible: usize) -> Self {
        self.eligible = eligible;
        self
    }

    pub fn finish(mut self) -> Self {
        let eligible = self.eligible.max(1) as f64;
        let suggested = self.suggested.max(1) as f64;
        self.suggestions_per_eligible = self.suggested as f64 / eligible;
        self.exact_precision = self.exact as f64 / suggested;
        self.template_match_precision = self.template_match as f64 / suggested;
        self.correct_prefix_precision = self.correct_prefix as f64 / suggested;
        self.first_token_precision = self.first_token as f64 / suggested;
        self.first_three_token_precision = self.first_three_tokens as f64 / suggested;
        self.chars_saved_per_eligible = self.chars_saved as f64 / eligible;
        self.incorrect_chars_per_eligible = self.incorrect_chars as f64 / eligible;
        self.wrong_to_saved_ratio = self.incorrect_chars as f64 / self.chars_saved.max(1) as f64;
        self.latency_us = Percentiles::of_ns(&mut self.latencies_ns);
        self
    }

    /// The counts that do not depend on timing, for the committed smoke
    /// expectations.
    pub fn deterministic(&self) -> DeterministicSummary {
        DeterministicSummary {
            eligible: self.eligible,
            suggested: self.suggested,
            exact: self.exact,
            template_match: self.template_match,
            correct_prefix: self.correct_prefix,
            first_token: self.first_token,
            first_three_tokens: self.first_three_tokens,
            chars_saved: self.chars_saved,
            incorrect_chars: self.incorrect_chars,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, serde::Deserialize)]
pub struct DeterministicSummary {
    pub eligible: usize,
    pub suggested: usize,
    pub exact: usize,
    pub template_match: usize,
    pub correct_prefix: usize,
    pub first_token: usize,
    pub first_three_tokens: usize,
    pub chars_saved: usize,
    pub incorrect_chars: usize,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Percentiles {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
}

impl Percentiles {
    pub fn of_ns(samples: &mut [u64]) -> Self {
        Self::of(samples, 1_000.0)
    }

    pub fn of_ms_from_ns(samples: &mut [u64]) -> Self {
        Self::of(samples, 1_000_000.0)
    }

    fn of(samples: &mut [u64], divisor: f64) -> Self {
        if samples.is_empty() {
            return Self::default();
        }
        samples.sort_unstable();
        let at = |quantile: f64| {
            let index = ((samples.len() - 1) as f64 * quantile).round() as usize;
            samples[index] as f64 / divisor
        };
        Self {
            p50: at(0.50),
            p95: at(0.95),
            p99: at(0.99),
            max: samples[samples.len() - 1] as f64 / divisor,
        }
    }
}

/// Summaries keyed by a breakdown value, such as a language or scenario.
pub type Breakdown = BTreeMap<String, Summary>;

/// One suggestion's score and whether it was right, for the curves.
#[derive(Clone, Copy, Debug)]
pub struct Scored {
    pub score: f64,
    pub exact: bool,
    pub correct_prefix: bool,
}

/// One point on a precision/coverage curve: suggestions with a score at least
/// `min_score`, as a fraction of eligible positions, and how many were right.
#[derive(Clone, Debug, Serialize)]
pub struct CurvePoint {
    pub min_score: f64,
    pub coverage: f64,
    pub exact_precision: f64,
    pub correct_prefix_precision: f64,
}

/// Precision and coverage as the minimum score rises, at each distinct score
/// decile among the suggestions.
pub fn precision_coverage_curve(scored: &[Scored], eligible: usize) -> Vec<CurvePoint> {
    if scored.is_empty() || eligible == 0 {
        return Vec::new();
    }
    let mut scores = scored.iter().map(|item| item.score).collect::<Vec<_>>();
    scores.sort_by(f64::total_cmp);
    let mut thresholds = (0..10)
        .map(|decile| scores[(scores.len() - 1) * decile / 10])
        .collect::<Vec<_>>();
    thresholds.dedup();

    thresholds
        .into_iter()
        .map(|min_score| {
            let kept = scored
                .iter()
                .filter(|item| item.score >= min_score)
                .collect::<Vec<_>>();
            let count = kept.len().max(1) as f64;
            CurvePoint {
                min_score,
                coverage: kept.len() as f64 / eligible as f64,
                exact_precision: kept.iter().filter(|item| item.exact).count() as f64 / count,
                correct_prefix_precision: kept.iter().filter(|item| item.correct_prefix).count()
                    as f64
                    / count,
            }
        })
        .collect()
}
