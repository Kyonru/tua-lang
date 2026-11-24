use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn dialogue_actions_execute_emitted_sources() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example = root.join("examples/dialogue-actions/src");
    let mut script = "package.path = 'vendor/runtime/?.lua;' .. package.path\n".to_owned();
    for name in [
        "actions", "forward", "events", "context", "scene", "bound", "wait",
    ] {
        let code = if matches!(name, "scene" | "bound" | "wait") {
            tua_core::emit::emit_dialogue_source(
                format!("{name}.tdialogue"),
                &std::fs::read_to_string(example.join(format!("{name}.tdialogue"))).unwrap(),
            )
            .unwrap()
            .code
        } else {
            tua_core::emit::emit_tua_source(
                format!("{name}.tua"),
                std::fs::read_to_string(example.join(format!("{name}.tua"))).unwrap(),
            )
            .unwrap()
        };
        script.push_str(&format!(
            "package.preload.{name} = function()\n{code}\nend\n"
        ));
    }
    script.push_str(
        &tua_core::emit::emit_tua_source(
            "main.tua",
            std::fs::read_to_string(example.join("main.tua")).unwrap(),
        )
        .unwrap(),
    );
    script.push_str(include_str!("fixtures/dialogue_actions.lua"));
    script.push_str(include_str!("fixtures/dialogue_async_actions.lua"));
    script.push_str(include_str!("fixtures/dialogue_waits.lua"));
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
        eprintln!("SKIP action runtime: no standalone Lua interpreter");
    }
}

#[test]
fn dialogue_actions_reject_invalid_authored_contracts() {
    let source = include_str!("../../../examples/dialogue-actions/src/scene.tdialogue");
    for invalid in [
        source.replace("@document shop\n", ""),
        source.replace("@actions \"forward\"\n", ""),
        source.replace("[id:shop.purchase] ", ""),
        source.replace("[success:done]", "[success:missing]"),
        source.replace("[failure:declined]", "[failure:missing]"),
        source.replace("{\"item\":\"shell\"}", "{\"item\":null}"),
        source.replace("[done]", ": Unexpected. (end)\n[done]"),
    ] {
        assert!(
            tua_core::emit::emit_dialogue_source("bad.tdialogue", &invalid).is_err(),
            "{invalid}"
        );
    }
    let duplicate = source.replace("[done]", "[another]\n@action purchase [id:shop.purchase] [success:done] [failure:declined] {\"item\":\"shell\"}\n[done]");
    assert!(tua_core::emit::emit_dialogue_source("duplicate.tdialogue", &duplicate).is_err());
    let edited = source.replace("Purchased.", "Thanks for your purchase.");
    let emitted = tua_core::emit::emit_dialogue_source("edited.tdialogue", &edited).unwrap();
    assert!(emitted.code.contains("shop.purchase"));
}
