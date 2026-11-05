mod support;

use std::fs;
use std::process::Command;

use support::{tua, TempDir};
use tua_core::{CONFIG_FILE, DEFAULT_CONFIG_TEXT};

#[test]
fn game_scale_fixture_checks_builds_and_runs_without_steady_update_garbage() {
    let temp = TempDir::new("gameplay-regression");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source directory");
    fs::write(
        temp.path().join("src/main.tua"),
        include_str!("fixtures/gameplay_regression.tua"),
    )
    .expect("failed to write gameplay fixture");

    let check = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");
    assert!(
        check.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");
    assert!(
        build.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(temp.path().join("build/tuaobjectpool.lua").is_file());

    let Some(lua) = support::lua_executable() else {
        eprintln!("skipping gameplay runtime validation; no Lua executable was found");
        return;
    };
    let runtime = Command::new(lua)
        .arg("main.lua")
        .current_dir(temp.path().join("build"))
        .output()
        .expect("failed to run emitted gameplay fixture");

    assert!(
        runtime.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&runtime.stdout),
        String::from_utf8_lossy(&runtime.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&runtime.stdout).trim(),
        "gameplay regression passed"
    );
}

#[test]
fn polymorphism_example_checks_and_dispatches_across_modules() {
    let temp = TempDir::new("polymorphism-example");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).unwrap();
    fs::create_dir_all(temp.path().join("src/polymorphism")).unwrap();
    for (path, source) in [
        (
            "src/main.tua",
            include_str!("../../../examples/features/src/polymorphism.tua"),
        ),
        (
            "src/polymorphism/actors.tua",
            include_str!("../../../examples/features/src/polymorphism/actors.tua"),
        ),
    ] {
        fs::write(temp.path().join(path), source).unwrap();
    }
    for operation in ["check", "build"] {
        let output = tua()
            .arg(operation)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{operation}:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let Some(lua) = support::lua_executable() else {
        eprintln!("skipping polymorphism runtime validation; no Lua executable was found");
        return;
    };
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

#[test]
fn class_and_following_function_defaults_check_build_and_run() {
    let temp = TempDir::new("class-default-arguments");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).unwrap();
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(
        temp.path().join("src/main.tua"),
        r#"final class Example {
  value: number
  init(value: number = 3) { self.value = value }
  run(first: boolean = false): boolean { return first }
  call(): boolean { return self:run() }
}
local function after(value: number = 7): number
  return value
end
local example = Example.new()
assert(example.value == 3)
assert(example:run() == false)
assert(example:run(true) == true)
assert(example:call() == false)
assert(after() == 7)
assert(after(0) == 0)
"#,
    )
    .unwrap();
    for operation in ["check", "build"] {
        let output = tua()
            .arg(operation)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let Some(lua) = support::lua_executable() else {
        return;
    };
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
