use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn dialogue_expressions_execute_emitted_sources() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example = root.join("examples/dialogue-expressions/src");
    let mut script = "package.path = 'vendor/runtime/?.lua;' .. package.path\n".to_owned();
    for name in ["characters", "forward", "scene"] {
        let code = if name == "scene" {
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
    script.push_str(include_str!("fixtures/dialogue_expressions.lua"));
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
        eprintln!("SKIP expression runtime: no standalone Lua interpreter");
    }
}

#[test]
fn dialogue_expressions_reject_invalid_placement() {
    let local_speaker = "@dialogue 1\n@character Cast.Guide \"Local\"\n[start]\n// @id hello\nCast.Guide: Hello (end)\n@expression happy\n";
    assert!(tua_core::emit::emit_dialogue_source("local.tdialogue", local_speaker).is_err());
    let source = include_str!("../../../examples/dialogue-expressions/src/scene.tdialogue");
    for invalid in [
        source.replace(
            "@expression happy",
            "@expression happy\n@expression talking",
        ),
        source.replace("Cast.Guide: Welcome!", ": Welcome!"),
        source
            .replace("Cast.Guide: Welcome!", "Local: Welcome!")
            .replace("[start]", "@character Local \"Local\"\n[start]"),
        source.replace(
            "@characters Cast \"forward\"",
            "@character Cast.Guide \"Local\"",
        ),
        source.replace("[start]", "[start]\n@expression happy"),
        source.replace(
            "@expression happy",
            "-> Continue [target:talking]\n@expression happy",
        ),
    ] {
        assert!(
            tua_core::emit::emit_dialogue_source("invalid.tdialogue", &invalid).is_err(),
            "{invalid}"
        );
    }
}
