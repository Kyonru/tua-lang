use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn dialogue_context_runtime_lifecycle() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = include_str!("fixtures/dialogue-context/lifecycle.lua");
    let context = tua_core::emit::emit_tua_source(
        "context.tua",
        include_str!("../../../examples/dialogue/context/src/context.tua"),
    )
    .unwrap();
    let scene = tua_core::emit::emit_dialogue_source(
        "scene.tdialogue",
        include_str!("../../../examples/dialogue/context/src/scene.tdialogue"),
    )
    .unwrap()
    .code;
    let main = tua_core::emit::emit_tua_source(
        "main.tua",
        include_str!("../../../examples/dialogue/context/src/main.tua"),
    )
    .unwrap();
    let emitted = format!("package.preload.story = function()\n{context}\nend\npackage.preload.scene = function()\n{scene}\nend\n{main}");
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
            format!("package.path = 'vendor/runtime/?.lua;' .. package.path\n{source}\n{emitted}");
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
        eprintln!("SKIP context runtime: no supported Lua interpreter available");
    }
}

#[test]
fn dialogue_context_parser_emits_data_and_rejects_executable_bindings() {
    let prefix = "@dialogue 1\n@context \"story\"\n";
    let suffix = "[start]\n// @id hello\n: Hello %{name} (end)\n";
    for declaration in [
        "@bind name context.player.name()\n",
        "@bind name context.name default run()\n",
        "@bind name context.name default 1e999\n",
        "@bind name context.name\n@bind name locals.name\n",
        "@context \"other\"\n",
    ] {
        let result = tua_core::emit::emit_dialogue_source(
            "test.tdialogue",
            &format!("{prefix}{declaration}{suffix}"),
        );
        assert!(result.is_err(), "{declaration}");
    }
    let source = format!("{prefix}@bind name context.name default false\n{suffix}");
    let emitted = tua_core::emit::emit_dialogue_source("test.tdialogue", &source).unwrap();
    assert!(emitted.code.contains("schemaVersion = 2"));
    assert!(emitted.code.contains("fallback = false"));
    assert!(emitted.code.contains("require(\"story\")"));
}
