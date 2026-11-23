use super::*;

const SCENE: &str = "@dialogue 1\n@character Guide \"Guide\"\n[start]\n// @id intro.hello\nGuide: Ready?\n// @id intro.yes\n-> Yes [target:done]\n// @id intro.again\n-> Again [target:start]\n[done]\n// @id intro.bye\n: Goodbye. (end)\n";

#[test]
fn parses_forward_choices_cycles_and_narration_with_stable_ids() {
    let document = parse_dialogue_source("scene.tdialogue", SCENE).unwrap();
    assert_eq!(document.sections[0].label.value, "start");
    let line = &document.sections[0].lines[0];
    assert_eq!(line.text.id.value, "intro.hello");
    assert_eq!(line.choices[0].text.id.value, "intro.yes");
    assert_eq!(line.choices[0].target.value, "done");
    assert_eq!(line.choices[1].target.value, "start");
    assert!(document.sections[1].lines[0].end);
    assert!(document.sections[1].lines[0].speaker.is_none());
    assert_eq!(line.text.span, span(5, 8, 6));
    assert_eq!(line.choices[0].target.span, span(7, 16, 4));
    assert_eq!(line.text.id.span, span(4, 8, 11));
}

#[test]
fn preserves_unicode_whitespace_and_explicit_breaks_with_crlf() {
    let source = "@dialogue 1\r\n[start]\r\n// @id welcome\r\n  :  é🦆  \\nnext\\tline (end)\r\n";
    let document = parse_dialogue_source("scene.tdialogue", source).unwrap();
    let text = &document.sections[0].lines[0].text;
    assert_eq!(text.value, " é🦆  \nnext\tline");
    assert_eq!(text.span.start, SourcePosition { line: 4, column: 5 });
    assert_eq!(text.span.end.column, 22);
}

#[test]
fn escaping_is_literal_and_never_interpreted_as_markup_or_ending() {
    let source = r#"@dialogue 1
@character Guide "A \"quoted\" guide"
[start]
// @id escaped
Guide: \$\{name\} \{wave\} \\ \(end)
// @id final
: Done (end)
"#;
    let document = parse_dialogue_source("scene.tdialogue", source).unwrap();
    assert_eq!(document.speakers[0].name, "A \"quoted\" guide");
    assert_eq!(
        document.sections[0].lines[0].text.value,
        "${name} {wave} \\ (end)"
    );
    assert!(!document.sections[0].lines[0].end);
}

fn errors(source: &str) -> Vec<Diagnostic> {
    let diagnostics = parse_dialogue_source("bad.tdialogue", source).unwrap_err();
    assert!(diagnostics
        .iter()
        .all(|d| d.code == Some(DiagnosticCode::DOMAIN_DIALOGUE)));
    diagnostics
}

#[test]
fn rejects_duplicate_ids_labels_speakers_and_unknown_references() {
    for (from, to, expected) in [
        ("intro.yes", "intro.hello", "duplicate text ID"),
        ("[done]", "[start]", "duplicate label"),
        ("Guide: Ready?", "Nobody: Ready?", "unknown speaker"),
        ("[target:done]", "[target:missing]", "unknown target"),
        (
            "[start]",
            "@character Guide \"Other\"\n[start]",
            "duplicate speaker",
        ),
    ] {
        assert!(
            errors(&SCENE.replace(from, to))
                .iter()
                .any(|d| d.message.contains(expected)),
            "{expected}"
        );
    }
    let diagnostics = errors(&SCENE.replace("[target:done]", "[target:missing]"));
    let missing = diagnostics
        .iter()
        .find(|d| d.message.contains("unknown target"))
        .unwrap();
    assert_eq!(
        (missing.line, missing.column, missing.end_column),
        (7, 16, 23)
    );
}

#[test]
fn malformed_or_unimplemented_syntax_is_not_silently_accepted() {
    for source in [
        "",
        "@dialogue 2\n[start]",
        "[start]\n// @id x\n: Hi (end)",
        "@dialogue 1\n[start]",
        "@dialogue 1\n// @id orphan",
        "@dialogue 1\n[start]\n: No ID (end)",
        "@dialogue 1\n[start]\n// @id x\n: No ending",
        "@dialogue 1\n[start]\n// @id x\n-> Orphan [target:start]",
        "@dialogue 1\n[start]\n// @id x\n: ${name} (end)",
        "@dialogue 1\n[start]\n// @id x\n: {wave}text{/wave} (end)",
        "@dialogue 1\n[start]\n[signal: go]",
        "@dialogue 1\n[start]\n$ x = 1",
        "@dialogue 1\n[start]\n// @id x\n: Choose\n// @id y\n-> Hidden [if: flag] [target:start]",
        "@dialogue 1\n[else]\n// @id x\n: Done (end)",
        "@dialogue 1\n[start]\n// @id x\n: bad\\q (end)",
        "@dialogue 1\n[start]\n// @id x\n: bad\\",
        "@dialogue 1\n[start]\n// @id x\n: Done (end)\n// @id y\n: Unreachable (end)",
        "@dialogue 1\n[start]\n// @id x\n: Done (end)\n// @id y\n-> Wrong [target:start]",
    ] {
        assert!(!errors(source).is_empty(), "{source}");
    }
}

#[test]
fn pending_ids_do_not_cross_structural_boundaries() {
    for middle in ["// @id second", "[other]", "@character Other \"Other\""] {
        let source = format!("@dialogue 1\n[start]\n// @id first\n{middle}\n: Done (end)");
        assert!(errors(&source)
            .iter()
            .any(|d| d.message.contains("must precede")));
    }
}

#[test]
fn enforces_shared_input_limit() {
    assert!(
        errors(&" ".repeat(crate::input_limits::MAX_SOURCE_BYTES + 1))[0]
            .message
            .contains("limit")
    );
}

proptest::proptest! {
    #[test]
    fn arbitrary_unicode_never_panics(source in ".{0,2048}") {
        let _ = parse_dialogue_source("random.tdialogue", &source);
    }
}
