use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn dialogue_typed_events_execute_emitted_sources() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example = root.join("examples/dialogue-events/src");
    let mut script = "package.path = 'vendor/runtime/?.lua;' .. package.path\n".to_owned();
    for name in ["events", "forward", "context", "scene", "bound"] {
        let code = if matches!(name, "scene" | "bound") {
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
    script.push_str(include_str!("fixtures/dialogue_typed_events.lua"));
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
        eprintln!("SKIP typed event runtime: no standalone Lua interpreter");
    }
}

#[test]
fn dialogue_typed_events_reject_invalid_payload_syntax_and_bounds() {
    let prefix = "@dialogue 1\n@document x\n@events \"events\"\n[start]\n// @id hello\n: Hello (end)\n@emit shown 0 reward [id:x.reward] ";
    for payload in [
        "{\"x\":1,\"x\":2}",
        "{\"x\":{\"y\":1,\"y\":2}}",
        "[]",
        "null",
        "{\"x\":null}",
        "{x: 1}",
        "{\"x\":call()}",
        "{\"x\":1e999}",
    ] {
        assert!(
            tua_core::emit::emit_dialogue_source("bad.tdialogue", &format!("{prefix}{payload}\n"))
                .is_err(),
            "{payload}"
        );
    }
    for payload in [
        format!("{{\"x\":\"{}\"}}", "a".repeat(8192)),
        format!("{{\"x\":[{}]}}", vec!["0"; 256].join(",")),
        format!("{}0{}", "{\"x\":".repeat(17), "}".repeat(17)),
    ] {
        assert!(tua_core::emit::emit_dialogue_source(
            "bad.tdialogue",
            &format!("{prefix}{payload}\n")
        )
        .is_err());
    }
    for source in [
        prefix.replace("@events \"events\"\n", ""),
        prefix.replace("[id:x.reward] ", ""),
        prefix.replace("@document x\n", ""),
    ] {
        assert!(
            tua_core::emit::emit_dialogue_source("bad.tdialogue", &format!("{source}{{}}\n"))
                .is_err()
        );
    }
}
