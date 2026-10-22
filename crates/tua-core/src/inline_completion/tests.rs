use super::eligibility::{
    cursor_context, lexical_region_at, strip_trailing_comment, suffix_allows_inline, word_before,
    CursorContext, LexicalRegion,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{
    AnalysisHost, AnalysisSourceKind, InlineCompletionBudget, InlineCompletionOptions,
    InlineCompletionSource, PatternCorpusBatchEntry, PreparedInlineCompletion, ProjectModel,
    RetrievalLimits, SourcePosition,
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
        "active/main.tua",
        "active/sibling.tua",
        "one/a.tua",
        "two/b.tua",
    ]
    .map(|path| {
        PatternCorpusBatchEntry::File(
            host.prepare_project_pattern_file(path)
                .unwrap()
                .build(&ProjectModel::default(), RetrievalLimits::default()),
        )
    });
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
        },
        &InlineCompletionBudget::unlimited(),
        &AtomicBool::new(false),
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].insert_text, " score);");
    assert_eq!(candidates[0].source, InlineCompletionSource::Project);
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
    let outcome = host
        .prepare_project_pattern_file("one/a.tua")
        .unwrap()
        .build(&ProjectModel::default(), RetrievalLimits::default());
    assert_eq!(
        host.adopt_project_pattern_batch(1, [PatternCorpusBatchEntry::File(outcome)])
            .adopted,
        1
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
        let outcome = host
            .prepare_project_pattern_file("one/a.tua")
            .unwrap()
            .build(&ProjectModel::default(), RetrievalLimits::default());
        assert_eq!(
            host.adopt_project_pattern_batch(1, [PatternCorpusBatchEntry::File(outcome)])
                .adopted,
            1
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
    let outcome = host
        .prepare_project_pattern_file("one/a.tua")
        .unwrap()
        .build(&ProjectModel::default(), RetrievalLimits::default());
    assert_eq!(
        host.adopt_project_pattern_batch(1, [PatternCorpusBatchEntry::File(outcome)])
            .adopted,
        1
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
