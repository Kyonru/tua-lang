use super::*;

fn line(source: &str) -> Template {
    parse(source, FieldKind::Line).unwrap()
}

fn rejected(source: &str, message: &str) {
    let error = parse(source, FieldKind::Line).unwrap_err();
    assert!(error.message.contains(message), "{source:?}: {error:?}");
    assert!(error.range.start <= error.range.end);
    assert!(error.range.end <= source.len());
    assert!(source.is_char_boundary(error.range.start));
    assert!(source.is_char_boundary(error.range.end));
}

#[test]
fn nested_marks_keep_tokens_and_plain_fallback() {
    let source =
        "Hello [em]%{ name }[/em]! [pause=0.25][style=warning][speed=15]Run[/speed][/style]";
    let result = line(source);
    assert_eq!(result.plain_template, "Hello %{name}! Run");
    assert_eq!(result.tokens[1].kind, TokenKind::Open(Mark::Emphasis));
    assert_eq!(result.tokens[2].kind, TokenKind::Placeholder("name".into()));
    assert_eq!(result.tokens[5].kind, TokenKind::Pause(0.25));
    assert_eq!(result.tokens[7].kind, TokenKind::Open(Mark::Speed(15.0)));
    let mut cursor = 0;
    for token in &result.tokens {
        assert_eq!(token.range.start, cursor);
        assert!(!source[token.range.clone()].is_empty());
        cursor = token.range.end;
    }
    assert_eq!(cursor, source.len());
}

#[test]
fn escaping_never_creates_marks_or_placeholders() {
    let source = r"\[em\] %%{name} %%%{real} 100% \{braces\} \\ \n";
    let result = line(source);
    assert_eq!(
        result.plain_template,
        "[em] %%{name} %%%{real} 100%% {braces} \\ \n"
    );
    assert_eq!(
        result
            .tokens
            .iter()
            .filter(|token| matches!(token.kind, TokenKind::Placeholder(_)))
            .count(),
        1
    );
    assert!(!result.tokens.iter().any(|token| matches!(
        token.kind,
        TokenKind::Open(_) | TokenKind::Close(_) | TokenKind::Pause(_)
    )));
    assert_eq!(line("%%{ name }").plain_template, "%%{ name }");
    let bracket = &result.tokens[0];
    assert_eq!(bracket.range, 0..2);
    assert_eq!(bracket.kind, TokenKind::Text("[".into()));
}

#[test]
fn source_ranges_are_utf8_bytes_not_characters() {
    let source = "é👩‍🚀[em]日本語[/em]";
    let result = line(source);
    assert_eq!(result.tokens[0].range, 0.."é👩‍🚀".len());
    assert_eq!(&source[result.tokens[2].range.clone()], "日本語");
    assert_eq!(result.plain_template, "é👩‍🚀日本語");
    // Grapheme alignment is a resolved-presentation check, not syntax validation.
    assert!(parse("e[em]\u{301}[/em]", FieldKind::Line).is_ok());
}

#[test]
fn pronunciation_quoting_does_not_terminate_a_mark() {
    let result = line(r#"[pronounce="a ] \"b\" \\ c"]harbor[/pronounce]"#);
    assert_eq!(
        result.tokens[0].kind,
        TokenKind::Open(Mark::Pronunciation("a ] \"b\" \\ c".into()))
    );
    for source in [
        r#"[pronounce=""]x[/pronounce]"#,
        r#"[pronounce="%{name}"]x[/pronounce]"#,
        r#"[pronounce="a\n"]x[/pronounce]"#,
        r#"[pronounce="a"b"]x[/pronounce]"#,
        r#"[pronounce="a"][pronounce="b"]x[/pronounce][/pronounce]"#,
    ] {
        assert!(parse(source, FieldKind::Line).is_err(), "{source}");
    }
    assert!(parse(
        &format!("[pronounce=\"{}\"]x[/pronounce]", "a".repeat(256)),
        FieldKind::Line
    )
    .is_ok());
    rejected(
        &format!("[pronounce=\"{}\"]x[/pronounce]", "a".repeat(257)),
        "pronunciation",
    );
}

#[test]
fn nesting_and_empty_regions_fail_at_useful_ranges() {
    rejected("[em][style=hot]x[/em][/style]", "reverse order");
    rejected("[/em]x", "no opening");
    rejected("[em][/em]", "must contain");
    rejected("[em][pause=1][/em]", "must contain");
    rejected("[em]x", "unclosed");
    rejected("x[unknown]", "unknown");
    rejected("[em]x[/EM]", "unknown");
    rejected("[em", "unterminated");
    let error = parse("é [em]x", FieldKind::Line).unwrap_err();
    assert_eq!(error.range, 3..7);
    assert!(parse("[em]%{possibly_empty}[/em]", FieldKind::Line).is_ok());
}

#[test]
fn timing_marks_are_line_only() {
    for field in [FieldKind::Choice, FieldKind::Reason] {
        assert!(parse("[em][style=warning]Hello[/style][/em]", field).is_ok());
        for source in ["[pause=0]x", "[speed=30]x[/speed]"] {
            assert!(parse(source, field)
                .unwrap_err()
                .message
                .contains("only supported in lines"));
        }
    }
}

#[test]
fn decimal_values_are_explicit_and_bounded() {
    for value in [
        "NaN", "inf", "-1", "+1", "1e2", ".5", "1.", "0x10", "1.2.3", " 1", "",
    ] {
        rejected(&format!("[pause={value}]x"), "decimal");
        rejected(&format!("[speed={value}]x[/speed]"), "decimal");
    }
    for source in [
        "[pause=0]x",
        "[pause=60]x",
        "[speed=0.01]x[/speed]",
        "[speed=1000]x[/speed]",
    ] {
        assert!(parse(source, FieldKind::Line).is_ok(), "{source}");
    }
    rejected("[speed=0]x[/speed]", "speed");
    rejected("[speed=1000.1]x[/speed]", "speed");
    rejected("[pause=60.1]x", "pause");
    rejected(&format!("[pause={}]x", "9".repeat(400)), "decimal");
}

#[test]
fn identifiers_and_placeholders_do_not_allow_expressions() {
    for name in ["a.b", "a-b", "1a", "日本", "", "a()", "a b"] {
        rejected(&format!("[style={name}]x[/style]"), "identifier");
        rejected(&format!("%{{{name}}}"), "placeholder");
    }
    assert!(parse("[style=_A1]%{_A1}[/style]", FieldKind::Line).is_ok());
    rejected(
        &format!("[style={}]x[/style]", "a".repeat(65)),
        "identifier",
    );
    rejected("%{unclosed", "unterminated");
    rejected("%{\nname}", "control");
}

#[test]
fn token_depth_byte_and_pause_limits_have_exact_edges() {
    assert!(parse(&"a".repeat(MAX_BYTES), FieldKind::Line).is_ok());
    rejected(&"é".repeat(MAX_BYTES / 2 + 1), "UTF-8 bytes");
    assert!(parse(&"\\[".repeat(MAX_TOKENS), FieldKind::Line).is_ok());
    rejected(&"\\[".repeat(MAX_TOKENS + 1), "tokens");
    let nested = |count| format!("{}x{}", "[em]".repeat(count), "[/em]".repeat(count));
    assert!(parse(&nested(MAX_DEPTH), FieldKind::Line).is_ok());
    rejected(&nested(MAX_DEPTH + 1), "nested");
    assert!(parse(
        &format!("{}x", "[pause=0]".repeat(MAX_PAUSES)),
        FieldKind::Line
    )
    .is_ok());
    rejected(
        &format!("{}x", "[pause=0]".repeat(MAX_PAUSES + 1)),
        "pause count",
    );
    assert!(parse("[pause=60][pause=60]x", FieldKind::Line).is_ok());
    rejected("[pause=60][pause=60][pause=0.01]x", "duration");
    assert!(parse(&format!("{}x", "[pause=1.2]".repeat(100)), FieldKind::Line).is_ok());
}

#[test]
fn malformed_text_controls_and_physical_lines_are_rejected() {
    for source in [
        "x]",
        "x{",
        "x}",
        "x\n",
        "x\r",
        "x\0",
        "\\q",
        "\\",
        "[em\n]x[/em]",
        "[pause=1]",
    ] {
        assert!(parse(source, FieldKind::Line).is_err(), "{source:?}");
    }
    assert!(parse("x\ty", FieldKind::Line).is_ok());
    assert!(parse(r"x\ny", FieldKind::Line).is_ok());
}

#[test]
fn arbitrary_unicode_inputs_terminate_with_valid_ranges() {
    // Deterministic malformed-input coverage, including multibyte boundaries.
    let alphabet = [
        "[",
        "]",
        "\\",
        "\"",
        "%",
        "{",
        "}",
        "é",
        "👩‍🚀",
        "a",
        "\n",
        "=",
    ];
    let mut state = 19_u32;
    for _ in 0..2000 {
        let mut source = String::new();
        for _ in 0..32 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            source.push_str(alphabet[state as usize % alphabet.len()]);
        }
        match parse(&source, FieldKind::Line) {
            Ok(template) => {
                let mut cursor = 0;
                for token in template.tokens {
                    assert_eq!(token.range.start, cursor);
                    assert!(source.get(token.range.clone()).is_some());
                    cursor = token.range.end;
                }
                assert_eq!(cursor, source.len());
            }
            Err(error) => assert!(source.get(error.range).is_some()),
        }
    }
}

#[test]
fn rich_authoring_requires_explicit_opt_in() {
    let plain = "@dialogue 1\n[start]\n// @id test\n: \\[em\\]literal\\[/em\\] (end)\n";
    let parsed = crate::dialogue_asset::parse_dialogue_source("plain.tdialogue", plain).unwrap();
    assert_eq!(parsed.sections[0].lines[0].text.value, "[em]literal[/em]");
    let emitted = crate::emit::emit_dialogue_source("plain.tdialogue", plain).unwrap();
    assert!(!emitted.code.contains("schemaVersion = 8"));
    let rich = plain.replace("@dialogue 1", "@dialogue 1\n@text rich");
    assert!(
        crate::dialogue_asset::parse_dialogue_source("rich.tdialogue", &rich)
            .unwrap()
            .rich_text
    );
    assert!(crate::emit::emit_dialogue_source("rich.tdialogue", &rich)
        .unwrap()
        .code
        .contains("schemaVersion = 8"));
    assert!(crate::dialogue_asset::parse_dialogue_source(
        "plain.tdialogue",
        &plain.replace("\\[", "[").replace("\\]", "]")
    )
    .is_err());
}

#[test]
fn lua_template_parser_matches_shared_rust_contract() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let quote = |value: &str| {
        let bytes = value
            .bytes()
            .map(|byte| format!("\\{byte:03}"))
            .collect::<String>();
        format!("\"{bytes}\"")
    };
    let mut cases = vec![
        "Hi [em]%{name}[/em]".to_owned(),
        r"\[em\] %%{ name } %%%{real}".to_owned(),
        "[style=warning][speed=0.5]é日本語[/speed][/style]".to_owned(),
        r#"[pronounce="a ] \"b\" \\ c"]hello[/pronounce]"#.to_owned(),
        "[pause=0][pause=60]x[pause=60]".to_owned(),
        "[em][style=a]x[/em][/style]".to_owned(),
        "[em][/em]".to_owned(),
        "[speed=.5]x[/speed]".to_owned(),
        "[speed=1e2]x[/speed]".to_owned(),
        "[pause=61]x".to_owned(),
        "%{a.b}".to_owned(),
        "[pronounce=\"%{name}\"]x[/pronounce]".to_owned(),
        "[pronounce=\"a\"][pronounce=\"b\"]x[/pronounce][/pronounce]".to_owned(),
        format!("{}x", "[pause=1.2]".repeat(100)),
        format!("{}x", "[pause=0]".repeat(257)),
        format!("{}x{}", "[em]".repeat(17), "[/em]".repeat(17)),
    ];
    let alphabet = ["[", "]", "\\", "\"", "%", "{", "}", "é", "a", "\n", "="];
    let mut state = 31_u32;
    for _ in 0..200 {
        let mut source = String::new();
        for _ in 0..24 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            source.push_str(alphabet[state as usize % alphabet.len()]);
        }
        cases.push(source);
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut script = "package.path = 'vendor/runtime/?.lua;' .. package.path\nlocal rich = require('tuadialogue_rich')\n".to_owned();
    for field in [FieldKind::Line, FieldKind::Choice, FieldKind::Reason] {
        for (index, source) in cases.iter().enumerate() {
            script.push_str(&format!(
                "do\nlocal ok, result = pcall(rich.parse, {}, {})\n",
                quote(source),
                field == FieldKind::Line
            ));
            match parse(source, field) {
                Ok(template) => {
                    script.push_str(&format!("assert(ok, 'case {index}: ' .. tostring(result))\nassert(result.plain == {})\nassert(#result.tokens == {})\n", quote(&template.plain_template), template.tokens.len()));
                    for (index, token) in template.tokens.iter().enumerate() {
                        script.push_str(&format!("assert(result.tokens[{}].first == {} and result.tokens[{}].last == {})\n", index + 1, token.range.start + 1, index + 1, token.range.end));
                    }
                }
                Err(_) => script.push_str(&format!(
                    "assert(not ok, 'unexpected valid case {index}')\n"
                )),
            }
            script.push_str("end\n");
        }
    }
    let mut ran = false;
    for interpreter in ["lua", "lua5.1", "luajit"] {
        if Command::new(interpreter).arg("-v").output().is_err() {
            continue;
        }
        ran = true;
        let mut child = Command::new(interpreter)
            .arg("-")
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(script.as_bytes())
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{interpreter}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    if !ran {
        eprintln!("SKIP rich parser parity: no standalone Lua interpreter");
    }
}
