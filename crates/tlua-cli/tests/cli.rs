use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use tlua_core::{CONFIG_FILE, DEFAULT_CONFIG_TEXT};

fn tlua() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tlua"))
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tlua-cli-{name}-{}-{unique}", process::id()));

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
    let output = tlua()
        .arg("--help")
        .output()
        .expect("failed to run tlua --help");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    for command in ["init", "check", "build", "watch", "lsp"] {
        assert!(stdout.contains(command), "missing command `{command}`");
    }

    assert!(stdout.contains("--emit-on-error"));
}

#[test]
fn unknown_command_exits_with_useful_error() {
    let output = tlua()
        .arg("compile")
        .output()
        .expect("failed to run tlua compile");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("unknown command `compile`"));
    assert!(stderr.contains("tlua --help"));
}

#[test]
fn invalid_flag_exits_with_useful_error() {
    let output = tlua()
        .args(["check", "--emit-on-error"])
        .output()
        .expect("failed to run tlua check --emit-on-error");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("only valid with `tlua build`"));
    assert!(stderr.contains("tlua --help"));
}

#[test]
fn init_creates_default_project_files() {
    let temp = TempDir::new("init");
    let output = tlua()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tlua init");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("initialized tlua project"));
    assert!(stdout.contains("created tlua.toml"));
    assert!(stdout.contains("created src/"));
    assert_eq!(
        fs::read_to_string(temp.path().join(CONFIG_FILE)).expect("failed to read config"),
        DEFAULT_CONFIG_TEXT
    );
    assert!(temp.path().join("src").is_dir());
}

#[test]
fn init_does_not_overwrite_existing_config() {
    let temp = TempDir::new("init-existing");
    let config_path = temp.path().join(CONFIG_FILE);
    let existing = "source = \"custom\"\n";

    fs::write(&config_path, existing).expect("failed to write existing config");

    let output = tlua()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tlua init");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("left unchanged"));
    assert_eq!(
        fs::read_to_string(config_path).expect("failed to read config"),
        existing
    );
    assert!(temp.path().join("src").is_dir());
}

#[test]
fn check_prints_config_diagnostics() {
    let temp = TempDir::new("check-invalid-config");

    fs::write(temp.path().join(CONFIG_FILE), "emit_maps = yes\n")
        .expect("failed to write invalid config");

    let output = tlua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tlua check");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("tlua.toml:1:1 error TL3001: expected boolean value"));
    assert!(!stderr.contains("panicked"));
}
