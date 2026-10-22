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
use super::ngram::EvalNgramIndex;
use super::retrieval::{
    EvalProjectRetrievalIndex, HoldoutMode, IndexState, RetrievalScope, RetrievalTier,
};

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
    pub path: &'a Path,
    pub language: &'static str,
    pub retrieval_index: Option<&'a EvalProjectRetrievalIndex>,
    pub ngram_index: Option<&'a EvalNgramIndex>,
    /// False for cut documents: lines after the cursor do not exist yet and
    /// must not be read from the original target-file table.
    pub allow_target_file_other_lines: bool,
}

/// Standalone evaluator-only Phase 3 predictor. The target file is excluded
/// by `EvalNgramIndex`, including for edit and auto-close scenarios.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Phase3TuningConfig {
    pub enabled_languages: BTreeSet<String>,
    pub contexts: BTreeSet<CursorContext>,
    pub min_score: f64,
}

impl Phase3TuningConfig {
    pub fn pinned() -> Self {
        Self {
            enabled_languages: BTreeSet::from(["tua".to_owned()]),
            contexts: BTreeSet::from([CursorContext::Assignment, CursorContext::Other]),
            min_score: 0.85,
        }
    }

    pub fn allows(&self, language: &str, context: CursorContext, score: f64) -> bool {
        self.enabled_languages.contains(language)
            && self.contexts.contains(&context)
            && score >= self.min_score
    }
}

impl Default for Phase3TuningConfig {
    fn default() -> Self {
        Self::pinned()
    }
}

pub struct Phase3Ngram {
    pub config: Option<Phase3TuningConfig>,
}

impl Backend for Phase3Ngram {
    fn name(&self) -> &'static str {
        if self.config.is_some() {
            "phase3-ngram"
        } else {
            "phase3-ngram-raw"
        }
    }

    fn suggest(&self, request: &Request<'_>) -> Option<Candidate> {
        if self.config.as_ref().is_some_and(|config| {
            !config.enabled_languages.contains(request.language)
                || !config.contexts.contains(&request.context)
        }) {
            return None;
        }
        let prediction = request.ngram_index?.predict(
            request.path,
            request.language,
            request.document,
            request.offset,
            Duration::from_millis(10),
            &AtomicBool::new(false),
        )?;
        if self.config.as_ref().is_some_and(|config| {
            !config.allows(request.language, request.context, prediction.score)
        }) {
            return None;
        }
        Some(Candidate {
            text: prediction.text,
            is_snippet: false,
            source: "project-ngram",
            score: prediction.score,
            tier: Some("project"),
        })
    }
}

/// Only the statistical suggestions that Phase 3 adds after the exact Phase 2
/// candidate abstains. This keeps Phase 3 precision and coverage measurable
/// without crediting it for the baseline's suggestions.
pub struct Phase3NgramIncremental {
    pub phase2: Phase2TuningConfig,
    pub ngram: Phase3TuningConfig,
}

impl Backend for Phase3NgramIncremental {
    fn name(&self) -> &'static str {
        "phase3-ngram-incremental"
    }

    fn suggest(&self, request: &Request<'_>) -> Option<Candidate> {
        if (Phase2Tuning {
            config: self.phase2.clone(),
        })
        .suggest(request)
        .is_some()
        {
            return None;
        }
        Phase3Ngram {
            config: Some(self.ngram.clone()),
        }
        .suggest(request)
    }

    fn suggest_with_host(
        &self,
        request: &Request<'_>,
        production: &ProductionRequest<'_>,
    ) -> Option<Candidate> {
        if (Phase2Tuning {
            config: self.phase2.clone(),
        })
        .suggest_with_host(request, production)
        .is_some()
        {
            return None;
        }
        Phase3Ngram {
            config: Some(self.ngram.clone()),
        }
        .suggest(request)
    }
}

/// Conservative comparison candidate: keep the measured Phase 2 result and
/// ask the statistical backend only when semantic and exact retrieval abstain.
pub struct Phase3NgramFallback {
    pub phase2: Phase2TuningConfig,
    pub ngram: Phase3TuningConfig,
}

impl Backend for Phase3NgramFallback {
    fn name(&self) -> &'static str {
        "phase3-phase2-plus-ngram-fallback"
    }

    fn suggest(&self, request: &Request<'_>) -> Option<Candidate> {
        Phase2Tuning {
            config: self.phase2.clone(),
        }
        .suggest(request)
        .or_else(|| {
            Phase3Ngram {
                config: Some(self.ngram.clone()),
            }
            .suggest(request)
        })
    }

    fn suggest_with_host(
        &self,
        request: &Request<'_>,
        production: &ProductionRequest<'_>,
    ) -> Option<Candidate> {
        Phase2Tuning {
            config: self.phase2.clone(),
        }
        .suggest_with_host(request, production)
        .or_else(|| {
            Phase3Ngram {
                config: Some(self.ngram.clone()),
            }
            .suggest(request)
        })
    }

    fn index_state(&self) -> &'static str {
        "complete"
    }
}

/// A suggestion: text inserted at the cursor, continuing what is typed.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Candidate {
    pub text: String,
    pub is_snippet: bool,
    pub source: &'static str,
    /// Comparable only within one source.
    pub score: f64,
    /// Present only for Phase 2 exact retrieval.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<&'static str>,
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

    fn index_state(&self) -> &'static str {
        "not-applicable"
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
        tier: None,
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
        tier: None,
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
            tier: None,
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
            tier: None,
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
                    project: false,
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
                tier: None,
            })
    }
}

/// Exact project retrieval with an explicit leakage holdout and corpus state.
pub struct ProjectExactRetrieval {
    pub holdout: HoldoutMode,
    pub state: IndexState,
    pub scope: RetrievalScope,
}

/// Frozen tuning policy for the first combined Phase 2 candidate. It keeps
/// the shipping Phase 1 result when one exists, then fills only three
/// high-signal contexts from the rest-of-project tier. Same-directory and
/// bundled evidence are intentionally absent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Phase2TuningConfig {
    pub contexts: BTreeSet<CursorContext>,
    pub min_prefix_chars: usize,
    pub min_support: usize,
    pub min_full_prefix_support: usize,
}

impl Phase2TuningConfig {
    pub fn pinned() -> Self {
        Self {
            contexts: BTreeSet::from([
                CursorContext::Comma,
                CursorContext::Operator,
                CursorContext::Other,
            ]),
            min_prefix_chars: 6,
            min_support: 2,
            min_full_prefix_support: 2,
        }
    }
}

impl Default for Phase2TuningConfig {
    fn default() -> Self {
        Self::pinned()
    }
}

pub struct Phase2Tuning {
    pub config: Phase2TuningConfig,
}

impl Phase2Tuning {
    fn project_candidate(&self, request: &Request<'_>) -> Option<Candidate> {
        if !self.config.contexts.contains(&request.context) {
            return None;
        }
        let line_start = request.document[..request.offset]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let prefix = &request.document[line_start..request.offset];
        if prefix
            .chars()
            .filter(|character| !character.is_whitespace())
            .count()
            < self.config.min_prefix_chars
        {
            return None;
        }
        let hit = request.retrieval_index?.query_project_common_prefix(
            IndexState::Complete,
            request.path,
            prefix,
            self.config.min_support,
            self.config.min_full_prefix_support,
        )?;
        Some(Candidate {
            text: hit.continuation,
            is_snippet: false,
            source: "project-common-prefix",
            score: hit.support as f64,
            tier: Some(hit.tier.label()),
        })
    }
}

impl Backend for Phase2Tuning {
    fn name(&self) -> &'static str {
        "phase2-tuning-phase1-plus-project-common-prefix"
    }

    fn suggest(&self, request: &Request<'_>) -> Option<Candidate> {
        self.project_candidate(request)
    }

    fn suggest_with_host(
        &self,
        _request: &Request<'_>,
        production: &ProductionRequest<'_>,
    ) -> Option<Candidate> {
        assert_eq!(
            self.config,
            Phase2TuningConfig::pinned(),
            "the production Phase 2 evaluator must use the pinned host policy"
        );
        production
            .host
            .local_inline_completion(
                production.path,
                production.position,
                &crate::InlineCompletionOptions {
                    semantic: true,
                    current_file: true,
                    project: true,
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
                tier: None,
            })
    }

    fn index_state(&self) -> &'static str {
        "complete"
    }
}

impl Backend for ProjectExactRetrieval {
    fn name(&self) -> &'static str {
        match (self.scope, self.holdout, self.state) {
            (RetrievalScope::Combined, HoldoutMode::TargetFile, IndexState::Cold) => {
                "project-file-holdout-cold"
            }
            (RetrievalScope::Combined, HoldoutMode::TargetFile, IndexState::Partial) => {
                "project-file-holdout-partial"
            }
            (RetrievalScope::Combined, HoldoutMode::TargetFile, IndexState::Complete) => {
                "project-file-holdout-complete"
            }
            (RetrievalScope::Combined, HoldoutMode::TargetLine, IndexState::Cold) => {
                "project-line-holdout-cold"
            }
            (RetrievalScope::Combined, HoldoutMode::TargetLine, IndexState::Partial) => {
                "project-line-holdout-partial"
            }
            (RetrievalScope::Combined, HoldoutMode::TargetLine, IndexState::Complete) => {
                "project-line-holdout-complete"
            }
            (RetrievalScope::SameDirectory, HoldoutMode::TargetFile, IndexState::Cold) => {
                "same-directory-only-file-holdout-cold"
            }
            (RetrievalScope::SameDirectory, HoldoutMode::TargetFile, IndexState::Partial) => {
                "same-directory-only-file-holdout-partial"
            }
            (RetrievalScope::SameDirectory, HoldoutMode::TargetFile, IndexState::Complete) => {
                "same-directory-only-file-holdout-complete"
            }
            (RetrievalScope::SameDirectory, HoldoutMode::TargetLine, IndexState::Cold) => {
                "same-directory-only-line-holdout-cold"
            }
            (RetrievalScope::SameDirectory, HoldoutMode::TargetLine, IndexState::Partial) => {
                "same-directory-only-line-holdout-partial"
            }
            (RetrievalScope::SameDirectory, HoldoutMode::TargetLine, IndexState::Complete) => {
                "same-directory-only-line-holdout-complete"
            }
            (RetrievalScope::Project, HoldoutMode::TargetFile, IndexState::Cold) => {
                "project-only-file-holdout-cold"
            }
            (RetrievalScope::Project, HoldoutMode::TargetFile, IndexState::Partial) => {
                "project-only-file-holdout-partial"
            }
            (RetrievalScope::Project, HoldoutMode::TargetFile, IndexState::Complete) => {
                "project-only-file-holdout-complete"
            }
            (RetrievalScope::Project, HoldoutMode::TargetLine, IndexState::Cold) => {
                "project-only-line-holdout-cold"
            }
            (RetrievalScope::Project, HoldoutMode::TargetLine, IndexState::Partial) => {
                "project-only-line-holdout-partial"
            }
            (RetrievalScope::Project, HoldoutMode::TargetLine, IndexState::Complete) => {
                "project-only-line-holdout-complete"
            }
        }
    }

    fn suggest(&self, request: &Request<'_>) -> Option<Candidate> {
        let line_start = request.document[..request.offset]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let prefix = &request.document[line_start..request.offset];
        let hit = request.retrieval_index?.query(
            self.state,
            self.holdout,
            request.path,
            request.line_index,
            prefix,
            request.allow_target_file_other_lines,
            self.scope,
        )?;
        Some(Candidate {
            text: hit.continuation,
            is_snippet: false,
            source: "project-exact",
            score: hit.support as f64,
            tier: Some(hit.tier.label()),
        })
    }

    fn index_state(&self) -> &'static str {
        self.state.label()
    }
}

/// The separately owned bundled Lua/LÖVE cold-start corpus.
pub struct BundledExactRetrieval;

impl Backend for BundledExactRetrieval {
    fn name(&self) -> &'static str {
        "bundled-corpus"
    }

    fn suggest(&self, request: &Request<'_>) -> Option<Candidate> {
        let line_start = request.document[..request.offset]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let prefix = &request.document[line_start..request.offset];
        let mut candidates = std::collections::BTreeMap::<String, usize>::new();
        for file in super::super::corpus::bundled_corpus().files() {
            if request.language == "lua"
                && file.language == super::super::corpus::BundledCorpusLanguage::Tua
            {
                continue;
            }
            for hit in file.table.query(prefix, None, usize::MAX) {
                *candidates.entry(hit.continuation).or_default() += hit.support();
            }
        }
        let (text, support) = candidates.into_iter().min_by(
            |(left_text, left_support), (right_text, right_support)| {
                right_support
                    .cmp(left_support)
                    .then_with(|| left_text.cmp(right_text))
            },
        )?;
        Some(Candidate {
            text,
            is_snippet: false,
            source: "bundled-corpus",
            score: support as f64,
            tier: Some(RetrievalTier::Bundled.label()),
        })
    }

    fn index_state(&self) -> &'static str {
        "bundled"
    }
}
