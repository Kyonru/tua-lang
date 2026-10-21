use super::eligibility::{
    cursor_context, lexical_region_at, strip_trailing_comment, suffix_allows_inline, word_before,
    CursorContext, LexicalRegion,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{
    AnalysisHost, AnalysisSourceKind, InlineCompletionBudget, InlineCompletionOptions,
    InlineCompletionSource, PreparedInlineCompletion, SourcePosition,
};

fn context_at_end(source: &str) -> Option<CursorContext> {
    cursor_context(source, source.len())
}

fn without_inline_stats(mut stats: crate::AnalysisCacheStats) -> crate::AnalysisCacheStats {
    stats.inline_completion_requests = 0;
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
