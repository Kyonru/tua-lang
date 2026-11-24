use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn dialogue_rich_execute_emitted_sources() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example = root.join("examples/dialogue-rich/src");
    let mut script = "package.path = 'vendor/runtime/?.lua;' .. package.path\n".to_owned();
    let scene = tua_core::emit::emit_tua_source(
        "scene.tua",
        std::fs::read_to_string(example.join("scene.tua")).unwrap(),
    )
    .unwrap();
    script.push_str(&format!(
        "package.preload.scene = function()\n{scene}\nend\n"
    ));
    let authored = tua_core::emit::emit_dialogue_source(
        "authored.tdialogue",
        &std::fs::read_to_string(example.join("authored.tdialogue")).unwrap(),
    )
    .unwrap()
    .code;
    script.push_str(&format!(
        "package.preload.authored = function()\n{authored}\nend\n"
    ));
    script.push_str(
        &tua_core::emit::emit_tua_source(
            "main.tua",
            std::fs::read_to_string(example.join("main.tua")).unwrap(),
        )
        .unwrap(),
    );
    script.push_str(include_str!("fixtures/dialogue_rich.lua"));
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
        eprintln!("SKIP rich runtime: no standalone Lua interpreter");
    }
}
