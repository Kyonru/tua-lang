use super::eligibility::{
    cursor_context, lexical_region_at, strip_trailing_comment, suffix_allows_inline, word_before,
    CursorContext, LexicalRegion,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{
    AnalysisHost, AnalysisSourceKind, InlineCompletionBudget, InlineCompletionOptions,
    InlineCompletionSource, PatternCorpusBatchEntry, PatternCorpusBuildOutcome, PatternCorpusTier,
    PreparedInlineCompletion, ProjectModel, RetrievalLimits, SourcePosition,
};

fn context_at_end(source: &str) -> Option<CursorContext> {
    cursor_context(source, source.len())
}

fn without_inline_stats(mut stats: crate::AnalysisCacheStats) -> crate::AnalysisCacheStats {
    stats.inline_completion_requests = 0;
    stats.inline_skipped_oversized = 0;
    stats.inline_cache_peek_hits = 0;
    stats.inline_cache_peek_misses = 0;
    stats.inline_validation_attempts = 0;
    stats.inline_candidate_parses = 0;
    stats.inline_baseline_parses = 0;
    stats.inline_edited_parse_sufficient = 0;
    stats.inline_candidates_returned = 0;
    stats.inline_returned_after_zero_parses = 0;
    stats.inline_returned_after_one_parse = 0;
    stats.inline_returned_after_two_parses = 0;
    stats
}

fn build_project_file_with_required_targets(
    host: &AnalysisHost,
    path: &str,
    required_targets: &[&str],
) -> PatternCorpusBuildOutcome {
    let mut outcome = host
        .prepare_project_pattern_file(path)
        .unwrap()
        .build(&ProjectModel::default(), RetrievalLimits::default());
    for target in required_targets {
        outcome.add_required_target_for_test(*target);
    }
    outcome
}

#[test]
fn strings_and_comments_are_ineligible() {
    for source in [
        "local name = \"Ad",
        "local name = 'Ad",
        "local text = [[multi\nline",
        "local text = [==[ has ]] inside",
        "x = 1 -- note",
        "--[[ block\ncomment",
        "--[=[ level one ]] still",
    ] {
        assert_eq!(context_at_end(source), None, "{source:?}");
    }

    assert_eq!(
        lexical_region_at("local a = \"x\" .. ", 17),
        LexicalRegion::Code
    );
    assert_eq!(
        lexical_region_at("--[[ done ]] local a", 20),
        LexicalRegion::Code
    );
    assert_eq!(
        lexical_region_at("local s = \"a\\\"b", 15),
        LexicalRegion::String
    );
    // A short string that is never closed ends at the newline.
    assert_eq!(
        lexical_region_at("local s = \"open\nlocal t = ", 26),
        LexicalRegion::Code
    );
}

#[test]
fn contexts_follow_the_character_before_the_cursor() {
    let cases = [
        ("print(", CursorContext::OpenParen),
        ("print(a,", CursorContext::Comma),
        ("local x =", CursorContext::Assignment),
        ("if x ==", CursorContext::Operator),
        ("if x ~=", CursorContext::Operator),
        ("local s = a ..", CursorContext::Operator),
        ("local y = x +", CursorContext::Operator),
        ("local x = ", CursorContext::Whitespace),
        ("love.graphics.", CursorContext::Member),
        ("self:", CursorContext::Member),
        ("  loc", CursorContext::StatementPrefix),
        ("local pos", CursorContext::Identifier),
        ("local t = {", CursorContext::Other),
    ];
    for (source, expected) in cases {
        assert_eq!(context_at_end(source), Some(expected), "{source:?}");
    }
}

#[test]
fn indentation_alone_is_ineligible() {
    assert_eq!(context_at_end("local a = 1\n    "), None);
    assert_eq!(context_at_end(""), None);
}

#[test]
fn only_closing_delimiters_may_follow_the_cursor() {
    assert!(suffix_allows_inline(""));
    assert!(suffix_allows_inline(")"));
    assert!(suffix_allows_inline(" })] "));
    assert!(!suffix_allows_inline("x)"));
    assert!(!suffix_allows_inline("\"")); // a closing quote is not a delimiter
}

#[test]
fn trailing_comments_are_not_part_of_the_code() {
    assert_eq!(strip_trailing_comment("x = 1 -- note"), "x = 1");
    assert_eq!(strip_trailing_comment("s = \"a -- b\"  "), "s = \"a -- b\"");
    assert_eq!(strip_trailing_comment("plain"), "plain");
}

#[test]
fn the_replaced_word_ends_at_the_cursor() {
    assert_eq!(word_before("local pos", 9), "pos");
    assert_eq!(word_before("love.gra", 8), "gra");
    assert_eq!(word_before("print(", 6), "");
}

#[test]
fn auto_close_suffix_closes_the_line_brackets_still_open() {
    use super::eligibility::auto_close_suffix;
    assert_eq!(auto_close_suffix("print(f(a, "), "))");
    assert_eq!(auto_close_suffix("local t = { x = g(1), y = "), "}");
    assert_eq!(auto_close_suffix("call(a) + b"), "");
    // Brackets in strings, comments, and long brackets don't count.
    assert_eq!(auto_close_suffix("print(\"(\", "), ")");
    assert_eq!(auto_close_suffix("local s = [[ ("), "");
    // A line that closes more than it opens has nothing to auto-close.
    assert_eq!(auto_close_suffix("end) + f("), "");
}

#[test]
fn host_query_recalls_without_building_analysis() {
    let path = PathBuf::from("src/main.lua");
    let source = "print(player.name)\nprint(player.name)\nprint(pla";
    let mut host = AnalysisHost::new();
    host.set_file_with_kind(&path, source, AnalysisSourceKind::Lua);
    let before = host.cache_stats();
    let candidates = host.local_inline_completion(
        &path,
        SourcePosition {
            line: 3,
            column: 10,
        },
        &InlineCompletionOptions::default(),
        &InlineCompletionBudget::unlimited(),
        &AtomicBool::new(false),
    );

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].insert_text, "yer.name)");
    assert_eq!(candidates[0].source, InlineCompletionSource::CurrentFile);
    let stats = host.cache_stats();
    assert_eq!(stats.inline_completion_requests, 1);
    assert_eq!(stats.inline_cache_peek_hits, 0);
    assert_eq!(stats.inline_cache_peek_misses, 1);
    assert_eq!(stats.inline_validation_attempts, 1);
    assert_eq!(stats.inline_candidate_parses, 1);
    assert_eq!(stats.inline_baseline_parses, 0);
    assert_eq!(stats.inline_edited_parse_sufficient, 1);
    assert_eq!(stats.inline_candidates_returned, 1);
    assert_eq!(stats.inline_returned_after_zero_parses, 0);
    assert_eq!(stats.inline_returned_after_one_parse, 1);
    assert_eq!(stats.inline_returned_after_two_parses, 0);
    assert_eq!(without_inline_stats(stats), before);
}

#[test]
fn prepared_query_is_owned_sendable_and_does_no_analysis_work() {
    fn assert_send<T: Send>() {}
    assert_send::<PreparedInlineCompletion>();

    let path = PathBuf::from("src/main.lua");
    let source = "print(player.name)\nprint(player.name)\nprint(pla";
    let mut host = AnalysisHost::new();
    host.set_file_with_kind(&path, source, AnalysisSourceKind::Lua);
    let before = host.cache_stats();
    let prepared = host.prepare_local_inline_completion(&path).unwrap();
    assert_eq!(prepared.source(), source);
    assert!(prepared.project.is_none());
    let after_prepare = host.cache_stats();

    assert_eq!(after_prepare.inline_completion_requests, 1);
    assert_eq!(after_prepare.inline_cache_peek_hits, 0);
    assert_eq!(after_prepare.inline_cache_peek_misses, 1);
    assert_eq!(after_prepare.inline_validation_attempts, 0);
    assert_eq!(after_prepare.inline_candidate_parses, 0);
    assert_eq!(without_inline_stats(after_prepare), before);

    // The worker payload owns everything it needs; it does not borrow or clone
    // the host after coordinator preparation.
    drop(host);
    let candidates = prepared.complete(
        SourcePosition {
            line: 3,
            column: 10,
        },
        &InlineCompletionOptions::default(),
        &InlineCompletionBudget::unlimited(),
        &AtomicBool::new(false),
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].insert_text, "yer.name)");
}

#[test]
fn host_query_reports_an_exact_current_source_cache_hit() {
    let path = PathBuf::from("src/main.lua");
    let source = "print(player.name)\nprint(player.name)\nprint(pla";
    let mut host = AnalysisHost::new();
    host.set_file_with_kind(&path, source, AnalysisSourceKind::Lua);
    assert!(host.warm_file(&path, crate::AnalysisWarmPriority::High));

    let candidates = host.local_inline_completion(
        &path,
        SourcePosition {
            line: 3,
            column: 10,
        },
        &InlineCompletionOptions::default(),
        &InlineCompletionBudget::unlimited(),
        &AtomicBool::new(false),
    );

    assert_eq!(candidates.len(), 1);
    let stats = host.cache_stats();
    assert_eq!(stats.inline_completion_requests, 1);
    assert_eq!(stats.inline_cache_peek_hits, 1);
    assert_eq!(stats.inline_cache_peek_misses, 0);
}

#[test]
fn returned_candidate_stats_record_two_parse_validation() {
    let path = PathBuf::from("src/main.lua");
    let source = "local broken = )\nprint(player.name)\nprint(pla";
    let mut host = AnalysisHost::new();
    host.set_file_with_kind(&path, source, AnalysisSourceKind::Lua);

    let candidates = host.local_inline_completion(
        &path,
        SourcePosition {
            line: 3,
            column: 10,
        },
        &InlineCompletionOptions::default(),
        &InlineCompletionBudget::unlimited(),
        &AtomicBool::new(false),
    );

    assert_eq!(candidates.len(), 1);
    let stats = host.cache_stats();
    assert_eq!(stats.inline_candidates_returned, 1);
    assert_eq!(stats.inline_returned_after_zero_parses, 0);
    assert_eq!(stats.inline_returned_after_one_parse, 0);
    assert_eq!(stats.inline_returned_after_two_parses, 1);
}

#[test]
fn source_only_snippets_work_on_a_cold_cache() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();
    host.set_file(&path, "whi");
    let before = host.cache_stats();
    let candidates = host.local_inline_completion(
        &path,
        SourcePosition { line: 1, column: 4 },
        &InlineCompletionOptions::default(),
        &InlineCompletionBudget::unlimited(),
        &AtomicBool::new(false),
    );

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].is_snippet);
    assert_eq!(
        candidates[0].insert_text,
        "le ${1:condition} do\n  ${0}\nend"
    );
    assert_eq!(candidates[0].source, InlineCompletionSource::Semantic);
    assert_eq!(without_inline_stats(host.cache_stats()), before);
}

#[test]
fn cancellation_deadline_and_source_switches_abstain() {
    let path = PathBuf::from("src/main.lua");
    let source = "print(player.name)\nprint(player.name)\nprint(pla";
    let mut host = AnalysisHost::new();
    host.set_file_with_kind(&path, source, AnalysisSourceKind::Lua);
    let position = SourcePosition {
        line: 3,
        column: 10,
    };
    let cancelled = AtomicBool::new(true);
    assert!(host
        .local_inline_completion(
            &path,
            position,
            &InlineCompletionOptions::default(),
            &InlineCompletionBudget::unlimited(),
            &cancelled,
        )
        .is_empty());
    cancelled.store(false, Ordering::Relaxed);
    assert!(host
        .local_inline_completion(
            &path,
            position,
            &InlineCompletionOptions::default(),
            &InlineCompletionBudget::expired(),
            &cancelled,
        )
        .is_empty());
    assert!(host
        .local_inline_completion(
            &path,
            position,
            &InlineCompletionOptions {
                semantic: false,
                current_file: false,
                project: false,
                statistical: false,
            },
            &InlineCompletionBudget::unlimited(),
            &cancelled,
        )
        .is_empty());
}

#[test]
fn frozen_context_allowlist_suppresses_assignment_and_member_recall() {
    for source in [
        "local value = player\nlocal value = player\nlocal value =",
        "love.graphics.print\nlove.graphics.print\nlove.graphics.",
    ] {
        let path = PathBuf::from("src/main.lua");
        let mut host = AnalysisHost::new();
        host.set_file_with_kind(&path, source, AnalysisSourceKind::Lua);
        let line = source.lines().count();
        let column = source.lines().last().unwrap().chars().count() + 1;
        assert!(host
            .local_inline_completion(
                &path,
                SourcePosition { line, column },
                &InlineCompletionOptions::default(),
                &InlineCompletionBudget::unlimited(),
                &AtomicBool::new(false),
            )
            .is_empty());
    }
}

#[test]
fn prepared_query_reads_the_immutable_project_corpus() {
    let mut host = AnalysisHost::new();
    host.set_project_pattern_eligibility_generation(1);
    host.set_file("active/main.tua", "draw(player,");
    host.set_file("active/sibling.tua", "draw(player, wrong)");
    host.set_file("one/a.tua", "draw(player, score); alpha()");
    host.set_file("two/b.tua", "draw(player, score); beta()");
    let outcomes = [
        build_project_file_with_required_targets(
            &host,
            "active/main.tua",
            &["one/a.tua", "two/b.tua"],
        ),
        build_project_file_with_required_targets(&host, "active/sibling.tua", &[]),
        build_project_file_with_required_targets(&host, "one/a.tua", &[]),
        build_project_file_with_required_targets(&host, "two/b.tua", &[]),
    ]
    .map(PatternCorpusBatchEntry::File);
    assert_eq!(host.adopt_project_pattern_batch(1, outcomes).adopted, 4);

    let prepared = host
        .prepare_local_inline_completion_with_project("active/main.tua", true)
        .unwrap();
    drop(host);
    let candidates = prepared.complete(
        SourcePosition {
            line: 1,
            column: 13,
        },
        &InlineCompletionOptions {
            semantic: false,
            current_file: false,
            project: true,
            statistical: false,
        },
        &InlineCompletionBudget::unlimited(),
        &AtomicBool::new(false),
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].insert_text, " score);");
    assert_eq!(candidates[0].source, InlineCompletionSource::Project);
    assert_eq!(candidates[0].tier, Some(PatternCorpusTier::RequiredModule));
}

#[test]
fn project_disabled_preparation_captures_no_project_or_vfs_version_arc() {
    let mut host = AnalysisHost::new();
    host.set_file("src/main.tua", "draw(player,");

    // Both the compatibility entry point and the explicit disabled form must
    // leave the sole container for the corpus and live VFS-version registry
    // absent. This keeps the default request independent of project corpus
    // lifetime and avoids adding a version-registry reader on every keypress.
    let default_prepared = host
        .prepare_local_inline_completion("src/main.tua")
        .unwrap();
    assert!(default_prepared.project.is_none());
    let explicitly_disabled = host
        .prepare_local_inline_completion_with_project("src/main.tua", false)
        .unwrap();
    assert!(explicitly_disabled.project.is_none());

    let enabled = host
        .prepare_local_inline_completion_with_project("src/main.tua", true)
        .unwrap();
    assert!(enabled.project.is_some());
}

#[test]
fn production_semantic_policy_keeps_unevaluated_love_callbacks_disabled() {
    let mut host = AnalysisHost::new();
    host.set_project_pattern_eligibility_generation(1);
    host.set_file("src/callbacks.tua", "function love.update(dt: number)\nend");
    host.set_file("src/main.tua", "function love.upd");
    let outcome = host
        .prepare_project_pattern_file("src/callbacks.tua")
        .unwrap()
        .build(&ProjectModel::default(), RetrievalLimits::default());
    assert_eq!(
        host.adopt_project_pattern_batch(1, [PatternCorpusBatchEntry::File(outcome)])
            .adopted,
        1
    );
    let prepared = host
        .prepare_local_inline_completion_with_project("src/main.tua", true)
        .unwrap();
    let position = SourcePosition {
        line: 1,
        column: 18,
    };
    let local_only = prepared.complete(
        position,
        &InlineCompletionOptions {
            semantic: true,
            current_file: false,
            project: false,
            statistical: false,
        },
        &InlineCompletionBudget::unlimited(),
        &AtomicBool::new(false),
    );
    assert!(local_only.is_empty());
    let with_project = prepared.complete(
        position,
        &InlineCompletionOptions {
            semantic: true,
            current_file: false,
            project: true,
            statistical: false,
        },
        &InlineCompletionBudget::unlimited(),
        &AtomicBool::new(false),
    );
    assert!(with_project.is_empty());
    assert!(host.project_declared_love_callbacks().contains("update"));
}

#[test]
fn prepared_project_query_observes_a_later_vfs_invalidation_without_copy_on_write() {
    let mut host = AnalysisHost::new();
    host.set_project_pattern_eligibility_generation(1);
    host.set_file("one/a.tua", "draw(player, score)\ndraw(player, score)");
    host.set_file("active/main.tua", "draw(player,");
    let active = build_project_file_with_required_targets(&host, "active/main.tua", &["one/a.tua"]);
    let outcome = build_project_file_with_required_targets(&host, "one/a.tua", &[]);
    assert_eq!(
        host.adopt_project_pattern_batch(1, [active, outcome].map(PatternCorpusBatchEntry::File),)
            .adopted,
        2
    );
    let prepared = host
        .prepare_local_inline_completion_with_project("active/main.tua", true)
        .unwrap();
    host.set_file("one/a.tua", "draw(player, changed)");
    assert!(prepared
        .complete(
            SourcePosition {
                line: 1,
                column: 13,
            },
            &InlineCompletionOptions {
                semantic: false,
                current_file: false,
                project: true,
                statistical: false,
            },
            &InlineCompletionBudget::unlimited(),
            &AtomicBool::new(false),
        )
        .is_empty());
}

#[test]
fn prepared_project_query_observes_generation_change_and_clear_without_a_vfs_edit() {
    fn prepared_host() -> (AnalysisHost, PreparedInlineCompletion) {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        host.set_file("one/a.tua", "draw(player, score)\ndraw(player, score)");
        host.set_file("active/main.tua", "draw(player,");
        let active =
            build_project_file_with_required_targets(&host, "active/main.tua", &["one/a.tua"]);
        let outcome = build_project_file_with_required_targets(&host, "one/a.tua", &[]);
        assert_eq!(
            host.adopt_project_pattern_batch(
                1,
                [active, outcome].map(PatternCorpusBatchEntry::File),
            )
            .adopted,
            2
        );
        let prepared = host
            .prepare_local_inline_completion_with_project("active/main.tua", true)
            .unwrap();
        (host, prepared)
    }

    let complete = |prepared: &PreparedInlineCompletion| {
        prepared.complete(
            SourcePosition {
                line: 1,
                column: 13,
            },
            &InlineCompletionOptions {
                semantic: false,
                current_file: false,
                project: true,
                statistical: false,
            },
            &InlineCompletionBudget::unlimited(),
            &AtomicBool::new(false),
        )
    };

    let (host, prepared) = prepared_host();
    assert_eq!(complete(&prepared)[0].insert_text, " score)");
    let mut coordinator_clone = host.clone();
    coordinator_clone.set_project_pattern_eligibility_generation(2);
    assert!(complete(&prepared).is_empty());

    let (host, prepared) = prepared_host();
    assert_eq!(complete(&prepared)[0].insert_text, " score)");
    let mut coordinator_clone = host.clone();
    let retired = coordinator_clone
        .clear_project_pattern_corpus()
        .expect("populated corpus should be retired");
    assert!(complete(&prepared).is_empty());
    std::thread::spawn(move || drop(retired)).join().unwrap();
}

#[test]
fn project_completion_rechecks_the_same_version_epoch_after_validation() {
    use std::cell::{Cell, RefCell};

    let mut host = AnalysisHost::new();
    host.set_project_pattern_eligibility_generation(1);
    host.set_file("one/a.tua", "draw(player, score)\ndraw(player, score)");
    host.set_file("active/main.tua", "if (\ndraw(player,");
    let active = build_project_file_with_required_targets(&host, "active/main.tua", &["one/a.tua"]);
    let outcome = build_project_file_with_required_targets(&host, "one/a.tua", &[]);
    assert_eq!(
        host.adopt_project_pattern_batch(1, [active, outcome].map(PatternCorpusBatchEntry::File),)
            .adopted,
        2
    );
    let prepared = host
        .prepare_local_inline_completion_with_project("active/main.tua", true)
        .unwrap();
    let position = SourcePosition {
        line: 2,
        column: 13,
    };
    let options = InlineCompletionOptions {
        semantic: false,
        current_file: false,
        project: true,
        statistical: false,
    };
    let run = |baseline: &dyn Fn() -> Option<Vec<crate::Diagnostic>>| {
        let project = prepared.project.as_ref().unwrap();
        super::complete(
            super::InlineCompletionInput {
                path: &prepared.path,
                source: &prepared.source,
                kind: prepared.kind,
                position,
                include_ecs: prepared.include_ecs,
                stats: &prepared.stats,
            },
            Some(super::ProjectCompletionInput {
                corpus: &project.corpus,
                versions: &project.versions,
                eligibility: &project.eligibility,
                generation: project.generation,
                eligibility_epoch: project.eligibility_epoch,
                limits: project.limits,
            }),
            &options,
            &InlineCompletionBudget::unlimited(),
            &AtomicBool::new(false),
            None,
            baseline,
        )
    };

    // The invalid source forces validation to consult the baseline after the
    // project lookup. Without a concurrent edit the real candidate survives.
    assert_eq!(run(&|| None)[0].insert_text, " score)");
    let returned_before = prepared.stats.candidates_returned.load(Ordering::Relaxed);

    let host = RefCell::new(host);
    let changed = Cell::new(false);
    let baseline = || {
        changed.set(true);
        host.borrow_mut()
            .set_file("one/a.tua", "draw(player, changed)");
        None
    };
    assert!(run(&baseline).is_empty());
    assert!(
        changed.get(),
        "validation must exercise the final epoch check"
    );
    assert_eq!(
        prepared.stats.candidates_returned.load(Ordering::Relaxed),
        returned_before,
        "stale candidates must not increment returned counters"
    );
}

#[test]
fn oversized_documents_are_refused_before_recall_and_counted() {
    let path = PathBuf::from("src/main.tua");
    let mut source = "print(player.name)\n".repeat(400);
    source.push_str("print(pla");
    assert!(!super::validate::within_parse_cap(
        AnalysisSourceKind::Tua,
        source.len()
    ));
    let mut host = AnalysisHost::new();
    host.set_file_with_kind(&path, &source, AnalysisSourceKind::Tua);
    let candidates = host.local_inline_completion(
        &path,
        SourcePosition {
            line: 401,
            column: 10,
        },
        &InlineCompletionOptions::default(),
        &InlineCompletionBudget::unlimited(),
        &AtomicBool::new(false),
    );

    assert!(candidates.is_empty());
    let stats = host.cache_stats();
    assert_eq!(stats.inline_completion_requests, 1);
    assert_eq!(stats.inline_skipped_oversized, 1);
    assert_eq!(stats.inline_validation_attempts, 0);
    assert_eq!(stats.inline_candidate_parses, 0);
}

#[test]
fn statistical_project_completion_uses_cached_scope_and_rejects_stale_evidence() {
    let mut host = AnalysisHost::new();
    host.set_project_pattern_eligibility_generation(1);
    let training = |name: &str| format!("local {name} = 0\n{}", format!("{name} = 1\n").repeat(40));
    host.set_file("train/a.tua", training("position"));
    host.set_file("train/b.tua", training("velocity"));
    host.set_file("active/main.tua", "local point = 0\npoint =");
    let cold = host
        .prepare_local_inline_completion_with_sources("active/main.tua", false, true)
        .unwrap();
    assert!(cold.statistical_scope.is_none());
    assert_eq!(host.retained_statistical_scope_files(), 0);
    host.set_inline_statistical_enabled(true);
    host.set_file("active/main.tua", "local point = 0\npoint =1");
    let warmed = host.clone();
    assert_eq!(
        host.statistical_scope_storage_ptr(),
        warmed.statistical_scope_storage_ptr(),
        "host clones should share the immutable scope map"
    );
    assert!(warmed.warm_file("active/main.tua", crate::AnalysisWarmPriority::High));
    assert_ne!(
        host.statistical_scope_storage_ptr(),
        warmed.statistical_scope_storage_ptr(),
        "warming should copy only the Arc-backed map before mutation"
    );
    assert!(host.adopt_warmed_file(&warmed, "active/main.tua"));
    assert!(host.retained_statistical_scope_files() >= 1);
    host.set_file("active/main.tua", "local point = 0\npoint =");
    let warmed_invalid = host.clone();
    assert!(warmed_invalid.warm_file("active/main.tua", crate::AnalysisWarmPriority::High));
    assert_eq!(
        warmed_invalid.peek_current_source_parses("active/main.tua"),
        Some(false)
    );
    assert!(host.adopt_warmed_file(&warmed_invalid, "active/main.tua"));
    for path in ["train/a.tua", "train/b.tua"] {
        let outcome = host
            .prepare_project_pattern_file(path)
            .unwrap()
            .build(&ProjectModel::default(), RetrievalLimits::default());
        assert_eq!(
            host.adopt_project_pattern_batch(1, [PatternCorpusBatchEntry::File(outcome)])
                .adopted,
            1
        );
    }
    // The invalid typing source agrees with the last-good source through the
    // cursor, so preparation reuses only compact retained facts and does not
    // build degraded analysis on the hot path.
    let options = InlineCompletionOptions {
        semantic: false,
        current_file: false,
        project: false,
        statistical: true,
    };
    let prepared = host
        .prepare_local_inline_completion_with_sources("active/main.tua", false, true)
        .unwrap();
    let position = SourcePosition { line: 2, column: 8 };
    assert_eq!(
        context_at_end(&prepared.source),
        Some(CursorContext::Assignment)
    );
    assert!(prepared.statistical_scope.is_some());
    let project = prepared.project.as_ref().unwrap();
    let versions = project.versions.begin_read().unwrap();
    let prediction = project
        .corpus
        .query_statistical(
            project.generation,
            &prepared.path,
            &prepared.source,
            prepared.source.len(),
            prepared.statistical_scope.as_ref().unwrap(),
            InlineCompletionBudget::unlimited(),
            &AtomicBool::new(false),
            |path| versions.get(path),
        )
        .unwrap();
    assert_eq!(prediction.text, " 1");
    let candidates = prepared.complete(
        position,
        &options,
        &InlineCompletionBudget::unlimited(),
        &AtomicBool::new(false),
    );
    assert_eq!(candidates.len(), 1, "stats: {:?}", host.cache_stats());
    assert_eq!(candidates[0].insert_text, " 1");
    assert_eq!(
        candidates[0].source,
        InlineCompletionSource::ProjectStatistical
    );
    assert!(host.project_pattern_corpus_stats().approximate_bytes <= 16 * 1024 * 1024);

    let cancelled = AtomicBool::new(true);
    assert!(prepared
        .complete(
            position,
            &options,
            &InlineCompletionBudget::unlimited(),
            &cancelled,
        )
        .is_empty());

    // A later valid edit is warmed and adopted without building a snapshot.
    // The non-ASCII prefix also proves the unchanged-prefix byte boundary is
    // computed on character boundaries before the next incomplete edit.
    host.set_file("active/main.tua", "-- café\nlocal target = 0\ntarget =1");
    let warmed = host.clone();
    assert!(warmed.warm_file("active/main.tua", crate::AnalysisWarmPriority::High));
    assert!(host.adopt_warmed_file(&warmed, "active/main.tua"));
    host.set_file("active/main.tua", "-- café\nlocal target = 0\ntarget =");
    let prepared = host
        .prepare_local_inline_completion_with_sources("active/main.tua", false, true)
        .unwrap();
    let position = SourcePosition { line: 3, column: 9 };
    let refreshed = prepared.complete(
        position,
        &options,
        &InlineCompletionBudget::unlimited(),
        &AtomicBool::new(false),
    );
    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].insert_text, " 1");

    // Invalidating one of the two distinct supporting files leaves only one
    // valid supporter, so the already-prepared request must now abstain.
    host.set_file("train/b.tua", "local changed = true\n");
    assert!(prepared
        .complete(
            position,
            &options,
            &InlineCompletionBudget::unlimited(),
            &AtomicBool::new(false),
        )
        .is_empty());

    host.set_inline_statistical_enabled(false);
    assert_eq!(host.retained_statistical_scope_files(), 0);
    assert!(host
        .prepare_local_inline_completion_with_sources("active/main.tua", false, true)
        .unwrap()
        .statistical_scope
        .is_none());
}

#[test]
fn statistical_scope_facts_have_a_total_file_and_byte_cap() {
    let mut host = AnalysisHost::new();
    host.set_inline_statistical_enabled(true);

    // An actively edited file is the only retention priority. Related
    // dependencies are also scheduled at High priority, so warming every
    // unopened file as High must not make those entries sticky or evict the
    // open document's request-side facts.
    host.set_file("zzz-active.tua", "local disk_version = true\n");
    host.open_document("zzz-active.tua", "local active = 1\n");
    let warmed = host.clone();
    assert!(warmed.warm_file("zzz-active.tua", crate::AnalysisWarmPriority::High));
    assert!(host.adopt_warmed_file(&warmed, "zzz-active.tua"));

    for index in 0..(super::statistical::MAX_SCOPE_FACT_FILES + 16) {
        host.set_file(
            format!("src/file-{index:03}.tua"),
            format!("local value{index} = {index}\n"),
        );
        let path = format!("src/file-{index:03}.tua");
        let warmed = host.clone();
        assert!(warmed.warm_file(&path, crate::AnalysisWarmPriority::High));
        assert!(host.adopt_warmed_file(&warmed, &path));
    }
    let stats = host.cache_stats();
    assert_eq!(
        stats.inline_statistical_scope_files,
        super::statistical::MAX_SCOPE_FACT_FILES
    );
    assert!(
        stats.inline_statistical_scope_bytes <= super::statistical::MAX_SCOPE_FACT_BYTES,
        "{}",
        stats.inline_statistical_scope_bytes
    );

    assert!(host
        .prepare_local_inline_completion_with_sources("zzz-active.tua", false, true)
        .unwrap()
        .statistical_scope
        .is_some());
    let stats = host.cache_stats();
    assert_eq!(
        stats.inline_statistical_scope_files,
        super::statistical::MAX_SCOPE_FACT_FILES
    );
    assert!(stats.inline_statistical_scope_bytes <= super::statistical::MAX_SCOPE_FACT_BYTES);

    host.close_document("zzz-active.tua");
    assert_eq!(
        host.cache_stats().inline_statistical_scope_files,
        super::statistical::MAX_SCOPE_FACT_FILES - 1,
        "closing a document must immediately release facts for its former overlay source"
    );
    assert!(host
        .prepare_local_inline_completion_with_sources("zzz-active.tua", false, true)
        .unwrap()
        .statistical_scope
        .is_none());
}

#[test]
fn statistical_scope_lookup_abstains_above_the_validation_parse_cap() {
    let mut host = AnalysisHost::new();
    host.set_inline_statistical_enabled(true);
    let source = format!("local active = 1\n-- {}", "x".repeat(7_000));
    assert!(!super::validate::within_parse_cap(
        AnalysisSourceKind::Tua,
        source.len()
    ));
    assert!(source.len() < super::statistical::MAX_SOURCE_BYTES);
    host.set_file("active.tua", source);

    // Analysis may retain bounded facts for other consumers, but request
    // preparation must reject the source before comparing it with last-good.
    let _ = host.snapshot();
    assert_eq!(host.cache_stats().inline_statistical_scope_files, 1);
    assert!(host
        .prepare_local_inline_completion_with_sources("active.tua", false, true)
        .unwrap()
        .statistical_scope
        .is_none());
}
