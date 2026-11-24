mod support;
use std::{fs, path::Path, process::Command};
use support::{copy_dir_all, tua, TempDir};

fn run(root: &Path, command: &str) {
    let result = tua().arg(command).current_dir(root).output().unwrap();
    assert!(
        result.status.success(),
        "{command}: {}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn dialogue_m0_preserves_v1_ownership_imports_and_helper_selection() {
    let temp = TempDir::new("dialogue-m0");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks/dialogue");
    copy_dir_all(&root.join("fixture"), temp.path()).unwrap();
    run(temp.path(), "check");
    run(temp.path(), "build");
    assert!(temp.path().join("build/tuadialogue.lua").is_file());
    if let Some(lua) = support::lua_executable() {
        fs::copy(
            root.join("ownership.lua"),
            temp.path().join("build/ownership.lua"),
        )
        .unwrap();
        for name in ["main.lua", "ownership.lua"] {
            let output = Command::new(&lua)
                .arg(name)
                .current_dir(temp.path().join("build"))
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    } else {
        eprintln!("SKIP emitted Lua: interpreter unavailable");
    }

    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local contracts = require("forward")
local facts = contracts.view()
facts.player.coins = 0
local wrong: string = facts.player.coins
print(wrong)
"#,
    )
    .unwrap();
    let invalid = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .unwrap();
    let diagnostics = String::from_utf8_lossy(&invalid.stderr);
    assert!(
        !invalid.status.success(),
        "invalid imported contracts accepted"
    );
    for expected in ["read-only", "expected string"] {
        assert!(diagnostics.contains(expected), "{diagnostics}");
    }

    fs::write(
        temp.path().join("src/main.tua"),
        "local scene = require('scene')\nassert(scene.entry == 'start')\n",
    )
    .unwrap();
    run(temp.path(), "build");
    assert!(
        !temp.path().join("build/tuadialogue.lua").exists(),
        "data import must not bundle the player"
    );
    fs::write(
        temp.path().join("src/tuadialogue.lua"),
        "return { replacement = true }\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("src/main.tua"),
        "local runtime = require('tuadialogue')\nassert(runtime.replacement)\n",
    )
    .unwrap();
    run(temp.path(), "build");
    assert!(
        fs::read_to_string(temp.path().join("build/tuadialogue.lua"))
            .unwrap()
            .contains("replacement = true")
    );
    let scene = fs::read_to_string(temp.path().join("src/scene.tdialogue")).unwrap();
    fs::write(
        temp.path().join("src/scene.tdialogue"),
        scene.replace("@dialogue 1", "@dialogue 999"),
    )
    .unwrap();
    let invalid = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("TL5012"));
}

#[test]
fn dialogue_localization_uses_emitted_runtime_and_non_destructive_extraction() {
    let temp = TempDir::new("dialogue-m1");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    copy_dir_all(&root.join("examples/dialogue-localization"), temp.path()).unwrap();
    run(temp.path(), "check");
    run(temp.path(), "build");
    let catalog = temp.path().join("src/locales/fr.lua");
    let original = fs::read(&catalog).unwrap();
    let output = tua()
        .arg("dialogue-extract")
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let data: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(data["entries"]["harbor.bye"]["text"], "Goodbye, %{name}.");
    assert_eq!(fs::read(&catalog).unwrap(), original);
    if let Some(lua) = support::lua_executable() {
        fs::copy(
            root.join("vendor/tests/dialogue_localization_test.lua"),
            temp.path().join("build/oracle.lua"),
        )
        .unwrap();
        for name in ["main.lua", "oracle.lua"] {
            let output = Command::new(&lua)
                .arg(name)
                .current_dir(temp.path().join("build"))
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    for invalid in [
        "local p = Dialogue.new(require('scene'), { locale = 4, resolve = I18n.dialogue })",
        "local p = Dialogue.new(require('scene'), { locale = 'en', resolve = I18n.dialogue, args = { name = {} } })",
        "local p = Dialogue.new(require('scene'))\np:setLocale(false)",
        "local p = Dialogue.new(require('scene'))\nlocal wrong: string = p:snapshot().presentationRevision",
    ] {
        fs::write(temp.path().join("src/main.tua"), invalid).unwrap();
        assert!(!tua().arg("check").current_dir(temp.path()).output().unwrap().status.success(), "accepted: {invalid}");
    }
    fs::write(
        temp.path().join("src/tuadialogue.lua"),
        "return { schemaVersion = 0 }\n",
    )
    .unwrap();
    fs::write(temp.path().join("src/main.tua"), "local runtime = require('tuadialogue')\nassert(runtime.schemaVersion >= 1, 'needs dialogue schema 1')\n").unwrap();
    run(temp.path(), "build");
    assert!(
        fs::read_to_string(temp.path().join("build/tuadialogue.lua"))
            .unwrap()
            .contains("schemaVersion = 0")
    );
    if let Some(lua) = support::lua_executable() {
        let output = Command::new(lua)
            .arg("main.lua")
            .current_dir(temp.path().join("build"))
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("needs dialogue schema 1"));
    }
}

#[test]
fn rich_authored_dialogue_builds_runs_and_extracts_templates() {
    let temp = TempDir::new("dialogue-rich-authoring");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    copy_dir_all(&root.join("examples/dialogue-rich"), temp.path()).unwrap();
    run(temp.path(), "check");
    run(temp.path(), "build");
    let output = tua()
        .arg("dialogue-extract")
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let entry = &manifest["entries"]["rich.greeting"];
    assert_eq!(entry["format"], "rich");
    assert_eq!(entry["plainTemplate"], "Hi %{name}!");
    assert!(entry["text"].as_str().unwrap().contains("[pause=0.5]"));
    assert!(fs::read_to_string(temp.path().join("build/authored.lua"))
        .unwrap()
        .contains("schemaVersion = 8"));
    if let Some(lua) = support::lua_executable() {
        let output = Command::new(lua)
            .arg("main.lua")
            .current_dir(temp.path().join("build"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn durable_dialogue_recipe_checks_builds_and_recovers_crash_windows() {
    let temp = TempDir::new("dialogue-durable");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    copy_dir_all(&root.join("examples/dialogue-durable"), temp.path()).unwrap();
    run(temp.path(), "check");
    run(temp.path(), "build");
    if let Some(lua) = support::lua_executable() {
        let output = Command::new(lua)
            .arg("../tests/recovery.lua")
            .current_dir(temp.path().join("build"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        eprintln!("SKIP durable dialogue faults: no Lua interpreter");
    }
    fs::write(
        temp.path().join("src/invalid.tua"),
        r#"
local journal = require("journal").new()
local value = journal.load()
value.coins = "wrong"
journal.receipt(123)
local intent = value.intent
if intent then intent.committed = "wrong" end
"#,
    )
    .unwrap();
    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .unwrap();
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "invalid imported journal types accepted"
    );
    assert!(diagnostics.matches("TL1003").count() >= 3, "{diagnostics}");
}
