#![allow(dead_code)]

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::process::Command;
use std::process::Output;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub fn tlua() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tlua"))
}

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("tlua-{name}-{}-{unique}", process::id()));

        fs::create_dir_all(&path).expect("failed to create temp dir");

        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn example_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples")
        .join(name)
}

pub fn copy_dir_all(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_all(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path)?;
        }
    }

    Ok(())
}

pub fn love_executable() -> Option<OsString> {
    if let Some(path) = env::var_os("TLUA_LOVE_BIN") {
        return love_version_works(&path).then_some(path);
    }

    [
        OsString::from("love"),
        OsString::from("/Applications/love.app/Contents/MacOS/love"),
    ]
    .into_iter()
    .find(love_version_works)
}

fn love_version_works(candidate: &OsString) -> bool {
    Command::new(candidate)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

pub fn wait_for_child_output(mut child: process::Child, timeout: Duration) -> Output {
    let deadline = Instant::now() + timeout;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().expect("failed to read output"),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let output = child
                    .wait_with_output()
                    .expect("failed to read timed out output");
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                panic!("process did not exit before timeout\nstdout:\n{stdout}\nstderr:\n{stderr}");
            }
            Err(error) => panic!("failed to wait for child process: {error}"),
        }
    }
}
