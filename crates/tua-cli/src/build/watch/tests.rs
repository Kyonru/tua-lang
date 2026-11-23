use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn dialogue_watch_keeps_last_valid_data_and_rechecks_duplicate_ids() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("tua-dialogue-watch-{}-{nonce}", std::process::id()));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("tua.toml"), "source = \"src\"\nout = \"build\"\n").unwrap();
    fs::write(root.join("src/main.tua"), "local Dialogue = require(\"tuadialogue\")\nlocal player = Dialogue.new(require(\"intro\"))\nplayer:start()\n").unwrap();
    let source = root.join("src/intro.tdialogue");
    let text = "@dialogue 1\n[start]\n// @id intro.hello\n: Hello (end)\n";
    fs::write(&source, text).unwrap();
    let loaded = tua_core::load_config_from_dir(&root).unwrap();
    let (mut session, initial) = WatchBuildSession::start(&root, loaded).unwrap();
    assert!(
        !has_errors(&initial.diagnostics),
        "{:?}",
        initial.diagnostics
    );
    let output = root.join("build/intro.lua");
    let valid = fs::read_to_string(&output).unwrap();
    fs::write(&source, text.replace("(end)", "")).unwrap();
    let broken = session.rebuild(vec![source.clone()], vec![]);
    assert!(has_errors(&broken.diagnostics));
    assert_eq!(fs::read_to_string(&output).unwrap(), valid);
    fs::write(&source, text.replace("Hello", "Welcome")).unwrap();
    let fixed = session.rebuild(vec![source.clone()], vec![]);
    assert!(!has_errors(&fixed.diagnostics), "{:?}", fixed.diagnostics);
    assert!(fs::read_to_string(&output).unwrap().contains("Welcome"));
    let second = root.join("src/other.tdialogue");
    fs::write(&second, text).unwrap();
    let duplicate = session.rebuild(vec![second.clone()], vec![]);
    assert!(duplicate
        .diagnostics
        .iter()
        .any(|d| d.message.contains("duplicated across project")));
    assert!(!root.join("build/other.lua").exists());
    fs::write(&second, text.replace("intro.hello", "other.hello")).unwrap();
    let unique = session.rebuild(vec![second.clone()], vec![]);
    assert!(!has_errors(&unique.diagnostics), "{:?}", unique.diagnostics);
    assert!(root.join("build/other.lua").exists());
    fs::remove_file(&second).unwrap();
    let removed = session.rebuild(vec![], vec![second.clone()]);
    assert!(!has_errors(&removed.diagnostics));
    assert!(session.host.snapshot().source(&second).is_none());
    fs::remove_dir_all(root).unwrap();
}
