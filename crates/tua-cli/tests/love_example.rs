use std::fs;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;

mod support;

use support::{copy_dir_all, example_root, love_executable, tua, wait_for_child_output, TempDir};

#[test]
fn love_minimal_example_builds_and_runs_when_love_is_available() {
    let temp = TempDir::new("love-minimal");
    let example = example_root("love-minimal");

    copy_dir_all(&example, temp.path()).expect("failed to copy love-minimal example");

    let build_output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build for love-minimal");

    assert!(
        build_output.status.success(),
        "tua build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let stdout = String::from_utf8(build_output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("built 3 file(s)"));
    assert!(temp.path().join("build/conf.lua").is_file());
    assert!(temp.path().join("build/demo.lua").is_file());
    assert!(temp.path().join("build/main.lua").is_file());

    let main_output =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read main.lua");

    assert!(main_output.contains(r#"require("demo")"#));
    assert!(!main_output.contains(": DemoState"));

    let Some(love) = love_executable() else {
        eprintln!("skipping LOVE runtime validation; no LOVE executable was found");
        return;
    };

    let child = Command::new(&love)
        .arg(temp.path().join("build"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|source| panic!("failed to run {:?}: {source}", love));

    let love_output = wait_for_child_output(child, Duration::from_secs(5));

    assert!(
        love_output.status.success(),
        "LOVE runtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&love_output.stdout),
        String::from_utf8_lossy(&love_output.stderr)
    );
}

#[test]
fn love_full_example_builds_and_runs_when_love_is_available() {
    let temp = TempDir::new("love-full");
    let example = example_root("love-full");

    copy_dir_all(&example, temp.path()).expect("failed to copy love-full example");

    let build_output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build for love-full");

    assert!(
        build_output.status.success(),
        "tua build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let stdout = String::from_utf8(build_output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("built 5 file(s)"));
    assert!(temp.path().join("game/main.lua").is_file());
    assert!(temp.path().join("game/player.lua").is_file());
    assert!(temp.path().join("game/obstacles.lua").is_file());
    assert!(temp.path().join("game/friends.lua").is_file());
    assert!(temp.path().join("game/score.lua").is_file());
    assert!(temp.path().join("game/background.lua").is_file());
    assert!(temp.path().join("game/assets/player/idle.png").is_file());

    let main_output =
        fs::read_to_string(temp.path().join("game/main.lua")).expect("failed to read main.lua");
    let player_output =
        fs::read_to_string(temp.path().join("game/player.lua")).expect("failed to read player.lua");

    assert!(main_output.contains(r#"require("background")"#));
    assert!(!main_output.contains(": number"));
    assert!(player_output.contains("player.lives = player.lives - amount"));
    assert!(!player_output.contains("type PlayerModule"));

    let Some(love) = love_executable() else {
        eprintln!("skipping LOVE full runtime validation; no LOVE executable was found");
        return;
    };

    let child = Command::new(&love)
        .arg(temp.path().join("game"))
        .env("TUA_LOVE_FULL_SMOKE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|source| panic!("failed to run {:?}: {source}", love));

    let love_output = wait_for_child_output(child, Duration::from_secs(5));

    assert!(
        love_output.status.success(),
        "LOVE full runtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&love_output.stdout),
        String::from_utf8_lossy(&love_output.stderr)
    );
}
