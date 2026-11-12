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

#[test]
fn package_web_copies_only_player_files_and_embeds_the_game_archive() {
    let temp = TempDir::new("package-web");
    write_packaging_project(&temp);
    let missing = tua()
        .args(["package", "--target", "web"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("--web-player"));
    let player = temp.path().join("love-web");
    for file in [
        "player.js",
        "style.css",
        "license.txt",
        "11.5/love.js",
        "11.5/love.wasm",
        "11.5/license.txt",
        "lua/normalize1.lua",
        "lua/normalize2.lua",
    ] {
        let path = player.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("fixture {file}")).unwrap();
    }
    fs::write(player.join("private-game.love"), b"do not copy").unwrap();
    let package = tua()
        .args([
            "package",
            "--target",
            "web",
            "--name",
            "Star Dust",
            "--web-player",
        ])
        .arg(&player)
        .current_dir(temp.path())
        .output()
        .expect("failed to package web game");
    assert!(package.status.success(), "{package:?}");
    let site = temp.path().join("dist/web/Star Dust");
    let game_files = fs::read_dir(&site)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("game-") && name.ends_with(".love"))
        .collect::<Vec<_>>();
    assert_eq!(game_files.len(), 1, "{game_files:?}");
    let game_file = &game_files[0];
    assert_eq!(
        fs::read(site.join(game_file)).unwrap(),
        fs::read(temp.path().join("dist/Star Dust.love")).unwrap()
    );
    assert!(site.join("11.5/love.wasm").exists());
    assert!(site.join("lua/normalize1.lua").exists());
    assert!(site.join("license.txt").exists());
    assert!(!site.join("private-game.love").exists());
    let headers = fs::read_to_string(site.join("_headers")).unwrap();
    assert!(headers.starts_with("/*\n"));
    assert!(headers.contains("Cross-Origin-Opener-Policy: same-origin"));
    assert!(headers.contains("Cross-Origin-Embedder-Policy: require-corp"));
    assert!(headers.contains("script-src 'self' 'unsafe-eval'"));
    let apache = fs::read_to_string(site.join(".htaccess")).unwrap();
    assert!(apache.contains("Header set Cross-Origin-Opener-Policy"));
    assert!(apache.contains("AddType application/wasm .wasm"));
    let hosting = fs::read_to_string(site.join("README.txt")).unwrap();
    assert!(hosting.contains("miniserve -i 127.0.0.1"));
    assert!(hosting.contains("SharedArrayBuffer support"));
    let html = fs::read_to_string(site.join("index.html")).unwrap();
    assert!(html.contains("<title>Star Dust</title>"), "{html}");
    assert!(
        html.contains(&format!("player.js?g={game_file}&amp;v=11.5")),
        "{html}"
    );
    fs::remove_file(player.join("11.5/love.wasm")).unwrap();
    let failed = tua()
        .args([
            "package",
            "--target",
            "web",
            "--name",
            "Star Dust",
            "--web-player",
        ])
        .arg(&player)
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("love.wasm"));
    assert_eq!(
        fs::read(site.join(game_file)).unwrap(),
        fs::read(temp.path().join("dist/Star Dust.love")).unwrap()
    );

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(player.join("player.js"), player.join("11.5/love.wasm"))
            .unwrap();
        let linked = tua()
            .args(["package", "--target", "web", "--web-player"])
            .arg(&player)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(!linked.status.success());
        assert!(String::from_utf8_lossy(&linked.stderr).contains("symbolic link"));
        fs::remove_file(player.join("11.5/love.wasm")).unwrap();
    }
    fs::write(player.join("11.5/love.wasm"), b"fixture wasm").unwrap();
    fs::write(player.join(".htaccess"), "# supplied by the player\n").unwrap();
    fs::write(
        temp.path().join("src/main.tua"),
        "function love.draw()\n  love.graphics.print(\"changed\", 10, 10)\nend\n",
    )
    .unwrap();
    let updated = tua()
        .args([
            "package",
            "--target",
            "web",
            "--name",
            "Star Dust",
            "--web-player",
        ])
        .arg(&player)
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(updated.status.success(), "{updated:?}");
    assert_eq!(
        fs::read_to_string(site.join(".htaccess")).unwrap(),
        "# supplied by the player\n",
        "an upstream Apache policy must be preserved"
    );
    let updated_html = fs::read_to_string(site.join("index.html")).unwrap();
    assert!(!updated_html.contains(game_file), "{updated_html}");
    assert!(
        !site.join(game_file).exists(),
        "stale game URL must be removed"
    );
}

#[test]
fn custom_package_helper() {
    if std::env::var("TUA_TEST_PACKAGE_HELPER").ok().as_deref() != Some("1") {
        return;
    }
    let Ok(output) = std::env::var("TUA_PACKAGE_OUTPUT_DIR") else {
        return;
    };
    let mode = std::env::var("TUA_TEST_PACKAGE_MODE").unwrap_or_default();
    if mode == "timeout" {
        thread::sleep(Duration::from_secs(2));
        return;
    }
    if mode == "empty" {
        return;
    }
    let archive = std::env::var("TUA_PACKAGE_ARCHIVE").unwrap();
    let archive_bytes = fs::read(&archive).unwrap();
    assert!(archive_bytes.starts_with(b"PK"));
    assert_eq!(std::env::var("TUA_PACKAGE_NAME").unwrap(), "Custom Game");
    assert_eq!(std::env::var("TUA_PACKAGE_TARGET").unwrap(), "portable");
    fs::write(
        Path::new(&output).join("result.txt"),
        format!("{} bytes", archive_bytes.len()),
    )
    .unwrap();
    if mode == "fail" {
        panic!("simulated package failure");
    }
}

#[test]
fn custom_package_targets_list_and_publish_atomically() {
    let temp = TempDir::new("package-custom");
    write_packaging_project(&temp);
    let executable = std::env::current_exe().unwrap();
    let program = serde_json::to_string(&executable.to_string_lossy().to_string()).unwrap();
    let pipeline = format!("[[package_pipelines]]\nname = \"portable\"\nlabel = \"Portable port\"\ncommand = [{program}, \"--exact\", \"custom_package_helper\", \"--nocapture\"]\n");
    let config_path = temp.path().join(CONFIG_FILE);
    let config = fs::read_to_string(&config_path)
        .unwrap()
        .replace("package_pipelines = []", &pipeline);
    fs::write(&config_path, config).unwrap();

    let listed = tua()
        .args(["package", "--list-targets"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(listed.status.success(), "{listed:?}");
    let targets: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(targets[0]["target"], "custom:portable");
    assert_eq!(targets[0]["label"], "Portable port");
    assert!(
        !temp.path().join("build/main.lua").exists(),
        "listing must not build"
    );

    let missing = tua()
        .args(["package", "--target", "custom:unknown"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(
        !temp.path().join("build/main.lua").exists(),
        "unknown target must fail before building"
    );
    let default = tua()
        .arg("package")
        .current_dir(temp.path())
        .env("TUA_TEST_PACKAGE_HELPER", "1")
        .output()
        .unwrap();
    assert!(default.status.success(), "{default:?}");
    assert!(
        !temp.path().join("dist/custom/portable").exists(),
        "the default target must not run the custom command"
    );

    let package = || {
        let mut command = tua();
        command
            .args([
                "package",
                "--target",
                "custom:portable",
                "--name",
                "Custom Game",
            ])
            .current_dir(temp.path())
            .env("TUA_TEST_PACKAGE_HELPER", "1");
        command
    };
    let success = package().output().unwrap();
    assert!(success.status.success(), "{success:?}");
    let destination = temp
        .path()
        .join("dist/custom/portable/Custom Game/result.txt");
    let original = fs::read(&destination).unwrap();
    assert!(String::from_utf8_lossy(&success.stdout)
        .contains("packaged dist/custom/portable/Custom Game"));

    for mode in ["fail", "empty"] {
        let failed = package()
            .env("TUA_TEST_PACKAGE_MODE", mode)
            .output()
            .unwrap();
        assert!(!failed.status.success(), "{mode}: {failed:?}");
        assert_eq!(
            fs::read(&destination).unwrap(),
            original,
            "{mode} must preserve the previous package"
        );
    }
    let short_timeout = fs::read_to_string(&config_path)
        .unwrap()
        .replace("command = [", "timeout_ms = 10\ncommand = [");
    fs::write(&config_path, short_timeout).unwrap();
    let timed_out = package()
        .env("TUA_TEST_PACKAGE_MODE", "timeout")
        .output()
        .unwrap();
    assert!(!timed_out.status.success());
    assert!(String::from_utf8_lossy(&timed_out.stderr).contains("timed out"));
    assert_eq!(fs::read(&destination).unwrap(), original);
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
fn generic_array_invariance_blocks_a_runtime_type_corruption() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let source = r#"
open class Animal {
  init() {}
}
final class Cat extends Animal {
  meow(): string { return "meow" }
}

local function overwrite<T>(items: T[], value: T): nil
  items[1] = value
end

local cats: Cat[] = { Cat.new() }
local animal: Animal = Animal.new()
overwrite(cats, animal)
local cat: Cat = cats[1]
assert(cat:meow() == "meow")
"#;
    let temp = TempDir::new("generic-array-invariance");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("write config");
    fs::create_dir_all(temp.path().join("src")).expect("create source directory");
    fs::write(temp.path().join("src/main.tua"), source).expect("write source");

    let checked = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("run check");
    assert!(
        !checked.status.success(),
        "unsafe generic write was accepted"
    );
    assert!(
        String::from_utf8_lossy(&checked.stderr).contains("TL1015"),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );

    // The emitter is deliberately exercised separately from checking to show
    // the concrete Lua failure the checker prevents. Generic annotations erase;
    // Lua itself permits the write, then the Cat-only method call fails.
    let emitted = tua_core::emit::emit_tua_source("src/main.tua", source)
        .expect("unchecked emission should preserve the ordinary Lua operation");
    let emitted_path = temp.path().join("unsafe.lua");
    fs::write(&emitted_path, emitted).expect("write emitted Lua");
    let runtime = Command::new(&lua)
        .arg(&emitted_path)
        .output()
        .expect("run emitted Lua oracle");
    assert!(
        !runtime.status.success(),
        "the emitted unsafe program unexpectedly kept the Cat invariant"
    );
    let runtime_stderr = String::from_utf8_lossy(&runtime.stderr);
    assert!(
        runtime_stderr.contains("meow") || runtime_stderr.contains("nil"),
        "unexpected runtime failure: {runtime_stderr}"
    );

    let safe_temp = TempDir::new("generic-array-safe-runtime");
    fs::write(safe_temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("write safe config");
    fs::create_dir_all(safe_temp.path().join("src")).expect("create safe source directory");
    let safe_source = source.replace(
        "local animal: Animal = Animal.new()\noverwrite(cats, animal)",
        "local other_cat: Cat = Cat.new()\noverwrite(cats, other_cat)",
    );
    fs::write(safe_temp.path().join("src/main.tua"), safe_source).expect("write safe source");
    let built = tua()
        .arg("build")
        .current_dir(safe_temp.path())
        .output()
        .expect("build safe generic write");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let safe_runtime = Command::new(&lua)
        .arg(safe_temp.path().join("build/main.lua"))
        .output()
        .expect("run safe emitted Lua");
    assert!(
        safe_runtime.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&safe_runtime.stdout),
        String::from_utf8_lossy(&safe_runtime.stderr)
    );
}

#[test]
fn generic_multiple_return_slots_preserve_lua_runtime_values() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let temp = TempDir::new("generic-multiple-return-runtime");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("write config");
    fs::create_dir_all(temp.path().join("src")).expect("create source directory");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local function split<T, U>(left: T, right: U): (T, U)
  return left, right
end

local text, count = split("enemy", 3)
assert(text == "enemy")
assert(count == 3)
"#,
    )
    .expect("write source");

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("run Tua build");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let emitted = fs::read_to_string(temp.path().join("build/main.lua")).expect("read emitted Lua");
    assert!(!emitted.contains("<T, U>"));
    assert!(emitted.contains("local text, count = split(\"enemy\", 3)"));

    let runtime = Command::new(&lua)
        .arg(temp.path().join("build/main.lua"))
        .output()
        .expect("run emitted Lua");
    assert!(
        runtime.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&runtime.stdout),
        String::from_utf8_lossy(&runtime.stderr)
    );
}

#[test]
fn protocol_polymorphic_dispatch_preserves_runtime_receiver_identity() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let temp = TempDir::new("protocol-dispatch-runtime");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("write config");
    fs::create_dir_all(temp.path().join("src")).expect("create source directory");
    let source = include_str!("../../../examples/features/src/protocols.tua");
    let main = temp.path().join("src/main.tua");
    fs::write(&main, source).expect("write protocol example");

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("run Tua build");
    assert!(
        build.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let emitted = fs::read_to_string(temp.path().join("build/main.lua")).expect("read emitted Lua");
    assert!(!emitted.contains("Updatable") && !emitted.contains("implements"));
    assert!(emitted.contains("item:update(dt)"));

    let runtime = Command::new(&lua)
        .arg(temp.path().join("build/main.lua"))
        .output()
        .expect("run emitted protocol example");
    assert!(
        runtime.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&runtime.stdout),
        String::from_utf8_lossy(&runtime.stderr)
    );

    let invalid = source.replace(
        "return { items = items, updateAll = updateAll }",
        "final class Incomplete implements Updatable {\n}\nreturn { items = items, updateAll = updateAll }",
    );
    fs::write(&main, invalid).expect("write missing-method negative case");
    let check = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("check missing-method negative case");
    assert!(
        !check.status.success(),
        "a nonconforming class was accepted"
    );
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        stderr.contains("Incomplete")
            && stderr.contains("missing method `update`")
            && stderr.contains("missing method `progress`"),
        "both missing protocol obligations should be reported:\n{stderr}"
    );
}

#[test]
fn imported_generic_protocol_bounds_keep_project_conformance_diagnostics() {
    let temp = TempDir::new("imported-generic-protocol-bound");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("write config");
    fs::create_dir_all(temp.path().join("src")).expect("create source directory");
    for (path, source) in [
        (
            "src/contracts.d.tua",
            "export protocol Updatable {\n  update(dt: number): nil\n}\n",
        ),
        (
            "src/relay.tua",
            "export type { Updatable } from \"./contracts\"\n",
        ),
        (
            "src/api.tua",
            "import type { Updatable } from \"./relay\"\nlocal function tick<T: Updatable>(value: T): T\n  value:update(1)\n  return value\nend\nreturn { tick = tick }\n",
        ),
        (
            "src/actors.tua",
            "import type { Updatable } from \"./contracts\"\nfinal class Actor implements Updatable {\n  update(dt: number): nil { return nil }\n}\nreturn { Actor = Actor }\n",
        ),
        (
            "src/lookalike.tua",
            "final class Lookalike {\n  update(dt: number): nil { return nil }\n}\nreturn { Lookalike = Lookalike }\n",
        ),
        (
            "src/main.tua",
            "local api = require(\"api\")\nlocal actors = require(\"actors\")\nlocal lookalikes = require(\"lookalike\")\nlocal actor = actors.Actor.new()\nlocal _valid = api.tick(actor)\nlocal _invalid = api.tick(lookalikes.Lookalike.new())\n",
        ),
    ] {
        fs::write(temp.path().join(path), source).expect("write project fixture");
    }

    let check = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("run project check");
    assert!(!check.status.success(), "the nonconforming class must fail");
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        stderr.contains("requires explicit conformance") && stderr.contains("Lookalike"),
        "project checking should retain the real nonconformance error:\n{stderr}"
    );
    assert!(
        !stderr.contains("inferred `Actor` does not conform"),
        "the project-aware pass should remove the earlier file-only false error:\n{stderr}"
    );
}

#[test]
fn concrete_generic_parent_substitution_preserves_emitted_lua_behavior() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let temp = TempDir::new("generic-parent-specialization-runtime");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("write config");
    fs::create_dir_all(temp.path().join("src")).expect("create source directory");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
open class Base<T> {
  value: T

  init(value: T) { self.value = value }
  get(): T { return self.value }
}

open class NumericBox extends Base<number> {
  init(value: number) { super(value) }
}

final class LabeledNumericBox extends NumericBox {
  init(value: number) { super(value) }
}

local box: LabeledNumericBox = LabeledNumericBox.new(42)
assert(box.value == 42)
assert(box:get() == 42)
box.value = 43
assert(box:get() == 43)
"#,
    )
    .expect("write source");

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("build specialized generic subclass");
    assert!(
        build.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let emitted = fs::read_to_string(temp.path().join("build/main.lua")).expect("read emitted Lua");
    assert!(!emitted.contains("Base<number>"));
    assert!(!emitted.contains("LabeledNumericBox<>"));

    let runtime = Command::new(lua)
        .arg(temp.path().join("build/main.lua"))
        .output()
        .expect("run emitted Lua");
    assert!(
        runtime.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&runtime.stdout),
        String::from_utf8_lossy(&runtime.stderr)
    );
}

#[test]
fn emitted_lua_preserves_imported_generic_protocol_runtime_behavior() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let temp = TempDir::new("generic-protocol-multimodule-runtime");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("write config");
    fs::create_dir_all(temp.path().join("src")).expect("create source directory");
    for (path, source) in [
        (
            "src/contracts.tua",
            r#"
export protocol Updatable {
  update(dt: number): nil
  progress(): number
}

return {}
"#,
        ),
        (
            "src/actor.tua",
            r#"
import type { Updatable } from "./contracts"

final class Actor implements Updatable {
  elapsed: number

  init(start: number) { self.elapsed = start }
  update(dt: number): nil { self.elapsed += dt }
  progress(): number { return self.elapsed }
}

export type ActorInstance = Actor
return { Actor = Actor }
"#,
        ),
        (
            "src/box.tua",
            r#"
final class Box<T> {
  value: T

  init(value: T) { self.value = value }
  get(): T { return self.value }
}

export type BoxInstance<T> = Box<T>
return { Box = Box }
"#,
        ),
        (
            "src/main.tua",
            r#"
import type { Updatable } from "./contracts"
import type { ActorInstance } from "./actor"
import type { BoxInstance } from "./box"

local ActorModule = require("actor")
local BoxModule = require("box")
local Actor = ActorModule.Actor
local evaluation_order = ""

local function make_actor(): ActorInstance
  evaluation_order = evaluation_order .. "A"
  return Actor.new(0)
end

local function make_delta(): number
  evaluation_order = evaluation_order .. "D"
  return 0.25
end

local function pair<T, U>(first: T, second: U): (T, U)
  return first, second
end

local actor, delta = pair(make_actor(), make_delta())
assert(evaluation_order == "AD")

local box: BoxInstance<ActorInstance> = BoxModule.Box.new(actor)
local function update_one<T: Updatable>(value: T, dt: number): T
  value:update(dt)
  return value
end

local updated: ActorInstance = update_one(box:get(), delta)
assert(updated == actor)
assert(box:get():progress() == 0.25)

local function identity<T>(value: T): T
  return value
end
local function apply_number(callback: (number) -> number): number
  return callback(9)
end
assert(apply_number(identity) == 9)
assert(identity(false) == false)

local function with_nil<T>(value: T): (T, nil)
  return value, nil
end
local same_box, missing = with_nil(box)
assert(same_box == box and missing == nil)

local function first<T>(...: T): T
  return ...
end
local first_text: string = "first"
local second_text: string = "second"
assert(first(first_text, second_text) == "first")

final class Clock implements Updatable {
  elapsed: number
  init(start: number) { self.elapsed = start }
  update(dt: number): nil { self.elapsed += dt }
  progress(): number { return self.elapsed }
}

local function update_all(items: Updatable[], dt: number): nil
  for _, item in ipairs(items) do
    item:update(dt)
  end
  return nil
end

local items: Updatable[] = { updated, Clock.new(1) }
update_all(items, 0.5)
assert(items[1]:progress() == 0.75)
assert(items[2]:progress() == 1.5)
assert(package.loaded["contracts"] == nil)
"#,
        ),
    ] {
        fs::write(temp.path().join(path), source).expect("write runtime project source");
    }

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("build imported generic/protocol project");
    assert!(
        build.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let main = fs::read_to_string(temp.path().join("build/main.lua")).expect("read emitted main");
    let actor =
        fs::read_to_string(temp.path().join("build/actor.lua")).expect("read emitted actor");
    let box_module =
        fs::read_to_string(temp.path().join("build/box.lua")).expect("read emitted box");
    for emitted in [&main, &actor, &box_module] {
        assert!(!emitted.contains("import type"));
        assert!(!emitted.contains("implements"));
        assert!(!emitted.contains("Updatable"));
        assert!(!emitted.contains("BoxInstance"));
    }
    assert!(main.contains("require(\"actor\")"));
    assert!(main.contains("require(\"box\")"));
    assert!(!main.contains("require(\"contracts\")"));

    let runtime = Command::new(lua)
        .arg("main.lua")
        .current_dir(temp.path().join("build"))
        .output()
        .expect("run emitted project entry point");
    assert!(
        runtime.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&runtime.stdout),
        String::from_utf8_lossy(&runtime.stderr)
    );
}

#[test]
fn runtime_class_checks_use_identity_across_modules() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let temp = TempDir::new("cross-module-class-is-runtime");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("write config");
    fs::create_dir_all(temp.path().join("src")).expect("create source directory");
    fs::write(
        temp.path().join("src/foreign.tua"),
        "final class Foreign {}\nreturn { Foreign = Foreign }\n",
    )
    .expect("write foreign class");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local foreign = require("foreign").Foreign.new()
open class Base {}
final class Child extends Base {}
local child = Child.new()
if foreign is Base then error("unrelated module class matched Base") end
if foreign is Child then error("unrelated module class matched Child") end
if child is Base then
  if child is Child then
    assert(true)
  else
    error("own class identity was lost")
  end
else
  error("inherited class identity was lost")
end
"#,
    )
    .expect("write class checks");

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("build cross-module class fixture");
    assert!(
        build.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let runtime = Command::new(lua)
        .arg("main.lua")
        .current_dir(temp.path().join("build"))
        .output()
        .expect("run emitted class fixture");
    assert!(
        runtime.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&runtime.stdout),
        String::from_utf8_lossy(&runtime.stderr)
    );
}

#[test]
fn emitted_lua_preserves_generic_override_and_initialization_order() {
    let Some(lua) = working_lua_runtime() else {
        return;
    };
    let temp = TempDir::new("generic-inheritance-runtime-order");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("write config");
    fs::create_dir_all(temp.path().join("src")).expect("create source directory");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
open class Base<T> {
  value: T
  init(value: T) { self.value = value }
  get(): T { return self.value }
  name(): string { return "base" }
}

final class Child extends Base<number> {
  marker: number = 9
  init(value: number) {
    super(value)
    self.marker = value
  }
  override name(): string { return "child" }
}

local child = Child.new(5)
assert(child.marker == 9)
assert(child:get() == 5)

local base: Base<number> = child
assert(base:name() == "child")
"#,
    )
    .expect("write generic inheritance source");

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("build generic inheritance runtime fixture");
    assert!(
        build.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let emitted = fs::read_to_string(temp.path().join("build/main.lua")).expect("read emitted Lua");
    assert!(!emitted.contains("Base<number>"));
    assert!(!emitted.contains("extends"));
    assert!(emitted.contains("setmetatable(Child, { __index = Base })"));
    assert!(emitted.contains("local self = setmetatable({}, Child)"));

    let runtime = Command::new(lua)
        .arg(temp.path().join("build/main.lua"))
        .output()
        .expect("run emitted generic inheritance Lua");
    assert!(
        runtime.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&runtime.stdout),
        String::from_utf8_lossy(&runtime.stderr)
    );
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
    assert!(stderr.contains("type mismatch at hp: expected number, found string"));
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
    assert_eq!(
        fs::read_to_string(temp.path().join("build/main.lua.map.json")).unwrap(),
        "{}\n"
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("build/main.lua.map")).unwrap(),
        "{}\n"
    );
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
    assert_eq!(
        fs::read_to_string(temp.path().join("build/types.d.lua")).unwrap(),
        "stale\n"
    );
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
fn traceback_remaps_runtime_errors_inside_generic_class_methods() {
    let Some(lua) = working_lua_runtime() else {
        eprintln!("skipping generic class traceback test; no Lua executable was found");
        return;
    };
    let temp = TempDir::new("traceback-generic-class-method");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    let source = r#"final class Box<T> {
  value: T

  init(value: T) {
    self.value = value
  }

  fail(): nil {
    error("generic class failure")
  }
}

local box = Box.new("value")
box:fail()
"#;
    fs::write(temp.path().join("src/main.tua"), source)
        .expect("failed to write generic class source");

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to build generic class source");
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
        "the generic class method should raise its intentional runtime error"
    );
    let traceback = String::from_utf8(runtime.stderr).expect("Lua traceback should be UTF-8");
    assert!(
        traceback.contains("generic class failure"),
        "unexpected runtime traceback:\n{traceback}"
    );
    fs::write(temp.path().join("generic-class-traceback.txt"), &traceback)
        .expect("failed to save runtime traceback");

    let remapped = tua()
        .args([
            "traceback",
            "--maps",
            "build",
            "generic-class-traceback.txt",
        ])
        .current_dir(temp.path())
        .output()
        .expect("failed to remap generic class traceback");
    assert!(
        remapped.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&remapped.stdout),
        String::from_utf8_lossy(&remapped.stderr)
    );
    let remapped = String::from_utf8(remapped.stdout)
        .expect("remapped generic class traceback should be UTF-8");
    assert!(
        remapped.contains("src/main.tua:9 (generated build/main.lua:"),
        "runtime failure should map to the error expression inside Box.fail:\n{remapped}"
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
fn build_bundles_audio_design_only_for_its_explicit_import() {
    let temp = TempDir::new("audio-design-opt-in");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("config");
    fs::create_dir_all(temp.path().join("src")).expect("source directory");
    fs::write(
        temp.path().join("src/main.tua"),
        "local hit: Source = AudioDesign.render({waveform='noise',duration=0.1})\nreturn hit\n",
    )
    .expect("source");
    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("build");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(temp.path().join("build/tua/audio/design.lua").is_file());
    assert!(!temp.path().join("build/tuaaudio.lua").exists());
    let emitted = fs::read_to_string(temp.path().join("build/main.lua")).expect("emitted Lua");
    assert!(
        emitted.starts_with("local AudioDesign = require(\"tua.audio.design\")"),
        "{emitted}"
    );
    fs::write(temp.path().join("src/main.tua"), "return 1\n").expect("remove import");
    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("rebuild");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !temp.path().join("build/tua/audio/design.lua").exists(),
        "owned helper removed after import disappears"
    );
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
fn build_refreshes_unchanged_owned_bundled_runtime() {
    let temp = TempDir::new("refresh-owned-signal-runtime");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("write config");
    fs::create_dir_all(temp.path().join("src")).expect("create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local events = signal()\nevents:declare(\"ready\", function() end)\nreturn events\n",
    )
    .expect("write source");

    let first = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("first build");
    assert!(first.status.success(), "{first:?}");

    // Model a helper emitted by an older Tua binary: its recorded fingerprint
    // still matches the file, but the bundled source has changed.
    let old_runtime = "return function() return {} end\n";
    fs::write(temp.path().join("build/tuasignal.lua"), old_runtime).expect("write old helper");
    let old_fingerprint = old_runtime
        .bytes()
        .fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    let manifest_path = temp.path().join("build/.tua-build-manifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    manifest["tuasignal.lua"] = json!(old_fingerprint);
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest).expect("serialize manifest"),
    )
    .expect("record previous helper ownership");

    let second = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("second build");
    assert!(second.status.success(), "{second:?}");
    assert_eq!(
        fs::read_to_string(temp.path().join("build/tuasignal.lua")).expect("read refreshed helper"),
        tua_core::tuasignal_runtime_source(),
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read updated manifest"))
            .expect("parse updated manifest");
    assert!(
        manifest["tuasignal.lua"].is_u64(),
        "refreshed helper must stay owned"
    );
}

#[test]
fn build_preserves_manually_changed_bundled_runtime_output() {
    let temp = TempDir::new("preserve-changed-signal-runtime");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("write config");
    fs::create_dir_all(temp.path().join("src")).expect("create source");
    fs::write(temp.path().join("src/main.tua"), "return signal()\n").expect("write source");

    let first = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("first build");
    assert!(first.status.success(), "{first:?}");
    let custom = "return function() return { custom = true } end\n";
    let runtime_path = temp.path().join("build/tuasignal.lua");
    fs::write(&runtime_path, custom).expect("change output helper");

    let second = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("second build");
    assert!(second.status.success(), "{second:?}");
    assert_eq!(
        fs::read_to_string(runtime_path).expect("read custom helper"),
        custom
    );
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
    assert!(stderr.contains("type mismatch at hp: expected number, found string"));
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

#[cfg(unix)]
#[test]
fn check_validates_referenced_shaders_via_love_and_respects_off() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new("shader-validation");
    fs::create_dir_all(temp.path.join("src")).unwrap();
    fs::write(temp.path.join("tua.toml"), DEFAULT_CONFIG_TEXT).unwrap();
    fs::write(
        temp.path.join("src/main.tua"),
        "local effect = shader \"effect.glsl\"\nprint(effect)\n",
    )
    .unwrap();
    fs::write(
        temp.path.join("src/effect.glsl"),
        "vec4 effect() { broken }",
    )
    .unwrap();
    let fake = temp.path.join("fake-love");
    fs::write(&fake, "#!/bin/sh\nprintf 'READY 11.5.0\\n'\nwhile read -r command id target size; do\n dd bs=1 count=\"$size\" >/dev/null 2>&1\n printf 'RESULT %s invalid ERROR: 0:3: syntax error\\n' \"$id\"\ndone\n").unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
    let output = tua()
        .arg("check")
        .current_dir(&temp.path)
        .env("TUA_LOVE", &fake)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(diagnostics.contains("TL2016"), "{diagnostics}");
    assert!(diagnostics.contains("effect.glsl:3:1"), "{diagnostics}");
    assert!(diagnostics.contains("main.tua:1:"), "{diagnostics}");
    fs::write(
        temp.path.join("tua.toml"),
        DEFAULT_CONFIG_TEXT.replace("validate = \"auto\"", "validate = \"off\""),
    )
    .unwrap();
    let output = tua()
        .arg("check")
        .current_dir(&temp.path)
        .env("TUA_LOVE", "/no/such/love")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("skipped"));
}

#[test]
fn dev_without_launch_publishes_ready_output_and_reloads() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("dev-without-launch");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).unwrap();
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(
        temp.path().join("src/main.tua"),
        "local value = 1\nreturn value\n",
    )
    .unwrap();
    let mut child = tua()
        .args([
            "dev",
            "--no-launch",
            "--love",
            "missing-love-must-not-be-launched",
        ])
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_POLL_MS", "20")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "40")
        .env("TUA_INTERNAL_WATCH_RECONCILE_MS", "100")
        .env("TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let capture = CapturedChildStdout::start(&mut child, "dev: ready ");
    capture
        .ready
        .recv_timeout(Duration::from_secs(10))
        .expect("dev did not prepare initial output");
    assert!(temp.path().join("build/tuahotreload.lua").is_file());
    assert!(fs::read_to_string(temp.path().join("build/main.lua"))
        .unwrap()
        .contains("require(\"tuahotreload\").install()"));
    let signal = temp.path().join("build/.tuahotreload");
    assert_eq!(fs::read_to_string(&signal).unwrap(), "1\n");
    fs::write(
        temp.path().join("src/main.tua"),
        "local value = 2\nreturn value\n",
    )
    .unwrap();
    let output = wait_for_child_output(child, Duration::from_secs(5));
    let stdout = String::from_utf8(capture.finish()).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(stdout.matches("dev: ready ").count(), 1);
    assert!(stdout.contains("dev: sent reload 2"));
    assert_eq!(fs::read_to_string(signal).unwrap(), "2\n");
}

#[cfg(unix)]
#[test]
fn check_shader_glslang_fallback_and_missing_tools() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new("shader-glslang");
    fs::create_dir_all(temp.path.join("src")).unwrap();
    fs::write(temp.path.join("tua.toml"), DEFAULT_CONFIG_TEXT).unwrap();
    fs::write(
        temp.path.join("src/main.tua"),
        "local effect = shader \"effect.glsl\"\nprint(effect)\n",
    )
    .unwrap();
    fs::write(
        temp.path.join("src/effect.glsl"),
        "vec4 effect(vec4 c, Image t, vec2 u, vec2 p) { return missing; }\n",
    )
    .unwrap();
    let fake = temp.path.join("fake-glslang");
    fs::write(
        &fake,
        "#!/bin/sh\necho 'ERROR: 1:1: undeclared identifier'; exit 2\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
    for mode in ["auto", "glslang"] {
        fs::write(
            temp.path.join("tua.toml"),
            DEFAULT_CONFIG_TEXT.replace("validate = \"auto\"", &format!("validate = \"{mode}\"")),
        )
        .unwrap();
        let output = tua()
            .arg("check")
            .current_dir(&temp.path)
            .env("TUA_LOVE", "/no/such/love")
            .env("TUA_GLSLANG", &fake)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            text.contains("TL2016") && text.contains("effect.glsl:1:1"),
            "{text}"
        );
    }
    fs::write(temp.path.join("tua.toml"), DEFAULT_CONFIG_TEXT).unwrap();
    let output = tua()
        .args(["check", "--deny-warnings"])
        .current_dir(&temp.path)
        .env("TUA_LOVE", "/no/such/love")
        .env("TUA_GLSLANG", "/no/such/glslang")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("shader compilation skipped"));
}

#[cfg(unix)]
#[test]
fn shader_tools_probes_without_loading_project() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new("shader-tools");
    // Invalid project config must never be read by the tool probe.
    fs::write(temp.path.join("tua.toml"), "invalid configuration !!!").unwrap();
    let fake = temp.path.join("glslang");
    fs::write(&fake, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
    for (command, works) in [(&fake, true), (&temp.path.join("missing"), false)] {
        let output = tua()
            .arg("shader-tools")
            .current_dir(&temp.path)
            .env("TUA_LOVE", "/no/such/love")
            .env("TUA_GLSLANG", command)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["love"]["canValidate"], false);
        assert_eq!(report["glslang"]["canValidate"], works);
        assert_eq!(report["preferred"], "love");
    }
}

#[cfg(unix)]
#[test]
fn check_maps_inline_shader_errors_into_the_source_string() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new("inline-shader");
    fs::create_dir_all(temp.path.join("src")).unwrap();
    fs::write(temp.path.join("tua.toml"), DEFAULT_CONFIG_TEXT).unwrap();
    fs::write(temp.path.join("src/main.tua"), "local effect = love.graphics.newShader([[\nvec4 effect() {\nbroken\n}\n]])\nprint(effect)\n").unwrap();
    let fake = temp.path.join("glslang");
    fs::write(&fake, "#!/bin/sh\necho 'ERROR: 1:2: bad GLSL'; exit 2\n").unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
    let output = tua()
        .arg("check")
        .current_dir(&temp.path)
        .env("TUA_LOVE", "/missing/love")
        .env("TUA_GLSLANG", &fake)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("main.tua:3:1") && text.contains("TL2016"),
        "{text}"
    );
}

#[test]
fn check_skips_unreadable_shaders_instead_of_aborting() {
    let temp = TempDir::new("latin1-shader");
    fs::create_dir_all(temp.path.join("src")).unwrap();
    fs::write(temp.path.join("tua.toml"), DEFAULT_CONFIG_TEXT).unwrap();
    // A Latin-1 comment is not UTF-8; LOVE accepts it, so it must not hide other diagnostics.
    fs::write(
        temp.path.join("src/latin.glsl"),
        b"// caf\xe9\nvec4 effect(vec4 c, Image t, vec2 u, vec2 p) { return c; }\n",
    )
    .unwrap();
    fs::write(
        temp.path.join("src/main.tua"),
        "local s = love.graphics.newShader(\"latin.glsl\")\nlocal x: number = \"oops\"\nprint(s, x)\n",
    )
    .unwrap();
    let output = tua()
        .arg("check")
        .current_dir(&temp.path)
        .env("TUA_LOVE", "/missing/love")
        .env("TUA_GLSLANG", "/missing/glslang")
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("main.tua:2:") && text.contains("TL1003"),
        "{text}"
    );
    assert!(!text.contains("invalid utf-8"), "{text}");
}

fn unit_test_project(temp: &TempDir) {
    fs::create_dir_all(temp.path().join("src/spec")).unwrap();
    fs::write(
        temp.path().join("tua.toml"),
        "source = 'src'\nout = 'build'\nemit_maps = false\n",
    )
    .unwrap();
    fs::write(temp.path().join("src/mathlib.tua"), "local function double(value: number): number\n return value * 2\nend\nreturn {double = double}\n").unwrap();
    fs::write(temp.path().join("src/spec/math_spec.tua"), "local mathlib = require('mathlib')\ndescribe('typed arithmetic', function()\n it('doubles', function()\n  assert.equals(6, mathlib.double(3))\n end)\nend)\n").unwrap();
}

#[test]
fn unit_test_command_rejects_empty_and_escaping_selections() {
    let temp = TempDir::new("test-selection");
    unit_test_project(&temp);
    for selector in ["missing", "../src/spec"] {
        let output = tua()
            .args(["test", selector])
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(!output.status.success());
    }
    assert!(!temp.path().join(".tua").exists());
}

#[test]
fn unit_test_command_reports_missing_backend_and_cleans_output() {
    let temp = TempDir::new("test-missing-backend");
    unit_test_project(&temp);
    let output = tua()
        .args(["test", "--busted"])
        .arg(temp.path().join("absent-busted"))
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Install Busted"),
        "{output:?}"
    );
    assert_eq!(fs::read_dir(temp.path().join(".tua")).unwrap().count(), 0);
    assert!(!temp.path().join("build").exists());
}

#[test]
fn unit_test_command_runs_real_busted_and_maps_failures() {
    if !Command::new("busted")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("skipping real Busted test: install busted to exercise this integration");
        return;
    }
    let temp = TempDir::new("test-real-busted");
    unit_test_project(&temp);
    let passed = tua().arg("test").current_dir(temp.path()).output().unwrap();
    assert!(passed.status.success(), "{passed:?}");
    assert!(String::from_utf8_lossy(&passed.stdout).contains("1 success"));
    let spec = temp.path().join("src/spec/math_spec.tua");
    fs::write(
        &spec,
        fs::read_to_string(&spec)
            .unwrap()
            .replace("assert.equals(6", "assert.equals(7"),
    )
    .unwrap();
    let failed = tua()
        .args(["test", "spec/math_spec.tua"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(
        String::from_utf8_lossy(&failed.stderr).contains("src/spec/math_spec.tua:4"),
        "{failed:?}"
    );
    assert_eq!(fs::read_dir(temp.path().join(".tua")).unwrap().count(), 0);
}

#[test]
fn unit_test_command_supports_selected_lua_with_project_module_paths() {
    if !Command::new("busted")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
        || !Command::new("lua")
            .arg("-v")
            .output()
            .is_ok_and(|output| output.status.success())
    {
        eprintln!("skipping selected Lua integration: install Busted and Lua to exercise it");
        return;
    }

    let temp = TempDir::new("test-selected-lua");
    unit_test_project(&temp);
    let output = tua()
        .args(["test", "--busted", "busted", "--lua", "lua"])
        .current_dir(temp.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 success"),
        "{output:?}"
    );
}

#[cfg(unix)]
#[test]
fn unit_test_build_errors_never_start_backend_or_modify_production_output() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new("test-build-errors");
    unit_test_project(&temp);
    fs::create_dir_all(temp.path().join("build")).unwrap();
    fs::write(temp.path().join("build/sentinel"), "keep").unwrap();
    let backend = temp.path().join("backend");
    fs::write(
        &backend,
        format!("#!/bin/sh\ntouch '{}'\n", temp.path().join("ran").display()),
    )
    .unwrap();
    fs::set_permissions(&backend, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        temp.path().join("src/mathlib.tua"),
        "local wrong: number = 'bad'\n",
    )
    .unwrap();
    let output = tua()
        .args(["test", "--busted"])
        .arg(backend)
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("TL1003"));
    assert!(!temp.path().join("ran").exists());
    assert_eq!(
        fs::read_to_string(temp.path().join("build/sentinel")).unwrap(),
        "keep"
    );
    assert_eq!(fs::read_dir(temp.path().join(".tua")).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn unit_test_output_rejects_symlinked_state_directory() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new("test-state-link");
    let outside = TempDir::new("test-state-target");
    unit_test_project(&temp);
    symlink(outside.path(), temp.path().join(".tua")).unwrap();
    let output = tua().arg("test").current_dir(temp.path()).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("symbolic link"));
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn unit_test_source_root_ignores_production_and_abandoned_test_output() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new("test-root-source");
    fs::write(
        temp.path().join("tua.toml"),
        "source = '.'\nout = 'build'\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("one_spec.tua"),
        "it('works', function() assert(true) end)\n",
    )
    .unwrap();
    for dir in ["build", ".tua/abandoned"] {
        fs::create_dir_all(temp.path().join(dir)).unwrap();
        fs::write(
            temp.path().join(dir).join("bad_spec.tua"),
            "this is not valid syntax ???",
        )
        .unwrap();
    }
    let backend = temp.path().join("backend");
    fs::write(
        &backend,
        "#!/bin/sh\nprintf '%s\\n' \"$@\"\ntest -f one_spec.lua\n",
    )
    .unwrap();
    fs::set_permissions(&backend, fs::Permissions::from_mode(0o700)).unwrap();
    let result = tua()
        .args(["test", "--busted"])
        .arg(backend)
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let report = String::from_utf8_lossy(&result.stdout);
    assert!(report.contains("one_spec.lua"));
    assert!(!report.contains("bad_spec.lua"));
    assert!(temp.path().join(".tua/abandoned/bad_spec.tua").exists());
    assert_eq!(fs::read_dir(temp.path().join(".tua")).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn unit_test_dedicated_root_checks_builds_and_excludes_production_specs() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new("dedicated-test-root");
    unit_test_project(&temp);
    fs::rename(temp.path().join("src/spec"), temp.path().join("tests")).unwrap();
    fs::write(
        temp.path().join("tua.toml"),
        "source='src'\nout='build'\n[test]\nsource='tests'\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("tests/helper.tua"),
        "return { value = 3 }\n",
    )
    .unwrap();
    fs::write(temp.path().join("tests/math_spec.tua"), "local mathlib = require('mathlib')\nlocal helper = require('helper')\nassert(mathlib.double(helper.value) == 6)\n").unwrap();
    // In-source specs are excluded by default too, even if malformed.
    fs::write(temp.path().join("src/unused_spec.tua"), "bad syntax !!!").unwrap();
    let production = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(production.status.success(), "{production:?}");
    assert!(temp.path().join("build/mathlib.lua").exists());
    assert!(!temp.path().join("build/unused_spec.lua").exists());
    assert!(!temp.path().join("build/__tua_tests__").exists());
    fs::remove_file(temp.path().join("src/unused_spec.tua")).unwrap();
    let backend = temp.path().join("backend");
    fs::write(&backend, "#!/bin/sh\nset -e\ntest -f mathlib.lua\ntest -f __tua_tests__/math_spec.lua\ntest -f __tua_tests__/helper.lua\nprintf '%s\\n' \"$@\"\n").unwrap();
    fs::set_permissions(&backend, fs::Permissions::from_mode(0o700)).unwrap();
    let test = tua()
        .args(["test", "math_spec.tua", "--busted"])
        .arg(&backend)
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(test.status.success(), "{test:?}");
    assert!(String::from_utf8_lossy(&test.stdout).contains("__tua_tests__/math_spec.lua"));
    let checked = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(checked.status.success(), "{checked:?}");
    assert!(String::from_utf8_lossy(&checked.stdout).contains("checked 3 file(s)"));
    fs::write(
        temp.path().join("tests/helper.tua"),
        "local bad: number = 'text'\nreturn bad\n",
    )
    .unwrap();
    assert!(!tua()
        .arg("check")
        .current_dir(temp.path())
        .status()
        .unwrap()
        .success());
    assert!(tua()
        .arg("build")
        .current_dir(temp.path())
        .status()
        .unwrap()
        .success());
}

#[test]
fn unit_test_spec_exclusion_can_be_disabled_and_cleans_owned_output() {
    let temp = TempDir::new("spec-exclusion");
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src/main.tua"), "print('main')\n").unwrap();
    fs::write(temp.path().join("src/demo_spec.tua"), "print('spec')\n").unwrap();
    fs::write(
        temp.path().join("tua.toml"),
        "[test]\nexclude_specs=false\n",
    )
    .unwrap();
    assert!(tua()
        .arg("build")
        .current_dir(temp.path())
        .status()
        .unwrap()
        .success());
    assert!(temp.path().join("build/demo_spec.lua").exists());
    fs::write(temp.path().join("tua.toml"), "[test]\nexclude_specs=true\n").unwrap();
    assert!(tua()
        .arg("build")
        .current_dir(temp.path())
        .status()
        .unwrap()
        .success());
    assert!(!temp.path().join("build/demo_spec.lua").exists());
    assert!(!temp.path().join("build/demo_spec.lua.map").exists());
}

#[test]
fn unit_test_discovery_is_static_and_reports_invalid_files() {
    let temp = TempDir::new("test-discovery");
    unit_test_project(&temp);
    fs::write(
        temp.path().join("src/spec/invalid_spec.tua"),
        "local broken =\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("src/spec/static_spec.tua"),
        "error('must not execute')\ndescribe('suite', function() it('case', function() end) end)\n",
    )
    .unwrap();
    let result = tua()
        .args(["test", "--list", "--busted", "/no/backend"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 3);
    assert!(files.iter().any(|file| file["error"].as_str().is_some()));
    assert!(files
        .iter()
        .any(|file| file["cases"][0]["full_name"] == "suite case"));
    let suite_file = files
        .iter()
        .find(|file| file["suites"][0]["full_name"] == "suite")
        .unwrap();
    assert_eq!(suite_file["suites"][0]["line"], 2);
    assert_eq!(suite_file["cases"][0]["suite"], "suite");
    assert!(!temp.path().join(".tua").exists());
}

#[test]
fn unit_test_json_report_keeps_stdout_and_maps_native_and_lua_locations() {
    if !Command::new("busted")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        return;
    }
    let temp = TempDir::new("test-report");
    unit_test_project(&temp);
    fs::rename(temp.path().join("src/spec"), temp.path().join("tests")).unwrap();
    fs::write(temp.path().join("tua.toml"), "[test]\nsource='tests'\n").unwrap();
    fs::write(
        temp.path().join("tests/plain_spec.lua"),
        "it('lua failure', function() assert(false, 'failure') end)\n",
    )
    .unwrap();
    fs::write(temp.path().join("tests/report_spec.tua"), "print('stdout noise')\nit('native failure', function() assert.equals(1, 2) end)\npending('later')\n").unwrap();
    let run = tua()
        .args(["test", "--report", "report.json"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(!run.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(temp.path().join("report.json")).unwrap()).unwrap();
    assert_eq!(report["completed"], true, "{report}");
    assert_eq!(report["success"], false);
    assert!(report["output"].as_str().unwrap().contains("stdout noise"));
    let tests = report["tests"].as_array().unwrap();
    let native = tests
        .iter()
        .find(|case| case["name"] == "native failure")
        .unwrap();
    assert!(
        native["file"]
            .as_str()
            .unwrap()
            .ends_with("tests/report_spec.tua"),
        "{native}"
    );
    assert_eq!(native["line"], 2);
    let lua = tests
        .iter()
        .find(|case| case["name"] == "lua failure")
        .unwrap();
    assert!(
        lua["file"]
            .as_str()
            .unwrap()
            .ends_with("tests/plain_spec.lua"),
        "{lua}"
    );
    fs::write(temp.path().join("tests/report_spec.tua"), "invalid ???").unwrap();
    assert!(!tua()
        .args(["test", "--report", "report.json"])
        .current_dir(temp.path())
        .status()
        .unwrap()
        .success());
    let incomplete: serde_json::Value =
        serde_json::from_slice(&fs::read(temp.path().join("report.json")).unwrap()).unwrap();
    assert_eq!(incomplete["completed"], false);
    assert!(incomplete["tests"].as_array().unwrap().is_empty());
}

#[test]
fn unit_test_coverage_maps_native_lines_and_includes_unloaded_lua() {
    if !Command::new("lua")
        .args(["-e", "require('luacov.runner'); require('busted')"])
        .current_dir(std::env::temp_dir())
        .output()
        .is_ok_and(|o| o.status.success())
    {
        return;
    }
    let temp = TempDir::new("test-coverage");
    unit_test_project(&temp);
    fs::write(
        temp.path().join("src/unused.lua"),
        "-- comment\nlocal function unused()\n  return 42\nend\nreturn unused\n",
    )
    .unwrap();
    let result = tua()
        .args([
            "test",
            "--coverage",
            "coverage.json",
            "--report",
            "report.json",
        ])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(temp.path().join("coverage.json")).unwrap()).unwrap();
    assert_eq!(report["completed"], true, "{report}");
    let files = report["files"].as_array().unwrap();
    assert!(
        files
            .iter()
            .all(|file| !file["path"].as_str().unwrap().contains(".tua/test-")),
        "{report}"
    );
    let native = files
        .iter()
        .find(|file| file["path"].as_str().unwrap().ends_with("src/mathlib.tua"))
        .unwrap();
    assert!(
        native["lines"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["line"] == 2 && row["hits"].as_u64().unwrap() > 0),
        "{report}"
    );
    let unused = files
        .iter()
        .find(|file| file["path"].as_str().unwrap().ends_with("src/unused.lua"))
        .unwrap();
    assert_eq!(unused["covered"], 0);
    assert!(unused["total"].as_u64().unwrap() > 0);
    assert!(
        !unused["lines"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["line"] == 1),
        "{report}"
    );
    fs::write(temp.path().join("src/mathlib.tua"), "invalid ???").unwrap();
    let result = tua()
        .args(["test", "--coverage", "coverage.json"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(!result.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(temp.path().join("coverage.json")).unwrap()).unwrap();
    assert_eq!(report["completed"], false);
    assert_eq!(report["files"], serde_json::json!([]));
}

#[test]
fn in_place_declarations_preserve_handwritten_lua() {
    let temp = TempDir::new("in-place-declaration-owner");
    fs::write(
        temp.path().join(CONFIG_FILE),
        "source = \"src\"\nout = \"src\"\n",
    )
    .unwrap();
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(
        temp.path().join("src/types.d.tua"),
        "export type Point = {x: number}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("src/types.d.lua"),
        "return { handwritten = true }\n",
    )
    .unwrap();
    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("src/types.d.lua")).unwrap(),
        "return { handwritten = true }\n"
    );
}

#[cfg(unix)]
#[test]
fn test_report_preserves_backend_startup_error() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new("test-report-backend-error");
    fs::write(temp.path().join(CONFIG_FILE), "source = \"src\"\n").unwrap();
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(
        temp.path().join("src/main_spec.tua"),
        "describe(\"suite\", function() end)\n",
    )
    .unwrap();
    let backend = temp.path().join("broken-busted");
    fs::write(
        &backend,
        "#!/bin/sh\necho 'missing Lua backend dependency' >&2\nexit 2\n",
    )
    .unwrap();
    fs::set_permissions(&backend, fs::Permissions::from_mode(0o755)).unwrap();
    let output = tua()
        .args(["test", "--busted"])
        .arg(&backend)
        .args(["--report", "results.json"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing Lua backend dependency"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn dev_output_root_change_prepares_new_location() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("dev-output-root-change");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).unwrap();
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(
        temp.path().join("src/main.tua"),
        "local value = 1\nreturn value\n",
    )
    .unwrap();
    let mut child = tua()
        .args([
            "dev",
            "--no-launch",
            "--love",
            "missing-love-must-not-be-launched",
        ])
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_POLL_MS", "20")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "40")
        .env("TUA_INTERNAL_WATCH_RECONCILE_MS", "100")
        .env("TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let capture = CapturedChildStdout::start(&mut child, "dev: ready ");
    capture
        .ready
        .recv_timeout(Duration::from_secs(10))
        .expect("dev did not prepare initial output");
    assert!(temp.path().join("build/tuahotreload.lua").is_file());
    assert!(fs::read_to_string(temp.path().join("build/main.lua"))
        .unwrap()
        .contains("require(\"tuahotreload\").install()"));
    let signal = temp.path().join("build/.tuahotreload");
    assert_eq!(fs::read_to_string(&signal).unwrap(), "1\n");
    fs::write(
        temp.path().join(CONFIG_FILE),
        DEFAULT_CONFIG_TEXT.replace("out = \"build\"", "out = \"other-build\""),
    )
    .unwrap();
    let output = wait_for_child_output(child, Duration::from_secs(5));
    let stdout = String::from_utf8(capture.finish()).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(stdout.matches("dev: ready ").count(), 2);
    assert!(temp.path().join("other-build/tuahotreload.lua").is_file());
    assert!(fs::read_to_string(temp.path().join("other-build/main.lua"))
        .unwrap()
        .contains("require(\"tuahotreload\").install()"));
    assert_eq!(fs::read_to_string(signal).unwrap(), "1\n");
    assert_eq!(
        fs::read_to_string(temp.path().join("other-build/.tuahotreload")).unwrap(),
        "2\n"
    );
}
