use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn dialogue_conditions_emitted_runtime() {
    run_conditions(false);
}

#[test]
fn dialogue_conditions_with_identified_events_emitted_runtime() {
    run_conditions(true);
}

fn run_conditions(identified: bool) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = concat!(
        include_str!("fixtures/dialogue-conditions/lifecycle.lua"),
        "\n",
        include_str!("fixtures/dialogue_history_context.lua"),
        "\n",
        include_str!("fixtures/dialogue_checkpoint_context.lua")
    );
    let context = tua_core::emit::emit_tua_source(
        "context.tua",
        include_str!("../../../examples/dialogue-conditions/src/context.tua"),
    )
    .unwrap();
    let mut scene_source =
        include_str!("../../../examples/dialogue-conditions/src/scene.tdialogue").to_owned();
    if identified {
        scene_source = scene_source.replace("greet ", "greet [id:shop.greet] ");
    }
    let scene = tua_core::emit::emit_dialogue_source("scene.tdialogue", &scene_source)
        .unwrap()
        .code;
    let main = tua_core::emit::emit_tua_source(
        "main.tua",
        include_str!("../../../examples/dialogue-conditions/src/main.tua"),
    )
    .unwrap();
    let emitted = format!("package.preload.context = function()\n{context}\nend\npackage.preload.scene = function()\n{scene}\nend\n{main}");
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
        let script =
            format!("package.path = 'vendor/runtime/?.lua;' .. package.path\n{emitted}\n{source}");
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
        eprintln!("SKIP conditional dialogue runtime: no supported Lua interpreter available");
    }
}

#[test]
fn dialogue_conditions_reject_invalid_syntax_and_targets() {
    let prefix = "@dialogue 1\n@context \"context\"\n[gate]\n";
    for condition in [
        "context.fn()",
        "context.x + 1",
        "true true",
        "true and",
        "not",
        "context..x",
        "1e999",
    ] {
        let source = format!("{prefix}@branch {condition} [target:gate] [else:gate]\n");
        assert!(
            tua_core::emit::emit_dialogue_source("bad.tdialogue", &source).is_err(),
            "{condition}"
        );
    }
    for count in [33, 300] {
        let source = format!(
            "{prefix}@branch {}true [target:gate] [else:gate]\n",
            "not ".repeat(count)
        );
        assert!(tua_core::emit::emit_dialogue_source("bad.tdialogue", &source).is_err());
    }
    let missing = format!("{prefix}@branch true [target:absent] [else:gate]\n");
    let errors = tua_core::emit::emit_dialogue_source("bad.tdialogue", &missing).unwrap_err();
    assert!(errors.iter().any(|d| d.message.contains("unknown target")));
    let source = include_str!("../../../examples/dialogue-conditions/src/scene.tdialogue");
    assert!(tua_core::emit::emit_dialogue_source(
        "bad.tdialogue",
        &source.replace("@fallback locked", "")
    )
    .is_err());
    let emitted = tua_core::emit::emit_dialogue_source("good.tdialogue", source).unwrap();
    assert!(emitted.code.contains("schemaVersion = 3"));
    assert!(emitted.code.contains("reasonId = \"shop.buy.reason\""));
}
