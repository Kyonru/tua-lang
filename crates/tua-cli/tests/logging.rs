use std::fs;
use std::process::Command;

mod support;

#[test]
fn referenced_log_is_bundled_and_emitted_lua_reports_its_call_site() {
    let project = support::TempDir::new("logging-runtime");
    fs::create_dir(project.path().join("src")).unwrap();
    fs::write(
        project.path().join("tua.toml"),
        "source = \"src\"\nemit_maps = true\n",
    )
    .unwrap();
    fs::write(
        project.path().join("src/main.tua"),
        "local combat = Log.scope(\"combat\")\ncombat.warn(\"low health\", 7)\n",
    )
    .unwrap();
    let build = support::tua()
        .arg("build")
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(project.path().join("build/tualog.lua").is_file());
    let emitted = fs::read_to_string(project.path().join("build/main.lua")).unwrap();
    assert!(
        emitted.starts_with("local Log = require(\"tualog\")\n"),
        "{emitted}"
    );
    assert!(project.path().join("build/main.lua.map.json").is_file());

    if let Some(lua) = support::lua_executable() {
        let run = Command::new(lua)
            .arg("main.lua")
            .current_dir(project.path().join("build"))
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let output = String::from_utf8_lossy(&run.stdout);
        assert!(
            output.contains("[warn] [combat] low health\t7 (main.lua:"),
            "{output}"
        );
    }
}

#[test]
fn unused_log_is_absent_from_a_build() {
    let project = support::TempDir::new("logging-unused");
    fs::create_dir(project.path().join("src")).unwrap();
    fs::write(project.path().join("tua.toml"), "source = \"src\"\n").unwrap();
    fs::write(project.path().join("src/main.tua"), "print(\"hello\")\n").unwrap();
    let build = support::tua()
        .arg("build")
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(!project.path().join("build/tualog.lua").exists());
}
