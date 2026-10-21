use super::eligibility::{
    cursor_context, lexical_region_at, strip_trailing_comment, suffix_allows_inline, word_before,
    CursorContext, LexicalRegion,
};

fn context_at_end(source: &str) -> Option<CursorContext> {
    cursor_context(source, source.len())
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
