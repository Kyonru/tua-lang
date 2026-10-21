//! Offline evaluation of local inline completion (Phase 0 of
//! `better-autocompletion.md`).
//!
//! The evaluator replays the source of real projects: at sampled positions it
//! hides what the author wrote after the cursor, asks each candidate source for
//! a suggestion, and scores that suggestion against the hidden text. It also
//! records the measurements the later phases are gated on: request latency,
//! complete-parse cost, how often a warm-up would have cached the current
//! file in time, and memory.
//!
//! Run it with `cargo run -p tua-core --features test-fixture --example
//! inline_eval`; the Make targets `inline-eval`, `inline-eval-smoke`, and
//! `inline-eval-external` wrap it.

pub mod backends;
pub mod dataset;
pub mod metrics;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use self::backends::{
    Backend, Candidate, CurrentFileCommonPrefix, CurrentFileRecall, Noop, Phase1, Phase1Config,
    RecallOptions, RecallVariant, Request, Semantic,
};
use self::dataset::{DuplicateReport, EvalFile, EvalProject, EvalSet, SetRole};
use self::metrics::{
    Breakdown, CurvePoint, DeterministicSummary, Outcome, Percentiles, Scored, Summary,
};
use super::eligibility::{
    auto_close_suffix, cursor_context, is_identifier_char, lexical_region_at,
    strip_trailing_comment, suffix_allows_inline, word_before, CursorContext, LexicalRegion,
};
use crate::{AnalysisHost, AnalysisWarmPriority, SourcePosition};

/// Delays after the last keystroke at which a request fires: the inline
/// debounce during continuous typing, then the pauses the plan asks about.
pub const REQUEST_DELAYS_MS: [u64; 4] = [60, 150, 300, 1000];

#[derive(Clone, Debug)]
pub struct EvalOptions {
    /// Positions evaluated per file, evenly spaced from the first to the
    /// last; 0 evaluates all of them.
    pub max_positions_per_file: usize,
    /// Simulate VS Code's suggestion menu with real menu completion.
    pub simulate_menu: bool,
    /// Build and time a real warm-up of each scenario's document, for the
    /// cache hit rate and for the parsed facts semantic sources use.
    pub measure_warmup: bool,
    /// Timed parses of each file for the parse baseline, after one untimed
    /// warm-up parse.
    pub parse_repeats: usize,
    /// Parse time an inline request may spend, for the recommended byte cap.
    pub parse_budget_ms: f64,
    /// Bytes currently allocated, when the caller installs a counting
    /// allocator.
    pub allocated_bytes: Option<fn() -> usize>,
    /// Options for the standalone recall backends.
    pub recall: RecallOptions,
    /// The candidate Phase 1 configuration.
    pub phase1: Phase1Config,
    /// Keep one record per evaluated suggestion in the report.
    pub keep_records: bool,
}

impl Default for EvalOptions {
    fn default() -> Self {
        Self {
            max_positions_per_file: 40,
            simulate_menu: true,
            measure_warmup: true,
            parse_repeats: 5,
            parse_budget_ms: 4.0,
            allocated_bytes: None,
            recall: RecallOptions::default(),
            phase1: Phase1Config::default(),
            keep_records: false,
        }
    }
}

/// How the document looks when the request fires.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scenario {
    /// New code at the end of a file: nothing after the cursor exists yet.
    Cut,
    /// Editing an existing file: the rest of the active line is withheld, and
    /// every other line is present.
    Edit,
    /// Like `Edit`, with the closers for the line's still-open brackets that
    /// an editor auto-inserts already after the cursor.
    AutoClose,
}

impl Scenario {
    fn label(self) -> &'static str {
        match self {
            Self::Cut => "cut",
            Self::Edit => "edit",
            Self::AutoClose => "auto-close",
        }
    }
}

/// How much of the current token is typed at a position.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrefixKind {
    /// After a whole token, such as `local` or `(`.
    TokenEnd,
    /// One, two, or three characters into a word.
    Partial1,
    Partial2,
    Partial3,
    /// One character short of the end of a word longer than four characters.
    PartialNear,
}

impl PrefixKind {
    fn label(self) -> &'static str {
        match self {
            Self::TokenEnd => "token-end",
            Self::Partial1 => "partial-1",
            Self::Partial2 => "partial-2",
            Self::Partial3 => "partial-3",
            Self::PartialNear => "partial-near",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MenuState {
    Closed,
    Open,
}

/// One suggestion, kept when `keep_records` is set.
#[derive(Clone, Debug, Serialize)]
pub struct Record {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub language: &'static str,
    pub scenario: Scenario,
    pub menu: MenuState,
    pub context: &'static str,
    pub prefix: PrefixKind,
    pub document: &'static str,
    pub backend: &'static str,
    pub truth: String,
    pub candidate: Option<Candidate>,
    pub outcome: Option<Outcome>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct BackendReport {
    /// Requests where the author typed more on the line.
    pub overall: Summary,
    pub by_language: Breakdown,
    pub by_scenario: Breakdown,
    pub by_menu: Breakdown,
    pub by_context: Breakdown,
    pub by_prefix: Breakdown,
    /// Whether the document parsed when the request fired: `parses`,
    /// `invalid`, or `unknown` when no analysis was built.
    pub by_document: Breakdown,
    pub by_project: Breakdown,
    /// Per source, with every eligible request in the denominator.
    pub by_source: Breakdown,
    /// Requests at the end of a line, where the right answer is no
    /// suggestion. Every suggestion here is wrong.
    pub abstain: Summary,
    pub precision_coverage: Vec<CurvePoint>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct WarmupReport {
    pub samples: usize,
    pub warm_ms: Percentiles,
    /// Fraction of requests whose current-file analysis would already be
    /// cached, keyed by the request delay in milliseconds.
    pub hit_rate_by_delay_ms: BTreeMap<u64, f64>,
    /// Fraction of warmed revisions whose cache entry had parsed declaration
    /// facts, from a clean or a recovered parse.
    pub parsed_facts_rate: f64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ParseReport {
    pub files: usize,
    pub bytes: usize,
    pub failed: usize,
    /// Every timed parse, not one per file.
    pub samples: usize,
    pub latency_ms: Percentiles,
    /// Latency by file size: `<4KiB`, `4-16KiB`, `16-64KiB`, `>=64KiB`.
    pub latency_ms_by_size: BTreeMap<String, Percentiles>,
    /// Bytes parsed per millisecond over files of at least 512 bytes, at the
    /// median sample and at the slowest 5% of samples.
    pub throughput_bytes_per_ms_p50: f64,
    pub throughput_bytes_per_ms_p5: f64,
    /// The largest document whose parse fits `parse_budget_ms` at the p5
    /// throughput with a 2x safety margin, capped at 256 KiB.
    pub recommended_max_bytes: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct MemoryReport {
    /// Live heap bytes before the project was loaded.
    pub before_load_bytes: usize,
    /// Added by setting the project's files.
    pub files_delta_bytes: usize,
    /// Added by a full snapshot, the representative project load.
    pub snapshot_delta_bytes: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SetReport {
    pub name: String,
    pub role: Option<SetRole>,
    pub projects: usize,
    pub files_by_language: BTreeMap<String, usize>,
    /// Sampled positions with text left to predict.
    pub positions: usize,
    /// Sampled positions at the end of a line.
    pub abstain_positions: usize,
    pub ineligible_positions: usize,
    pub phase1: Phase1Config,
    pub backends: BTreeMap<String, BackendReport>,
    pub warmup_by_language: BTreeMap<String, WarmupReport>,
    pub parse_by_language: BTreeMap<String, ParseReport>,
    pub memory: Vec<(String, MemoryReport)>,
    pub records: Vec<Record>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct EvalReport {
    pub duplicates: DuplicateReport,
    pub recall_options: Option<RecallOptions>,
    pub sets: Vec<SetReport>,
}

impl EvalReport {
    /// Timing-independent counts per set and backend, compared against the
    /// committed smoke expectations.
    pub fn deterministic(&self) -> BTreeMap<String, BTreeMap<String, DeterministicTotals>> {
        let counts = |breakdown: &Breakdown| {
            breakdown
                .iter()
                .map(|(key, summary)| (key.clone(), summary.deterministic()))
                .collect()
        };
        self.sets
            .iter()
            .map(|set| {
                let backends = set
                    .backends
                    .iter()
                    .map(|(name, report)| {
                        (
                            name.clone(),
                            DeterministicTotals {
                                overall: report.overall.deterministic(),
                                abstain: report.abstain.deterministic(),
                                by_language: counts(&report.by_language),
                                by_scenario: counts(&report.by_scenario),
                                by_prefix: counts(&report.by_prefix),
                            },
                        )
                    })
                    .collect();
                (set.name.clone(), backends)
            })
            .collect()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeterministicTotals {
    pub overall: DeterministicSummary,
    pub abstain: DeterministicSummary,
    pub by_language: BTreeMap<String, DeterministicSummary>,
    pub by_scenario: BTreeMap<String, DeterministicSummary>,
    pub by_prefix: BTreeMap<String, DeterministicSummary>,
}

pub fn evaluate(
    sets: &[EvalSet],
    duplicates: DuplicateReport,
    options: &EvalOptions,
) -> EvalReport {
    let backends: Vec<Box<dyn Backend>> = vec![
        Box::new(Noop),
        Box::new(Semantic),
        Box::new(CurrentFileRecall {
            options: options.recall,
        }),
        Box::new(CurrentFileCommonPrefix {
            recall: CurrentFileRecall {
                options: options.recall,
            },
        }),
        Box::new(Phase1 {
            config: options.phase1.clone(),
        }),
    ];

    EvalReport {
        duplicates,
        recall_options: Some(options.recall),
        sets: sets
            .iter()
            .map(|set| evaluate_set(set, &backends, options))
            .collect(),
    }
}

#[derive(Default)]
struct BackendAccumulator {
    overall: Summary,
    by_language: Breakdown,
    by_scenario: Breakdown,
    by_menu: Breakdown,
    by_context: Breakdown,
    by_prefix: Breakdown,
    by_document: Breakdown,
    by_project: Breakdown,
    by_source: Breakdown,
    abstain: Summary,
    scored: Vec<Scored>,
}

impl BackendAccumulator {
    fn add(
        &mut self,
        keys: &Keys<'_>,
        candidate: Option<&Candidate>,
        outcome: Option<Outcome>,
        latency_ns: u64,
    ) {
        if keys.abstain {
            self.abstain.add(outcome, latency_ns);
            return;
        }
        self.overall.add(outcome, latency_ns);
        for (breakdown, key) in [
            (&mut self.by_language, keys.language),
            (&mut self.by_scenario, keys.scenario),
            (&mut self.by_menu, keys.menu),
            (&mut self.by_context, keys.context),
            (&mut self.by_prefix, keys.prefix),
            (&mut self.by_document, keys.document),
            (&mut self.by_project, keys.project),
        ] {
            breakdown
                .entry(key.to_owned())
                .or_default()
                .add(outcome, latency_ns);
        }
        if let (Some(candidate), Some(outcome)) = (candidate, outcome) {
            self.by_source
                .entry(candidate.source.to_owned())
                .or_default()
                .add_suggestion(outcome);
            self.scored.push(Scored {
                score: candidate.score,
                exact: outcome.exact,
                correct_prefix: outcome.correct_prefix,
            });
        }
    }

    fn finish(self) -> BackendReport {
        let eligible = self.overall.eligible;
        let finish = |breakdown: Breakdown| {
            breakdown
                .into_iter()
                .map(|(key, summary)| (key, summary.finish()))
                .collect()
        };
        BackendReport {
            precision_coverage: metrics::precision_coverage_curve(&self.scored, eligible),
            overall: self.overall.finish(),
            by_language: finish(self.by_language),
            by_scenario: finish(self.by_scenario),
            by_menu: finish(self.by_menu),
            by_context: finish(self.by_context),
            by_prefix: finish(self.by_prefix),
            by_document: finish(self.by_document),
            by_project: finish(self.by_project),
            by_source: self
                .by_source
                .into_iter()
                .map(|(key, summary)| (key, summary.rebase(eligible).finish()))
                .collect(),
            abstain: self.abstain.finish(),
        }
    }
}

struct Keys<'a> {
    abstain: bool,
    language: &'a str,
    scenario: &'a str,
    menu: &'a str,
    context: &'a str,
    prefix: &'a str,
    document: &'a str,
    project: &'a str,
}

#[derive(Default)]
struct WarmupAccumulator {
    durations_ns: Vec<u64>,
    with_facts: usize,
}

fn evaluate_set(set: &EvalSet, backends: &[Box<dyn Backend>], options: &EvalOptions) -> SetReport {
    let mut report = SetReport {
        name: set.name.clone(),
        role: Some(set.role),
        projects: set.projects.len(),
        phase1: options.phase1.clone(),
        ..SetReport::default()
    };
    let mut accumulators = backends
        .iter()
        .map(|_| BackendAccumulator::default())
        .collect::<Vec<_>>();
    let mut warmups = BTreeMap::<&'static str, WarmupAccumulator>::new();
    let mut parse_samples = BTreeMap::<&'static str, Vec<(usize, Option<Vec<u64>>)>>::new();

    for project in &set.projects {
        for file in &project.files {
            *report
                .files_by_language
                .entry(file.language().to_owned())
                .or_default() += 1;
            parse_samples.entry(file.language()).or_default().push((
                file.source.len(),
                measure_parse(file, options.parse_repeats),
            ));
        }

        let (mut host, memory) = load_host(project, options);
        report.memory.push((project.name.clone(), memory));

        for file in &project.files {
            evaluate_file(
                &mut host,
                project,
                file,
                backends,
                options,
                &mut accumulators,
                &mut warmups,
                &mut report,
            );
        }
    }

    report.backends = backends
        .iter()
        .zip(accumulators)
        .map(|(backend, accumulator)| (backend.name().to_owned(), accumulator.finish()))
        .collect();
    report.warmup_by_language = warmups
        .into_iter()
        .map(|(language, mut warmup)| {
            let samples = warmup.durations_ns.len();
            let hit_rate_by_delay_ms = REQUEST_DELAYS_MS
                .iter()
                .map(|delay| {
                    let limit = delay * 1_000_000;
                    let hits = warmup
                        .durations_ns
                        .iter()
                        .filter(|duration| **duration <= limit)
                        .count();
                    (*delay, hits as f64 / samples.max(1) as f64)
                })
                .collect();
            (
                language.to_owned(),
                WarmupReport {
                    samples,
                    warm_ms: Percentiles::of_ms_from_ns(&mut warmup.durations_ns),
                    hit_rate_by_delay_ms,
                    parsed_facts_rate: warmup.with_facts as f64 / samples.max(1) as f64,
                },
            )
        })
        .collect();
    report.parse_by_language = parse_samples
        .into_iter()
        .map(|(language, samples)| {
            (
                language.to_owned(),
                parse_report(samples, options.parse_budget_ms),
            )
        })
        .collect();

    report
}

fn load_host(project: &EvalProject, options: &EvalOptions) -> (AnalysisHost, MemoryReport) {
    let allocated = || options.allocated_bytes.map_or(0, |current| current());
    let before_load_bytes = allocated();
    let mut host = AnalysisHost::with_config(project.config.clone());
    host.set_source_roots(project.source_roots.iter().cloned());
    host.set_output_roots(project.output_roots.iter().cloned());
    host.set_asset_roots(project.asset_roots.iter().cloned());
    for file in &project.files {
        host.set_file_with_kind(&file.path, file.source.clone(), file.kind);
    }
    let after_files_bytes = allocated();
    // Representative project loading: a full snapshot, as the language server
    // builds for diagnostics.
    let snapshot = host.snapshot();
    let after_snapshot_bytes = allocated();
    drop(snapshot);

    (
        host,
        MemoryReport {
            before_load_bytes,
            files_delta_bytes: after_files_bytes.saturating_sub(before_load_bytes),
            snapshot_delta_bytes: after_snapshot_bytes.saturating_sub(after_files_bytes),
        },
    )
}

/// A position in a file, with the text the author wrote after it.
#[derive(Clone, Copy, Debug)]
struct Position {
    offset: usize,
    line_index: usize,
    line_start: usize,
    line_end: usize,
    prefix: PrefixKind,
}

/// What a request in one scenario can know, from the analysis built for that
/// scenario's own document.
#[derive(Default)]
struct DocumentFacts {
    declared_callbacks: Option<BTreeSet<String>>,
    parses: Option<bool>,
    selected: Option<String>,
}

impl DocumentFacts {
    fn document_label(&self) -> &'static str {
        match self.parses {
            Some(true) => "parses",
            Some(false) => "invalid",
            None => "unknown",
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_file(
    host: &mut AnalysisHost,
    project: &EvalProject,
    file: &EvalFile,
    backends: &[Box<dyn Backend>],
    options: &EvalOptions,
    accumulators: &mut [BackendAccumulator],
    warmups: &mut BTreeMap<&'static str, WarmupAccumulator>,
    report: &mut SetReport,
) {
    let source = file.source.replace("\r\n", "\n");
    let (positions, ineligible) = sample_positions(&source, options.max_positions_per_file);
    report.ineligible_positions += ineligible;
    let include_ecs = project.config.ecs.enabled;
    let file_label = dataset::display_name(&file.path);
    let analyze = options.measure_warmup || options.simulate_menu;

    for position in &positions {
        let remainder = &source[position.offset..position.line_end];
        let truth = strip_trailing_comment(remainder);
        let abstain = truth.is_empty();
        if abstain {
            report.abstain_positions += 1;
        } else {
            report.positions += 1;
        }
        let after_line = &source[position.line_end..];
        let prefix = &source[..position.offset];
        let column = source[position.line_start..position.offset].chars().count() + 1;
        let source_position = SourcePosition {
            line: position.line_index + 1,
            column,
        };
        let Some(context) = cursor_context(prefix, position.offset) else {
            continue;
        };

        let closers = auto_close_suffix(&source[position.line_start..position.offset]);
        let mut scenarios = vec![
            (Scenario::Cut, prefix.to_owned(), String::new(), truth),
            (
                Scenario::Edit,
                format!("{prefix}{after_line}"),
                String::new(),
                truth,
            ),
        ];
        // The editor already shows the closers, so the author's line must end
        // with them for the hidden text to be what remains to type.
        if !closers.is_empty() && truth.ends_with(closers.as_str()) {
            let expected = truth[..truth.len() - closers.len()].trim_end();
            if !expected.is_empty() {
                scenarios.push((
                    Scenario::AutoClose,
                    format!("{prefix}{closers}{after_line}"),
                    closers.clone(),
                    expected,
                ));
            }
        }

        for (scenario, document, suffix, expected) in scenarios {
            if !suffix_allows_inline(&suffix) {
                continue;
            }
            let facts = if analyze {
                document_facts(
                    host,
                    file,
                    &document,
                    source_position,
                    position.offset,
                    context,
                    options,
                    // Time the warm-up once per position, on the edited file.
                    (options.measure_warmup && scenario == Scenario::Edit)
                        .then(|| warmups.entry(file.language()).or_default()),
                )
            } else {
                DocumentFacts::default()
            };
            let menu = if facts.selected.is_some() {
                MenuState::Open
            } else {
                MenuState::Closed
            };
            let request = Request {
                document: &document,
                offset: position.offset,
                line_index: position.line_index,
                context,
                declared_callbacks: facts.declared_callbacks.as_ref(),
                include_ecs,
            };
            let keys = Keys {
                abstain,
                language: file.language(),
                scenario: scenario.label(),
                menu: match menu {
                    MenuState::Closed => "closed",
                    MenuState::Open => "open",
                },
                context: context.label(),
                prefix: position.prefix.label(),
                document: facts.document_label(),
                project: &project.name,
            };

            for (backend, accumulator) in backends.iter().zip(accumulators.iter_mut()) {
                let started = Instant::now();
                let raw = backend.suggest(&request);
                let latency_ns = started.elapsed().as_nanos() as u64;
                let candidate = raw
                    .and_then(|candidate| agree_with_suffix(candidate, &suffix))
                    .and_then(|candidate| {
                        extends_selection(
                            candidate,
                            facts.selected.as_deref(),
                            &document,
                            position.offset,
                        )
                    });
                let outcome = candidate.as_ref().map(|candidate| {
                    metrics::score(&candidate.text, candidate.is_snippet, expected)
                });
                accumulator.add(&keys, candidate.as_ref(), outcome, latency_ns);
                if options.keep_records && candidate.is_some() {
                    report.records.push(Record {
                        file: file_label.clone(),
                        line: position.line_index + 1,
                        column,
                        language: file.language(),
                        scenario,
                        menu,
                        context: context.label(),
                        prefix: position.prefix,
                        document: facts.document_label(),
                        backend: backend.name(),
                        truth: expected.to_owned(),
                        candidate,
                        outcome,
                    });
                }
            }
        }
    }

    if analyze {
        // Closing returns the file to its on-disk text for later files.
        host.close_document(&file.path);
    }
}

/// Opens `document` as the current revision, warms it, and reads what a
/// request would know: cached callback facts, whether it parses, and the
/// simulated menu's selection.
#[allow(clippy::too_many_arguments)]
fn document_facts(
    host: &mut AnalysisHost,
    file: &EvalFile,
    document: &str,
    position: SourcePosition,
    offset: usize,
    context: CursorContext,
    options: &EvalOptions,
    warmup: Option<&mut WarmupAccumulator>,
) -> DocumentFacts {
    host.open_document_with_kind(&file.path, document.to_owned(), file.kind);
    let started = Instant::now();
    host.warm_file(&file.path, AnalysisWarmPriority::High);
    let elapsed = started.elapsed().as_nanos() as u64;
    let declared_callbacks = host.peek_declared_love_callbacks(&file.path);
    if let Some(warmup) = warmup {
        warmup.durations_ns.push(elapsed);
        warmup.with_facts += usize::from(declared_callbacks.is_some());
    }
    let selected = (options.simulate_menu && context.menu_may_be_open())
        .then(|| menu_selection(host, &file.path, position, document, offset))
        .flatten();

    DocumentFacts {
        parses: host.peek_current_source_parses(&file.path),
        declared_callbacks,
        selected,
    }
}

/// Positions a user plausibly pauses at, on code lines outside strings,
/// comments, and leading indentation:
///
/// - after each whole token, including the end of the line;
/// - inside each word of two or more characters, after its first one, two,
///   and three characters and one character before its end.
///
/// At most `limit` positions are kept, evenly spaced from the first to the
/// last, so results are deterministic.
fn sample_positions(source: &str, limit: usize) -> (Vec<Position>, usize) {
    let mut positions = Vec::new();
    let mut ineligible = 0;
    let mut line_start = 0;

    for (line_index, line) in source.split('\n').enumerate() {
        let line_end = line_start + line.len();
        let code = strip_trailing_comment(line);
        if !code.trim().is_empty() && lexical_region_at(source, line_start) == LexicalRegion::Code {
            let mut candidates = BTreeMap::new();
            let indent = code.len() - code.trim_start().len();
            let mut offset = indent;
            for token in metrics::tokens(&code[indent..]) {
                let start = code[offset..]
                    .find(token)
                    .map_or(offset, |found| offset + found);
                offset = start + token.len();
                candidates.insert(offset, PrefixKind::TokenEnd);
                for (typed, kind) in partial_prefixes(token) {
                    candidates.entry(start + typed).or_insert(kind);
                }
            }
            for (boundary, prefix) in candidates {
                let absolute = line_start + boundary;
                if cursor_context(source, absolute).is_some() {
                    positions.push(Position {
                        offset: absolute,
                        line_index,
                        line_start,
                        line_end,
                        prefix,
                    });
                } else {
                    ineligible += 1;
                }
            }
        }
        line_start = line_end + 1;
    }

    if limit > 0 && positions.len() > limit {
        let total = positions.len();
        let keep = (0..limit)
            .map(|index| {
                if limit == 1 {
                    0
                } else {
                    index * (total - 1) / (limit - 1)
                }
            })
            .collect::<BTreeSet<_>>();
        positions = positions
            .into_iter()
            .enumerate()
            .filter(|(index, _)| keep.contains(index))
            .map(|(_, position)| position)
            .collect();
    }

    (positions, ineligible)
}

/// Byte offsets inside a word where a user is still typing it, with their
/// kinds. Numbers and punctuation have none.
fn partial_prefixes(token: &str) -> Vec<(usize, PrefixKind)> {
    let starts_word = token
        .chars()
        .next()
        .is_some_and(|first| first == '_' || first.is_alphabetic());
    if !starts_word || !token.chars().all(is_identifier_char) {
        return Vec::new();
    }
    let boundaries = token
        .char_indices()
        .map(|(index, _)| index)
        .skip(1)
        .collect::<Vec<_>>();
    let mut prefixes = Vec::new();
    for (typed, kind) in [
        (1, PrefixKind::Partial1),
        (2, PrefixKind::Partial2),
        (3, PrefixKind::Partial3),
    ] {
        if let Some(offset) = boundaries.get(typed - 1) {
            prefixes.push((*offset, kind));
        }
    }
    if boundaries.len() >= 4 {
        prefixes.push((boundaries[boundaries.len() - 1], PrefixKind::PartialNear));
    }
    prefixes
}

/// Text after the cursor must agree with the suggestion. The suggestion is
/// a continuation of the line as the author would type it, so the editor's
/// closers are already part of it whenever it closes brackets opened before
/// the cursor:
///
/// - if it closes none of them, the closers stay after it, as they are;
/// - if it closes some and ends with the closers, those are dropped from it,
///   so the line reads exactly as the suggestion does;
/// - otherwise the line would end with extra closers, so it is rejected.
fn agree_with_suffix(mut candidate: Candidate, suffix: &str) -> Option<Candidate> {
    if suffix.is_empty() {
        return Some(candidate);
    }
    let first_line_end = candidate.text.find('\n').unwrap_or(candidate.text.len());
    let first_line = &candidate.text[..first_line_end];
    if closes_outer_brackets(first_line) == 0 {
        return Some(candidate);
    }
    let trimmed = first_line.trim_end();
    let kept = trimmed.strip_suffix(suffix)?.len();
    if candidate.text[..kept].trim().is_empty() {
        return None;
    }
    candidate.text.replace_range(kept..first_line_end, "");
    Some(candidate)
}

/// How many closing brackets in `text` close a bracket opened before it.
fn closes_outer_brackets(text: &str) -> usize {
    let mut depth = 0usize;
    let mut outer = 0;
    for (index, character) in text.char_indices() {
        if !matches!(character, '(' | '[' | '{' | ')' | ']' | '}')
            || lexical_region_at(text, index) != LexicalRegion::Code
        {
            continue;
        }
        if matches!(character, '(' | '[' | '{') {
            depth += 1;
        } else if depth == 0 {
            outer += 1;
        } else {
            depth -= 1;
        }
    }
    outer
}

/// With the menu open, VS Code shows an inline suggestion only when the
/// replaced word plus the suggestion starts with the selected item's text.
fn extends_selection(
    candidate: Candidate,
    selected: Option<&str>,
    document: &str,
    offset: usize,
) -> Option<Candidate> {
    let Some(selected) = selected else {
        return Some(candidate);
    };
    let word = word_before(document, offset);
    let full = format!("{word}{}", candidate.text);
    full.starts_with(selected).then_some(candidate)
}

/// The item VS Code would select in the open suggestion menu: the first menu
/// completion whose label continues the typed word. `None` when the menu
/// would be closed because nothing matches.
fn menu_selection(
    host: &AnalysisHost,
    path: &Path,
    position: SourcePosition,
    document: &str,
    offset: usize,
) -> Option<String> {
    let word = word_before(document, offset);
    let after_member = document[..offset].ends_with(['.', ':']);
    if word.is_empty() && !after_member {
        return None;
    }
    let lower = word.to_lowercase();
    host.current_file_completion(path, position)
        .into_iter()
        .find(|completion| {
            let filter = completion
                .filter_text
                .as_deref()
                .unwrap_or(&completion.label);
            filter.to_lowercase().starts_with(&lower)
        })
        .map(|completion| match completion.insert_text {
            Some(text) if !completion.is_snippet => text,
            _ => completion.label,
        })
}

/// Every timed parse of `file`, after one untimed warm-up, with the source
/// copied outside the timed section. `None` when the file doesn't parse.
fn measure_parse(file: &EvalFile, repeats: usize) -> Option<Vec<u64>> {
    let parse = |source: String| match file.kind {
        crate::AnalysisSourceKind::Tua => crate::parse_tua_source(&file.path, source),
        crate::AnalysisSourceKind::Lua => crate::parse_lua_source(&file.path, source),
    };
    parse(file.source.clone()).ok()?;
    let mut samples = Vec::with_capacity(repeats.max(1));
    for _ in 0..repeats.max(1) {
        let source = file.source.clone();
        let started = Instant::now();
        let parsed = parse(source);
        samples.push(started.elapsed().as_nanos() as u64);
        drop(parsed);
    }
    Some(samples)
}

fn parse_report(samples: Vec<(usize, Option<Vec<u64>>)>, budget_ms: f64) -> ParseReport {
    const MAX_BYTES: usize = 256 * 1024;
    let files = samples.len();
    let bytes = samples.iter().map(|(bytes, _)| bytes).sum();
    let parsed = samples
        .iter()
        .filter_map(|(bytes, nanos)| nanos.as_ref().map(|nanos| (*bytes, nanos)))
        .collect::<Vec<_>>();
    let mut latencies = parsed
        .iter()
        .flat_map(|(_, nanos)| nanos.iter().copied())
        .collect::<Vec<_>>();
    let mut by_size = BTreeMap::<String, Vec<u64>>::new();
    for (bytes, nanos) in &parsed {
        let bucket = match *bytes {
            0..4096 => "<4KiB",
            4096..16384 => "4-16KiB",
            16384..65536 => "16-64KiB",
            _ => ">=64KiB",
        };
        by_size
            .entry(bucket.to_owned())
            .or_default()
            .extend(nanos.iter().copied());
    }
    let mut throughputs = parsed
        .iter()
        .filter(|(bytes, _)| *bytes >= 512)
        .flat_map(|(bytes, nanos)| {
            nanos
                .iter()
                .map(move |nanos| *bytes as f64 / (*nanos as f64 / 1_000_000.0).max(1e-6))
        })
        .collect::<Vec<_>>();
    throughputs.sort_by(f64::total_cmp);
    let quantile = |values: &[f64], quantile: f64| {
        if values.is_empty() {
            0.0
        } else {
            values[((values.len() - 1) as f64 * quantile).round() as usize]
        }
    };
    let p5 = quantile(&throughputs, 0.05);

    ParseReport {
        files,
        bytes,
        failed: files - parsed.len(),
        samples: latencies.len(),
        latency_ms: Percentiles::of_ms_from_ns(&mut latencies),
        latency_ms_by_size: by_size
            .into_iter()
            .map(|(bucket, mut nanos)| (bucket, Percentiles::of_ms_from_ns(&mut nanos)))
            .collect(),
        throughput_bytes_per_ms_p50: quantile(&throughputs, 0.5),
        throughput_bytes_per_ms_p5: p5,
        recommended_max_bytes: ((p5 * budget_ms / 2.0) as usize).min(MAX_BYTES),
    }
}

/// Committed acceptance criteria. Phase 1 may ship only when every evaluated
/// gate set meets the limits for the languages that set is designed to test.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Thresholds {
    /// The backend that has to pass, normally `phase1`.
    pub backend: String,
    /// The Phase 1 configuration the gate evaluates. Gate runs use it in place
    /// of command-line settings, so they test exactly what was approved.
    #[serde(default)]
    pub phase1: Option<Phase1Thresholds>,
    /// Acceptance criteria keyed by manifest set name. Each gate names only
    /// the languages that the corresponding dataset is intended to measure.
    pub sets: BTreeMap<String, GateSetThresholds>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateSetThresholds {
    pub languages: BTreeMap<String, LanguageThresholds>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Phase1Thresholds {
    #[serde(default)]
    pub recall_variant: RecallVariant,
    /// Cursor contexts allowed to show suggestions, by label; all when unset.
    #[serde(default)]
    pub contexts: Option<Vec<String>>,
    #[serde(default)]
    pub min_prefix_chars: Option<usize>,
    #[serde(default)]
    pub dominance_ratio: Option<f64>,
    #[serde(default)]
    pub min_support: Option<usize>,
    #[serde(default)]
    pub min_full_prefix_support: Option<usize>,
    pub love_callbacks: bool,
    pub statement_snippets: bool,
    pub snippet_min_prefix_chars: usize,
}

impl Phase1Thresholds {
    pub fn config(&self) -> Result<Phase1Config, String> {
        let mut recall = RecallOptions::default();
        if let Some(chars) = self.min_prefix_chars {
            recall.min_prefix_chars = chars;
        }
        if let Some(ratio) = self.dominance_ratio {
            recall.dominance_ratio = ratio;
        }
        if let Some(support) = self.min_support {
            recall.min_support = support;
        }
        if let Some(support) = self.min_full_prefix_support {
            recall.min_full_prefix_support = support;
        }
        let contexts = self
            .contexts
            .as_ref()
            .map(|labels| parse_contexts(labels.iter().map(String::as_str)))
            .transpose()?;
        Ok(Phase1Config {
            recall_variant: self.recall_variant,
            contexts,
            recall,
            love_callbacks: self.love_callbacks,
            statement_snippets: self.statement_snippets,
            snippet_min_prefix_chars: self.snippet_min_prefix_chars,
        })
    }
}

/// Cursor contexts named by their labels, such as `open-paren`.
pub fn parse_contexts<'a>(
    labels: impl IntoIterator<Item = &'a str>,
) -> Result<BTreeSet<CursorContext>, String> {
    labels
        .into_iter()
        .map(|label| {
            CursorContext::from_label(label.trim()).ok_or_else(|| {
                let known = CursorContext::ALL.map(CursorContext::label).join(", ");
                format!("unknown cursor context `{label}`; expected one of {known}")
            })
        })
        .collect()
}

/// Limits for one language. Every limit is optional; at least one must be
/// set. `min_suggestions` guards against a verdict from too few suggestions.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageThresholds {
    pub min_suggestions: Option<usize>,
    pub min_exact_precision: Option<f64>,
    pub min_correct_prefix_precision: Option<f64>,
    pub max_incorrect_chars_per_eligible: Option<f64>,
    pub max_wrong_to_saved_ratio: Option<f64>,
    pub min_chars_saved_per_eligible: Option<f64>,
    pub max_latency_p95_ms: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GateResult {
    pub set: String,
    pub language: String,
    pub check: &'static str,
    pub actual: f64,
    pub limit: f64,
    pub passed: bool,
}

/// Checks every gate set (held-out and external) against the thresholds.
/// Anything the contract names but the report lacks fails: no gate set, a
/// missing backend or language, no eligible requests, too few suggestions,
/// or a language with no limits.
pub fn check_gates(report: &EvalReport, thresholds: &Thresholds) -> Vec<GateResult> {
    let mut results = Vec::new();
    let mut fail = |set: &str, language: &str, check| {
        results.push(GateResult {
            set: set.to_owned(),
            language: language.to_owned(),
            check,
            actual: 0.0,
            limit: 0.0,
            passed: false,
        });
    };
    let gate_sets = report
        .sets
        .iter()
        .filter(|set| set.role.is_some_and(SetRole::is_gate))
        .collect::<Vec<_>>();
    if gate_sets.is_empty() {
        fail("", "", "no gate set was evaluated");
    }
    let mut checks = Vec::new();
    for set in gate_sets {
        let Some(set_thresholds) = thresholds.sets.get(&set.name) else {
            fail(&set.name, "", "the gate set has no thresholds");
            continue;
        };
        if set_thresholds.languages.is_empty() {
            fail(&set.name, "", "the gate set names no languages");
            continue;
        }
        let Some(backend) = set.backends.get(&thresholds.backend) else {
            fail(&set.name, "", "the thresholds' backend was not evaluated");
            continue;
        };
        for (language, limits) in &set_thresholds.languages {
            let Some(summary) = backend
                .by_language
                .get(language)
                .filter(|summary| summary.eligible > 0)
            else {
                fail(&set.name, language, "no eligible requests in this language");
                continue;
            };
            let latency_ms = summary.latency_us.p95 / 1_000.0;
            let limited = [
                (
                    "min_suggestions",
                    summary.suggested as f64,
                    limits.min_suggestions.map(|limit| limit as f64),
                    true,
                ),
                (
                    "min_exact_precision",
                    summary.exact_precision,
                    limits.min_exact_precision,
                    true,
                ),
                (
                    "min_correct_prefix_precision",
                    summary.correct_prefix_precision,
                    limits.min_correct_prefix_precision,
                    true,
                ),
                (
                    "max_incorrect_chars_per_eligible",
                    summary.incorrect_chars_per_eligible,
                    limits.max_incorrect_chars_per_eligible,
                    false,
                ),
                (
                    "max_wrong_to_saved_ratio",
                    summary.wrong_to_saved_ratio,
                    limits.max_wrong_to_saved_ratio,
                    false,
                ),
                (
                    "min_chars_saved_per_eligible",
                    summary.chars_saved_per_eligible,
                    limits.min_chars_saved_per_eligible,
                    true,
                ),
                (
                    "max_latency_p95_ms",
                    latency_ms,
                    limits.max_latency_p95_ms,
                    false,
                ),
            ];
            if limited.iter().all(|(_, _, limit, _)| limit.is_none()) {
                fail(&set.name, language, "this language has no limits");
                continue;
            }
            for (check, actual, limit, is_minimum) in limited {
                let Some(limit) = limit else {
                    continue;
                };
                checks.push(GateResult {
                    set: set.name.clone(),
                    language: language.clone(),
                    check,
                    actual,
                    limit,
                    passed: if is_minimum {
                        actual >= limit
                    } else {
                        actual <= limit
                    },
                });
            }
        }
    }
    results.extend(checks);
    results
}

#[cfg(test)]
mod tests;
