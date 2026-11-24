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
