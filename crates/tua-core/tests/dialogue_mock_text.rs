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
    let mut script = String::from("package.path = 'vendor/runtime/?.lua;' .. package.path\nlocal Text = require('tuadialogue_text')\n");
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
