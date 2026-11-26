use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};
use tua_core::dialogue_asset::{mock::DialogueMockContext, parse_dialogue_source};

fn source(expression: &str) -> String {
    format!("@dialogue 1\n@context \"context\"\n[gate]\n@branch {expression} [target:gate] [else:gate]\n")
}

#[test]
fn dialogue_mock_matches_emitted_runtime_conditions() {
    let facts = json!({"context": {"flag": false, "coins": 20, "bad": 3, "name": "é", "record": {}}, "locals": {"ready": true}});
    let mock = DialogueMockContext::parse(&facts.to_string()).unwrap();
    let mut script = String::from(
        "package.path = 'vendor/runtime/?.lua;' .. package.path\nlocal Conditions = require('tuadialogue.conditions')\npackage.preload.context = function() return {} end\n",
    );
    script.push_str(&format!("local facts = {}\n", lua_value(&facts)));
    let cases = [
        ("true", Ok(true)),
        ("nil", Ok(false)),
        ("not nil", Ok(true)),
        ("context.flag", Ok(false)),
        ("locals.ready and not context.flag", Ok(true)),
        ("context.coins >= 20", Ok(true)),
        ("context.coins < 20", Ok(false)),
        ("context.coins <= 20", Ok(true)),
        ("context.coins > 20", Ok(false)),
        ("context.coins == 20.0", Ok(true)),
        ("context.flag != true", Ok(true)),
        ("context.flag ~= false", Ok(false)),
        ("context.name == \"é\"", Ok(true)),
        ("context.record != nil", Ok(true)),
        ("context.missing.child == nil", Ok(true)),
        ("context.missing > 0", Ok(false)),
        ("true or context.bad.child", Ok(true)),
        ("false and context.bad.child", Ok(false)),
        (
            "context.bad.child",
            Err("condition path needs a plain record"),
        ),
        ("context.coins", Err("condition needs boolean or nil")),
        ("\"\"", Err("condition needs boolean or nil")),
        ("not 0", Err("condition needs boolean or nil")),
        (
            "context.name > \"a\"",
            Err("ordered comparison needs numbers"),
        ),
        (
            "context.coins == \"20\"",
            Err("equality needs matching scalars or nil"),
        ),
        (
            "context.record == context.record",
            Err("equality needs matching scalars or nil"),
        ),
    ];
    for (index, (expression, expected)) in cases.into_iter().enumerate() {
        let source = source(expression);
        let parsed = parse_dialogue_source("mock.tdialogue", &source).unwrap();
        let condition = &parsed.sections[0].branch.as_ref().unwrap().condition;
        let result = mock.test(condition);
        assert_eq!(
            result.as_ref().copied().map_err(|error| error.message),
            expected,
            "{expression}"
        );
        if let Err(issue) = result {
            assert_eq!(issue.span.start.line, 4);
        }
        let emitted = tua_core::emit::emit_dialogue_source("mock.tdialogue", &source).unwrap();
        script.push_str(&format!("do\n  local scene = (function()\n{}\n  end)()\n  local ok, result = pcall(Conditions.test, Conditions.compile(scene.nodes.gate.condition), facts.context, facts.locals)\n", emitted.code));
        let assertion = match expected {
            Ok(value) => format!("ok and result == {value}"),
            Err(message) => format!(
                "not ok and tostring(result):find({}, 1, true)",
                lua_string(message)
            ),
        };
        script.push_str(&format!(
            "  assert({assertion}, 'mock parity case {index}: ' .. tostring(result))\nend\n"
        ));
    }
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
        eprintln!("SKIP mock/runtime parity: no supported Lua interpreter available");
    }
}

#[test]
fn dialogue_mock_rejects_invalid_or_excessive_input_and_reports_numeric_limits() {
    for input in [
        "[]",
        "null",
        "{",
        "{\"other\":{}}",
        "{\"context\":null}",
        "{\"locals\":[]}",
        "{\"context\":{\"x\":[]}}",
        "{\"context\":{\"x\":9007199254740993}}",
    ] {
        assert!(DialogueMockContext::parse(input).is_err(), "{input}");
    }
    assert!(DialogueMockContext::parse(&" ".repeat(65_537)).is_err());
    let many = json!({"context": (0..4096).map(|n| (format!("x{n}"), json!(0))).collect::<serde_json::Map<_, _>>()});
    assert!(DialogueMockContext::parse(&many.to_string()).is_err());
    let mut deep = json!(true);
    for _ in 0..33 {
        deep = json!({"child": deep});
    }
    assert!(DialogueMockContext::parse(&json!({"context": deep}).to_string()).is_err());
    let mock = DialogueMockContext::parse("{}").unwrap();
    for expression in ["context.absent == nil", "locals.absent == nil"] {
        let parsed = parse_dialogue_source("mock.tdialogue", &source(expression)).unwrap();
        assert!(mock
            .test(&parsed.sections[0].branch.as_ref().unwrap().condition)
            .unwrap());
    }
    let parsed = parse_dialogue_source(
        "mock.tdialogue",
        &source("9007199254740993 == 9007199254740992"),
    )
    .unwrap();
    assert!(mock
        .test(&parsed.sections[0].branch.as_ref().unwrap().condition)
        .unwrap_err()
        .message
        .contains("safe numeric range"));
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

fn lua_value(value: &Value) -> String {
    match value {
        Value::Null => "nil".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => lua_string(value),
        Value::Object(fields) => format!(
            "{{{}}}",
            fields
                .iter()
                .map(|(key, value)| format!("[{}] = {}", lua_string(key), lua_value(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Array(_) => panic!("mock facts do not allow arrays"),
    }
}

#[test]
fn dialogue_mock_bounds_manually_constructed_conditions() {
    use tua_core::dialogue_asset::{Condition, ConditionKind, DialogueLiteral};
    let parsed = parse_dialogue_source("mock.tdialogue", &source("true")).unwrap();
    let base = parsed.sections[0]
        .branch
        .as_ref()
        .unwrap()
        .condition
        .clone();
    let mock = DialogueMockContext::parse("{}").unwrap();
    let mut deep = base.clone();
    for _ in 0..33 {
        deep = Condition {
            kind: ConditionKind::Not(Box::new(deep)),
            span: base.span,
        };
    }
    assert_eq!(
        mock.test(&deep).unwrap_err().message,
        "mock condition evaluation budget exceeded"
    );
    let mut wide = Condition {
        kind: ConditionKind::Value(DialogueLiteral::Boolean(true)),
        span: base.span,
    };
    for _ in 0..8 {
        wide = Condition {
            kind: ConditionKind::Binary("and".into(), Box::new(wide.clone()), Box::new(wide)),
            span: base.span,
        };
    }
    assert_eq!(
        mock.test(&wide).unwrap_err().message,
        "mock condition evaluation budget exceeded"
    );
    let exact_bytes = format!("{{}}{}", " ".repeat(65_534));
    assert!(DialogueMockContext::parse(&exact_bytes).is_ok());
    let exact_nodes = json!({"context": (0..4094).map(|n| (format!("x{n}"), json!(0))).collect::<serde_json::Map<_, _>>()});
    assert!(DialogueMockContext::parse(&exact_nodes.to_string()).is_ok());
}

#[test]
fn dialogue_mock_preview_probes_conditions_independently_with_ownership() {
    use tua_core::analysis::AnalysisHost;
    use tua_core::dialogue_asset::preview::DialogueMockConditionKind;
    let source = "@dialogue 1\n@context \"context\"\n[gate]\n@branch context.ready [target:shop] [else:shop]\n[shop]\n// @id prompt\n: Choose\n// @id choice\n-> Buy [target:gate]\n@visible false\n@enabled context.coins > 0\n@fallback gate\n";
    let mut host = AnalysisHost::new();
    host.set_file("scene.tdialogue", source);
    let preview = host.snapshot().dialogue_preview("scene.tdialogue").unwrap();
    let facts =
        DialogueMockContext::parse(r#"{"context":{"ready":true,"coins":"invalid"}}"#).unwrap();
    let result = preview.evaluate_conditions(&facts);
    assert!(!result.truncated);
    assert_eq!(result.conditions.len(), 3);
    assert_eq!(result.conditions[0].kind, DialogueMockConditionKind::Branch);
    assert_eq!(result.conditions[0].result, Ok(true));
    assert_eq!(result.conditions[1].result, Ok(false));
    assert_eq!(result.conditions[2].section, 1);
    assert_eq!(result.conditions[2].line, Some(0));
    assert_eq!(result.conditions[2].choice, Some(0));
    assert_eq!(
        result.conditions[2].kind,
        DialogueMockConditionKind::Availability
    );
    assert_eq!(
        result.conditions[2]
            .result
            .as_ref()
            .unwrap_err()
            .span
            .start
            .line,
        11
    );
    assert_eq!(
        preview.routes.len(),
        4,
        "probing must not prune authored routes"
    );
}

#[test]
fn dialogue_mock_preview_limits_evaluation_count() {
    use std::sync::Arc;
    use tua_core::dialogue_asset::preview::DialoguePreview;
    let mut document = parse_dialogue_source("scene.tdialogue", &source("true")).unwrap();
    document.sections = vec![document.sections[0].clone(); 4097];
    let preview = DialoguePreview {
        characters: Default::default(),
        document: Arc::new(document),
        routes: Vec::new(),
    };
    let result = preview.evaluate_conditions(&DialogueMockContext::parse("{}").unwrap());
    assert_eq!(result.conditions.len(), 4096);
    assert!(result.truncated);
}
