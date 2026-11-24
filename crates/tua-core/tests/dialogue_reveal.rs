use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn dialogue_reveal_execute_emitted_sources() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example = root.join("examples/dialogue-reveal/src");
    let mut script = "package.path = 'vendor/runtime/?.lua;' .. package.path\n".to_owned();
    for name in ["scene", "controller", "context", "bound"] {
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
    script.push_str(include_str!("fixtures/dialogue_reveal.lua"));
    script.push_str("\nlocal verify = (function()\n");
    script.push_str(include_str!("fixtures/dialogue_graphemes.lua"));
    script.push_str("\nend)()\nverify([====[\n");
    script.push_str(include_str!(
        "fixtures/unicode/GraphemeBreakTest-15.1.0.txt"
    ));
    script.push_str("\n]====])\n");
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
        eprintln!("SKIP reveal runtime: no standalone Lua interpreter");
    }
}
