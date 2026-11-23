use std::io::Write;
use std::process::{Command, Stdio};
use tua_core::emit::{emit_dialogue_source, LoweringKind};

const SCENE: &str = include_str!("../../../examples/dialogue/src/intro.tdialogue");

#[test]
fn emitted_asset_is_lua51_data_with_complete_source_provenance() {
    let output = emit_dialogue_source("intro.tdialogue", SCENE).unwrap();
    tua_core::parse_lua_source("intro.lua", &output.code).unwrap();
    assert_eq!(output.provenance.len(), output.code.lines().count());
    for (index, origin) in output.provenance.iter().enumerate() {
        assert_eq!(origin.generated_line, index + 1);
        assert_eq!(origin.lowering, LoweringKind::Dialogue);
        assert!(origin.source_span.start.line <= SCENE.lines().count());
    }
    let origin = output
        .provenance
        .iter()
        .find(|origin| {
            output
                .code
                .lines()
                .nth(origin.generated_line - 1)
                .unwrap()
                .contains("Ready?")
        })
        .unwrap();
    assert_eq!(origin.source_span.start.line, 6);
    assert_eq!(origin.source_span.start.column, 8);
    assert!(!output.code.contains("require"));
    assert_eq!(
        output,
        emit_dialogue_source("intro.tdialogue", SCENE).unwrap()
    );
}

#[test]
fn emitted_assets_drive_the_actual_headless_runtime_and_do_not_execute_text() {
    // Runtime availability is explicit; the frontend tests above always run.
    if Command::new("lua").arg("-v").output().is_err() {
        eprintln!("SKIP dialogue runtime: standalone Lua is unavailable");
        return;
    }
    let output = emit_dialogue_source("intro.tdialogue", SCENE).unwrap();
    let escaped = emit_dialogue_source(
        "literal.tdialogue",
        r#"@dialogue 1
[start]
// @id literal
: "); error('injected'); -- (end)
"#,
    )
    .unwrap();
    let script = format!(
        r#"
local Dialogue = (function()
{}
end)()
local document = (function()
{}
end)()
local player = Dialogue.new(document)
player:start()
assert(player:snapshot().node.text == "Ready?")
assert(player:snapshot().node.speaker == "Guide")
assert(player:advance())
assert(player:choose("intro.again"))
assert(player:snapshot().nodeId == "start")
assert(player:advance())
assert(player:choose("intro.yes"))
assert(player:snapshot().node.text == "Goodbye.")
assert(player:advance())
assert(player:snapshot().state == "completed")
player:start("done")
assert(player:snapshot().nodeId == "done")
player:dispose()
local literal = (function()
{}
end)()
assert(literal.nodes.start.text == [["); error('injected'); --]])
print("dialogue asset runtime passed")
"#,
        include_str!("../../../vendor/runtime/tuadialogue.lua"),
        output.code,
        escaped.code
    );
    let mut child = Command::new("lua")
        .arg("-")
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
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        "dialogue asset runtime passed"
    );
}

#[test]
fn invalid_documents_never_produce_partial_output() {
    assert!(emit_dialogue_source(
        "bad.tdialogue",
        &SCENE.replace("target:done", "target:unknown")
    )
    .is_err());
}

#[test]
fn consecutive_lines_use_stable_internal_ids_without_label_collisions() {
    let source = "@dialogue 1\n[start]\n// @id first\n: One\n// @id second\n: Two (end)\n";
    let output = emit_dialogue_source("sequence.tdialogue", source).unwrap();
    assert!(output.code.contains("next = \"@line:second\""));
    assert!(output.code.contains("[\"@line:second\"] = {"));
    assert!(output.code.contains("next = \"@end\""));
    full_moon::parse_fallible(&output.code, full_moon::ast::LuaVersion::lua51())
        .into_result()
        .unwrap();
}

#[test]
fn shared_character_emission_preserves_import_and_speaker_provenance() {
    let source = "@dialogue 1\n@characters Cast \"characters\"\n[start]\n// @id shared\n  Cast.Guide: Hello (end)\n";
    let output = emit_dialogue_source("shared.tdialogue", source).unwrap();
    tua_core::parse_lua_source("shared.lua", &output.code).unwrap();
    assert_eq!(output.provenance.len(), output.code.lines().count());
    for (index, line) in output.code.lines().enumerate() {
        let origin = &output.provenance[index];
        if line.contains("require(") {
            assert_eq!(origin.source_span.start.line, 2);
        }
        if line.contains("speaker =") {
            assert_eq!(origin.source_span.start.line, 5);
            assert_eq!(origin.source_span.start.column, 3);
            assert!(line.contains("[\"Guide\"].name"));
        }
    }
}

#[test]
fn dialogue_timing_rejects_invalid_metadata() {
    let prefix = "@dialogue 1\n[start]\n// @id line\n: Hello (end)\n";
    for invalid in [
        "@event later 0 pulse",
        "@event shown -1 pulse",
        "@event shown NaN pulse",
        "@event revealed inf pulse",
        "@event shown 0 pulse unquoted",
        "@timing hold -1 fade 0",
        "@timing hold 1 fade 0\n@timing hold 2 fade 1",
    ] {
        assert!(
            emit_dialogue_source("bad.tdialogue", &format!("{prefix}{invalid}\n")).is_err(),
            "{invalid}"
        );
    }
    assert!(emit_dialogue_source(
        "bad.tdialogue",
        "@dialogue 1\n@event shown 0 pulse\n[start]\n// @id a\n: Hi (end)\n"
    )
    .is_err());
    assert!(emit_dialogue_source(
        "bad.tdialogue",
        &format!("{prefix}{}", "@event shown 0 pulse\n".repeat(65))
    )
    .is_err());
}

#[test]
fn dialogue_portraits_and_both_event_clocks_execute_in_emitted_lua() {
    let source = r#"@dialogue 1
@characters Cast "characters"
[start]
// @id timed
Cast.Guide: Hello (end)
@timing hold 2 fade 0.5
@event shown 0.2 pulse "hello\nworld"
@event revealed 0.25 ready
@event revealed 0.25 tie
"#;
    let output = emit_dialogue_source("timed.tdialogue", source).unwrap();
    full_moon::parse_fallible(&output.code, full_moon::ast::LuaVersion::lua51())
        .into_result()
        .unwrap();
    assert_eq!(output.provenance.len(), output.code.lines().count());
    let script = format!("local Dialogue = (function()\n{}\nend)()\npackage.preload.characters = function() return {{ Guide = {{ name = 'Guide', portrait = 'guide.png' }} }} end\nlocal document = (function()\n{}\nend)()\n{}",
        include_str!("../../../vendor/runtime/tuadialogue.lua"), output.code,
        include_str!("fixtures/dialogue_timed_events.lua"));
    let mut checked = 0;
    for interpreter in ["lua", "luajit"] {
        if Command::new(interpreter).arg("-v").output().is_err() {
            continue;
        }
        let mut child = Command::new(interpreter)
            .arg("-")
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
        checked += 1;
    }
    if checked == 0 {
        eprintln!("SKIP: no standalone Lua interpreter");
    }
}
