use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tua_core::{CONFIG_FILE, DEFAULT_CONFIG_TEXT, DEFAULT_MAIN_TEXT};

fn tua() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tua"))
}

struct TempDir {
    path: PathBuf,
}

struct CapturedChildStdout {
    ready: std::sync::mpsc::Receiver<()>,
    reader: thread::JoinHandle<Vec<u8>>,
}

impl CapturedChildStdout {
    fn wait_for_watch_ready(child: &mut process::Child, failure_message: &str) -> Self {
        let capture = Self::start(child, "initial build:");
        capture
            .ready
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or_else(|_| panic!("{failure_message}"));
        capture
    }

    fn start(child: &mut process::Child, readiness_marker: &str) -> Self {
        let stdout = child.stdout.take().expect("child stdout should be piped");
        let readiness_marker = readiness_marker.to_owned();
        let (ready_sender, ready) = std::sync::mpsc::channel();
        let reader = thread::spawn(move || {
            let mut stdout = BufReader::new(stdout);
            let mut captured = Vec::new();
            let mut ready_sender = Some(ready_sender);

            loop {
                let mut line = Vec::new();
                let read = stdout
                    .read_until(b'\n', &mut line)
                    .expect("failed to read child stdout");
                if read == 0 {
                    break;
                }

                if ready_sender.is_some()
                    && String::from_utf8_lossy(&line).contains(&readiness_marker)
                {
                    if let Some(sender) = ready_sender.take() {
                        let _ = sender.send(());
                    }
                }
                captured.extend_from_slice(&line);
            }

            captured
        });

        Self { ready, reader }
    }

    fn finish(self) -> Vec<u8> {
        self.reader.join().expect("child stdout reader panicked")
    }
}

impl TempDir {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tua-cli-{name}-{}-{unique}", process::id()));

        fs::create_dir_all(&path).expect("failed to create temp dir");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn help_prints_command_list() {
    let output = tua()
        .arg("--help")
        .output()
        .expect("failed to run tua --help");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    for command in [
        "init",
        "check",
        "fmt",
        "lint",
        "build",
        "traceback",
        "watch",
        "lsp",
    ] {
        assert!(stdout.contains(command), "missing command `{command}`");
    }

    assert!(stdout.contains("--emit-on-error"));
    assert!(stdout.contains("--check"));
    assert!(stdout.contains("--strict"));
    assert!(stdout.contains("--maps"));
}

#[test]
fn unknown_command_exits_with_useful_error() {
    let output = tua()
        .arg("compile")
        .output()
        .expect("failed to run tua compile");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("unknown command `compile`"));
    assert!(stderr.contains("tua --help"));
}

#[test]
fn invalid_flag_exits_with_useful_error() {
    let output = tua()
        .args(["check", "--emit-on-error"])
        .output()
        .expect("failed to run tua check --emit-on-error");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("only valid with `tua build`"));
    assert!(stderr.contains("tua --help"));
}

#[test]
fn build_rejects_output_outside_project_root() {
    let temp = TempDir::new("outside-output");
    let project = temp.path().join("project");
    let outside = temp.path().join("outside");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(project.join("src/main.tua"), "local value: number = 1\n").unwrap();
    fs::write(
        project.join(CONFIG_FILE),
        DEFAULT_CONFIG_TEXT.replace("out = \"build\"", "out = \"../outside\""),
    )
    .unwrap();

    let output = tua().arg("build").current_dir(&project).output().unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("escaped the project root"));
    assert!(!outside.join("main.lua").exists());
}

#[cfg(unix)]
#[test]
fn build_rejects_symlinked_output_component() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new("symlink-output");
    let project = temp.path().join("project");
    let outside = temp.path().join("outside");
    fs::create_dir_all(project.join("src/pkg")).unwrap();
    fs::create_dir_all(project.join("build")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(
        project.join("src/pkg/main.tua"),
        "local value: number = 1\n",
    )
    .unwrap();
    fs::write(project.join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).unwrap();
    symlink(&outside, project.join("build/pkg")).unwrap();

    let output = tua().arg("build").current_dir(&project).output().unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("crosses a symbolic link"));
    assert!(!outside.join("main.lua").exists());
}

#[test]
fn init_creates_default_project_files() {
    let temp = TempDir::new("init");
    let output = tua()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua init");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("initialized tua project"));
    assert!(stdout.contains("created tua.toml"));
    assert!(stdout.contains("created src/"));
    assert!(stdout.contains("created src/main.tua"));
    assert!(stdout.contains("created .gitignore"));
    assert_eq!(
        fs::read_to_string(temp.path().join(CONFIG_FILE)).expect("failed to read config"),
        DEFAULT_CONFIG_TEXT
    );
    assert!(temp.path().join("src").is_dir());
    assert_eq!(
        fs::read_to_string(temp.path().join("src/main.tua")).expect("failed to read main"),
        DEFAULT_MAIN_TEXT
    );
    assert_eq!(
        fs::read_to_string(temp.path().join(".gitignore")).expect("failed to read gitignore"),
        "# Tua generated files\n/build/\n/dist/\n/.tua/\n"
    );
}

#[test]
fn init_updates_an_existing_gitignore() {
    let temp = TempDir::new("init-existing-gitignore");
    let gitignore_path = temp.path().join(".gitignore");
    fs::write(&gitignore_path, "node_modules/\n/dist/\n")
        .expect("failed to write existing gitignore");

    let output = tua()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua init");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("updated .gitignore"));
    assert_eq!(
        fs::read_to_string(gitignore_path).expect("failed to read gitignore"),
        "node_modules/\n/dist/\n\n# Tua generated files\n/build/\n/.tua/\n"
    );
}

#[test]
fn init_help_is_contextual() {
    let output = tua()
        .args(["init", "--help"])
        .output()
        .expect("failed to run tua init --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("tua init - create a Tua project"));
    assert!(stdout.contains("Usage:\n  tua init"));
    assert!(stdout.contains("adds generated folders to .gitignore"));
    assert!(!stdout.contains("Commands:"));
}

#[test]
fn init_does_not_overwrite_existing_config() {
    let temp = TempDir::new("init-existing");
    let config_path = temp.path().join(CONFIG_FILE);
    let existing = "source = \"custom\"\n";

    fs::write(&config_path, existing).expect("failed to write existing config");

    let output = tua()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua init");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("left unchanged"));
    assert_eq!(
        fs::read_to_string(config_path).expect("failed to read config"),
        existing
    );
    assert!(temp.path().join("src").is_dir());
    assert_eq!(
        fs::read_to_string(temp.path().join("src/main.tua")).expect("failed to read main"),
        DEFAULT_MAIN_TEXT
    );
}

#[test]
fn init_does_not_overwrite_existing_main() {
    let temp = TempDir::new("init-existing-main");
    let source_dir = temp.path().join("src");
    let main_path = source_dir.join("main.tua");
    let existing = "print(\"custom\")\n";

    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    fs::write(&main_path, existing).expect("failed to write existing main");

    let output = tua()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua init");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("src/ already exists"));
    assert!(stdout.contains("src/main.tua already exists; left unchanged"));
    assert_eq!(
        fs::read_to_string(main_path).expect("failed to read main"),
        existing
    );
}

#[test]
fn builtin_tween_checks_bundles_and_yields_to_a_project_runtime() {
    let temp = TempDir::new("builtin-tween");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local banner = { alpha = 0 }

function love.load()
  tween.to(banner, 0.5, { alpha = 1 }):ease("quadout")
end

function love.update(dt: number)
  tween.update(dt)
end
"#,
    )
    .expect("failed to write main source");

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");
    assert!(build.status.success(), "{build:?}");
    let main = fs::read_to_string(temp.path().join("build/main.lua")).expect("main.lua");
    assert!(
        main.starts_with("local tween = require(\"tuatween\")\n"),
        "{main}"
    );
    let runtime =
        fs::read_to_string(temp.path().join("build/tuatween.lua")).expect("bundled tuatween");
    assert!(runtime.contains("Copyright (c) 2016 rxi"));

    fs::write(
        temp.path().join("src/tuatween.lua"),
        "return { custom = true }\n",
    )
    .expect("failed to write project runtime");
    fs::remove_file(temp.path().join("build/tuatween.lua")).expect("remove bundled copy");
    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to rebuild");
    assert!(build.status.success(), "{build:?}");
    assert_eq!(
        fs::read_to_string(temp.path().join("build/tuatween.lua")).expect("project tuatween"),
        "return { custom = true }\n"
    );

    fs::write(
        temp.path().join("src/main.tua"),
        "tween.to({ x = 0 }, 1, { x = 1 }):ease(\"bounce\")\n",
    )
    .expect("failed to write misuse");
    let check = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");
    assert!(!check.status.success());
    assert!(String::from_utf8_lossy(&check.stderr).contains("found \"bounce\""));
}

#[test]
fn builtin_camera_checks_and_bundles() {
    let temp = TempDir::new("builtin-camera");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local player = { x = 0, y = 0 }
local camera = Camera.new({ style = "platformer", smoothing = 8, zoom = 2 })

function love.update(dt: number)
  camera:follow(player.x, player.y)
  camera:update(dt)
end

function love.draw()
  camera:draw(function()
    love.graphics.circle("fill", player.x, player.y, 8)
  end)
end
"#,
    )
    .expect("failed to write main source");

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");
    assert!(build.status.success(), "{build:?}");
    let main = fs::read_to_string(temp.path().join("build/main.lua")).expect("main.lua");
    assert!(
        main.contains("local Camera = require(\"tuacamera\")"),
        "{main}"
    );
    assert!(temp.path().join("build/tuacamera.lua").exists());

    fs::write(
        temp.path().join("src/main.tua"),
        "local cam = Camera.new({ style = \"orbit\" })\ncam:shake(\"hard\")\n",
    )
    .expect("failed to write misuse");
    let check = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(stderr.contains("found \"orbit\""), "{stderr}");
    assert!(stderr.contains("main.tua:2:"), "{stderr}");
}

fn stored_zip_names(bytes: &[u8]) -> Vec<String> {
    let read_u16 = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
    let read_u32 = |at: usize| {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
    };
    let mut names = Vec::new();
    let mut at = 0;
    while at + 4 <= bytes.len() && read_u32(at) == 0x0403_4b50 {
        let size = read_u32(at + 18);
        let name_len = read_u16(at + 26);
        let extra_len = read_u16(at + 28);
        names.push(String::from_utf8(bytes[at + 30..at + 30 + name_len].to_vec()).expect("utf-8"));
        at += 30 + name_len + extra_len + size;
    }
    names
}

fn write_packaging_project(temp: &TempDir) {
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/assets")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "function love.draw()\n  love.graphics.print(\"hi\", 10, 10)\nend\n",
    )
    .expect("failed to write main source");
    fs::write(temp.path().join("src/assets/level.txt"), "1 2 3\n").expect("failed to write asset");
}

#[test]
fn rebuild_and_package_drop_deleted_outputs_but_preserve_unowned_files() {
    let temp = TempDir::new("stale-build");
    write_packaging_project(&temp);
    fs::write(temp.path().join("src/removed.tua"), "return 1\n").unwrap();
    assert!(tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .unwrap()
        .status
        .success());
    fs::write(temp.path().join("build/manual.txt"), "keep me").unwrap();
    fs::remove_file(temp.path().join("src/removed.tua")).unwrap();
    fs::remove_file(temp.path().join("src/assets/level.txt")).unwrap();
    let result = tua()
        .args(["package", "--name", "clean"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!temp.path().join("build/removed.lua").exists());
    assert!(!temp.path().join("build/removed.lua.map.json").exists());
    assert!(!temp.path().join("build/assets/level.txt").exists());
    assert_eq!(
        fs::read_to_string(temp.path().join("build/manual.txt")).unwrap(),
        "keep me"
    );
    let archive = fs::read(temp.path().join("dist/clean.love")).unwrap();
    assert!(!archive
        .windows(b"removed.lua".len())
        .any(|w| w == b"removed.lua"));
    assert!(!archive
        .windows(b"manual.txt".len())
        .any(|w| w == b"manual.txt"));
}

#[test]
fn build_manifest_cannot_delete_outside_output_root() {
    let temp = TempDir::new("hostile-build-manifest");
    write_packaging_project(&temp);
    fs::create_dir_all(temp.path().join("build")).unwrap();
    fs::write(temp.path().join("victim"), "keep me").unwrap();
    fs::write(
        temp.path().join("build/.tua-build-manifest.json"),
        "{\"../victim\":0}",
    )
    .unwrap();
    assert!(!tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(
        fs::read_to_string(temp.path().join("victim")).unwrap(),
        "keep me"
    );
}

#[test]
fn package_writes_a_reproducible_love_archive_without_source_maps() {
    let temp = TempDir::new("package-love");
    write_packaging_project(&temp);

    let package = tua()
        .args(["package", "--name", "Star Dust"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua package");
    assert!(package.status.success(), "{package:?}");
    let stdout = String::from_utf8_lossy(&package.stdout);
    let archive_path = Path::new("dist").join("Star Dust.love");
    assert!(
        stdout.contains(&format!("packaged {}", archive_path.display())),
        "{stdout}"
    );

    let archive = fs::read(temp.path().join("dist/Star Dust.love")).expect("archive");
    let names = stored_zip_names(&archive);
    assert_eq!(
        names,
        ["assets/", "assets/level.txt", "main.lua"],
        "{names:?}"
    );

    let again = tua()
        .args(["package", "--name", "Star Dust"])
        .current_dir(temp.path())
        .output()
        .expect("failed to rerun tua package");
    assert!(again.status.success());
    assert_eq!(
        fs::read(temp.path().join("dist/Star Dust.love")).expect("archive"),
        archive,
        "identical inputs should produce identical archives"
    );
}

#[test]
fn package_fuses_a_windows_build_with_the_supplied_runtime() {
    let temp = TempDir::new("package-windows");
    write_packaging_project(&temp);
    let runtime = temp.path().join("love-win64");
    fs::create_dir_all(&runtime).expect("failed to create runtime");
    fs::write(runtime.join("love.exe"), b"MZ fake love").expect("love.exe");
    fs::write(runtime.join("SDL2.dll"), b"dll").expect("dll");
    fs::write(runtime.join("license.txt"), b"zlib").expect("license");
    fs::write(runtime.join("lovec.exe"), b"console").expect("lovec");

    let missing = tua()
        .args(["package", "--target", "windows"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua package");
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("--love"));

    let package = tua()
        .args([
            "package",
            "--target",
            "windows",
            "--name",
            "Star Dust",
            "--love",
        ])
        .arg(&runtime)
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua package");
    assert!(package.status.success(), "{package:?}");

    let game = temp.path().join("dist/windows/Star Dust");
    let executable = fs::read(game.join("Star Dust.exe")).expect("fused exe");
    let archive = fs::read(temp.path().join("dist/Star Dust.love")).expect("archive");
    assert!(executable.starts_with(b"MZ fake love"));
    assert!(executable.ends_with(&archive));
    assert!(game.join("SDL2.dll").exists());
    assert!(game.join("license.txt").exists());
    assert!(!game.join("lovec.exe").exists());
}

#[cfg(unix)]
#[test]
fn dev_prepares_a_hot_reloadable_build_and_exits_with_the_game() {
    let temp = TempDir::new("dev-hot-reload");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "function love.draw()\n  love.graphics.print(\"hi\", 10, 10)\nend\n",
    )
    .expect("failed to write main source");

    // `true` stands in for LOVE: it starts and exits at once, so `tua dev`
    // stops after the initial build.
    let output = tua()
        .arg("dev")
        .env("TUA_LOVE", "true")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua dev");

    assert!(output.status.success(), "{output:?}");
    let main = fs::read_to_string(temp.path().join("build/main.lua")).expect("main.lua");
    assert!(
        main.ends_with("require(\"tuahotreload\").install()\n"),
        "{main}"
    );
    assert!(temp.path().join("build/tuahotreload.lua").exists());
    assert_eq!(
        fs::read_to_string(temp.path().join("build/.tuahotreload")).expect("signal"),
        "1\n"
    );

    let package = tua()
        .arg("package")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua package");
    assert!(package.status.success(), "{package:?}");
    let name = temp
        .path()
        .file_name()
        .expect("temp name")
        .to_string_lossy()
        .into_owned();
    let archive = fs::read(temp.path().join(format!("dist/{name}.love"))).expect("archive");
    let names = stored_zip_names(&archive);
    assert_eq!(names, ["main.lua"], "dev artifacts stay out of packages");
    let main = fs::read_to_string(temp.path().join("build/main.lua")).expect("main.lua");
    assert!(
        !main.contains("tuahotreload"),
        "a normal build drops the hook"
    );
}

#[test]
fn check_prints_config_diagnostics() {
    let temp = TempDir::new("check-invalid-config");

    fs::write(temp.path().join(CONFIG_FILE), "emit_maps = yes\n")
        .expect("failed to write invalid config");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains(
        "tua.toml:1:13 error TL3001: string values must be quoted, expected literal string"
    ));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn check_exits_successfully_for_clean_project() {
    let temp = TempDir::new("check-clean");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
type Player = {
  hp: number,
}

local player: Player = { hp = 10 }
player.hp = 9
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("checked 1 file(s)"));
}

#[test]
fn check_accepts_project_aware_types_from_tua_and_lua_modules() {
    let temp = TempDir::new("check-project-types");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local Types = require("types")
local LuaDocTypes = require("lua_doc_types")

local player: Types.Player = nil
local enemy: LuaDocTypes.Enemy = nil

return {
  player = player,
  enemy = enemy,
}
"#,
    )
    .expect("failed to write main source");
    fs::write(
        temp.path().join("src/types.tua"),
        r#"
export type Player = { hp: number }

return {}
"#,
    )
    .expect("failed to write tua type provider");
    fs::write(
        temp.path().join("src/lua_doc_types.lua"),
        r#"
---@class Enemy
---@field hp number
local Enemy = {}

return {
  Enemy = Enemy,
}
"#,
    )
    .expect("failed to write lua type provider");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("checked 2 file(s)"));
}

#[test]
fn check_exits_nonzero_for_parse_errors() {
    let temp = TempDir::new("check-parse-error");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(temp.path().join("src/main.tua"), "local hp = \n").expect("failed to write source");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("main.tua:1:10 error TL0001"));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn check_exits_nonzero_for_type_errors() {
    let temp = TempDir::new("check-type-error");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local hp: number = "full"
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("main.tua:2:20 error TL1003"));
    assert!(stderr.contains("type mismatch: expected number, found string"));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn check_keeps_inner_arity_for_method_calls_on_required_module_results() {
    let temp = TempDir::new("check-chained-module-method");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local Things = require(\"things\")\n\nThings.make(1):go()\nThings.make(1):go():go()\nThings.make()\n",
    )
    .expect("failed to write main source");
    fs::write(
        temp.path().join("src/things.lua"),
        r#"---@class Thing
---@field go fun(self: Thing): Thing

---@param seed number
---@return Thing
local function make(seed)
  return {}
end

return { make = make }
"#,
    )
    .expect("failed to write Lua module");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    // A later `:go()` must not make `make(1)` look like a method call that
    // also receives `Things` as `self`.
    assert!(!stderr.contains("main.tua:3:1"), "{stderr}");
    assert!(!stderr.contains("main.tua:4:1"), "{stderr}");
    assert!(
        stderr.contains("main.tua:5:1 error TL1004: argument count mismatch: expected 1, found 0"),
        "{stderr}"
    );
}

#[test]
fn check_keeps_literal_parameter_contracts_across_module_boundaries() {
    let temp = TempDir::new("check-module-literal-params");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local Easing = require(\"easing\")\n\nEasing.set(\"linear\")\nEasing.set(\"bounce\")\n",
    )
    .expect("failed to write main source");
    fs::write(
        temp.path().join("src/easing.lua"),
        r#"---@alias EasingName "linear" | "quadout"

---@param name EasingName
local function set(name) end

return { set = set }
"#,
    )
    .expect("failed to write Lua module");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(!stderr.contains("main.tua:3:"), "{stderr}");
    assert!(
        stderr.contains("main.tua:4:12 error TL1003")
            && stderr.contains("expected \"linear\" | \"quadout\", found \"bounce\""),
        "{stderr}"
    );
}

#[test]
fn check_lets_callers_omit_optional_lua_doc_parameters() {
    let temp = TempDir::new("check-optional-lua-doc-params");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local Camera = require(\"camera\")\n\nCamera.new()\nCamera.new({ zoom = 2 })\nCamera.shake(1)\nCamera.shake()\nCamera.shake(1, \"long\")\n",
    )
    .expect("failed to write main source");
    fs::write(
        temp.path().join("src/camera.lua"),
        r#"---@param options? { zoom?: number }
local function new(options) end

---@param amount number
---@param seconds number?
local function shake(amount, seconds) end

return { new = new, shake = shake }
"#,
    )
    .expect("failed to write Lua module");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    for line in [3, 4, 5] {
        assert!(!stderr.contains(&format!("main.tua:{line}:")), "{stderr}");
    }
    assert!(
        stderr.contains("main.tua:6:1 error TL1004: argument count mismatch: expected 1, found 0"),
        "{stderr}"
    );
    assert!(stderr.contains("main.tua:7:"), "{stderr}");
}

#[test]
fn check_reports_bad_arguments_in_calls_assigned_to_unannotated_locals() {
    let temp = TempDir::new("check-local-call-arguments");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local M = require(\"m\")\n\nlocal one = M.double(\"x\")\nlocal two, three = M.double(\"y\")\nlocal fine = M.double(2)\nprint(one, two, three, fine)\n",
    )
    .expect("failed to write main source");
    fs::write(
        temp.path().join("src/m.lua"),
        r#"---@param n number
---@return number
local function double(n) return n * 2 end

return { double = double }
"#,
    )
    .expect("failed to write Lua module");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert_eq!(stderr.matches("main.tua:3:").count(), 1, "{stderr}");
    assert_eq!(stderr.matches("main.tua:4:").count(), 1, "{stderr}");
    assert!(!stderr.contains("main.tua:5:"), "{stderr}");
}

#[test]
fn check_enables_standard_lints_by_default() {
    let temp = TempDir::new("check-default-lints");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local first, first = 1, 2\nreturn first\n",
    )
    .expect("failed to write source");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("warning TL4043"));
    assert!(stdout.contains("variable `first` already defined on column 7"));
}

#[test]
fn check_applies_default_inline_ignores() {
    let temp = TempDir::new("check-inline-ignores");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"
out = "build"
target = "love11-luajit"
extension = "tua"
emit_maps = true
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
-- tua-ignore-next-line TL1003
local hp: number = "full"
print(hp)
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("checked 1 file(s)"));
}

#[test]
fn build_compiles_nested_source_tree_to_matching_output_tree() {
    let temp = TempDir::new("build-nested");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/systems/player")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("src/assets")).expect("failed to create ignored source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local Player = require("systems.player.core")
local current: any = nil

return Player
"#,
    )
    .expect("failed to write main.tua");
    fs::write(
        temp.path().join("src/systems/player/core.tua"),
        r#"
type Player = {
  hp: number,
}

local function damage(player: Player, amount: number): Player
  player.hp -= amount
  return player
end

return {
  damage = damage,
}
"#,
    )
    .expect("failed to write nested source");
    fs::write(
        temp.path().join("src/assets/ignored.lua"),
        "return { ignored = true }\n",
    )
    .expect("failed to write ignored lua");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("built 3 file(s)"));

    let main_output =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read main output");
    let nested_output = fs::read_to_string(temp.path().join("build/systems/player/core.lua"))
        .expect("failed to read nested output");
    let lua_output = fs::read_to_string(temp.path().join("build/assets/ignored.lua"))
        .expect("failed to read copied lua output");

    assert!(main_output.contains(r#"require("systems.player.core")"#));
    assert!(!main_output.contains(": Player"));
    assert!(nested_output.contains("player.hp = player.hp - amount"));
    assert!(!nested_output.contains("type Player"));
    assert_eq!(lua_output, "return { ignored = true }\n");

    let nested_map = fs::read_to_string(temp.path().join("build/systems/player/core.lua.map.json"))
        .expect("failed to read nested source map");
    let nested_map: Value = serde_json::from_str(&nested_map).expect("source map should be JSON");

    assert_eq!(nested_map["version"], 1);
    assert_eq!(nested_map["generated"], "build/systems/player/core.lua");
    assert_eq!(nested_map["source"], "src/systems/player/core.tua");
    assert!(nested_map["lineMappings"]
        .as_array()
        .is_some_and(|mappings| !mappings.is_empty()));

    let debugger_map = fs::read_to_string(temp.path().join("build/systems/player/core.lua.map"))
        .expect("failed to read debugger source map");
    let debugger_map: Value =
        serde_json::from_str(&debugger_map).expect("debugger source map should be JSON");

    assert_eq!(debugger_map["version"], 3);
    assert_eq!(debugger_map["file"], "core.lua");
    assert_eq!(debugger_map["sourceRoot"], "../../..");
    assert_eq!(
        debugger_map["sources"],
        json!(["src/systems/player/core.tua"])
    );
    assert!(debugger_map["mappings"]
        .as_str()
        .is_some_and(|mappings| !mappings.is_empty()));
}

#[test]
fn build_emits_configured_main_source_as_root_main_lua() {
    let temp = TempDir::new("build-configured-main");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "src"
out = "build"
main = "love/boot.tua"
target = "love11-luajit"
extension = "tua"
emit_maps = true
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/love")).expect("failed to create source");
    fs::write(
        temp.path().join("src/love/boot.tua"),
        "local ready: boolean = true\nreturn ready\n",
    )
    .expect("failed to write boot source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        fs::read_to_string(temp.path().join("build/love/boot.lua"))
            .expect("failed to read mirrored output"),
        "local ready = true\nreturn ready\n"
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("build/main.lua"))
            .expect("failed to read configured main output"),
        "local ready = true\nreturn ready\n"
    );

    let map: Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join("build/main.lua.map.json"))
            .expect("failed to read configured main source map"),
    )
    .expect("configured main source map should be JSON");

    assert_eq!(map["generated"], "build/main.lua");
    assert_eq!(map["source"], "src/love/boot.tua");
}

#[test]
fn build_reports_duplicate_root_main_output_for_configured_main_source() {
    let temp = TempDir::new("build-configured-main-collision");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "src"
out = "build"
main = "love/boot.tua"
target = "love11-luajit"
extension = "tua"
emit_maps = true
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/love")).expect("failed to create love source");
    fs::write(temp.path().join("src/main.tua"), "return \"default\"\n")
        .expect("failed to write default main");
    fs::write(temp.path().join("src/love/boot.tua"), "return \"boot\"\n")
        .expect("failed to write configured main");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("generated output `"));
    assert!(stderr.contains("build/main.lua"));
    assert!(stderr.contains("src/main.tua"));
    assert!(stderr.contains("src/love/boot.tua"));
    assert!(!temp.path().join("build/main.lua").exists());
    assert!(!temp.path().join("build/love/boot.lua").exists());
}

#[test]
fn build_emit_on_error_keeps_non_conflicting_output_for_configured_main_source() {
    let temp = TempDir::new("build-configured-main-collision-emit");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "src"
out = "build"
main = "love/boot.tua"
target = "love11-luajit"
extension = "tua"
emit_maps = true
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/love")).expect("failed to create love source");
    fs::write(temp.path().join("src/main.tua"), "return \"default\"\n")
        .expect("failed to write default main");
    fs::write(temp.path().join("src/love/boot.tua"), "return \"boot\"\n")
        .expect("failed to write configured main");

    let output = tua()
        .args(["build", "--emit-on-error"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build --emit-on-error");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("build/main.lua"));
    assert_eq!(
        fs::read_to_string(temp.path().join("build/love/boot.lua"))
            .expect("failed to read mirrored output"),
        "return \"boot\"\n"
    );
    assert!(!temp.path().join("build/main.lua").exists());
    assert!(!temp.path().join("build/main.lua.map.json").exists());
    assert!(!temp.path().join("build/main.lua.map").exists());
}

#[test]
fn build_respects_disabled_source_maps() {
    let temp = TempDir::new("build-no-source-maps");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "src"
out = "build"
target = "love11-luajit"
extension = "tua"
emit_maps = false
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(temp.path().join("src/main.tua"), "return 1\n").expect("failed to write source");
    fs::create_dir_all(temp.path().join("build")).expect("failed to create stale output");
    fs::write(temp.path().join("build/main.lua.map.json"), "{}\n")
        .expect("failed to write stale Tua source map");
    fs::write(temp.path().join("build/main.lua.map"), "{}\n")
        .expect("failed to write stale debugger source map");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());
    assert!(temp.path().join("build/main.lua").is_file());
    assert!(!temp.path().join("build/main.lua.map.json").exists());
    assert!(!temp.path().join("build/main.lua.map").exists());
}

#[test]
fn declaration_modules_are_checked_but_never_emitted() {
    let temp = TempDir::new("declaration-module-output");
    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"
out = "build"
main = "main.tua"
extension = "tua"
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("build")).expect("failed to create build");
    fs::write(
        temp.path().join("src/main.tua"),
        "import type { Player } from \"./types\"\nlocal player: Player = { hp = 1 }\n",
    )
    .expect("failed to write main");
    fs::write(
        temp.path().join("src/types.d.tua"),
        "export type Player = { hp: number }\n",
    )
    .expect("failed to write declarations");
    fs::write(temp.path().join("build/types.d.lua"), "stale\n")
        .expect("failed to write stale declaration output");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(temp.path().join("build/main.lua").is_file());
    assert!(!temp.path().join("build/types.d.lua").exists());
}

#[test]
fn declaration_modules_reject_runtime_statements() {
    let temp = TempDir::new("declaration-module-runtime");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(temp.path().join("src/main.tua"), "return {}\n").expect("failed to write main");
    fs::write(temp.path().join("src/bad.d.tua"), "local value = 1\n")
        .expect("failed to write declaration");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("declaration files"));
    assert!(!temp.path().join("build/bad.d.lua").exists());
}

#[test]
fn asset_pipeline_caches_typed_generated_modules_and_invalidates_by_content() {
    let temp = TempDir::new("asset-pipeline-cache");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("assets")).expect("failed to create assets");
    fs::create_dir_all(temp.path().join("tools")).expect("failed to create tools");
    fs::write(temp.path().join("assets/world.txt"), "first\n")
        .expect("failed to write pipeline input");
    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"
out = "build"
main = "main.tua"
extension = "tua"

[[asset_pipelines]]
name = "world"
inputs = ["assets/**/*.txt", "tools/import.sh"]
command = ["sh", "tools/import.sh"]
output = "src/generated/world"
version = "1"
"#,
    )
    .expect("failed to write config");
    fs::write(
        temp.path().join("src/main.tua"),
        "import type { Room } from \"./generated/world/types\"\nlocal _room: Room = { name = \"start\" }\n",
    )
    .expect("failed to write main");
    fs::write(
        temp.path().join("tools/import.sh"),
        r#"set -eu
count_file="$TUA_ASSET_PIPELINE_PROJECT_ROOT/runs.txt"
count=0
if [ -f "$count_file" ]; then count=$(sed -n '1p' "$count_file"); fi
count=$((count + 1))
printf '%s\n' "$count" > "$count_file"
mkdir -p "$TUA_ASSET_PIPELINE_OUTPUT_DIR"
printf 'export type Room = { name: string }\n' > "$TUA_ASSET_PIPELINE_OUTPUT_DIR/types.d.tua"
printf '{"version":1,"outputs":["types.d.tua"],"diagnostics":[]}\n' > "$TUA_ASSET_PIPELINE_MANIFEST"
"#,
    )
    .expect("failed to write importer");

    let first = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run first check");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("runs.txt")).expect("missing importer count"),
        "1\n"
    );
    assert!(temp
        .path()
        .join("src/generated/world/types.d.tua")
        .is_file());

    let cached = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run cached check");
    assert!(cached.status.success());
    assert_eq!(
        fs::read_to_string(temp.path().join("runs.txt")).expect("missing importer count"),
        "1\n",
        "unchanged inputs should reuse the content-addressed cache"
    );

    fs::write(temp.path().join("assets/world.txt"), "second\n")
        .expect("failed to update pipeline input");
    let rebuilt = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to build after input edit");
    assert!(
        rebuilt.status.success(),
        "{}",
        String::from_utf8_lossy(&rebuilt.stderr)
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("runs.txt")).expect("missing importer count"),
        "2\n"
    );
    assert!(temp.path().join("build/main.lua").is_file());
    assert!(!temp
        .path()
        .join("build/generated/world/types.d.lua")
        .exists());
}

#[test]
fn asset_pipeline_surfaces_structured_importer_diagnostics() {
    let temp = TempDir::new("asset-pipeline-diagnostic");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("assets")).expect("failed to create assets");
    fs::create_dir_all(temp.path().join("tools")).expect("failed to create tools");
    fs::write(temp.path().join("src/main.tua"), "return {}\n").expect("failed to write main");
    fs::write(temp.path().join("assets/world.txt"), "bad room\n").expect("failed to write input");
    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"
out = "build"

[[asset_pipelines]]
name = "world"
inputs = ["assets/world.txt", "tools/import.sh"]
command = ["sh", "tools/import.sh"]
output = "src/generated/world"
"#,
    )
    .expect("failed to write config");
    fs::write(
        temp.path().join("tools/import.sh"),
        r#"set -eu
printf '{"version":1,"outputs":[],"diagnostics":[{"severity":"error","message":"unknown room symbol","path":"assets/world.txt","line":1,"column":5}]}\n' > "$TUA_ASSET_PIPELINE_MANIFEST"
"#,
    )
    .expect("failed to write importer");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run check");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("assets/world.txt:1:5 error TL2011"),
        "{stderr}"
    );
    assert!(stderr.contains("asset pipeline `world`: unknown room symbol"));
}

#[cfg(unix)]
#[test]
fn asset_pipeline_times_out_and_removes_staging_output() {
    let temp = TempDir::new("asset-pipeline-timeout");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("assets")).expect("failed to create assets");
    fs::create_dir_all(temp.path().join("tools")).expect("failed to create tools");
    fs::write(temp.path().join("src/main.tua"), "return {}\n").expect("failed to write main");
    fs::write(temp.path().join("assets/world.txt"), "world\n").expect("failed to write input");
    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"
out = "build"

[[asset_pipelines]]
name = "world"
inputs = ["assets/world.txt", "tools/import.sh"]
command = ["sh", "tools/import.sh"]
output = "src/generated/world"
timeout_ms = 20
"#,
    )
    .expect("failed to write config");
    fs::write(temp.path().join("tools/import.sh"), "sleep 2\n").expect("failed to write importer");

    // A freshly linked macOS executable can spend seconds in OS validation.
    // Measure importer cancellation independently of that first launch cost.
    assert!(tua().arg("--version").output().unwrap().status.success());
    let started = Instant::now();
    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run check");
    assert!(!output.status.success());
    assert!(started.elapsed() < Duration::from_millis(1_500));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("importer timed out after 20 ms"),
        "{stderr}"
    );
    let cache = temp.path().join(".tua/cache/asset-pipelines/world");
    assert!(
        fs::read_dir(cache)
            .expect("pipeline cache directory should exist")
            .next()
            .is_none(),
        "timed-out importer staging directory should be removed"
    );
}

#[cfg(unix)]
#[test]
fn asset_pipeline_rejects_oversized_manifest_and_removes_staging_output() {
    let temp = TempDir::new("asset-pipeline-manifest-limit");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("assets")).expect("failed to create assets");
    fs::create_dir_all(temp.path().join("tools")).expect("failed to create tools");
    fs::write(temp.path().join("src/main.tua"), "return {}\n").expect("failed to write main");
    fs::write(temp.path().join("assets/world.txt"), "world\n").expect("failed to write input");
    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"
out = "build"

[[asset_pipelines]]
name = "world"
inputs = ["assets/world.txt", "tools/import.sh"]
command = ["sh", "tools/import.sh"]
output = "src/generated/world"
max_manifest_bytes = 64
"#,
    )
    .expect("failed to write config");
    fs::write(
        temp.path().join("tools/import.sh"),
        "printf '%0128d' 0 > \"$TUA_ASSET_PIPELINE_MANIFEST\"\n",
    )
    .expect("failed to write importer");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run check");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("manifest exceeds the 64-byte limit"),
        "{stderr}"
    );
    let cache = temp.path().join(".tua/cache/asset-pipelines/world");
    assert!(
        fs::read_dir(cache)
            .expect("pipeline cache directory should exist")
            .next()
            .is_none(),
        "invalid importer staging directory should be removed"
    );
}

#[cfg(unix)]
#[test]
fn asset_pipeline_bounds_failed_importer_stderr() {
    let temp = TempDir::new("asset-pipeline-stderr-limit");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("assets")).expect("failed to create assets");
    fs::create_dir_all(temp.path().join("tools")).expect("failed to create tools");
    fs::write(temp.path().join("src/main.tua"), "return {}\n").expect("failed to write main");
    fs::write(temp.path().join("assets/world.txt"), "world\n").expect("failed to write input");
    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"
out = "build"

[[asset_pipelines]]
name = "world"
inputs = ["assets/world.txt", "tools/import.sh"]
command = ["sh", "tools/import.sh"]
output = "src/generated/world"
max_stderr_bytes = 8
"#,
    )
    .expect("failed to write config");
    fs::write(
        temp.path().join("tools/import.sh"),
        "printf 'abcdefghSHOULD_NOT_APPEAR' >&2\nexit 7\n",
    )
    .expect("failed to write importer");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run check");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("abcdefgh… [stderr truncated]"), "{stderr}");
    assert!(!stderr.contains("SHOULD_NOT_APPEAR"), "{stderr}");
}

#[test]
fn builtin_audio_analysis_generates_typed_cached_catalog() {
    let temp = TempDir::new("builtin-audio-analysis");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("assets/audio")).expect("failed to create assets");
    fs::write(
        temp.path().join("src/main.tua"),
        "local catalog = require(\"generated.audio.catalog\")\nreturn catalog\n",
    )
    .expect("failed to write main");
    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"
out = "build"
main = "main.tua"
extension = "tua"

[[asset_pipelines]]
name = "audio"
builtin = "audio-analysis"
inputs = ["assets/audio/*.wav"]
output = "src/generated/audio"
version = "1"

[asset_pipelines.options]
envelope_hz = 20
tempo_min = 60
tempo_max = 180
confidence_warning = 0
"#,
    )
    .expect("failed to write config");
    let rate = 8000_u32;
    let frames = rate * 4;
    let data_bytes = frames * 2;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    for index in 0..frames {
        let pulse: i16 = if index % (rate / 2) < 80 { 16000 } else { 0 };
        wav.extend_from_slice(&pulse.to_le_bytes());
    }
    fs::write(temp.path().join("assets/audio/pulse.wav"), wav).expect("failed to write wav");

    let first = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run check");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    for output in ["catalog.tua", "catalog_data.lua", "audio-index.json"] {
        assert!(
            temp.path()
                .join("src/generated/audio")
                .join(output)
                .is_file(),
            "missing {output}"
        );
    }
    let index: Value = serde_json::from_slice(
        &fs::read(temp.path().join("src/generated/audio/audio-index.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(index[0]["channels"], 1);
    assert_eq!(index[0]["sampleRate"], rate);
    assert!(index[0]["energy"]
        .as_array()
        .is_some_and(|values| !values.is_empty()));
    assert!(index[0]["tempoCandidates"].is_array());
    assert!(index[0]["tempoAmbiguity"].is_number());
    assert!(index[0]["boundaryInfluence"].is_number());
    let cached = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run cached check");
    assert!(
        cached.status.success(),
        "{}",
        String::from_utf8_lossy(&cached.stderr)
    );

    let built = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to build analyzed audio project");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(temp
        .path()
        .join("build/generated/audio/catalog.lua")
        .is_file());
    assert!(temp
        .path()
        .join("build/generated/audio/catalog_data.lua")
        .is_file());
    assert!(!temp
        .path()
        .join("build/generated/audio/audio-index.json")
        .exists());
}

#[test]
fn audio_runtime_priority_fades_pooling_and_snapshots() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(&lua)
        .arg(root.join("crates/tua-cli/tests/fixtures/audio_runtime.lua"))
        .arg(&root)
        .output()
        .expect("failed to run audio runtime fixture");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn signal_runtime_uses_copy_on_write_listener_versions() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(&lua)
        .arg(root.join("crates/tua-cli/tests/fixtures/signal_runtime.lua"))
        .arg(&root)
        .output()
        .expect("failed to run signal runtime fixture");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn object_pool_runtime_uses_explicit_counts_and_one_state_map() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(&lua)
        .arg(root.join("crates/tua-cli/tests/fixtures/object_pool_runtime.lua"))
        .arg(&root)
        .output()
        .expect("failed to run object pool runtime fixture");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn sprite_runtime_draws_leaves_directly_and_preserves_nested_transforms() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(&lua)
        .arg(root.join("crates/tua-cli/tests/fixtures/sprite_runtime.lua"))
        .arg(&root)
        .output()
        .expect("failed to run sprite runtime fixture");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn fused_vec2_arithmetic_matches_brinevector_results() {
    let temp = TempDir::new("vec2-fusion-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/fused.tua"),
        r#"local function cases(p: Vec2, v: Vec2, g: Vec2, dt: number, k: number)
  local moved = p
  moved += v * dt
  local scaled = vec2(p.x, p.y)
  scaled *= k + 1
  local halved = v
  halved /= 2
  return {
    p + v * dt,
    -(p - v) * 2,
    p + (v + g),
    (p * v) / 2,
    0.5 * dt * v,
    vec2(1, 2) * k,
    p - -v,
    p + v * dt + g * (0.5 * dt * dt),
    v * k / 4,
    moved,
    scaled,
    halved,
  }
end

return { cases = cases }
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let emitted =
        fs::read_to_string(temp.path().join("build/fused.lua")).expect("failed to read output");
    assert!(
        emitted.contains("__tua_vec2(p.x + v.x * dt, p.y + v.y * dt)"),
        "expected fused Vec2 arithmetic:\n{emitted}"
    );
    assert!(
        !emitted.contains("v * dt"),
        "every expression should be fused:\n{emitted}"
    );

    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(&lua)
        .arg(root.join("crates/tua-cli/tests/fixtures/vec2_fusion_check.lua"))
        .arg(temp.path().join("build"))
        .output()
        .expect("failed to run Vec2 fusion fixture");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn vec2_fusion_uses_types_inferred_from_required_modules() {
    let temp = TempDir::new("vec2-fusion-required-module");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/points.tua"),
        r#"local function origin(): Vec2
  return vec2(10, 20)
end

return { origin = origin }
"#,
    )
    .expect("failed to write typed module");
    fs::write(
        temp.path().join("src/lua_points.lua"),
        r#"local Vector = require("brinevector")

---@return Vec2
local function origin()
  return Vector(30, 40)
end

return { origin = origin }
"#,
    )
    .expect("failed to write annotated Lua module");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local points = require("points")
local luaPoints = require("lua_points")
local position = points.origin()
local other = luaPoints.origin()
local doubled = position + position
local mixed = doubled + other
return mixed
"#,
    )
    .expect("failed to write consumer");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let consumer =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read consumer");
    assert!(
        consumer.contains(
            "local doubled = __tua_vec2(position.x + position.x, position.y + position.y)"
        ),
        "required-module Vec2 should fuse without spelling vec2 or Vec2 in the consumer:\n{consumer}"
    );
    assert!(
        consumer.contains("local mixed = __tua_vec2(doubled.x + other.x, doubled.y + other.y)"),
        "annotated Lua Vec2 should feed the same project-aware fusion:\n{consumer}"
    );
}

#[test]
fn hoisted_vec2_reads_happen_once_and_match_brinevector() {
    let temp = TempDir::new("vec2-hoisting-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/hoisted.tua"),
        r#"type Body = {
  pos: Vec2,
  vel: Vec2,
  speed: number,
}

local function integrate(body: Body, dt: number)
  body.pos += body.vel * dt
end

local function advance(positions: Vec2[], velocities: Vec2[], i: number, dt: number)
  positions[i] = positions[i] + velocities[i] * dt
end

local function predict(body: Body, dt: number): Vec2
  local predicted = body.pos + body.vel * (body.speed * dt)
  return predicted
end

local function offset(body: Body, extra: Vec2): Vec2
  return -body.pos + extra * 2
end

local function failAfterReads(body: Body): Vec2
  return body.pos + body.vel
end

return {
  integrate = integrate,
  advance = advance,
  predict = predict,
  offset = offset,
  failAfterReads = failAfterReads,
}
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let emitted =
        fs::read_to_string(temp.path().join("build/hoisted.lua")).expect("failed to read output");
    for expected in [
        "local __tua_body_pos = body.pos; local __tua_body_vel = body.vel; body.pos = __tua_vec2(",
        "local __tua_positions_i = positions[i]; local __tua_velocities_i = velocities[i]; positions[i] = __tua_vec2(",
        "local predicted; do local __tua_body_pos = body.pos;",
        "do local __tua_body_pos = body.pos; return __tua_vec2(-__tua_body_pos.x + extra.x * 2,",
    ] {
        assert!(
            emitted.contains(expected),
            "expected `{expected}` in:\n{emitted}"
        );
    }

    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(&lua)
        .arg(root.join("crates/tua-cli/tests/fixtures/vec2_hoisting_check.lua"))
        .arg(temp.path().join("build"))
        .output()
        .expect("failed to run Vec2 hoisting fixture");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ecs_runtime_iteration_semantics_and_allocations() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(&lua)
        .arg(root.join("crates/tua-cli/tests/fixtures/ecs_runtime.lua"))
        .arg(&root)
        .output()
        .expect("failed to run ECS runtime fixture");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn store_runtime_notification_semantics_and_allocations() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(&lua)
        .arg(root.join("crates/tua-cli/tests/fixtures/store_runtime.lua"))
        .arg(&root)
        .output()
        .expect("failed to run store runtime fixture");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn build_emits_audio_runtime_and_transitive_helpers_on_demand() {
    let temp = TempDir::new("audio-runtime-build");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::write(temp.path().join("src/main.tua"), "local audio = audioSystem({ buses = { master = {} }, cues = {}, music = {} })\nreturn audio\n").expect("failed to write main");
    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to build");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for runtime in ["tuaaudio.lua", "tuasignal.lua", "tuastore.lua"] {
        assert!(
            temp.path().join("build").join(runtime).is_file(),
            "missing {runtime}"
        );
    }
    assert!(fs::read_to_string(temp.path().join("build/main.lua"))
        .unwrap()
        .contains("require(\"tuaaudio\")"));
}

#[test]
fn traceback_remaps_generated_lua_locations_from_source_maps() {
    let temp = TempDir::new("traceback-source-map");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"type Player = {
  hp: number,
}

local player: Player = { hp = 10 }
player.hp += 1
"#,
    )
    .expect("failed to write source");

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

    fs::write(
        temp.path().join("sentry-traceback.txt"),
        "build/main.lua:2: attempt to index nil\nstack traceback:\n",
    )
    .expect("failed to write traceback");

    let output = tua()
        .args(["traceback", "--maps", "build", "sentry-traceback.txt"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua traceback");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert_eq!(
        stdout,
        "src/main.tua:6 (generated build/main.lua:2): attempt to index nil\nstack traceback:\n"
    );
}

#[test]
fn traceback_remaps_mixed_lowering_from_compiler_provenance() {
    let temp = TempDir::new("traceback-mixed-lowering");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"type Count = number

local function tick(value: Count = 1)
  local position = vec2(1, 2)
  local events = signal()
  value += 2
  if value is Count then
    events:emit("tick", position)
  end
  return value
end

tick()
"#,
    )
    .expect("failed to write source");

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

    let map = fs::read_to_string(temp.path().join("build/main.lua.map.json"))
        .expect("failed to read source map");
    let map: Value = serde_json::from_str(&map).expect("source map should be JSON");
    let mappings = map["lineMappings"]
        .as_array()
        .expect("source map should contain line mappings");
    let source_line_for = |generated_line: u64| {
        mappings
            .iter()
            .find(|mapping| mapping["generatedLine"] == generated_line)
            .and_then(|mapping| mapping["sourceLine"].as_u64())
    };

    assert_eq!(source_line_for(1), Some(5));
    assert_eq!(source_line_for(4), Some(3));
    assert_eq!(source_line_for(9), Some(6));
    assert_eq!(source_line_for(10), Some(7));

    fs::write(
        temp.path().join("mixed-traceback.txt"),
        "build/main.lua:1: runtime setup failed\nbuild/main.lua:9: arithmetic failed\n",
    )
    .expect("failed to write traceback");

    let output = tua()
        .args(["traceback", "--maps", "build", "mixed-traceback.txt"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua traceback");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        "src/main.tua:5 (generated build/main.lua:1): runtime setup failed\nsrc/main.tua:6 (generated build/main.lua:9): arithmetic failed\n"
    );
}

#[test]
fn traceback_remaps_an_actual_emitted_lua_runtime_error() {
    let Some(lua) = working_lua_runtime() else {
        eprintln!("skipping runtime traceback test; no Lua executable was found");
        return;
    };
    let temp = TempDir::new("traceback-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"type Message = {
  text: string,
}

local greeting: string = "Olá"
local player: any = nil
print(player.name)
"#,
    )
    .expect("failed to write runtime traceback source");

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

    let runtime = Command::new(lua)
        .arg("build/main.lua")
        .current_dir(temp.path())
        .output()
        .expect("failed to execute emitted Lua");
    assert!(
        !runtime.status.success(),
        "emitted Lua should fail at the intentional nil field access"
    );
    let traceback = String::from_utf8(runtime.stderr).expect("Lua traceback should be UTF-8");
    assert!(
        traceback.contains("build/main.lua:3:"),
        "unexpected runtime traceback:\n{traceback}"
    );
    fs::write(temp.path().join("runtime-traceback.txt"), &traceback)
        .expect("failed to write captured runtime traceback");

    let remapped = tua()
        .args(["traceback", "--maps", "build", "runtime-traceback.txt"])
        .current_dir(temp.path())
        .output()
        .expect("failed to remap captured runtime traceback");
    assert!(
        remapped.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&remapped.stdout),
        String::from_utf8_lossy(&remapped.stderr)
    );
    let remapped = String::from_utf8(remapped.stdout).expect("remapped traceback should be UTF-8");

    assert!(
        remapped.contains("src/main.tua:7 (generated build/main.lua:3)"),
        "unexpected remapped traceback:\n{remapped}"
    );
}

#[test]
fn build_lowers_static_imports_to_require_calls() {
    let temp = TempDir::new("build-relative-import");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/entities")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local Player: any = import("./entities/player")

return Player
"#,
    )
    .expect("failed to write main.tua");
    fs::write(
        temp.path().join("src/entities/player.tua"),
        r#"
return {
  name = "Ada",
}
"#,
    )
    .expect("failed to write player.tua");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let main_output =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read main output");

    assert!(main_output.contains(r#"local Player = require("entities.player")"#));
    assert!(!main_output.contains("import("));
}

#[test]
fn build_emits_luacats_annotations_when_configured() {
    let temp = TempDir::new("build-luacats");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "src"
out = "build"
target = "love11-luajit"
extension = "tua"
emit_maps = true

[emit]
annotation_mode = "luacats"
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
type Player = {
  hp: number,
}

local player: Player = { hp = 10 }

local function make_player(name: string): Player
  player.hp = #name
  return player
end

return {
  player = player,
  make_player = make_player,
}
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");

    assert!(emitted.contains("---@alias Player { hp: number }"));
    assert!(emitted.contains("---@type Player\nlocal player = { hp = 10 }"));
    assert!(emitted.contains("---@param name string\n---@return Player"));
    assert!(emitted.contains("---@field player Player"));
    assert!(emitted.contains("---@field make_player fun(name: string): Player"));
    assert!(!emitted.contains("local player: Player"));
}

#[test]
fn build_makes_imported_luacats_aliases_self_contained() {
    let temp = TempDir::new("build-imported-luacats");
    fs::write(
        temp.path().join(CONFIG_FILE),
        "source = \"src\"\nout = \"build\"\n\n[emit]\nannotation_mode = \"luacats\"\n",
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/types.tua"),
        "export type Kind = \"I\" | \"O\"\nexport type Game = { score: number, next: Kind }\n",
    )
    .expect("failed to write types");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"import type Types from "./types"
import type { Kind as PieceKind } from "./types"
local game: Types.Game = { score = 0, next = "I" }
local function set_next(kind: PieceKind): Types.Game
  print(kind)
  return game
end
return { game = game, set_next = set_next }
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");

    assert!(emitted.contains("---@type { next: \"I\" | \"O\", score: number }"));
    assert!(emitted.contains("---@param kind \"I\" | \"O\""));
    assert!(!emitted.contains("Types.Game"));
    assert!(!emitted.contains("PieceKind"));
}

#[test]
fn build_accepts_project_aware_types_from_tua_and_lua_modules() {
    let temp = TempDir::new("build-project-types");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local Types = require("types")
local LuaDocTypes = require("lua_doc_types")

local player: Types.Player = nil
local enemy: LuaDocTypes.Enemy = nil

return {
  player = player,
  enemy = enemy,
}
"#,
    )
    .expect("failed to write main source");
    fs::write(
        temp.path().join("src/types.tua"),
        r#"
export type Player = { hp: number }

return {}
"#,
    )
    .expect("failed to write tua type provider");
    fs::write(
        temp.path().join("src/lua_doc_types.lua"),
        r#"
---@class Enemy
---@field hp number
local Enemy = {}

return {
  Enemy = Enemy,
}
"#,
    )
    .expect("failed to write lua type provider");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");

    assert!(stdout.contains("built 3 file(s)"));
    assert!(!emitted.contains("Types.Player"));
    assert!(!emitted.contains("LuaDocTypes.Enemy"));
    assert!(temp.path().join("build/lua_doc_types.lua").is_file());
}

#[test]
fn build_indexes_type_libraries_without_copying_or_emitting_them() {
    let temp = TempDir::new("build-type-library");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "src"
out = "build"

[[type_libraries]]
path = "src/vendor-types"
declaration_prefix = "feel"
runtime_prefix = "lib.feel"
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/vendor-types"))
        .expect("failed to create type library");
    fs::write(
        temp.path().join("src/main.tua"),
        "local feel = require(\"lib.feel\")\nreturn feel\n",
    )
    .expect("failed to write source");
    fs::write(
        temp.path().join("src/vendor-types/feel.lua"),
        "---@meta feel\nreturn { define = function() end }\n",
    )
    .expect("failed to write declaration");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        "built 1 file(s)\n"
    );
    assert!(temp.path().join("build/main.lua").is_file());
    assert!(!temp.path().join("build/vendor-types").exists());
}

#[test]
fn build_preserves_custom_relative_paths_with_only_extension_changed() {
    let temp = TempDir::new("build-custom-layout");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "game"
out = "dist/lua"
target = "love11-luajit"
extension = "tua"
emit_maps = true
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("game/client/ui")).expect("failed to create source");
    fs::write(
        temp.path().join("game/client/ui/menu.tua"),
        r#"
local Menu = {}
return Menu
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());
    assert!(temp.path().join("dist/lua/client/ui/menu.lua").is_file());
    assert!(!temp.path().join("dist/lua/client/ui/menu.tua").exists());
}

#[test]
fn build_copies_non_tua_files_without_compiling_them() {
    let temp = TempDir::new("build-copies-assets");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/lib")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("src/assets")).expect("failed to create assets");
    fs::write(
        temp.path().join("src/lib/legacy.lua"),
        "local value = 1\nvalue = value + 1\nreturn value\n",
    )
    .expect("failed to write lua source");
    fs::write(
        temp.path().join("src/assets/player.png"),
        [0x89, b'P', b'N', b'G'],
    )
    .expect("failed to write asset");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let copied = fs::read_to_string(temp.path().join("build/lib/legacy.lua"))
        .expect("failed to read copied lua output");
    let copied_asset =
        fs::read(temp.path().join("build/assets/player.png")).expect("failed to read copied asset");

    assert!(stdout.contains("built 2 file(s)"));
    assert_eq!(copied, "local value = 1\nvalue = value + 1\nreturn value\n");
    assert_eq!(copied_asset, [0x89, b'P', b'N', b'G']);
}

#[test]
fn build_writes_brinevector_runtime_when_vec2_is_used() {
    let temp = TempDir::new("build-vec2-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local position: Vec2 = vec2(100, 200)\nreturn { position = position }\n",
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");
    let runtime = fs::read_to_string(temp.path().join("build/brinevector.lua"))
        .expect("failed to read BrineVector runtime");

    assert!(stdout.contains("built 2 file(s)"));
    assert!(emitted.contains(r#"local __tua_vec2 = require("brinevector")"#));
    assert!(emitted.contains("local position = __tua_vec2(100, 200)"));
    assert!(runtime.contains("BrineVector: a luajit ffi-accelerated vector library"));
}

#[test]
fn build_bundles_runtimes_required_only_by_lua_sources() {
    let temp = TempDir::new("lua-explicit-runtime-imports");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    let source = r#"local store = require('tuastore')
local audioSystem = require("tuaaudio")

return {
  store = store,
  audioSystem = audioSystem,
}
"#;
    fs::write(temp.path().join("src/main.lua"), source).expect("failed to write Lua source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("build/main.lua"))
            .expect("failed to read copied Lua source"),
        source
    );
    for runtime in ["tuastore.lua", "tuaaudio.lua", "tuasignal.lua"] {
        assert!(
            temp.path().join("build").join(runtime).is_file(),
            "expected {runtime} to be bundled"
        );
    }
}

#[test]
fn in_place_build_bundles_runtimes_required_only_by_lua_sources() {
    let temp = TempDir::new("lua-explicit-runtime-imports-in-place");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "game"
out = "game"
target = "love11-luajit"
extension = "tua"
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("game")).expect("failed to create source");
    let source = "local Store = require('tuastore')\nreturn Store({ score = 0 })\n";
    fs::write(temp.path().join("game/main.lua"), source).expect("failed to write Lua source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("game/main.lua"))
            .expect("failed to read original Lua source"),
        source
    );
    assert!(temp.path().join("game/tuastore.lua").is_file());
}

#[test]
fn build_does_not_overwrite_project_brinevector_runtime() {
    let temp = TempDir::new("build-custom-vec2-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local position = vec2(100, 200)\nreturn { position = position }\n",
    )
    .expect("failed to write source");
    fs::write(
        temp.path().join("src/brinevector.lua"),
        "return function(x, y) return { x = x, y = y, custom = true } end\n",
    )
    .expect("failed to write custom runtime");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let runtime = fs::read_to_string(temp.path().join("build/brinevector.lua"))
        .expect("failed to read custom runtime");

    assert_eq!(
        runtime,
        "return function(x, y) return { x = x, y = y, custom = true } end\n"
    );
}

#[test]
fn build_writes_state_machine_runtime_when_helper_is_used() {
    let temp = TempDir::new("build-state-machine-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local light = stateMachine({
  initial = "green",
  events = {
    { name = "warn", from = "green", to = "yellow" },
  },
})
return light
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");
    let runtime = fs::read_to_string(temp.path().join("build/tuastatemachine.lua"))
        .expect("failed to read state machine runtime");

    assert!(stdout.contains("built 2 file(s)"));
    assert!(emitted.contains(r#"local __tua_state_machine = require("tuastatemachine").create"#));
    assert!(emitted.contains("local light = __tua_state_machine({"));
    assert!(runtime.contains("Copyright (c) 2012 Kyle Conroy"));
    assert!(runtime.contains("function machine.create(options)"));
}

#[test]
fn state_machine_runtime_preserves_sync_async_and_cancel_semantics() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(&lua)
        .arg(root.join("crates/tua-cli/tests/fixtures/state_machine_runtime.lua"))
        .arg(&root)
        .output()
        .expect("failed to run state machine runtime fixture");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn build_does_not_overwrite_project_state_machine_runtime() {
    let temp = TempDir::new("build-custom-state-machine-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local light = stateMachine({
  initial = "green",
  events = {
    { name = "warn", from = "green", to = "yellow" },
  },
})
return light
"#,
    )
    .expect("failed to write source");
    fs::write(
        temp.path().join("src/tuastatemachine.lua"),
        "return { create = function(options) return { custom = true } end }\n",
    )
    .expect("failed to write custom runtime");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let runtime = fs::read_to_string(temp.path().join("build/tuastatemachine.lua"))
        .expect("failed to read custom runtime");

    assert_eq!(
        runtime,
        "return { create = function(options) return { custom = true } end }\n"
    );
}

#[test]
fn build_writes_anim8_runtime_when_sprite_sheet_helper_is_used() {
    let temp = TempDir::new("build-sprite-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/assets")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = { from = 0, to = 3, fps = 8 },
  },
})

return Player
"#,
    )
    .expect("failed to write source");
    fs::write(
        temp.path().join("src/assets/player.png"),
        [0x89, b'P', b'N', b'G'],
    )
    .expect("failed to write asset");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");
    let runtime =
        fs::read_to_string(temp.path().join("build/anim8.lua")).expect("failed to read anim8");

    assert!(stdout.contains("built 4 file(s)"));
    assert!(emitted.contains(r#"local anim8 = require("anim8")"#));
    assert!(emitted.contains(r#"local tuasprite = require("tuasprite")"#));
    assert!(emitted.contains("idle = anim8.newAnimation(PlayerGrid(\"1-4\", 1), 1 / 8)"));
    assert!(runtime.contains("_VERSION     = 'anim8 v2.3.1'"));
    assert!(runtime.contains("_URL         = 'https://github.com/kikito/anim8'"));
    let player_runtime = fs::read_to_string(temp.path().join("build/tuasprite.lua"))
        .expect("failed to read sprite player runtime");
    assert!(player_runtime.contains("function SpritePlayer:gotoAndPlay"));
    assert!(player_runtime.contains("function SpritePlayer:seekFrame"));
}

#[test]
fn build_does_not_overwrite_project_anim8_runtime() {
    let temp = TempDir::new("build-custom-sprite-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/assets")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = { from = 0, to = 3, fps = 8 },
  },
})

return Player
"#,
    )
    .expect("failed to write source");
    fs::write(
        temp.path().join("src/assets/player.png"),
        [0x89, b'P', b'N', b'G'],
    )
    .expect("failed to write asset");
    fs::write(
        temp.path().join("src/anim8.lua"),
        "return { custom = true }\n",
    )
    .expect("failed to write custom runtime");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let runtime =
        fs::read_to_string(temp.path().join("build/anim8.lua")).expect("failed to read anim8");

    assert_eq!(runtime, "return { custom = true }\n");
}

#[test]
fn build_does_not_overwrite_project_tuasprite_runtime() {
    let temp = TempDir::new("build-custom-sprite-player-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/assets")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = { from = 0, to = 3, fps = 8 },
  },
})

return Player:player("idle")
"#,
    )
    .expect("failed to write source");
    fs::write(
        temp.path().join("src/assets/player.png"),
        [0x89, b'P', b'N', b'G'],
    )
    .expect("failed to write asset");
    fs::write(
        temp.path().join("src/tuasprite.lua"),
        "return { new = function(sheet) return { custom = sheet } end }\n",
    )
    .expect("failed to write custom runtime");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());
    let runtime = fs::read_to_string(temp.path().join("build/tuasprite.lua"))
        .expect("failed to read sprite player runtime");
    assert_eq!(
        runtime,
        "return { new = function(sheet) return { custom = sheet } end }\n"
    );
}

#[test]
fn build_writes_tuasignal_runtime_when_signal_helper_is_used() {
    let temp = TempDir::new("build-signal-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local events = signal()
events:on("player.hit", function(player, damage)
  print(player, damage)
end)
events:emit("player.hit", "ada", 4)
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");
    let runtime = fs::read_to_string(temp.path().join("build/tuasignal.lua"))
        .expect("failed to read tuasignal");

    assert!(stdout.contains("built 2 file(s)"));
    assert!(emitted.contains(r#"local __tua_signal = require("tuasignal")"#));
    assert!(emitted.contains("local events = __tua_signal()"));
    assert!(runtime.contains("function SignalBus:emit(name, ...)"));
}

#[test]
fn build_does_not_overwrite_project_tuasignal_runtime() {
    let temp = TempDir::new("build-custom-signal-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local events = signal()\nreturn events\n",
    )
    .expect("failed to write source");
    fs::write(
        temp.path().join("src/tuasignal.lua"),
        "return function() return { custom = true } end\n",
    )
    .expect("failed to write custom runtime");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let runtime = fs::read_to_string(temp.path().join("build/tuasignal.lua"))
        .expect("failed to read tuasignal");

    assert_eq!(runtime, "return function() return { custom = true } end\n");
}

#[test]
fn build_writes_tuaecs_runtime_when_builtin_ecs_is_used() {
    let temp = TempDir::new("build-ecs-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local Position = ecs.component("Position", { x = "number" })
local world = ecs.world()
local entity = world:create()
world:add(entity, Position, { x = 10 })
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");
    let runtime =
        fs::read_to_string(temp.path().join("build/tuaecs.lua")).expect("failed to read tuaecs");

    assert!(stdout.contains("built 2 file(s)"));
    assert!(emitted.contains(r#"local ecs = require("tuaecs")"#));
    assert!(runtime.contains("function World:query(components)"));
    assert!(runtime.contains("function ecs.system(world, components, update)"));
}

#[test]
fn build_writes_tuaobjectpool_runtime_when_builtin_pool_is_used() {
    let temp = TempDir::new("build-object-pool-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local bullets = objectPool({
  create = function()
    return { active = false }
  end,
  initial = 8,
})
return bullets
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");
    let runtime = fs::read_to_string(temp.path().join("build/tuaobjectpool.lua"))
        .expect("failed to read tuaobjectpool");

    assert!(emitted.contains(r#"local __tua_object_pool = require("tuaobjectpool")"#));
    assert!(runtime.contains("function pool:acquire()"));
    assert!(runtime.contains("function pool:release(object)"));
}

#[test]
fn build_writes_tuastore_runtime_when_store_is_used() {
    let temp = TempDir::new("build-store-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local game = store({ score = 0 })
game:update(function(state)
  state.score += 1
end)
return game
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");
    let runtime = fs::read_to_string(temp.path().join("build/tuastore.lua"))
        .expect("failed to read tuastore");

    assert!(emitted.contains(r#"local __tua_store = require("tuastore")"#));
    assert!(runtime.contains("function Store:update(mutator)"));
    assert!(runtime.contains("function Store:watch(key, listener)"));
}

#[test]
fn build_does_not_overwrite_project_tuastore_runtime() {
    let temp = TempDir::new("build-custom-store-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local game = store({ score = 0 })\nreturn game\n",
    )
    .expect("failed to write source");
    fs::write(
        temp.path().join("src/tuastore.lua"),
        "return function(initial) return { custom = initial } end\n",
    )
    .expect("failed to write custom runtime");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let runtime = fs::read_to_string(temp.path().join("build/tuastore.lua"))
        .expect("failed to read custom tuastore");

    assert_eq!(
        runtime,
        "return function(initial) return { custom = initial } end\n"
    );
}

#[test]
fn build_writes_baton_runtime_when_input_helper_is_used() {
    let temp = TempDir::new("build-baton-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local controls = input({
  controls = { confirm = { "key:return", "button:a" } },
})
return controls
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");
    let runtime =
        fs::read_to_string(temp.path().join("build/baton.lua")).expect("failed to read baton");

    assert!(stdout.contains("built 2 file(s)"));
    assert!(emitted.contains(r#"local __tua_input = require("baton").new"#));
    assert!(emitted.contains("local controls = __tua_input({"));
    assert!(runtime.contains("_VERSION = 'Baton v1.0.2'"));
    assert!(runtime.contains("function Player:pressed(name)"));
}

#[test]
fn build_does_not_overwrite_project_baton_runtime() {
    let temp = TempDir::new("build-custom-baton-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local controls = input({ controls = {} })\nreturn controls\n",
    )
    .expect("failed to write source");
    fs::write(
        temp.path().join("src/baton.lua"),
        "return { new = function(config) return { custom = config } end }\n",
    )
    .expect("failed to write custom runtime");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let runtime = fs::read_to_string(temp.path().join("build/baton.lua"))
        .expect("failed to read custom baton");
    assert_eq!(
        runtime,
        "return { new = function(config) return { custom = config } end }\n"
    );
}

#[test]
fn build_does_not_inject_tuaecs_when_ecs_support_is_disabled() {
    let temp = TempDir::new("build-disabled-ecs-runtime");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"
out = "build"

[ecs]
enabled = false
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local world = ecs.world()\nreturn world\n",
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");

    assert!(!emitted.contains("tuaecs"));
    assert!(!temp.path().join("build/tuaecs.lua").exists());
}

#[test]
fn build_excludes_output_directory_when_source_is_project_root() {
    let temp = TempDir::new("build-root-excludes-dist");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "."
out = "dist"
target = "love11-luajit"
extension = "tua"
emit_maps = true
"#,
    )
    .expect("failed to write config");
    fs::write(temp.path().join("main.tua"), "return 1\n").expect("failed to write source");
    fs::create_dir_all(temp.path().join("dist")).expect("failed to create previous dist");
    fs::write(temp.path().join("dist/old.lua"), "return 'old'\n")
        .expect("failed to write previous dist file");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());
    assert!(temp.path().join("dist/main.lua").is_file());
    assert!(!temp.path().join("dist/dist/old.lua").exists());
}

#[test]
fn build_skips_nested_git_metadata_directories() {
    let temp = TempDir::new("build-skips-git-metadata");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "src"
out = "build"
target = "love11-luajit"
extension = "tua"
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/vendor/love.js/.git/objects/pack"))
        .expect("failed to create nested git metadata");
    fs::write(temp.path().join("src/main.tua"), "return 1\n").expect("failed to write source");
    fs::write(
        temp.path().join("src/vendor/love.js/index.html"),
        "<!doctype html>\n",
    )
    .expect("failed to write asset");
    fs::write(
        temp.path()
            .join("src/vendor/love.js/.git/objects/pack/pack.pack"),
        "pack\n",
    )
    .expect("failed to write git pack");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());
    assert!(temp.path().join("build/main.lua").is_file());
    assert!(temp
        .path()
        .join("build/vendor/love.js/index.html")
        .is_file());
    assert!(!temp.path().join("build/vendor/love.js/.git").exists());
}

#[test]
fn build_warns_and_keeps_generated_lua_when_lua_asset_collides_with_tua_output() {
    let temp = TempDir::new("build-collision");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/player.tua"),
        "local hp: number = 1\nreturn hp\n",
    )
    .expect("failed to write tua source");
    fs::write(temp.path().join("src/player.lua"), "return 'handwritten'\n")
        .expect("failed to write lua source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/player.lua")).expect("failed to read output");

    assert!(stdout.contains("warning TL2003"));
    assert!(stdout.contains("skipping asset copy"));
    assert!(stdout.contains("built 1 file(s)"));
    assert!(emitted.contains("local hp = 1"));
    assert!(!emitted.contains("handwritten"));
}

#[test]
fn build_can_transpile_tua_in_place_without_copying_assets() {
    let temp = TempDir::new("build-in-place");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "game"
out = "game"
target = "love11-luajit"
extension = "tua"
emit_maps = true
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("game/assets")).expect("failed to create source");
    fs::write(
        temp.path().join("game/main.tua"),
        "local hp: number = 2\nhp += 3\nreturn hp\n",
    )
    .expect("failed to write tua source");
    fs::write(temp.path().join("game/main.lua"), "return 'old'\n")
        .expect("failed to write colliding lua source");
    fs::write(temp.path().join("game/assets/player.png"), [1, 2, 3, 4])
        .expect("failed to write asset");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("game/main.lua")).expect("failed to read output");
    let asset = fs::read(temp.path().join("game/assets/player.png")).expect("asset should remain");

    assert!(stdout.contains("warning TL2003"));
    assert!(stdout.contains("will be overwritten"));
    assert!(stdout.contains("built 1 file(s)"));
    assert!(emitted.contains("local hp = 2"));
    assert!(emitted.contains("hp = hp + 3"));
    assert_eq!(asset, [1, 2, 3, 4]);
}

#[test]
fn build_blocks_type_errors_by_default() {
    let temp = TempDir::new("build-blocks-type-errors");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local hp: number = "full"
return hp
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("main.tua:2:20 error TL1003"));
    assert!(stderr.contains("type mismatch: expected number, found string"));
    assert!(!temp.path().join("build/main.lua").exists());
}

#[test]
fn build_emit_on_error_writes_parseable_files_and_exits_nonzero() {
    let temp = TempDir::new("build-emit-on-error");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local hp: number = "full"
hp += 1
return hp
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .args(["build", "--emit-on-error"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build --emit-on-error");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("main.tua:2:20 error TL1003"));

    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");

    assert!(emitted.contains(r#"local hp = "full""#));
    assert!(emitted.contains("hp = hp + 1"));
    assert!(!emitted.contains(": number"));
}

#[test]
fn build_rejects_luau_syntax_at_the_tua_parse_boundary() {
    let temp = TempDir::new("build-rejects-unlowered-luau");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
while running do
  continue
end
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .args(["build", "--emit-on-error"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build --emit-on-error");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("main.tua:3:3 error TL0004"));
    assert!(stderr.contains("`continue` is not part of Tua"));
    assert!(!temp.path().join("build/main.lua").exists());
}

#[test]
fn build_accepts_native_array_types_and_erases_them_from_lua() {
    let temp = TempDir::new("build-native-arrays");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
type Values = number[]

local function first(values: number[]): number
  return values[1]
end

local values: Values = { 1, 2 }
return first(values)
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");
    assert!(emitted.contains("local function first(values)"));
    assert!(emitted.contains("local values = { 1, 2 }"));
    assert!(!emitted.contains("number[]"));
    assert!(!emitted.contains("Values"));
}

#[test]
fn fmt_formats_project_files_in_place() {
    let temp = TempDir::new("fmt-in-place");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "\nlocal value=1\n\n\nlocal other=2\n\n",
    )
    .expect("failed to write source");

    let output = tua()
        .arg("fmt")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua fmt");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let formatted =
        fs::read_to_string(temp.path().join("src/main.tua")).expect("failed to read source");

    assert!(stdout.contains("formatted 1 file(s)"));
    assert_eq!(formatted, "local value = 1\n\nlocal other = 2\n");
}

#[test]
fn fmt_check_reports_unformatted_files_without_writing() {
    let temp = TempDir::new("fmt-check");
    let original = "local value=1\n";

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(temp.path().join("src/main.tua"), original).expect("failed to write source");

    let output = tua()
        .args(["fmt", "--check"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua fmt --check");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");
    let current =
        fs::read_to_string(temp.path().join("src/main.tua")).expect("failed to read source");

    assert!(stderr.contains("format check failed"));
    assert!(stderr.contains("would reformat src/main.tua"));
    assert_eq!(current, original);
}

#[cfg(unix)]
#[test]
fn fmt_rejects_symlinked_source_files() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new("fmt-symlink");
    let outside = TempDir::new("fmt-symlink-outside");
    let victim = outside.path().join("main.tua");
    let original = "local value=1\n";

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::write(&victim, original).expect("failed to write victim");
    symlink(outside.path(), temp.path().join("src")).expect("failed to create source symlink");

    let output = tua()
        .arg("fmt")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua fmt");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");
    assert!(stderr.contains("crosses a symbolic link"), "{stderr}");
    assert_eq!(fs::read_to_string(&victim).unwrap(), original);
}

#[test]
fn lint_respects_project_config() {
    let temp = TempDir::new("lint-config");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"

[diagnostics]
self_assignments = true
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local hp = 10\nhp = hp\nreturn hp\n",
    )
    .expect("failed to write source");

    let output = tua()
        .arg("lint")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua lint");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("main.tua:2"));
    assert!(stderr.contains("warning TL4003"));
    assert!(stderr.contains("self-assignment of `hp`"));
}

#[test]
fn check_reports_configured_warnings_without_failing() {
    let temp = TempDir::new("check-warning-policy");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"

[diagnostics.severity]
TL1003 = "warning"
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local value: number = \"wrong\"\nreturn value\n",
    )
    .expect("failed to write source");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    assert!(stdout.contains("warning TL1003"), "{stdout}");
    assert!(stdout.contains("checked 1 file(s)"), "{stdout}");
}

#[test]
fn check_deny_warnings_enforces_configured_warning_policy() {
    let temp = TempDir::new("check-deny-warnings");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"

[diagnostics.severity]
TL1003 = "warning"
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local value: number = \"wrong\"\nreturn value\n",
    )
    .expect("failed to write source");

    let output = tua()
        .args(["check", "--deny-warnings"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check --deny-warnings");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");
    assert!(stderr.contains("warning TL1003"), "{stderr}");
}

#[test]
fn exact_code_allow_policy_suppresses_a_diagnostic() {
    let temp = TempDir::new("check-allow-policy");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"

[diagnostics.severity]
TL1003 = "allow"
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local value: number = \"wrong\"\nreturn value\n",
    )
    .expect("failed to write source");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    assert!(!stdout.contains("TL1003"), "{stdout}");
}

#[test]
fn lint_reports_unused_functions_with_their_own_code() {
    let temp = TempDir::new("lint-unused-function");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "function unusedGlobal()\nend\nlocal function unusedLocal()\nend\n",
    )
    .expect("failed to write source");

    let output = tua()
        .arg("lint")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua lint");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert_eq!(stderr.matches("warning TL4055").count(), 2, "{stderr}");
    assert!(stderr.contains("function `unusedGlobal` is never used"));
    assert!(stderr.contains("function `unusedLocal` is never used"));
}

#[test]
fn lint_reports_dynamic_require_bindings_as_unused_imports() {
    let temp = TempDir::new("lint-unused-dynamic-require");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local moduleName = \"runtime\"\nlocal Dynamic = require(moduleName)\n",
    )
    .expect("failed to write source");

    let output = tua()
        .arg("lint")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua lint");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert_eq!(stderr.matches("warning TL4054").count(), 1, "{stderr}");
    assert!(stderr.contains("import `Dynamic` is never used; prefix with `_` to silence"));
    assert!(!stderr.contains("unused local `moduleName`"));
}

#[test]
fn lint_ignores_formatting_differences() {
    let temp = TempDir::new("lint-empty-lines");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local first = 1\n\n\nlocal second = 2\nreturn first + second\n",
    )
    .expect("failed to write source");

    let output = tua()
        .arg("lint")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua lint");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    assert!(stdout.contains("linted 1 file(s)"));
}

#[test]
fn lint_does_not_report_type_errors() {
    let temp = TempDir::new("lint-no-typecheck");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local value: number = \"wrong\"\nreturn value\n",
    )
    .expect("failed to write source");

    let output = tua()
        .arg("lint")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua lint");

    assert!(output.status.success());
}

#[test]
fn lint_strict_enables_all_lints() {
    let temp = TempDir::new("lint-strict");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local hp = 10\nhp = hp\nreturn hp\n",
    )
    .expect("failed to write source");

    let output = tua()
        .args(["lint", "--strict"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua lint --strict");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("warning TL4003"));
    assert!(stderr.contains("self-assignment of `hp`"));
}

#[test]
fn lint_strict_keeps_direct_call_only_function_guidance_opt_in() {
    let temp = TempDir::new("lint-strict-callback");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local function callback()\n  return true\nend\nprint(callback)\n",
    )
    .expect("failed to write source");

    let output = tua()
        .args(["lint", "--strict"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua lint --strict");

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    assert!(stdout.contains("linted 1 file(s)"), "{stdout}");
}

#[test]
fn watch_rebuilds_after_debounced_source_change() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("watch-rebuild");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local hp: number = 1
return hp
"#,
    )
    .expect("failed to write initial source");

    let mut child = tua()
        .arg("watch")
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_POLL_MS", "20")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "40")
        .env("TUA_INTERNAL_WATCH_RECONCILE_MS", "100")
        .env("TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run tua watch");
    let stdout_capture = CapturedChildStdout::wait_for_watch_ready(
        &mut child,
        "watch did not finish its initial build",
    );

    assert!(temp.path().join("build/main.lua").is_file());

    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local hp: number = 2
hp += 3
return hp
"#,
    )
    .expect("failed to write changed source");

    let mut output = wait_for_child_output(child, Duration::from_secs(5));
    output.stdout = stdout_capture.finish();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.is_empty(), "unexpected watch stderr:\n{stderr}");
    assert!(stdout.contains("watching src/**/*"));
    assert!(stdout.contains("initial build: built 1 file(s)"));
    assert_eq!(stdout.matches("changed 1 file(s)").count(), 1);
    assert_eq!(stdout.matches("rebuild: rebuilt 1 file(s)").count(), 1);

    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");

    assert!(emitted.contains("local hp = 2"));
    assert!(emitted.contains("hp = hp + 3"));
    assert!(!emitted.contains(": number"));
}

#[test]
fn watch_bundles_a_runtime_added_to_a_lua_source() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("watch-lua-runtime-import");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(temp.path().join("src/main.lua"), "return {}\n")
        .expect("failed to write initial source");

    let mut child = tua()
        .arg("watch")
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_POLL_MS", "20")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "40")
        .env("TUA_INTERNAL_WATCH_RECONCILE_MS", "100")
        .env("TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run tua watch");
    let stdout_capture = CapturedChildStdout::wait_for_watch_ready(
        &mut child,
        "watch did not finish its initial build",
    );

    assert!(!temp.path().join("build/tuastore.lua").exists());
    fs::write(
        temp.path().join("src/main.lua"),
        "local Store = require('tuastore')\nreturn Store({ score = 0 })\n",
    )
    .expect("failed to add runtime import");

    let mut output = wait_for_child_output(child, Duration::from_secs(5));
    output.stdout = stdout_capture.finish();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(temp.path().join("build/tuastore.lua").is_file());
}

#[test]
fn watch_polling_fallback_reloads_configuration() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("watch-config");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local hp: number = 1\nreturn hp\n",
    )
    .expect("failed to write source");

    let mut child = tua()
        .arg("watch")
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_FORCE_POLL", "1")
        .env("TUA_INTERNAL_WATCH_POLL_MS", "20")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "40")
        .env("TUA_INTERNAL_WATCH_RECONCILE_MS", "100")
        .env("TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run tua watch");
    let stdout_capture = CapturedChildStdout::wait_for_watch_ready(
        &mut child,
        "watch did not finish its initial build",
    );

    assert!(temp.path().join("build/main.lua").is_file());

    let reconfigured = DEFAULT_CONFIG_TEXT.replace("out = \"build\"", "out = \"dist\"");
    fs::write(temp.path().join(CONFIG_FILE), reconfigured).expect("failed to update config");

    let mut output = wait_for_child_output(child, Duration::from_secs(5));
    output.stdout = stdout_capture.finish();

    assert!(output.status.success());
    assert!(temp.path().join("dist/main.lua").is_file());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.is_empty(), "unexpected watch stderr:\n{stderr}");
    assert!(stdout.contains("using polling file watcher"));
    assert!(stdout.contains("changed 1 file(s)"));
    assert!(stdout.contains("configuration rebuild: rebuilt 1 file(s)"));
}

#[test]
fn watch_keeps_source_changes_when_configuration_reload_fails() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("watch-invalid-config-batch");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local hp: number = 1\nreturn hp\n",
    )
    .expect("failed to write source");

    let mut child = tua()
        .arg("watch")
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_FORCE_POLL", "1")
        .env("TUA_INTERNAL_WATCH_POLL_MS", "100")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "80")
        .env("TUA_INTERNAL_WATCH_RECONCILE_MS", "100")
        .env("TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run tua watch");
    let stdout_capture = CapturedChildStdout::wait_for_watch_ready(
        &mut child,
        "watch did not finish its initial build",
    );

    assert!(temp.path().join("build/main.lua").is_file());

    fs::write(temp.path().join(CONFIG_FILE), "[build\n").expect("failed to invalidate config");
    fs::write(
        temp.path().join("src/main.tua"),
        "local hp: number = 2\nreturn hp\n",
    )
    .expect("failed to update source");

    let mut output = wait_for_child_output(child, Duration::from_secs(5));
    output.stdout = stdout_capture.finish();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");

    assert!(
        stdout.contains("configuration reload failed; keeping previous configuration"),
        "{stdout}"
    );
    assert!(stdout.contains("rebuild: rebuilt 1 file(s)"), "{stdout}");
    assert!(stderr.contains("tua.toml"), "{stderr}");
    assert!(emitted.contains("local hp = 2"), "{emitted}");
}

#[test]
fn watch_handles_asset_create_rename_and_remove_in_uniform_batches() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("watch-assets");
    let source_assets = temp.path().join("src/assets");
    let output_assets = temp.path().join("build/assets");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(&source_assets).expect("failed to create assets");
    fs::write(
        temp.path().join("src/main.tua"),
        "local hp: number = 1\nreturn hp\n",
    )
    .expect("failed to write source");
    fs::write(source_assets.join("original.txt"), "original").expect("failed to write asset");

    let mut child = tua()
        .arg("watch")
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_FORCE_POLL", "1")
        .env("TUA_INTERNAL_WATCH_POLL_MS", "20")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "40")
        .env("TUA_INTERNAL_WATCH_RECONCILE_MS", "100")
        .env("TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS", "3")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run tua watch");
    let stdout_capture = CapturedChildStdout::wait_for_watch_ready(
        &mut child,
        "watch did not finish its initial build",
    );
    assert!(output_assets.join("original.txt").is_file());

    // Publish one complete input: a polled create-then-write can otherwise
    // consume two of the three rebuilds before the rename/delete steps.
    let staged_asset = temp.path().join("created.txt");
    fs::write(&staged_asset, "created").expect("failed to stage asset");
    fs::rename(&staged_asset, source_assets.join("created.txt")).expect("failed to publish asset");
    wait_until(
        || output_assets.join("created.txt").is_file(),
        Duration::from_secs(10),
        "watch did not copy the created asset",
    );

    fs::rename(
        source_assets.join("original.txt"),
        source_assets.join("renamed.txt"),
    )
    .expect("failed to rename asset");
    wait_until(
        || {
            output_assets.join("renamed.txt").is_file()
                && !output_assets.join("original.txt").exists()
        },
        Duration::from_secs(10),
        "watch did not apply the asset rename",
    );

    fs::remove_file(source_assets.join("created.txt")).expect("failed to remove asset");
    wait_until(
        || !output_assets.join("created.txt").exists(),
        Duration::from_secs(10),
        "watch did not remove the deleted asset output",
    );

    let mut output = wait_for_child_output(child, Duration::from_secs(5));
    output.stdout = stdout_capture.finish();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.is_empty(), "unexpected watch stderr:\n{stderr}");
    assert_eq!(stdout.matches("changed ").count(), 3, "{stdout}");
    assert_eq!(stdout.matches("rebuild: rebuilt").count(), 3, "{stdout}");
    assert!(stdout.contains("changed 2 file(s)"), "{stdout}");
    assert!(stdout.contains("removed 1 output file(s)"), "{stdout}");
}

#[test]
fn watch_rechecks_unchanged_importers_after_exported_type_changes() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("watch-dependent-types");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/types.tua"),
        r#"
local function accept(value: number): number
  return value
end

return { accept = accept }
"#,
    )
    .expect("failed to write initial type provider");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local Types = require("types")
local value: number = Types.accept(10)

return value
"#,
    )
    .expect("failed to write unchanged importer");

    let mut child = tua()
        .arg("watch")
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_POLL_MS", "20")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "40")
        .env("TUA_INTERNAL_WATCH_RECONCILE_MS", "100")
        .env("TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run tua watch");
    let stdout_capture = CapturedChildStdout::wait_for_watch_ready(
        &mut child,
        "watch did not finish its initial dependency-aware build",
    );

    assert!(temp.path().join("build/main.lua").is_file());
    assert!(temp.path().join("build/types.lua").is_file());

    fs::write(
        temp.path().join("src/types.tua"),
        r#"
local function accept(value: string): string
  return value
end

return { accept = accept }
"#,
    )
    .expect("failed to change exported type");

    let mut output = wait_for_child_output(child, Duration::from_secs(5));
    output.stdout = stdout_capture.finish();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(
        stdout.contains("initial build: built 2 file(s)"),
        "unexpected stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("changed 1 file(s)"),
        "unexpected stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("rebuild: 1 file(s) emitted with errors"),
        "unexpected stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("src/main.tua"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("expected string, found number"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn watch_reruns_asset_pipelines_when_external_inputs_change() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("watch-asset-pipeline");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("assets")).expect("failed to create assets");
    fs::create_dir_all(temp.path().join("tools")).expect("failed to create tools");
    fs::write(temp.path().join("assets/world.txt"), "first\n").expect("failed to write input");
    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"
out = "build"

[[asset_pipelines]]
name = "world"
inputs = ["assets/**/*.txt", "tools/import.sh"]
command = ["sh", "tools/import.sh"]
output = "src/generated/world"
"#,
    )
    .expect("failed to write config");
    fs::write(
        temp.path().join("src/main.tua"),
        "import type { Room } from \"./generated/world/types\"\nlocal _room: Room = { name = \"start\" }\n",
    )
    .expect("failed to write main");
    fs::write(
        temp.path().join("tools/import.sh"),
        r#"set -eu
count_file="$TUA_ASSET_PIPELINE_PROJECT_ROOT/runs.txt"
count=0
if [ -f "$count_file" ]; then count=$(sed -n '1p' "$count_file"); fi
printf '%s\n' "$((count + 1))" > "$count_file"
mkdir -p "$TUA_ASSET_PIPELINE_OUTPUT_DIR"
printf 'export type Room = { name: string }\n' > "$TUA_ASSET_PIPELINE_OUTPUT_DIR/types.d.tua"
printf '{"version":1,"outputs":["types.d.tua"],"diagnostics":[]}\n' > "$TUA_ASSET_PIPELINE_MANIFEST"
"#,
    )
    .expect("failed to write importer");

    let mut child = tua()
        .arg("watch")
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_FORCE_POLL", "1")
        .env("TUA_INTERNAL_WATCH_POLL_MS", "20")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "40")
        .env("TUA_INTERNAL_WATCH_RECONCILE_MS", "100")
        .env("TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run tua watch");
    let stdout_capture = CapturedChildStdout::wait_for_watch_ready(
        &mut child,
        "watch did not finish its initial asset pipeline build",
    );

    assert_eq!(
        fs::read_to_string(temp.path().join("runs.txt")).expect("missing importer count"),
        "1\n"
    );
    assert!(temp.path().join("build/main.lua").is_file());
    fs::write(temp.path().join("assets/world.txt"), "second\n").expect("failed to change input");

    let mut output = wait_for_child_output(child, Duration::from_secs(5));
    output.stdout = stdout_capture.finish();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        fs::read_to_string(temp.path().join("runs.txt")).expect("missing importer count"),
        "2\n"
    );
    assert!(
        stdout.contains("asset pipeline rebuild: rebuilt"),
        "{stdout}"
    );
    assert!(stderr.is_empty(), "{stderr}");
}

fn wait_until(mut predicate: impl FnMut() -> bool, timeout: Duration, failure_message: &str) {
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        if predicate() {
            return;
        }

        thread::sleep(Duration::from_millis(20));
    }

    panic!("{failure_message}");
}

fn watch_test_guard() -> MutexGuard<'static, ()> {
    static WATCH_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    WATCH_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn watch_rechecks_importers_after_declaration_library_changes() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("watch-declaration-types");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"

[[type_libraries]]
path = "lib/feel/types"
declaration_prefix = "feel"
runtime_prefix = "lib.feel"
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("lib/feel/types"))
        .expect("failed to create declaration library");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local feel = require("lib.feel")
local value: number = feel.accept(10)
return value
"#,
    )
    .expect("failed to write importer");
    let declaration_path = temp.path().join("lib/feel/types/feel.lua");
    fs::write(
        &declaration_path,
        r#"---@meta feel
local feel = {}
---@param value number
---@return number
function feel.accept(value) return value end
return feel
"#,
    )
    .expect("failed to write initial declarations");

    let mut child = tua()
        .arg("watch")
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_FORCE_POLL", "1")
        .env("TUA_INTERNAL_WATCH_POLL_MS", "20")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "40")
        .env("TUA_INTERNAL_WATCH_RECONCILE_MS", "100")
        .env("TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run tua watch");
    let stdout_capture = CapturedChildStdout::wait_for_watch_ready(
        &mut child,
        "watch did not finish its initial declaration-aware build",
    );

    assert!(temp.path().join("build/main.lua").is_file());

    fs::write(
        declaration_path,
        r#"---@meta feel
local feel = {}
---@param value string
---@return string
function feel.accept(value) return value end
return feel
"#,
    )
    .expect("failed to change declarations");

    let mut output = wait_for_child_output(child, Duration::from_secs(5));
    output.stdout = stdout_capture.finish();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(
        stdout.contains("initial build: built 1 file(s)"),
        "{stdout}"
    );
    assert!(stdout.contains("changed 1 file(s)"), "{stdout}");
    assert!(
        stdout.contains("rebuild: 0 file(s) emitted with errors"),
        "{stdout}"
    );
    assert!(stderr.contains("src/main.tua"), "{stderr}");
    assert!(stderr.contains("expected string, found number"), "{stderr}");
    assert!(!temp.path().join("build/lib/feel/types/feel.lua").exists());
}

fn wait_for_child_output(mut child: process::Child, timeout: Duration) -> Output {
    let deadline = Instant::now() + timeout;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child.wait_with_output().expect("failed to read output");
            }
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                let _ = child.kill();
                let output = child
                    .wait_with_output()
                    .expect("failed to read timed out output");
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                panic!("watch did not exit before timeout\nstdout:\n{stdout}\nstderr:\n{stderr}");
            }
            Err(error) => panic!("failed to wait for child process: {error}"),
        }
    }
}

fn working_lua_runtime() -> Option<OsString> {
    // CI pins the LuaJIT target runtime; a pinned interpreter that does not
    // run must fail the test instead of falling back or skipping.
    if let Some(luajit) = std::env::var_os("TUA_LUAJIT_BIN").filter(|value| !value.is_empty()) {
        assert!(
            lua_version_works(&luajit),
            "TUA_LUAJIT_BIN={} is not a working Lua interpreter",
            luajit.to_string_lossy()
        );
        return Some(luajit);
    }

    [
        std::env::var_os("TUA_LUA_BIN"),
        Some(OsString::from("luajit")),
        Some(OsString::from("lua5.1")),
        Some(OsString::from("lua")),
        Some(OsString::from("lua5.4")),
        Some(OsString::from("lua5.3")),
        Some(OsString::from("lua5.2")),
    ]
    .into_iter()
    .flatten()
    .find(lua_version_works)
}

#[test]
fn compound_lowering_matches_bounded_integer_runtime_oracle() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let temp = TempDir::new("lowering-oracle");
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).unwrap();
    let mut source = String::new();
    let mut expected = String::new();
    for a in -8..=8 {
        for b in 1..=4 {
            source.push_str(&format!(
                "do local x: number = {a}; x += {b}; x *= 3; x -= 2; print(x) end\n"
            ));
            expected.push_str(&format!("{}\n", (a + b) * 3 - 2));
        }
    }
    fs::write(temp.path().join("src/main.tua"), source).unwrap();
    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(lua)
        .arg("build/main.lua")
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
        expected
    );
}

fn lua_version_works(candidate: &OsString) -> bool {
    Command::new(candidate)
        .arg("-v")
        .output()
        .is_ok_and(|output| output.status.success())
}
