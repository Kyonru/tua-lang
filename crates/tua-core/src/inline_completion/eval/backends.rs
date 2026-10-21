//! The candidate sources the evaluator compares. These are measurement
//! prototypes of the Phase 1 sources, following its rules, so Phase 0 can
//! decide source order and thresholds before anything ships.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde::Serialize;

use super::super::eligibility::CursorContext;
pub use super::super::recall::RecallOptions;

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

    fn suggest_with_host(
        &self,
        request: &Request<'_>,
        _production: &ProductionRequest<'_>,
    ) -> Option<Candidate> {
        self.suggest(request)
    }
}

pub struct ProductionRequest<'a> {
    pub host: &'a crate::AnalysisHost,
    pub path: &'a Path,
    pub position: crate::SourcePosition,
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
    let text = super::super::semantic::statement_snippet(
        request.document,
        request.offset,
        request.context,
        request.include_ecs,
        min_prefix_chars,
    )?;
    Some(Candidate {
        text,
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
        let suggestion = super::super::recall::unique(
            request.document,
            request.offset,
            request.line_index,
            self.options,
        )?;
        Some(Candidate {
            text: suggestion.text,
            is_snippet: false,
            source: "current-file",
            score: suggestion.score,
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
        let suggestion = super::super::recall::common_prefix(
            request.document,
            request.offset,
            request.line_index,
            self.recall.options,
        )?;
        Some(Candidate {
            text: suggestion.text,
            is_snippet: false,
            source: "current-file-prefix",
            score: suggestion.score,
        })
    }
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
#[derive(Clone, Debug, PartialEq, Serialize)]
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

impl Phase1Config {
    /// The policy compiled into the shipping Phase 1 host query. Gate files
    /// must name this exact policy; exploratory tuning uses the separately
    /// labelled `phase1-prototype` backend.
    pub fn production() -> Self {
        Self {
            recall_variant: RecallVariant::CommonPrefix,
            contexts: Some(BTreeSet::from([
                CursorContext::OpenParen,
                CursorContext::Comma,
                CursorContext::Operator,
                CursorContext::Whitespace,
                CursorContext::StatementPrefix,
                CursorContext::Identifier,
                CursorContext::Other,
            ])),
            recall: RecallOptions::phase1(),
            love_callbacks: false,
            statement_snippets: true,
            snippet_min_prefix_chars: 3,
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
        "phase1-prototype"
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

/// The exact shipping Phase 1 host query, including its validation and core
/// deadline. Quality gates use this backend rather than the raw source
/// prototypes above; queue and protocol latency are measured separately.
pub struct ProductionPhase1 {
    pub semantic: bool,
    pub current_file: bool,
}

impl Backend for ProductionPhase1 {
    fn name(&self) -> &'static str {
        "phase1"
    }

    fn suggest(&self, _request: &Request<'_>) -> Option<Candidate> {
        None
    }

    fn suggest_with_host(
        &self,
        _request: &Request<'_>,
        production: &ProductionRequest<'_>,
    ) -> Option<Candidate> {
        production
            .host
            .local_inline_completion(
                production.path,
                production.position,
                &crate::InlineCompletionOptions {
                    semantic: self.semantic,
                    current_file: self.current_file,
                },
                &crate::InlineCompletionBudget::new(Duration::from_millis(10)),
                &AtomicBool::new(false),
            )
            .into_iter()
            .next()
            .map(|candidate| Candidate {
                text: candidate.insert_text,
                is_snippet: candidate.is_snippet,
                source: candidate.source.as_str(),
                score: candidate.score.0,
            })
    }
}
