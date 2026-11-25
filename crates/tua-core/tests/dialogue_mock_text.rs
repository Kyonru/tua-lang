use serde_json::json;
use std::io::Write;
use std::process::{Command, Stdio};
use tua_core::dialogue_asset::{
    mock::DialogueMockContext, mock_text::DialogueMockTextArguments, parse_dialogue_source,
};

fn plain_source(text: &str) -> String {
    format!("@dialogue 1\n[start]\n// @id greeting\n: {text} (end)\n")
}

#[test]
fn mock_text_resolves_defaults_false_and_binding_precedence() {
    let source = "@dialogue 1\n@context \"context\"\n@bind name context.name default \"friend\"\n@bind ready locals.ready default true\n[start]\n// @id greeting\n: Hello %{name}: %{ready}, %{coins}. (end)\n";
    let document = parse_dialogue_source("scene.tdialogue", source).unwrap();
    let facts = DialogueMockContext::parse(r#"{"context":{"name":null},"locals":{"ready":false},"args":{"name":"ignored","ready":true,"coins":20}}"#).unwrap();
    let args = DialogueMockTextArguments::resolve(&document, &facts).unwrap();
    assert_eq!(
        args.format(&document.sections[0].lines[0].text).unwrap(),
        "Hello friend: false, 20."
    );
    let facts = DialogueMockContext::parse(r#"{"context":{"name":{}}}"#).unwrap();
    let error = DialogueMockTextArguments::resolve(&document, &facts).unwrap_err();
    assert_eq!(error.span.start.line, 3);
    assert!(error.message.contains("scalar"));
    let nested = source.replace("context.name", "context.name.first");
    let document = parse_dialogue_source("scene.tdialogue", &nested).unwrap();
    let facts = DialogueMockContext::parse(r#"{"context":{"name":"wrong"}}"#).unwrap();
    assert_eq!(
        DialogueMockTextArguments::resolve(&document, &facts)
            .unwrap_err()
            .message,
        "binding needs a plain record"
    );
    let required = source
        .replace(" default \"friend\"", "")
        .replace("%{name}", "literal");
    let document = parse_dialogue_source("scene.tdialogue", &required).unwrap();
    assert_eq!(
        DialogueMockTextArguments::resolve(&document, &DialogueMockContext::parse("{}").unwrap())
            .unwrap_err()
            .message,
        "missing dialogue binding"
    );
}

#[test]
fn mock_text_uses_emitted_plain_templates_and_never_reinterprets_values() {
    let cases = [
        (plain_source("Hello %{name}: %%{name}; %{flag}."), "Hello é %{flag} [em]literal[/em]: %{name}; false."),
        ("@dialogue 1\n@text rich\n[start]\n// @id greeting\n: [em]Hello %{name}[/em][pause=0.2] (end)\n".into(), "Hello é %{flag} [em]literal[/em]"),
    ];
    let facts = DialogueMockContext::parse(
        r#"{"args":{"name":"é %{flag} [em]literal[/em]","flag":false}}"#,
    )
    .unwrap();
    let mut script = String::from("package.path = 'vendor/runtime/?.lua;' .. package.path\nlocal Text = require('tuadialogue.text')\n");
    for (source, expected) in cases {
        let document = parse_dialogue_source("scene.tdialogue", &source).unwrap();
        let args = DialogueMockTextArguments::resolve(&document, &facts).unwrap();
        assert_eq!(
            args.format(&document.sections[0].lines[0].text).unwrap(),
            expected
        );
        let emitted = tua_core::emit::emit_dialogue_source("scene.tdialogue", &source).unwrap();
        script.push_str(&format!("do\n  local scene = (function()\n{}\n  end)()\n  local args = {{ name = {}, flag = false }}\n  assert(Text.format(scene.nodes.start.text, args) == {})\nend\n", emitted.code, lua_string("é %{flag} [em]literal[/em]"), lua_string(expected)));
    }
    run_lua(&script);
}

#[test]
fn mock_text_checks_arguments_and_output_limits() {
    for input in [
        r#"{"args":{"x":null}}"#,
        r#"{"args":{"x":{}}}"#,
        r#"{"args":{"x":[]}}"#,
    ] {
        assert!(DialogueMockContext::parse(input).is_err());
    }
    let document = parse_dialogue_source("scene.tdialogue", &plain_source("%{missing}")).unwrap();
    let args =
        DialogueMockTextArguments::resolve(&document, &DialogueMockContext::parse("{}").unwrap())
            .unwrap();
    let error = args
        .format(&document.sections[0].lines[0].text)
        .unwrap_err();
    assert_eq!(error.span.start.line, 4);
    assert_eq!(error.message, "missing or invalid dialogue argument");
    let document =
        parse_dialogue_source("scene.tdialogue", &plain_source("%{large}%{large}%{large}"))
            .unwrap();
    let facts =
        DialogueMockContext::parse(&json!({"args":{"large":"x".repeat(30_000)}}).to_string())
            .unwrap();
    let args = DialogueMockTextArguments::resolve(&document, &facts).unwrap();
    assert_eq!(
        args.format(&document.sections[0].lines[0].text)
            .unwrap_err()
            .message,
        "mock text output exceeds 64 KiB"
    );
    // Short exponent input can expand significantly under deterministic decimal formatting.
    let args = (0..300)
        .map(|index| (format!("x{index}"), json!(1e-300)))
        .collect::<serde_json::Map<_, _>>();
    let facts = DialogueMockContext::parse(&json!({"args":args}).to_string()).unwrap();
    assert_eq!(
        DialogueMockTextArguments::resolve(&document, &facts)
            .unwrap_err()
            .message,
        "mock argument text exceeds 64 KiB"
    );
}

fn lua_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .bytes()
            .map(|byte| format!("\\{byte:03}"))
            .collect::<String>()
    )
}

fn run_lua(script: &str) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
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
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{interpreter}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    if !ran {
        eprintln!("SKIP mock text runtime parity: no supported Lua interpreter available");
    }
}

#[test]
fn mock_text_handles_literal_escape_and_exact_output_boundary() {
    let document = parse_dialogue_source("scene.tdialogue", &plain_source("%%{missing}")).unwrap();
    let args =
        DialogueMockTextArguments::resolve(&document, &DialogueMockContext::parse("{}").unwrap())
            .unwrap();
    let mut text = document.sections[0].lines[0].text.clone();
    assert_eq!(args.format(&text).unwrap(), "%{missing}");
    text.value = "x".repeat(65_536);
    assert_eq!(args.format(&text).unwrap().len(), 65_536);
    text.value.push('x');
    assert_eq!(
        args.format(&text).unwrap_err().message,
        "mock text output exceeds 64 KiB"
    );
    let source = format!("@dialogue 1\n@context \"context\"\n@bind large context.large default \"{}\"\n[start]\n// @id hello\n: Hello (end)\n", "x".repeat(65_537));
    let document = parse_dialogue_source("scene.tdialogue", &source).unwrap();
    assert_eq!(
        DialogueMockTextArguments::resolve(&document, &DialogueMockContext::parse("{}").unwrap())
            .unwrap_err()
            .message,
        "mock argument text exceeds 64 KiB"
    );
}

#[test]
fn mock_text_preview_maps_lines_choices_reasons_and_errors() {
    use tua_core::analysis::AnalysisHost;
    use tua_core::dialogue_asset::preview::DialogueMockTextKind;
    let source = "@dialogue 1\n@context \"context\"\n@bind name context.name\n[start]\n// @id hello\n: Hello %{name}\n// @id buy\n-> Buy %{item} [target:done]\n@enabled false\n// @id reason\n@reason Need %{coins}\n@fallback done\n[done]\n// @id bye\n: Bye (end)\n";
    let mut host = AnalysisHost::new();
    host.set_file("scene.tdialogue", source);
    let preview = host.snapshot().dialogue_preview("scene.tdialogue").unwrap();
    let facts = DialogueMockContext::parse(
        r#"{"context":{"name":"Ada"},"args":{"item":"shell","coins":20}}"#,
    )
    .unwrap();
    let result = preview.format_texts(&facts);
    assert!(result.binding_error.is_none());
    assert!(!result.truncated);
    assert_eq!(result.texts.len(), 4);
    assert_eq!(result.texts[0].result.as_ref().unwrap(), "Hello Ada");
    assert_eq!(result.texts[1].kind, DialogueMockTextKind::Choice);
    assert_eq!(result.texts[1].choice, Some(0));
    assert_eq!(result.texts[1].result.as_ref().unwrap(), "Buy shell");
    assert_eq!(result.texts[2].kind, DialogueMockTextKind::Reason);
    assert_eq!(result.texts[2].result.as_ref().unwrap(), "Need 20");
    let missing = DialogueMockContext::parse(r#"{"context":{"name":"Ada"}}"#).unwrap();
    let result = preview.format_texts(&missing);
    assert!(result.texts[0].result.is_ok());
    assert!(result.texts[1].result.is_err());
    assert_eq!(result.texts[1].span.start.line, 8);
    let result = preview.format_texts(&DialogueMockContext::parse("{}").unwrap());
    assert_eq!(result.binding_error.unwrap().span.start.line, 3);
    assert!(result.texts.is_empty());
}

#[test]
fn mock_text_preview_caps_combined_bytes_and_field_count() {
    use std::sync::Arc;
    use tua_core::dialogue_asset::preview::DialoguePreview;
    let mut document = parse_dialogue_source("scene.tdialogue", &plain_source("x")).unwrap();
    document.sections = vec![document.sections[0].clone(); 4097];
    let preview = DialoguePreview {
        document: Arc::new(document.clone()),
        routes: Vec::new(),
    };
    let facts = DialogueMockContext::parse("{}").unwrap();
    let result = preview.format_texts(&facts);
    assert_eq!(result.texts.len(), 4096);
    assert!(result.truncated);
    document.sections.truncate(17);
    for section in &mut document.sections {
        section.lines[0].text.value = "x".repeat(65_536);
    }
    let preview = DialoguePreview {
        document: Arc::new(document),
        routes: Vec::new(),
    };
    let result = preview.format_texts(&facts);
    assert_eq!(result.texts.len(), 16);
    assert_eq!(
        result
            .texts
            .iter()
            .map(|text| text.result.as_ref().unwrap().len())
            .sum::<usize>(),
        1024 * 1024
    );
    assert!(result.truncated);
}
