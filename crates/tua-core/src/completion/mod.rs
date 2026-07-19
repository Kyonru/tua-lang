use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use self::context::{completion_context, CompletionContext, FieldAccessSeparator};
use self::require::{complete_import_paths, complete_require_paths};
use self::sequence::{
    score_completion_sequence, CompletionSequenceCandidate, CompletionSequenceContextSnapshot,
};
use self::symbol::lua_symbols_before_position;
use crate::love_callback;
use crate::lua::{self, LuaItem, LuaItemKind};
use crate::module_index::{exported_field_for_fields, required_module_exports_for_receiver};
use crate::profile;
use crate::scope::{scope_map_parsed_source, ScopeDeclaration, ScopeDeclarationKind};
use crate::semantic::{resolve_aliases, SemanticModel};
use crate::source_text::line_at;
use crate::text_match::{text_match_rank, text_matches, TextMatchRank};
use crate::types::{build_type_environment_with_diagnostics, TableShape, Type, TypeEnvironment};
use crate::usage_index::{MemberUsageCounts, MemberUsageSummary, ProjectUsageIndex};
use crate::{
    asset, ecs, input, love_conf, object_pool, signal, sprite, state_machine, store, vector,
};
use crate::{
    parse_tua_source, Diagnostic, FunctionAnnotation, ParsedSource, SourcePosition, TextEdit,
    TuaType,
};

pub mod adaptive;
pub(crate) mod context;
pub mod multitoken;
mod require;
pub mod sequence;
mod symbol;

pub use adaptive::{CompletionAdaptiveKey, CompletionAdaptiveMemberStats, CompletionAdaptiveStats};
pub use multitoken::{CompletionMultiTokenConfig, CompletionMultiTokenContextKind};
pub use sequence::{
    CompletionSequenceRanker, CompletionSequenceRankingBudget, CompletionSequenceRankingConfig,
    CompletionSequenceRankingError, CompletionSequenceRankingInput,
    CompletionSequenceRankingProvider,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Completion {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: Option<String>,
    pub filter_text: Option<String>,
    pub is_snippet: bool,
    pub module_path: Option<PathBuf>,
    pub source_module: Option<String>,
    pub additional_text_edits: Vec<TextEdit>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionKind {
    Snippet,
    Function,
    TypeAlias,
    Field,
    Keyword,
    Module,
    Variable,
    Value,
}

impl CompletionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Snippet => "snippet",
            Self::Function => "function",
            Self::TypeAlias => "typeAlias",
            Self::Field => "field",
            Self::Keyword => "keyword",
            Self::Module => "module",
            Self::Variable => "variable",
            Self::Value => "value",
        }
    }

    pub fn parse_kind(value: &str) -> Option<Self> {
        match value {
            "snippet" => Some(Self::Snippet),
            "function" => Some(Self::Function),
            "typeAlias" => Some(Self::TypeAlias),
            "field" => Some(Self::Field),
            "keyword" => Some(Self::Keyword),
            "module" => Some(Self::Module),
            "variable" => Some(Self::Variable),
            "value" => Some(Self::Value),
            _ => None,
        }
    }
}

pub(crate) fn filter_and_rank_completions(
    completions: Vec<Completion>,
    prefix: &str,
) -> Vec<Completion> {
    filter_and_rank_completions_with_context(completions, prefix, None)
}

fn filter_and_rank_completions_with_context(
    completions: Vec<Completion>,
    prefix: &str,
    ranking_context: Option<&CompletionRankingContext>,
) -> Vec<Completion> {
    let _profile = profile::span("analysis.completion.ranking");

    if prefix.is_empty() && ranking_context.is_none() {
        return completions;
    }

    let matched = completions
        .into_iter()
        .enumerate()
        .filter_map(|completion| {
            let (index, completion) = completion;

            completion_match_rank(&completion, prefix)
                .map(|rank| (rank, completion_priority(&completion), index, completion))
        })
        .collect::<Vec<_>>();
    let sequence_scores = ranking_context
        .map(|context| {
            context.sequence_scores(
                &matched
                    .iter()
                    .map(|(_, _, index, completion)| (*index, completion))
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default();
    let mut ranked = matched
        .into_iter()
        .map(|(rank, priority, index, completion)| {
            let context_score = ranking_context
                .map(|context| context.score(&completion))
                .unwrap_or_default();
            let sequence_score = sequence_scores.get(&index).copied().unwrap_or_default();

            (
                rank,
                context_score.saturating_add(sequence_score),
                priority,
                index,
                completion,
            )
        })
        .collect::<Vec<_>>();

    if prefix.is_empty() && ranked.iter().all(|(_, score, _, _, _)| *score == 0) {
        return ranked
            .into_iter()
            .map(|(_, _, _, _, completion)| completion)
            .collect();
    }

    ranked.sort_by(
        |(left_rank, left_score, left_priority, left_index, left),
         (right_rank, right_score, right_priority, right_index, right)| {
            let base_order = left_rank
                .category
                .cmp(&right_rank.category)
                .then_with(|| left_rank.index.cmp(&right_rank.index));

            if base_order != std::cmp::Ordering::Equal {
                return base_order;
            }

            let score_order = right_score.cmp(left_score);

            if score_order != std::cmp::Ordering::Equal {
                return score_order;
            }

            let priority_order = left_priority.cmp(right_priority);

            if priority_order != std::cmp::Ordering::Equal {
                return priority_order;
            }

            if (*left_score > 0 || *right_score > 0)
                || (completion_uses_source_order(left) && completion_uses_source_order(right))
            {
                left_index
                    .cmp(right_index)
                    .then_with(|| left_rank.len.cmp(&right_rank.len))
                    .then_with(|| left.label.cmp(&right.label))
            } else {
                left_rank
                    .len
                    .cmp(&right_rank.len)
                    .then_with(|| left_index.cmp(right_index))
                    .then_with(|| left.label.cmp(&right.label))
            }
        },
    );

    ranked
        .into_iter()
        .map(|(_, _, _, _, mut completion)| {
            if completion.filter_text.is_none()
                && !completion
                    .label
                    .to_ascii_lowercase()
                    .starts_with(&prefix.to_ascii_lowercase())
            {
                completion.filter_text = Some(prefix.to_owned());
            }
            completion
        })
        .collect()
}

fn completion_priority(completion: &Completion) -> u8 {
    match completion.detail.as_deref() {
        Some("parameter") => 0,
        Some("local") | Some("local function") | Some("module") => 1,
        Some("function") => 2,
        _ => match completion.kind {
            CompletionKind::Snippet => 3,
            CompletionKind::Keyword => 4,
            CompletionKind::Function => 5,
            CompletionKind::Variable => 6,
            CompletionKind::TypeAlias => 7,
            CompletionKind::Module => 8,
            CompletionKind::Field => 9,
            CompletionKind::Value => 10,
        },
    }
}

fn completion_uses_source_order(completion: &Completion) -> bool {
    matches!(
        completion.detail.as_deref(),
        Some("parameter" | "local" | "local function" | "module" | "function" | "SignalBus event")
    )
}

#[derive(Clone, Debug)]
struct CompletionRankingContext<'a> {
    member_usage: Option<MemberUsageRanking<'a>>,
    adaptive_stats: Option<&'a CompletionAdaptiveStats>,
    file_scope_hash: u64,
    receiver_parts: Vec<String>,
    sequence_ranking: CompletionSequenceRankingConfig,
    sequence_context: CompletionSequenceContextSnapshot,
}

#[derive(Clone, Copy)]
enum CompletionProjectData<'a> {
    Sources(&'a [&'a str]),
    UsageIndex(&'a ProjectUsageIndex),
}

#[derive(Clone, Copy)]
pub(crate) struct CompletionRankingInputs<'a> {
    project_data: CompletionProjectData<'a>,
    adaptive_stats: Option<&'a CompletionAdaptiveStats>,
    file_scope_hash: u64,
    sequence_ranking: &'a CompletionSequenceRankingConfig,
}

impl<'a> CompletionRankingInputs<'a> {
    pub(crate) fn from_sources(
        project_sources: &'a [&'a str],
        adaptive_stats: Option<&'a CompletionAdaptiveStats>,
        file_scope_hash: u64,
        sequence_ranking: &'a CompletionSequenceRankingConfig,
    ) -> Self {
        Self {
            project_data: CompletionProjectData::Sources(project_sources),
            adaptive_stats,
            file_scope_hash,
            sequence_ranking,
        }
    }

    pub(crate) fn from_usage_index(
        usage_index: &'a ProjectUsageIndex,
        adaptive_stats: Option<&'a CompletionAdaptiveStats>,
        file_scope_hash: u64,
        sequence_ranking: &'a CompletionSequenceRankingConfig,
    ) -> Self {
        Self {
            project_data: CompletionProjectData::UsageIndex(usage_index),
            adaptive_stats,
            file_scope_hash,
            sequence_ranking,
        }
    }
}

#[derive(Clone, Debug)]
enum MemberUsageRanking<'a> {
    Summary(MemberUsageSummary),
    Indexed {
        index: &'a ProjectUsageIndex,
        current_path: PathBuf,
        receiver: Option<String>,
        position: SourcePosition,
    },
}

impl<'a> CompletionRankingContext<'a> {
    fn from_context(
        current_path: &Path,
        source: &str,
        context: &CompletionContext,
        ranking: CompletionRankingInputs<'a>,
    ) -> Option<Self> {
        let (receiver_parts, position, prefix, member_usage) = match context {
            CompletionContext::FieldAccess {
                receiver_parts,
                prefix,
                position,
                ..
            } => (
                receiver_parts.clone(),
                *position,
                prefix.as_str(),
                Some(MemberUsageRanking::collect(
                    current_path,
                    source,
                    ranking.project_data,
                    receiver_parts,
                    *position,
                )),
            ),
            CompletionContext::Identifier { prefix, position } => {
                (Vec::new(), *position, prefix.as_str(), None)
            }
            _ => return None,
        };

        Some(Self {
            member_usage,
            adaptive_stats: ranking.adaptive_stats,
            file_scope_hash: ranking.file_scope_hash,
            receiver_parts: receiver_parts.clone(),
            sequence_ranking: ranking.sequence_ranking.clone(),
            sequence_context: CompletionSequenceContextSnapshot::from_source(
                source,
                position,
                prefix,
                &receiver_parts,
            ),
        })
    }

    fn score(&self, completion: &Completion) -> usize {
        let usage_score = self
            .member_usage
            .as_ref()
            .map(|usage| usage.score(completion))
            .unwrap_or_default();
        let adaptive_score = self
            .adaptive_stats
            .map(|stats| {
                stats.score(
                    self.file_scope_hash,
                    &self.receiver_parts,
                    completion.label.as_str(),
                )
            })
            .unwrap_or_default();

        usage_score + adaptive_score
    }

    fn sequence_scores(&self, completions: &[(usize, &Completion)]) -> BTreeMap<usize, usize> {
        let scores = score_completion_sequence(
            &self.sequence_ranking,
            CompletionSequenceRankingInput {
                context: self.sequence_context.clone(),
                candidates: completions
                    .iter()
                    .map(|(index, completion)| {
                        CompletionSequenceCandidate::from_completion(*index, completion)
                    })
                    .collect(),
            },
        );

        scores
            .into_iter()
            .filter_map(|score| {
                (score.score > 0).then_some((
                    score.candidate_index,
                    usize::try_from(score.score).unwrap_or(usize::MAX),
                ))
            })
            .collect()
    }
}

impl<'a> MemberUsageRanking<'a> {
    fn collect(
        current_path: &Path,
        source: &str,
        project_data: CompletionProjectData<'a>,
        receiver_parts: &[String],
        position: SourcePosition,
    ) -> Self {
        let receiver = receiver_parts.last().cloned();

        match project_data {
            CompletionProjectData::Sources(project_sources) => {
                let transient_index =
                    ProjectUsageIndex::from_sources(current_path, source, project_sources);
                Self::Summary(transient_index.member_usage(
                    current_path,
                    receiver.as_deref(),
                    position,
                ))
            }
            CompletionProjectData::UsageIndex(index) => Self::Indexed {
                index,
                current_path: current_path.to_path_buf(),
                receiver,
                position,
            },
        }
    }

    fn score(&self, completion: &Completion) -> usize {
        let label = completion.label.as_str();
        let counts = match self {
            Self::Summary(summary) => MemberUsageCounts {
                member: summary
                    .member_counts
                    .get(label)
                    .copied()
                    .unwrap_or_default(),
                receiver_member: summary
                    .receiver_member_counts
                    .get(label)
                    .copied()
                    .unwrap_or_default(),
                call: summary.call_counts.get(label).copied().unwrap_or_default(),
                proximity: summary
                    .proximity_scores
                    .get(label)
                    .copied()
                    .unwrap_or_default(),
            },
            Self::Indexed {
                index,
                current_path,
                receiver,
                position,
            } => index.member_usage_counts(current_path, receiver.as_deref(), *position, label),
        };
        let contextual_score = counts.receiver_member * 1_000
            + counts.call * 200
            + counts.member * 50
            + counts.proximity;

        if contextual_score == 0 {
            return 0;
        }

        contextual_score + completion_quality_score(completion)
    }
}

fn completion_quality_score(completion: &Completion) -> usize {
    let documentation = usize::from(completion.documentation.is_some()) * 4;
    let precise_detail = completion
        .detail
        .as_deref()
        .filter(|detail| {
            let lower = detail.to_ascii_lowercase();

            !lower.contains("any") && !lower.contains("unknown")
        })
        .map(|_| 2)
        .unwrap_or_default();
    let callable = usize::from(completion.kind == CompletionKind::Function);

    documentation + precise_detail + callable
}

fn completion_match_rank(completion: &Completion, prefix: &str) -> Option<TextMatchRank> {
    let label_rank = completion_text_rank(&completion.label, prefix);
    let filter_rank = completion
        .filter_text
        .as_deref()
        .and_then(|filter_text| completion_text_rank(filter_text, prefix));

    label_rank.into_iter().chain(filter_rank).min()
}

fn completion_text_matches(value: &str, prefix: &str) -> bool {
    text_matches(value, prefix)
}

fn completion_text_rank(value: &str, prefix: &str) -> Option<TextMatchRank> {
    text_match_rank(value, prefix)
}

pub fn complete_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    position: SourcePosition,
) -> Result<Vec<Completion>, Vec<Diagnostic>> {
    let path = path.as_ref().to_path_buf();
    let source = source.into();
    let Some(context) = completion_context(&source, position) else {
        return Ok(Vec::new());
    };
    let parsed = match parse_completion_source(&path, &source, position) {
        Ok(parsed) => parsed,
        Err(_) => return Ok(complete_lossy_source(&path, &source, context)),
    };
    let (environment, _) = build_type_environment_with_diagnostics(&parsed);
    let environment = ecs::augment_environment(environment);

    Ok(complete_parsed_source_with_context(
        &parsed,
        &source,
        context,
        &environment,
    ))
}

pub fn complete_lossy_identifier_source(
    source: &str,
    position: SourcePosition,
) -> Option<(String, Vec<Completion>)> {
    let CompletionContext::Identifier { prefix, position } = completion_context(source, position)?
    else {
        return None;
    };
    let completions = rank_completions_for_prefix(
        complete_lossy_identifier_items(source, prefix.as_str(), position),
        Some(prefix.as_str()),
    );

    Some((prefix, completions))
}

pub fn complete_parsed_source(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Result<Vec<Completion>, Vec<Diagnostic>> {
    let (environment, _) = build_type_environment_with_diagnostics(parsed);
    let environment = ecs::augment_environment(environment);

    Ok(complete_parsed_source_with_environment(
        parsed,
        position,
        &environment,
    ))
}

pub(crate) fn complete_parsed_source_with_environment(
    parsed: &ParsedSource,
    position: SourcePosition,
    environment: &TypeEnvironment,
) -> Vec<Completion> {
    complete_source_with_parsed_environment(&parsed.source, parsed, position, environment)
}

pub(crate) fn complete_source_with_parsed_environment(
    source: &str,
    parsed: &ParsedSource,
    position: SourcePosition,
    environment: &TypeEnvironment,
) -> Vec<Completion> {
    complete_source_with_parsed_environment_and_project_sources(
        source,
        parsed,
        position,
        environment,
        &[],
    )
}

pub(crate) fn complete_source_with_parsed_environment_and_project_sources(
    source: &str,
    parsed: &ParsedSource,
    position: SourcePosition,
    environment: &TypeEnvironment,
    project_sources: &[&str],
) -> Vec<Completion> {
    complete_source_with_parsed_environment_and_ranking(
        source,
        parsed,
        position,
        environment,
        project_sources,
        None,
        CompletionAdaptiveStats::file_scope_hash(&parsed.path),
    )
}

pub(crate) fn complete_source_with_parsed_environment_and_ranking(
    source: &str,
    parsed: &ParsedSource,
    position: SourcePosition,
    environment: &TypeEnvironment,
    project_sources: &[&str],
    adaptive_stats: Option<&CompletionAdaptiveStats>,
    file_scope_hash: u64,
) -> Vec<Completion> {
    complete_source_with_parsed_environment_and_ranking_inputs(
        source,
        parsed,
        position,
        environment,
        CompletionRankingInputs::from_sources(
            project_sources,
            adaptive_stats,
            file_scope_hash,
            &CompletionSequenceRankingConfig::default(),
        ),
    )
}

pub(crate) fn complete_source_with_parsed_environment_and_ranking_inputs(
    source: &str,
    parsed: &ParsedSource,
    position: SourcePosition,
    environment: &TypeEnvironment,
    ranking: CompletionRankingInputs<'_>,
) -> Vec<Completion> {
    let Some(context) = completion_context(source, position) else {
        return Vec::new();
    };

    complete_parsed_source_with_context_and_ranking_options(
        parsed,
        source,
        context,
        environment,
        ranking,
    )
}

pub(crate) fn complete_field_access_in_source_with_environment_and_ranking_inputs(
    source: &str,
    parsed: &ParsedSource,
    position: SourcePosition,
    environment: &TypeEnvironment,
    ranking: CompletionRankingInputs<'_>,
) -> Vec<Completion> {
    let Some(CompletionContext::FieldAccess {
        receiver_parts,
        prefix,
        position,
        separator,
        separator_span,
    }) = completion_context(source, position)
    else {
        return Vec::new();
    };

    let context = CompletionContext::FieldAccess {
        receiver_parts: receiver_parts.clone(),
        prefix: prefix.clone(),
        position,
        separator,
        separator_span,
    };
    let field_context = FieldCompletionContext {
        receiver_parts: &receiver_parts,
        prefix: prefix.as_str(),
        position,
        separator,
        separator_span,
    };
    let completions = complete_fields(parsed, source, environment, field_context);
    let ranking_context =
        CompletionRankingContext::from_context(&parsed.path, source, &context, ranking);

    rank_completions_for_context(completions, Some(prefix.as_str()), ranking_context.as_ref())
}

pub(crate) fn complete_current_file_field_access_with_environment_and_ranking_inputs(
    source: &str,
    parsed: &ParsedSource,
    position: SourcePosition,
    environment: &TypeEnvironment,
    ranking: CompletionRankingInputs<'_>,
) -> Option<Vec<Completion>> {
    let Some(CompletionContext::FieldAccess {
        receiver_parts,
        prefix,
        position,
        separator,
        separator_span,
    }) = completion_context(source, position)
    else {
        return None;
    };
    let root = receiver_parts.first()?;

    // Required modules need the revisioned project graph so unsaved exports in another
    // document remain authoritative over the on-disk module fallback.
    if crate::require_bindings_for_parsed_source(parsed)
        .iter()
        .any(|binding| binding.alias == *root)
    {
        return None;
    }

    let context = CompletionContext::FieldAccess {
        receiver_parts: receiver_parts.clone(),
        prefix: prefix.clone(),
        position,
        separator,
        separator_span,
    };
    let ranking_context =
        CompletionRankingContext::from_context(&parsed.path, source, &context, ranking);
    let mut completions = complete_fields(
        parsed,
        source,
        environment,
        FieldCompletionContext {
            receiver_parts: &receiver_parts,
            prefix: "",
            position,
            separator,
            separator_span,
        },
    );

    if completions.is_empty() && environment.local(root).is_none() {
        completions = complete_lua_fields(source, &receiver_parts, "", position);
    }

    if completions.is_empty() && environment.local(root).is_none() {
        return None;
    }

    Some(rank_completions_for_context(
        completions,
        Some(&prefix),
        ranking_context.as_ref(),
    ))
}

pub(crate) fn complete_current_file_lua_field_access(
    source: &str,
    position: SourcePosition,
) -> Option<Vec<Completion>> {
    let Some(CompletionContext::FieldAccess {
        receiver_parts,
        prefix,
        position,
        ..
    }) = completion_context(source, position)
    else {
        return None;
    };
    let completions = complete_lua_fields(source, &receiver_parts, "", position);

    if completions.is_empty() {
        return None;
    }

    Some(rank_completions_for_prefix(completions, Some(&prefix)))
}

fn complete_parsed_source_with_context(
    parsed: &ParsedSource,
    source: &str,
    context: CompletionContext,
    environment: &TypeEnvironment,
) -> Vec<Completion> {
    complete_parsed_source_with_context_and_project_sources(
        parsed,
        source,
        context,
        environment,
        &[],
    )
}

fn complete_parsed_source_with_context_and_project_sources(
    parsed: &ParsedSource,
    source: &str,
    context: CompletionContext,
    environment: &TypeEnvironment,
    project_sources: &[&str],
) -> Vec<Completion> {
    complete_parsed_source_with_context_and_ranking(
        parsed,
        source,
        context,
        environment,
        project_sources,
        None,
        CompletionAdaptiveStats::file_scope_hash(&parsed.path),
    )
}

fn complete_parsed_source_with_context_and_ranking(
    parsed: &ParsedSource,
    source: &str,
    context: CompletionContext,
    environment: &TypeEnvironment,
    project_sources: &[&str],
    adaptive_stats: Option<&CompletionAdaptiveStats>,
    file_scope_hash: u64,
) -> Vec<Completion> {
    complete_parsed_source_with_context_and_ranking_options(
        parsed,
        source,
        context,
        environment,
        CompletionRankingInputs::from_sources(
            project_sources,
            adaptive_stats,
            file_scope_hash,
            &CompletionSequenceRankingConfig::default(),
        ),
    )
}

fn complete_parsed_source_with_context_and_ranking_options(
    parsed: &ParsedSource,
    source: &str,
    context: CompletionContext,
    environment: &TypeEnvironment,
    ranking: CompletionRankingInputs<'_>,
) -> Vec<Completion> {
    let prefix = context.filter_prefix().map(str::to_owned);
    let ranking_context =
        CompletionRankingContext::from_context(&parsed.path, source, &context, ranking);
    let completions = match context {
        CompletionContext::AnnotationTag { prefix } => complete_annotation_tags(prefix.as_str()),
        CompletionContext::DocumentationSnippet { position } => {
            complete_documentation_snippet(parsed, position)
        }
        CompletionContext::TypeAnnotation { prefix } => {
            complete_type_names(environment, prefix.as_str())
        }
        CompletionContext::FieldAccess {
            receiver_parts,
            prefix,
            position,
            separator,
            separator_span,
        } => {
            let module_fields =
                complete_required_module_fields(parsed, &receiver_parts, prefix.as_str());

            if !module_fields.is_empty() {
                module_fields
            } else {
                let field_context = FieldCompletionContext {
                    receiver_parts: &receiver_parts,
                    prefix: prefix.as_str(),
                    position,
                    separator,
                    separator_span,
                };
                let fields = complete_fields(parsed, source, environment, field_context);

                if !fields.is_empty() || environment.local(&receiver_parts[0]).is_some() {
                    fields
                } else {
                    complete_lua_fields(source, &receiver_parts, prefix.as_str(), position)
                }
            }
        }
        CompletionContext::StringArgument {
            callee_parts,
            active_parameter,
            prefix,
            position,
        } => {
            let string_context = StringArgumentCompletionContext {
                callee_parts: &callee_parts,
                active_parameter,
                prefix: prefix.as_str(),
                position,
                project_data: ranking.project_data,
            };

            complete_string_argument_values(parsed, source, environment, string_context)
        }
        CompletionContext::RequirePath { prefix } => {
            complete_require_paths(&parsed.path, prefix.as_str())
        }
        CompletionContext::ImportPath { prefix } => {
            complete_import_paths(&parsed.path, prefix.as_str())
        }
        CompletionContext::Identifier { prefix, position } => {
            complete_domain_config_items(parsed, source, prefix.as_str(), position).unwrap_or_else(
                || complete_identifier_items(parsed, environment, prefix.as_str(), position),
            )
        }
    };

    rank_completions_for_context(completions, prefix.as_deref(), ranking_context.as_ref())
}

fn complete_lossy_source(path: &Path, source: &str, context: CompletionContext) -> Vec<Completion> {
    let prefix = context.filter_prefix().map(str::to_owned);
    let completions = match context {
        CompletionContext::AnnotationTag { prefix } => complete_annotation_tags(prefix.as_str()),
        CompletionContext::DocumentationSnippet { position } => {
            complete_lossy_documentation_snippet(source, position)
        }
        CompletionContext::TypeAnnotation { prefix } => {
            complete_builtin_types(prefix.as_str(), true)
        }
        CompletionContext::FieldAccess {
            receiver_parts,
            prefix,
            position,
            ..
        } => complete_lua_fields(source, &receiver_parts, prefix.as_str(), position),
        CompletionContext::StringArgument { .. } => Vec::new(),
        CompletionContext::RequirePath { prefix } => complete_require_paths(path, prefix.as_str()),
        CompletionContext::ImportPath { prefix } => complete_import_paths(path, prefix.as_str()),
        CompletionContext::Identifier { prefix, position } => {
            complete_lossy_identifier_items(source, prefix.as_str(), position)
        }
    };

    rank_completions_for_prefix(completions, prefix.as_deref())
}

fn rank_completions_for_prefix(
    completions: Vec<Completion>,
    prefix: Option<&str>,
) -> Vec<Completion> {
    rank_completions_for_context(completions, prefix, None)
}

fn rank_completions_for_context(
    completions: Vec<Completion>,
    prefix: Option<&str>,
    ranking_context: Option<&CompletionRankingContext>,
) -> Vec<Completion> {
    match prefix {
        Some(prefix) => {
            filter_and_rank_completions_with_context(completions, prefix, ranking_context)
        }
        None => completions,
    }
}

fn syntax_snippet_completions(prefix: &str, include_ecs: bool) -> Vec<Completion> {
    [
        (
            "local",
            None,
            "Lua local declaration",
            "local ${1:name} = ${0:nil}",
        ),
        (
            "local function",
            Some("lfn"),
            "Lua local function declaration",
            "local function ${1:name}(${2})\n  ${0}\nend",
        ),
        (
            "function",
            Some("fn"),
            "Lua function declaration",
            "function ${1:name}(${2})\n  ${0}\nend",
        ),
        (
            "type",
            None,
            "Tua type alias declaration",
            "type ${1:Name} = {\n  ${0}\n}",
        ),
        (
            "class",
            None,
            "Tua class declaration",
            "class ${1:Name} {\n  ${0}\n}",
        ),
        (
            "open class",
            Some("openclass"),
            "Tua inheritable class declaration",
            "open class ${1:Name} {\n  ${0}\n}",
        ),
        (
            "final class",
            Some("finalclass"),
            "Tua final class declaration",
            "final class ${1:Name} {\n  ${0}\n}",
        ),
        (
            "if",
            None,
            "Lua if statement",
            "if ${1:condition} then\n  ${0}\nend",
        ),
        (
            "if else",
            Some("ifelse"),
            "Lua if/else statement",
            "if ${1:condition} then\n  ${2}\nelse\n  ${0}\nend",
        ),
        (
            "for",
            Some("fori"),
            "Lua numeric for loop",
            "for ${1:i} = ${2:1}, ${3:n} do\n  ${0}\nend",
        ),
        (
            "for pairs",
            Some("forp"),
            "Lua pairs loop",
            "for ${1:key}, ${2:value} in pairs(${3:table}) do\n  ${0}\nend",
        ),
        (
            "for ipairs",
            Some("forip"),
            "Lua ipairs loop",
            "for ${1:index}, ${2:value} in ipairs(${3:table}) do\n  ${0}\nend",
        ),
        (
            "while",
            None,
            "Lua while loop",
            "while ${1:condition} do\n  ${0}\nend",
        ),
        (
            "repeat",
            None,
            "Lua repeat/until loop",
            "repeat\n  ${1}\nuntil ${0:condition}",
        ),
        ("do", None, "Lua do/end block", "do\n  ${0}\nend"),
        (
            crate::sprite::HELPER_NAME,
            None,
            "Tua sprite sheet helper",
            crate::sprite::sprite_sheet_snippet(),
        ),
        (
            signal::CONSTRUCTOR_NAME,
            None,
            "Tua signal bus helper",
            signal::signal_snippet(),
        ),
        (
            object_pool::CONSTRUCTOR_NAME,
            None,
            "Tua object pool helper",
            object_pool::workflow_snippet(),
        ),
        (
            state_machine::CONSTRUCTOR_NAME,
            None,
            "Tua finite state machine helper",
            state_machine::workflow_snippet(),
        ),
        (
            store::CONSTRUCTOR_NAME,
            None,
            "Tua typed state store helper",
            store::workflow_snippet(),
        ),
        (
            input::CONSTRUCTOR_NAME,
            None,
            "Tua Baton input helper",
            input::workflow_snippet(),
        ),
        (
            "ecs component",
            None,
            "Tua ECS component and world workflow",
            ecs::workflow_snippet(),
        ),
    ]
    .into_iter()
    .filter(|(label, _, _, _)| include_ecs || *label != "ecs component")
    .filter(|(label, filter_text, _, _)| {
        completion_text_matches(label, prefix)
            || filter_text.is_some_and(|filter_text| completion_text_matches(filter_text, prefix))
    })
    .map(|(label, filter_text, detail, insert_text)| Completion {
        label: label.to_owned(),
        kind: CompletionKind::Snippet,
        detail: Some(detail.to_owned()),
        documentation: None,
        insert_text: Some(insert_text.to_owned()),
        filter_text: filter_text.map(str::to_owned),
        is_snippet: true,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    })
    .collect()
}

fn complete_annotation_tags(prefix: &str) -> Vec<Completion> {
    [
        ("alias", "type alias annotation"),
        ("class", "class annotation"),
        ("field", "field annotation"),
        ("generic", "generic annotation"),
        ("overload", "function overload annotation"),
        ("param", "function parameter annotation"),
        ("return", "function return annotation"),
        ("type", "type annotation"),
        ("vararg", "function vararg annotation"),
        ("version", "version annotation"),
    ]
    .into_iter()
    .filter(|(tag, _)| completion_text_matches(tag, prefix))
    .map(|(tag, detail)| Completion {
        label: tag.to_owned(),
        kind: CompletionKind::Keyword,
        detail: Some(detail.to_owned()),
        documentation: None,
        insert_text: None,
        filter_text: None,
        is_snippet: false,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    })
    .collect()
}

fn complete_type_names(environment: &TypeEnvironment, prefix: &str) -> Vec<Completion> {
    let mut completions = BTreeMap::new();

    for completion in complete_type_aliases(environment, prefix) {
        completions.insert(completion.label.clone(), completion);
    }

    for completion in complete_builtin_types(prefix, false) {
        completions.insert(completion.label.clone(), completion);
    }

    completions.into_values().collect()
}

fn complete_builtin_types(prefix: &str, include_ecs: bool) -> Vec<Completion> {
    ["any", "boolean", "nil", "number", "string"]
        .into_iter()
        .chain(asset::builtin_type_names().iter().copied())
        .chain(lua::love_type_names().iter().copied())
        .chain(vector::builtin_type_names().iter().copied())
        .chain(signal::builtin_type_names().iter().copied())
        .chain(object_pool::builtin_type_names().iter().copied())
        .chain(state_machine::builtin_type_names().iter().copied())
        .chain(store::builtin_type_names().iter().copied())
        .chain(input::builtin_type_names().iter().copied())
        .chain(
            ecs::builtin_type_names()
                .iter()
                .copied()
                .filter(|_| include_ecs),
        )
        .filter(|primitive| completion_text_matches(primitive, prefix))
        .map(|primitive| Completion {
            label: primitive.to_owned(),
            kind: CompletionKind::TypeAlias,
            detail: Some("built-in type".to_owned()),
            documentation: None,
            insert_text: None,
            filter_text: None,
            is_snippet: false,
            module_path: None,
            source_module: None,
            additional_text_edits: Vec::new(),
        })
        .collect()
}

fn complete_type_aliases(environment: &TypeEnvironment, prefix: &str) -> Vec<Completion> {
    environment
        .aliases()
        .filter(|(name, _)| completion_text_matches(name, prefix))
        .map(|(name, _)| Completion {
            label: name.to_owned(),
            kind: CompletionKind::TypeAlias,
            detail: Some("type alias".to_owned()),
            documentation: environment.alias_documentation(name).map(str::to_owned),
            insert_text: None,
            filter_text: None,
            is_snippet: false,
            module_path: None,
            source_module: None,
            additional_text_edits: Vec::new(),
        })
        .collect()
}

fn complete_documentation_snippet(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Vec<Completion> {
    let Some(function) = function_after_documentation_line(parsed, position) else {
        return Vec::new();
    };
    let Some(snippet) = documentation_snippet_for_function(parsed, function) else {
        return Vec::new();
    };

    vec![Completion {
        label: "document function".to_owned(),
        kind: CompletionKind::Snippet,
        detail: Some("LuaDoc function documentation".to_owned()),
        documentation: Some(
            "Generate LuaHelper-style parameter and return annotations.".to_owned(),
        ),
        insert_text: Some(snippet),
        filter_text: None,
        is_snippet: true,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    }]
}

fn complete_lossy_documentation_snippet(source: &str, position: SourcePosition) -> Vec<Completion> {
    let Some(snippet) = lossy_documentation_snippet_after_line(source, position.line) else {
        return Vec::new();
    };

    vec![Completion {
        label: "document function".to_owned(),
        kind: CompletionKind::Snippet,
        detail: Some("LuaDoc function documentation".to_owned()),
        documentation: Some(
            "Generate LuaHelper-style parameter and return annotations.".to_owned(),
        ),
        insert_text: Some(snippet),
        filter_text: None,
        is_snippet: true,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    }]
}

fn function_after_documentation_line(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Option<&FunctionAnnotation> {
    parsed
        .function_annotations
        .iter()
        .filter_map(|function| Some((function, function.span?.start.line)))
        .filter(|(_, line)| *line > position.line)
        .min_by_key(|(_, line)| *line)
        .and_then(|(function, line)| {
            doc_gap_is_clear(&parsed.source, position.line + 1, line).then_some(function)
        })
}

fn doc_gap_is_clear(source: &str, start_line: usize, function_line: usize) -> bool {
    (start_line..function_line).all(|line_number| {
        line_at(source, line_number)
            .map(|line| {
                let trimmed = line.trim();

                trimmed.is_empty() || trimmed.starts_with("---")
            })
            .unwrap_or(true)
    })
}

fn documentation_snippet_for_function(
    parsed: &ParsedSource,
    function: &FunctionAnnotation,
) -> Option<String> {
    let line = function
        .span
        .and_then(|span| line_at(&parsed.source, span.start.line))?;
    let mut signature = parse_function_signature_line(line)?;

    for parameter in &mut signature.parameters {
        if parameter.ty == "any" {
            if let Some(annotated) = function
                .parameters
                .iter()
                .find(|annotated| annotated.name == parameter.name)
            {
                parameter.ty = format_tua_type(&annotated.ty);
            }
        }
    }

    if signature.return_type.is_none() {
        signature.return_type = function.return_type.as_ref().map(format_tua_type);
    }

    documentation_snippet_from_signature(signature)
}

fn lossy_documentation_snippet_after_line(source: &str, line_number: usize) -> Option<String> {
    let function_line = ((line_number + 1)..)
        .map_while(|line| line_at(source, line).map(|content| (line, content)))
        .find(|(_, line)| !line.trim().is_empty() && !line.trim_start().starts_with("---"))?
        .1;

    documentation_snippet_from_signature(parse_function_signature_line(function_line)?)
}

fn documentation_snippet_from_signature(signature: FunctionSignature) -> Option<String> {
    if signature.parameters.is_empty() && signature.return_type.is_none() {
        return None;
    }

    let mut lines = Vec::new();
    let mut placeholder = 1;

    for parameter in signature.parameters {
        lines.push(format!(
            "@param {} {} ${{{}:description}}",
            parameter.name, parameter.ty, placeholder
        ));
        placeholder += 1;
    }

    if let Some(return_type) = signature.return_type {
        lines.push(format!(
            "@return {} ${{{}:description}}",
            return_type, placeholder
        ));
    }

    let (first, rest) = lines.split_first()?;
    let mut snippet = first.clone();

    for line in rest {
        snippet.push('\n');
        snippet.push_str("---");
        snippet.push_str(line);
    }

    Some(snippet)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FunctionSignature {
    parameters: Vec<DocParameter>,
    return_type: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DocParameter {
    name: String,
    ty: String,
}

fn parse_function_signature_line(line: &str) -> Option<FunctionSignature> {
    let open = line.find('(')?;
    let close = matching_close_paren(line, open)?;
    let parameters = line[open + 1..close]
        .split(',')
        .filter_map(parse_doc_parameter)
        .collect::<Vec<_>>();
    let return_type = line[close + 1..]
        .trim_start()
        .strip_prefix(':')
        .map(|ty| ty.trim())
        .filter(|ty| !ty.is_empty())
        .map(type_prefix);

    Some(FunctionSignature {
        parameters,
        return_type,
    })
}

fn matching_close_paren(line: &str, open: usize) -> Option<usize> {
    let mut depth = 0;

    for (index, character) in line[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;

                if depth == 0 {
                    return Some(open + index);
                }
            }
            _ => {}
        }
    }

    None
}

fn parse_doc_parameter(raw: &str) -> Option<DocParameter> {
    let raw = raw.trim();

    if raw.is_empty() {
        return None;
    }

    if raw == "..." {
        return Some(DocParameter {
            name: "...".to_owned(),
            ty: "any".to_owned(),
        });
    }

    let (name, ty) = raw
        .split_once(':')
        .map(|(name, ty)| (name.trim(), type_prefix(ty.trim())))
        .unwrap_or((raw, "any".to_owned()));

    if name.is_empty() {
        return None;
    }

    Some(DocParameter {
        name: name.to_owned(),
        ty,
    })
}

fn type_prefix(ty: &str) -> String {
    ty.split("--")
        .next()
        .map(str::trim)
        .filter(|ty| !ty.is_empty())
        .unwrap_or("any")
        .to_owned()
}

fn format_tua_type(ty: &TuaType) -> String {
    match ty {
        TuaType::Named(name) => name.clone(),
        TuaType::StringLiteral(value) => format!("{value:?}"),
        TuaType::Union(types) => types
            .iter()
            .map(format_tua_type)
            .collect::<Vec<_>>()
            .join(" | "),
        TuaType::Table(fields) => {
            let fields = fields
                .iter()
                .map(|field| {
                    let marker = if field.optional { "?:" } else { ":" };

                    format!("{}{marker} {}", field.name, format_tua_type(&field.ty))
                })
                .collect::<Vec<_>>()
                .join(", ");

            format!("{{ {fields} }}")
        }
        TuaType::Array(item) => format!("{}[]", format_tua_array_item(item)),
        TuaType::Tuple(items) => format!(
            "[{}]",
            items
                .iter()
                .map(format_tua_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TuaType::Dictionary { key, value } => {
            format!(
                "{{ [{}]: {} }}",
                format_tua_type(key),
                format_tua_type(value)
            )
        }
        TuaType::Function {
            parameters,
            returns,
        } => {
            let parameters = parameters
                .iter()
                .map(|parameter| {
                    if parameter.variadic {
                        return format!("...: {}", format_tua_type(&parameter.ty));
                    }
                    let label = parameter
                        .name
                        .as_ref()
                        .map(|name| {
                            format!("{name}{}: ", if parameter.optional { "?" } else { "" })
                        })
                        .unwrap_or_default();
                    format!("{label}{}", format_tua_type(&parameter.ty))
                })
                .collect::<Vec<_>>()
                .join(", ");
            let returns = returns
                .iter()
                .map(format_tua_type)
                .collect::<Vec<_>>()
                .join(", ");

            if returns.is_empty() {
                format!("fun({parameters})")
            } else {
                format!("fun({parameters}): {returns}")
            }
        }
    }
}

fn format_tua_array_item(ty: &TuaType) -> String {
    let rendered = format_tua_type(ty);

    if matches!(ty, TuaType::Union(_) | TuaType::Function { .. }) {
        format!("({rendered})")
    } else {
        rendered
    }
}

fn complete_domain_config_items(
    parsed: &ParsedSource,
    source: &str,
    prefix: &str,
    position: SourcePosition,
) -> Option<Vec<Completion>> {
    if !table_key_completion_position(source, prefix, position) {
        return None;
    }

    if let Some(items) = state_machine::config_completion_items(parsed, position) {
        return Some(
            items
                .into_iter()
                .filter(|item| completion_text_matches(&item.label, prefix))
                .map(|item| Completion {
                    label: item.label,
                    kind: CompletionKind::Field,
                    detail: Some(item.detail),
                    documentation: Some(item.documentation),
                    insert_text: Some(item.insert_text),
                    filter_text: None,
                    is_snippet: true,
                    module_path: None,
                    source_module: None,
                    additional_text_edits: Vec::new(),
                })
                .collect(),
        );
    }

    if let Some(items) = input::config_completion_items(parsed, position) {
        return Some(
            items
                .into_iter()
                .filter(|item| completion_text_matches(&item.label, prefix))
                .map(|item| Completion {
                    label: item.label,
                    kind: CompletionKind::Field,
                    detail: Some(item.detail),
                    documentation: Some(item.documentation),
                    insert_text: Some(item.insert_text),
                    filter_text: None,
                    is_snippet: true,
                    module_path: None,
                    source_module: None,
                    additional_text_edits: Vec::new(),
                })
                .collect(),
        );
    }

    let context = sprite::config_context_at(parsed, position)?;

    Some(
        sprite::config_completion_items(context)
            .iter()
            .filter(|item| completion_text_matches(item.label, prefix))
            .map(|item| Completion {
                label: item.label.to_owned(),
                kind: CompletionKind::Field,
                detail: Some(item.detail.to_owned()),
                documentation: Some(item.documentation.to_owned()),
                insert_text: Some(item.insert_text.to_owned()),
                filter_text: None,
                is_snippet: true,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect(),
    )
}

fn table_key_completion_position(source: &str, prefix: &str, position: SourcePosition) -> bool {
    let Some(line) = line_at(source, position.line) else {
        return false;
    };
    let before_cursor = line
        .chars()
        .take(position.column.saturating_sub(1))
        .collect::<String>();
    let key_start = before_cursor
        .char_indices()
        .nth_back(prefix.chars().count().saturating_sub(1))
        .map(|(index, _)| index)
        .unwrap_or(before_cursor.len());
    let before_key = before_cursor[..key_start].trim_end();

    before_key.is_empty() || before_key.ends_with(['{', ','])
}

fn sprite_on_loop_value_position(source: &str, position: SourcePosition) -> bool {
    let Some(line) = line_at(source, position.line) else {
        return false;
    };
    let before_cursor = line
        .chars()
        .take(position.column.saturating_sub(1))
        .collect::<String>();
    let Some(quote_start) = before_cursor.rfind(['\'', '"']) else {
        return false;
    };
    let Some((field, _)) = before_cursor[..quote_start].rsplit_once('=') else {
        return false;
    };

    field.trim_end().ends_with("onLoop")
}

fn complete_identifier_items(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    prefix: &str,
    position: SourcePosition,
) -> Vec<Completion> {
    let mut completions = Vec::new();
    let mut labels = BTreeSet::new();

    insert_scope_identifier_items(&mut completions, &mut labels, parsed, prefix, position);

    insert_visible_global_function_items(&mut completions, &mut labels, parsed, prefix, position);
    let include_ecs = environment
        .local(ecs::MODULE_NAME)
        .is_some_and(|ty| ecs::is_module_type(environment, ty));
    insert_keyword_and_builtin_identifier_items(&mut completions, &mut labels, prefix, include_ecs);

    completions
}

fn complete_lossy_identifier_items(
    source: &str,
    prefix: &str,
    position: SourcePosition,
) -> Vec<Completion> {
    let mut completions = Vec::new();
    let mut labels = BTreeSet::new();

    insert_lossy_identifier_items(&mut completions, &mut labels, source, prefix, position);

    completions
}

fn insert_scope_identifier_items(
    completions: &mut Vec<Completion>,
    labels: &mut BTreeSet<String>,
    parsed: &ParsedSource,
    prefix: &str,
    position: SourcePosition,
) {
    let scope = scope_map_parsed_source(parsed);
    let mut declarations = scope
        .declarations()
        .iter()
        .filter(|declaration| scope_declaration_visible_at(declaration, position))
        .filter(|declaration| {
            matches!(
                declaration.kind,
                ScopeDeclarationKind::Local
                    | ScopeDeclarationKind::Parameter
                    | ScopeDeclarationKind::Function
                    | ScopeDeclarationKind::ModuleAlias { .. }
            )
        })
        .collect::<Vec<_>>();

    declarations.sort_by(|left, right| {
        position_key(right.name_span.start)
            .cmp(&position_key(left.name_span.start))
            .then_with(|| left.name.cmp(&right.name))
    });

    for declaration in declarations {
        if completion_text_matches(&declaration.name, prefix) {
            push_unique_completion(
                completions,
                labels,
                completion_from_scope_declaration(declaration),
            );
        }
    }
}

fn scope_declaration_visible_at(declaration: &ScopeDeclaration, position: SourcePosition) -> bool {
    position_key(declaration.name_span.start) <= position_key(position)
        && span_contains_position(declaration.scope_span, position)
}

fn span_contains_position(span: crate::SourceSpan, position: SourcePosition) -> bool {
    position_key(span.start) <= position_key(position)
        && position_key(position) <= position_key(span.end)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn completion_from_scope_declaration(declaration: &ScopeDeclaration) -> Completion {
    let (kind, detail) = match &declaration.kind {
        ScopeDeclarationKind::Parameter => (CompletionKind::Variable, "parameter"),
        ScopeDeclarationKind::Function => (CompletionKind::Function, "local function"),
        ScopeDeclarationKind::ModuleAlias { .. } => (CompletionKind::Module, "module"),
        ScopeDeclarationKind::Local => (CompletionKind::Variable, "local"),
        ScopeDeclarationKind::TableField => (CompletionKind::Field, "field"),
    };

    Completion {
        label: declaration.name.clone(),
        kind,
        detail: Some(detail.to_owned()),
        documentation: None,
        insert_text: None,
        filter_text: None,
        is_snippet: false,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    }
}

fn insert_visible_global_function_items(
    completions: &mut Vec<Completion>,
    labels: &mut BTreeSet<String>,
    parsed: &ParsedSource,
    prefix: &str,
    position: SourcePosition,
) {
    for function in &parsed.function_annotations {
        let Some(name) = &function.name else {
            continue;
        };
        if name.contains(['.', ':']) || !completion_text_matches(name, prefix) {
            continue;
        }
        if function
            .name_span
            .is_some_and(|span| position_key(span.start) > position_key(position))
        {
            continue;
        }

        push_unique_completion(
            completions,
            labels,
            Completion {
                label: name.clone(),
                kind: CompletionKind::Function,
                detail: Some("function".to_owned()),
                documentation: None,
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            },
        );
    }
}

fn insert_keyword_and_builtin_identifier_items(
    completions: &mut Vec<Completion>,
    labels: &mut BTreeSet<String>,
    prefix: &str,
    include_ecs: bool,
) {
    if include_ecs && completion_text_matches(ecs::MODULE_NAME, prefix) {
        push_unique_completion(
            completions,
            labels,
            Completion {
                label: ecs::MODULE_NAME.to_owned(),
                kind: CompletionKind::Module,
                detail: Some("built-in ECS module".to_owned()),
                documentation: Some(ecs::module_documentation().to_owned()),
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            },
        );
    }

    for completion in syntax_snippet_completions(prefix, include_ecs) {
        push_unique_completion(completions, labels, completion);
    }

    for item in lua::keyword_items(prefix)
        .into_iter()
        .chain(lua::top_level_items(prefix))
    {
        push_unique_completion(completions, labels, completion_from_lua_item(item));
    }
}

fn insert_lossy_identifier_items(
    completions: &mut Vec<Completion>,
    labels: &mut BTreeSet<String>,
    source: &str,
    prefix: &str,
    position: SourcePosition,
) {
    for symbol in lua_symbols_before_position(source, position) {
        if completion_text_matches(&symbol.name, prefix) {
            push_unique_completion(completions, labels, symbol.into_completion());
        }
    }

    insert_keyword_and_builtin_identifier_items(completions, labels, prefix, true);
}

fn push_unique_completion(
    completions: &mut Vec<Completion>,
    labels: &mut BTreeSet<String>,
    completion: Completion,
) {
    if labels.insert(completion.label.clone()) {
        completions.push(completion);
    }
}

#[derive(Clone, Copy)]
struct FieldCompletionContext<'a> {
    receiver_parts: &'a [String],
    prefix: &'a str,
    position: SourcePosition,
    separator: FieldAccessSeparator,
    separator_span: crate::SourceSpan,
}

#[derive(Clone, Copy)]
struct StringArgumentCompletionContext<'a> {
    callee_parts: &'a [String],
    active_parameter: usize,
    prefix: &'a str,
    position: SourcePosition,
    project_data: CompletionProjectData<'a>,
}

fn complete_fields(
    parsed: &ParsedSource,
    source: &str,
    environment: &TypeEnvironment,
    context: FieldCompletionContext<'_>,
) -> Vec<Completion> {
    let _profile = profile::span("analysis.completion.receiver");
    let FieldCompletionContext {
        receiver_parts,
        prefix,
        position,
        separator,
        separator_span,
    } = context;
    let semantic = SemanticModel::new(parsed, environment);
    let Some(receiver_type) = love_conf_receiver_type(parsed, source, receiver_parts, position)
        .or_else(|| semantic.type_of_field_chain(receiver_parts, position))
    else {
        return Vec::new();
    };
    let mut completions = BTreeMap::new();
    let mut fields = BTreeMap::new();

    collect_table_fields(environment, &receiver_type, &mut fields);

    for (name, (detail, kind)) in fields
        .into_iter()
        .filter(|(name, _)| completion_text_matches(name, prefix))
    {
        let documentation = ecs::typed_member(environment, &receiver_type, &name)
            .or_else(|| object_pool::typed_member(environment, &receiver_type, &name))
            .or_else(|| state_machine::typed_member(environment, &receiver_type, &name))
            .or_else(|| store::typed_member(environment, &receiver_type, &name))
            .or_else(|| input::typed_member(environment, &receiver_type, &name))
            .and_then(|function| function.documentation);

        completions.insert(
            name.clone(),
            Completion {
                label: name,
                kind,
                detail: Some(detail),
                documentation,
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            },
        );
    }

    {
        let _profile = profile::span("analysis.completion.love_fields");

        collect_love_object_fields(
            parsed,
            environment,
            &receiver_type,
            prefix,
            separator,
            separator_span,
            &mut completions,
        );
    }

    if receiver_parts.len() == 1 && receiver_parts[0] == "love" {
        apply_love_callback_snippets(source, position, &mut completions);
    }

    completions.into_values().collect()
}

fn complete_string_argument_values(
    parsed: &ParsedSource,
    source: &str,
    environment: &TypeEnvironment,
    context: StringArgumentCompletionContext<'_>,
) -> Vec<Completion> {
    let _profile = profile::span("analysis.completion.string_argument");
    let StringArgumentCompletionContext {
        callee_parts,
        active_parameter,
        prefix,
        position,
        project_data,
    } = context;
    let semantic = SemanticModel::new(parsed, environment);

    if let Some(items) = input::string_value_completion_items(parsed, position) {
        return items
            .into_iter()
            .filter(|item| completion_text_matches(&item.label, prefix))
            .map(|item| Completion {
                label: item.label,
                kind: CompletionKind::Value,
                detail: Some(item.detail),
                documentation: Some(item.documentation),
                insert_text: Some(item.insert_text),
                filter_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect();
    }

    if let Some(items) = state_machine::string_value_completion_items(parsed, position) {
        return items
            .into_iter()
            .filter(|item| completion_text_matches(&item.label, prefix))
            .map(|item| Completion {
                label: item.label,
                kind: CompletionKind::Value,
                detail: Some(item.detail),
                documentation: Some(item.documentation),
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect();
    }

    if sprite::config_context_at(parsed, position) == Some(sprite::SpriteConfigContext::Animation)
        && sprite_on_loop_value_position(source, position)
    {
        return sprite::on_loop_method_values()
            .iter()
            .filter(|value| completion_text_matches(value, prefix))
            .map(|value| Completion {
                label: (*value).to_owned(),
                kind: CompletionKind::Value,
                detail: Some("anim8 onLoop method".to_owned()),
                documentation: Some(
                    "Animation method invoked by anim8 whenever the animation loops.".to_owned(),
                ),
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect();
    }

    if let Some(completions) = complete_signal_event_names(
        parsed,
        environment,
        callee_parts,
        active_parameter,
        prefix,
        position,
        project_data,
    ) {
        return completions;
    }

    if let Some((method, receiver_parts)) = callee_parts.split_last() {
        if let Some(receiver_type) = semantic.type_of_field_chain(receiver_parts, position) {
            if let Some(items) = state_machine::method_string_completion_items(
                environment,
                &receiver_type,
                method,
                active_parameter,
            ) {
                return items
                    .into_iter()
                    .filter(|item| completion_text_matches(&item.label, prefix))
                    .map(|item| Completion {
                        label: item.label,
                        kind: CompletionKind::Value,
                        detail: Some(item.detail),
                        documentation: Some(item.documentation),
                        insert_text: None,
                        filter_text: None,
                        is_snippet: false,
                        module_path: None,
                        source_module: None,
                        additional_text_edits: Vec::new(),
                    })
                    .collect();
            }

            if let Some(items) = input::method_string_completion_items(
                environment,
                &receiver_type,
                method,
                active_parameter,
            ) {
                return items
                    .into_iter()
                    .filter(|item| completion_text_matches(&item.label, prefix))
                    .map(|item| Completion {
                        label: item.label,
                        kind: CompletionKind::Value,
                        detail: Some(item.detail),
                        documentation: Some(item.documentation),
                        insert_text: Some(item.insert_text),
                        filter_text: None,
                        is_snippet: false,
                        module_path: None,
                        source_module: None,
                        additional_text_edits: Vec::new(),
                    })
                    .collect();
            }
        }
    }

    if let Some(function) = semantic.function_type_for_call_parts(callee_parts, position) {
        let Some(parameter) = function
            .parameters
            .get(active_parameter)
            .map(|parameter| &parameter.ty)
            .or(function.vararg_type.as_deref())
        else {
            return Vec::new();
        };

        let parameter = resolve_aliases(environment, parameter, 0);

        return complete_lua_enum_values(&parameter.to_string(), prefix);
    }

    let Some(item) = semantic.lua_function_item_for_call_parts(callee_parts, position) else {
        return Vec::new();
    };
    let Some(parameter_type) = parameter_type_from_lua_item(item, active_parameter) else {
        return Vec::new();
    };

    complete_lua_enum_values(&parameter_type, prefix)
}

#[derive(Clone, Debug)]
struct SignalEventCandidate {
    name: String,
    receiver_parts: Vec<String>,
    position: SourcePosition,
    source_rank: usize,
    declaration_rank: usize,
    project_usage: usize,
    same_bus: bool,
    allow_receiver_fallback: bool,
    source_order: usize,
}

fn complete_signal_event_names(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    callee_parts: &[String],
    active_parameter: usize,
    prefix: &str,
    position: SourcePosition,
    project_data: CompletionProjectData<'_>,
) -> Option<Vec<Completion>> {
    if active_parameter != 0 || callee_parts.len() < 2 {
        return None;
    }

    let method = callee_parts.last()?;

    if !signal::is_event_name_method(method) {
        return None;
    }

    let receiver_parts = &callee_parts[..callee_parts.len() - 1];
    let semantic = SemanticModel::new(parsed, environment);
    let receiver_type = semantic.type_of_field_chain(receiver_parts, position)?;

    if !signal::type_is_signal_bus(environment, &receiver_type) {
        return None;
    }

    let mut candidates = match project_data {
        CompletionProjectData::Sources(project_sources) => {
            let mut candidates =
                collect_signal_event_candidates(parsed, environment, Some(position), 0, 0);

            for (index, source) in project_sources.iter().enumerate() {
                if !signal::source_may_contain_event_call(source) {
                    continue;
                }

                let Ok(project_parsed) =
                    parse_tua_source(format!("__project_signal_events_{index}.tua"), *source)
                else {
                    continue;
                };
                let (project_environment, _) =
                    build_type_environment_with_diagnostics(&project_parsed);

                candidates.extend(collect_signal_event_candidates(
                    &project_parsed,
                    &project_environment,
                    None,
                    1,
                    candidates.len(),
                ));
            }

            candidates
        }
        CompletionProjectData::UsageIndex(index) => index
            .signal_events()
            .iter()
            .filter(|event| {
                event.path != parsed.path || !span_contains(event.mention.span, position)
            })
            .enumerate()
            .map(|(source_order, event)| SignalEventCandidate {
                name: event.mention.name.clone(),
                receiver_parts: event.mention.receiver_parts.clone(),
                position: event.mention.position,
                source_rank: usize::from(event.path != parsed.path),
                declaration_rank: signal_event_declaration_rank(
                    &event.mention.method,
                    event.mention.payload.is_some(),
                ),
                project_usage: index.signal_event_index().usage_count(&event.mention.name),
                same_bus: index.same_signal_bus(&parsed.path, receiver_parts, event, position),
                allow_receiver_fallback: false,
                source_order,
            })
            .collect(),
    };

    if candidates.is_empty() {
        return Some(Vec::new());
    }

    candidates.sort_by(|left, right| {
        let left_same_bus = left.same_bus
            || (left.allow_receiver_fallback && left.receiver_parts == receiver_parts);
        let right_same_bus = right.same_bus
            || (right.allow_receiver_fallback && right.receiver_parts == receiver_parts);
        right_same_bus
            .cmp(&left_same_bus)
            .then_with(|| left.source_rank.cmp(&right.source_rank))
            .then_with(|| left.declaration_rank.cmp(&right.declaration_rank))
            .then_with(|| {
                line_distance(left.position, position).cmp(&line_distance(right.position, position))
            })
            .then_with(|| right.project_usage.cmp(&left.project_usage))
            .then_with(|| left.source_order.cmp(&right.source_order))
            .then_with(|| left.name.cmp(&right.name))
    });

    let mut completions = signal_event_completions(candidates.clone(), prefix, None);

    if completions.is_empty() && !prefix.is_empty() {
        completions = signal_event_completions(candidates, "", Some(prefix));
    }

    Some(completions)
}

fn collect_signal_event_candidates(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    skip_position: Option<SourcePosition>,
    source_rank: usize,
    source_order_offset: usize,
) -> Vec<SignalEventCandidate> {
    signal::collect_event_mentions(parsed, environment, None, skip_position)
        .into_iter()
        .enumerate()
        .map(|(index, mention)| {
            let declaration_rank =
                signal_event_declaration_rank(&mention.method, mention.payload.is_some());

            SignalEventCandidate {
                name: mention.name,
                receiver_parts: mention.receiver_parts,
                position: mention.position,
                source_rank,
                declaration_rank,
                project_usage: 0,
                same_bus: false,
                allow_receiver_fallback: true,
                source_order: source_order_offset + index,
            }
        })
        .collect()
}

fn signal_event_declaration_rank(method: &str, has_payload: bool) -> usize {
    match method {
        "declare" => 0,
        "on" | "once" | "register" if has_payload => 1,
        "on" | "once" | "register" => 2,
        _ => 3,
    }
}

fn signal_event_completions(
    candidates: Vec<SignalEventCandidate>,
    prefix: &str,
    filter_text: Option<&str>,
) -> Vec<Completion> {
    let mut completions = Vec::new();
    let mut labels = BTreeSet::new();

    for candidate in candidates {
        if !completion_text_matches(&candidate.name, prefix) {
            continue;
        }

        if !labels.insert(candidate.name.clone()) {
            continue;
        }

        completions.push(Completion {
            label: candidate.name,
            kind: CompletionKind::Value,
            detail: Some("SignalBus event".to_owned()),
            documentation: Some(
                "Known SignalBus event name from existing signal calls.".to_owned(),
            ),
            insert_text: None,
            filter_text: filter_text.map(str::to_owned),
            is_snippet: false,
            module_path: None,
            source_module: None,
            additional_text_edits: Vec::new(),
        });
    }

    completions
}

fn line_distance(left: SourcePosition, right: SourcePosition) -> usize {
    left.line.abs_diff(right.line)
}

fn span_contains(span: crate::SourceSpan, position: SourcePosition) -> bool {
    let position = (position.line, position.column);

    (span.start.line, span.start.column) <= position && position <= (span.end.line, span.end.column)
}

fn parameter_type_from_lua_item(item: LuaItem, index: usize) -> Option<String> {
    if let Some(signatures) = lua::signature_variants(item) {
        let mut types = Vec::new();

        for signature in signatures {
            let Some(parameter) = lua_signature_parameter_at(signature, index) else {
                continue;
            };
            let Some(ty) = parameter_type_from_label(parameter.label) else {
                continue;
            };

            if !types.iter().any(|existing| existing == &ty) {
                types.push(ty);
            }
        }

        if !types.is_empty() {
            return Some(types.join(" | "));
        }
    }

    parameter_type_from_function_detail(item.detail, index)
}

fn lua_signature_parameter_at(
    signature: &lua::LuaSignature,
    index: usize,
) -> Option<&lua::LuaSignatureParameter> {
    signature.parameters.get(index).or_else(|| {
        signature
            .parameters
            .last()
            .filter(|parameter| parameter.label.starts_with("..."))
    })
}

fn parameter_type_from_function_detail(detail: &str, index: usize) -> Option<String> {
    let start = detail.find('(')? + 1;
    let end = detail[start..].find(')')? + start;
    let parameter = detail[start..end]
        .split(',')
        .map(str::trim)
        .filter(|parameter| !parameter.is_empty())
        .nth(index)?;
    parameter_type_from_label(parameter)
}

fn parameter_type_from_label(parameter: &str) -> Option<String> {
    let (_, ty) = parameter.split_once(':')?;
    Some(ty.trim().to_owned())
}

fn complete_lua_enum_values(type_label: &str, prefix: &str) -> Vec<Completion> {
    let completions = complete_lua_enum_value_candidates(type_label, prefix);

    if !completions.is_empty() || prefix.is_empty() {
        return completions;
    }

    let mut fallback = complete_lua_enum_value_candidates(type_label, "");

    for completion in &mut fallback {
        completion.filter_text = Some(prefix.to_owned());
    }

    fallback
}

fn complete_lua_enum_value_candidates(type_label: &str, prefix: &str) -> Vec<Completion> {
    type_label
        .split('|')
        .map(str::trim)
        .flat_map(|type_name| {
            literal_value_item(type_name, prefix).into_iter().chain(
                lua::enum_value_items(type_name, prefix)
                    .into_iter()
                    .map(completion_from_lua_item),
            )
        })
        .collect()
}

fn literal_value_item(type_name: &str, prefix: &str) -> Option<Completion> {
    let value = quoted_string_literal_value(type_name)?;

    if !completion_text_matches(&value, prefix) {
        return None;
    }

    Some(Completion {
        label: value,
        kind: CompletionKind::Value,
        detail: Some(type_name.to_owned()),
        documentation: None,
        insert_text: None,
        filter_text: None,
        is_snippet: false,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    })
}

fn quoted_string_literal_value(value: &str) -> Option<String> {
    let value = value.trim();
    let quote = value.chars().next()?;

    if !matches!(quote, '\'' | '"') || !value.ends_with(quote) || value.len() < 2 {
        return None;
    }

    Some(value[quote.len_utf8()..value.len() - quote.len_utf8()].to_owned())
}

fn collect_love_object_fields(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    ty: &Type,
    prefix: &str,
    separator: FieldAccessSeparator,
    separator_span: crate::SourceSpan,
    completions: &mut BTreeMap<String, Completion>,
) {
    if let Type::Alias(name) = ty {
        for item in lua::type_field_items(name, prefix) {
            insert_love_object_item(completions, parsed, item, separator, separator_span);
        }
    }

    match resolve_aliases(environment, ty, 0) {
        Type::Alias(name) => {
            for item in lua::type_field_items(&name, prefix) {
                insert_love_object_item(completions, parsed, item, separator, separator_span);
            }
        }
        Type::Union(types) => {
            for ty in types {
                collect_love_object_fields(
                    parsed,
                    environment,
                    &ty,
                    prefix,
                    separator,
                    separator_span,
                    completions,
                );
            }
        }
        Type::Any
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::Table(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => {}
    }
}

fn collect_table_fields(
    environment: &TypeEnvironment,
    ty: &Type,
    fields: &mut BTreeMap<String, (String, CompletionKind)>,
) {
    match resolve_aliases(environment, ty, 0) {
        Type::Table(table) => collect_shape_fields(environment, &table, fields),
        Type::Union(types) => {
            for ty in types {
                collect_table_fields(environment, &ty, fields);
            }
        }
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => {}
    }
}

fn collect_shape_fields(
    environment: &TypeEnvironment,
    table: &TableShape,
    fields: &mut BTreeMap<String, (String, CompletionKind)>,
) {
    for (name, field) in table.semantic_fields() {
        let suffix = if field.optional { " optional" } else { "" };
        let resolved = resolve_aliases(environment, &field.ty, 0);
        let display_type = if matches!(resolved, Type::Function(_)) {
            &resolved
        } else {
            &field.ty
        };

        fields.insert(
            name,
            (
                format!("field{suffix}: {display_type}"),
                completion_kind_for_type(&resolved),
            ),
        );
    }
}

fn complete_lua_fields(
    source: &str,
    receiver_parts: &[String],
    prefix: &str,
    position: SourcePosition,
) -> Vec<Completion> {
    lua::field_items(receiver_parts, prefix)
        .into_iter()
        .map(|item| completion_from_lua_field_item(source, receiver_parts, item, position))
        .collect()
}

fn complete_required_module_fields(
    parsed: &ParsedSource,
    receiver_parts: &[String],
    prefix: &str,
) -> Vec<Completion> {
    let Some((receiver, fields)) = receiver_parts.split_first() else {
        return Vec::new();
    };
    let Some(exports) = required_module_exports_for_receiver(parsed, receiver) else {
        return Vec::new();
    };

    if fields.is_empty() {
        let module_path = exports.path.clone();

        return exports
            .fields
            .into_iter()
            .filter(|(name, _)| completion_text_matches(name, prefix))
            .map(|(name, field)| Completion {
                kind: completion_kind_for_type(&field.ty),
                detail: Some(format!("field: {}", field.ty)),
                label: name,
                documentation: field.documentation,
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: Some(module_path.clone()),
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect();
    }

    let Some(export) = exported_field_for_fields(&exports, fields) else {
        return Vec::new();
    };
    let module_path = exports.path;
    let mut fields = BTreeMap::new();

    collect_export_fields(&export.ty, &mut fields);

    fields
        .into_iter()
        .filter(|(name, _)| completion_text_matches(name, prefix))
        .map(|(name, ty)| Completion {
            kind: completion_kind_for_type(&ty),
            detail: Some(format!("field: {ty}")),
            label: name,
            documentation: export.documentation.clone(),
            insert_text: None,
            filter_text: None,
            is_snippet: false,
            module_path: Some(module_path.clone()),
            source_module: None,
            additional_text_edits: Vec::new(),
        })
        .collect()
}

fn collect_export_fields(ty: &Type, fields: &mut BTreeMap<String, Type>) {
    match ty {
        Type::Table(table) => {
            fields.extend(
                table
                    .fields()
                    .map(|(name, field)| (name.to_owned(), field.ty.clone())),
            );
        }
        Type::Union(types) => {
            for ty in types {
                collect_export_fields(ty, fields);
            }
        }
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => {}
    }
}

fn completion_kind_for_type(ty: &Type) -> CompletionKind {
    match ty {
        Type::Function(_) => CompletionKind::Function,
        _ => CompletionKind::Field,
    }
}

fn completion_from_lua_item(item: LuaItem) -> Completion {
    Completion {
        label: item.label.to_owned(),
        kind: completion_kind_from_lua_item(item.kind),
        detail: Some(item.detail.to_owned()),
        documentation: Some(lua::documentation_with_love_wiki_link(item)),
        insert_text: None,
        filter_text: None,
        is_snippet: false,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    }
}

fn completion_from_lua_field_item(
    source: &str,
    receiver_parts: &[String],
    item: LuaItem,
    position: SourcePosition,
) -> Completion {
    let mut completion = completion_from_lua_item(item);

    if receiver_parts.len() == 1 && receiver_parts[0] == "love" {
        if let Some(callback) = lua::love_callback_item(item.label) {
            append_love_callback_parameter_documentation(&mut completion, callback);
        }
        if let Some(snippet) = love_callback_snippet(item.label, source, position) {
            completion.insert_text = Some(snippet);
            completion.is_snippet = true;
        }
    } else if receiver_parts.first().map(String::as_str) == Some("love")
        && item.kind == LuaItemKind::Function
    {
        if let Some(snippet) = function_call_snippet_from_lua_item(item) {
            completion.insert_text = Some(snippet);
            completion.is_snippet = true;
        }
    }

    completion
}

fn apply_love_callback_snippets(
    source: &str,
    position: SourcePosition,
    completions: &mut BTreeMap<String, Completion>,
) {
    for completion in completions.values_mut() {
        let Some(callback) = lua::love_callback_item(&completion.label) else {
            continue;
        };

        append_love_callback_parameter_documentation(completion, callback);
        if let Some(snippet) = love_callback_snippet(&completion.label, source, position) {
            completion.insert_text = Some(snippet);
            completion.is_snippet = true;
        }
    }
}

fn love_callback_snippet(label: &str, source: &str, position: SourcePosition) -> Option<String> {
    if !field_completion_is_function_declaration(source, position) {
        return None;
    }

    love_callback::declaration_suffix(label, "$0")
}

fn append_love_callback_parameter_documentation(completion: &mut Completion, callback: LuaItem) {
    let Some(signatures) = lua::signature_variants(callback) else {
        return;
    };
    let Some(signature) = signatures.first() else {
        return;
    };
    let parameters = signature
        .parameters
        .iter()
        .filter(|parameter| !parameter.documentation.trim().is_empty())
        .map(|parameter| {
            format!(
                "- `{}`: {}",
                parameter.label,
                parameter.documentation.trim()
            )
        })
        .collect::<Vec<_>>();

    if parameters.is_empty() {
        return;
    }

    let documentation = completion.documentation.get_or_insert_default();
    if !documentation.trim().is_empty() {
        documentation.push_str("\n\n");
    }
    documentation.push_str("**Callback parameters**\n\n");
    documentation.push_str(&parameters.join("\n"));
}

fn function_call_snippet_from_lua_item(item: LuaItem) -> Option<String> {
    let signature = parse_function_signature_line(item.detail)?;
    let mut arguments = Vec::new();
    let mut placeholder = 1;

    for parameter in signature.parameters {
        let Some(argument) = function_call_snippet_argument(&parameter, placeholder) else {
            continue;
        };

        arguments.push(argument);
        placeholder += 1;
    }

    if arguments.is_empty() {
        return Some(format!("{}($0)", item.label));
    }

    Some(format!("{}({})$0", item.label, arguments.join(", ")))
}

fn function_call_snippet_argument(parameter: &DocParameter, placeholder: usize) -> Option<String> {
    let name = parameter.name.trim();

    if name.ends_with('?') {
        return None;
    }

    if name == "..." {
        return Some(format!(
            "${{{placeholder}:{}}}",
            escape_snippet_placeholder("...")
        ));
    }

    let name = name.trim_end_matches('?');
    let enum_values = lua_enum_choice_values(&parameter.ty);

    if !enum_values.is_empty() {
        let choices = enum_values
            .into_iter()
            .map(|value| escape_snippet_choice(&value))
            .collect::<Vec<_>>()
            .join(",");

        return Some(format!("\"${{{placeholder}|{choices}|}}\""));
    }

    Some(format!(
        "${{{placeholder}:{}}}",
        escape_snippet_placeholder(name)
    ))
}

fn lua_enum_choice_values(type_label: &str) -> Vec<String> {
    let mut values = Vec::new();

    for type_name in type_label.split('|').map(str::trim) {
        if let Some(value) = quoted_string_literal_value(type_name) {
            values.push(value);
        }

        for item in lua::enum_value_items(type_name, "") {
            values.push(item.label.to_owned());
        }
    }

    values.dedup();
    values
}

fn escape_snippet_placeholder(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('$', "\\$")
        .replace('}', "\\}")
}

fn escape_snippet_choice(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace('|', "\\|")
}

fn field_completion_is_function_declaration(source: &str, position: SourcePosition) -> bool {
    let Some(line) = line_at(source, position.line) else {
        return false;
    };
    let cursor = crate::source_text::byte_for_column(line, position.column).min(line.len());

    line[..cursor].trim_start().starts_with("function ")
}

fn love_conf_receiver_type(
    parsed: &ParsedSource,
    source: &str,
    receiver_parts: &[String],
    position: SourcePosition,
) -> Option<Type> {
    let receiver = receiver_parts.first()?;
    let parameter = love_conf::parameter_name_at(parsed, position)
        .or_else(|| love_conf_parameter_in_source(source, position))?;

    if receiver != &parameter {
        return None;
    }

    field_chain_type(love_conf::config_type(), &receiver_parts[1..])
}

fn field_chain_type(mut ty: Type, fields: &[String]) -> Option<Type> {
    for field in fields {
        ty = match ty {
            Type::Table(table) => table.semantic_field(field)?.ty.clone(),
            Type::Union(types) => {
                let mut resolved = Vec::new();

                for ty in types {
                    if let Some(field_ty) = field_chain_type(ty, std::slice::from_ref(field)) {
                        resolved.push(field_ty);
                    }
                }

                match resolved.len() {
                    0 => return None,
                    1 => resolved.remove(0),
                    _ => Type::union(resolved),
                }
            }
            Type::Any
            | Type::Alias(_)
            | Type::Function(_)
            | Type::Primitive(_)
            | Type::StringLiteral(_)
            | Type::Array(_)
            | Type::Tuple(_)
            | Type::Dictionary { .. } => return None,
        };
    }

    Some(ty)
}

fn love_conf_parameter_in_source(source: &str, position: SourcePosition) -> Option<String> {
    let mut parameter = None;

    for line_number in 1..=position.line {
        let line = line_at(source, line_number)?;
        let trimmed = line.trim_start();

        if let Some(rest) = trimmed.strip_prefix("function love.conf(") {
            parameter = rest
                .split([',', ')'])
                .next()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned);
            continue;
        }

        if parameter.is_some() && trimmed == "end" {
            parameter = None;
        }
    }

    parameter
}

fn insert_love_object_item(
    completions: &mut BTreeMap<String, Completion>,
    parsed: &ParsedSource,
    item: LuaItem,
    separator: FieldAccessSeparator,
    separator_span: crate::SourceSpan,
) {
    completions.insert(item.label.to_owned(), {
        let mut completion = completion_from_lua_item(item);

        if item.kind == LuaItemKind::Function {
            completion.insert_text = Some(
                function_call_snippet_from_lua_item(item)
                    .unwrap_or_else(|| format!("{}($0)", item.label)),
            );
            completion.is_snippet = true;

            if separator == FieldAccessSeparator::Dot {
                completion.additional_text_edits.push(TextEdit {
                    path: parsed.path.clone(),
                    span: separator_span,
                    new_text: ":".to_owned(),
                });
            }
        }

        completion
    });
}

fn completion_kind_from_lua_item(kind: LuaItemKind) -> CompletionKind {
    match kind {
        LuaItemKind::Function => CompletionKind::Function,
        LuaItemKind::Keyword => CompletionKind::Keyword,
        LuaItemKind::Module => CompletionKind::Module,
        LuaItemKind::Value => CompletionKind::Value,
    }
}

fn parse_completion_source(
    path: &Path,
    source: &str,
    position: SourcePosition,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    parse_tua_source(path, source.to_owned())
        .or_else(|_| parse_tua_source(path, source_with_line_removed(source, position.line)))
}

fn source_with_line_removed(source: &str, line_to_remove: usize) -> String {
    let mut output = String::with_capacity(source.len());
    let mut current_line = 1;
    let mut line_start = 0;

    for (index, character) in source.char_indices() {
        if character != '\n' {
            continue;
        }

        if current_line == line_to_remove {
            output.push('\n');
        } else {
            output.push_str(&source[line_start..=index]);
        }

        current_line += 1;
        line_start = index + 1;
    }

    if line_start < source.len() {
        if current_line == line_to_remove {
            output.push('\n');
        } else {
            output.push_str(&source[line_start..]);
        }
    }

    output
}

#[cfg(test)]
mod tests;
