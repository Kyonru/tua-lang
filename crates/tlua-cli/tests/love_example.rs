use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
        let path = env::temp_dir().join(format!("tlua-{name}-{}-{unique}", process::id()));

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
fn love_minimal_example_builds_and_runs_when_love_is_available() {
    let temp = TempDir::new("love-minimal");
    let example = example_root();

    copy_dir_all(&example, temp.path()).expect("failed to copy love-minimal example");

    let build_output = tlua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tlua build for love-minimal");

    assert!(
        build_output.status.success(),
        "tlua build failed\nstdout:\n{}\nstderr:\n{}",
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

fn example_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/love-minimal")
}

fn copy_dir_all(source: &Path, destination: &Path) -> std::io::Result<()> {
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

fn love_executable() -> Option<OsString> {
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

fn wait_for_child_output(mut child: process::Child, timeout: Duration) -> Output {
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

                panic!("LOVE did not exit before timeout\nstdout:\n{stdout}\nstderr:\n{stderr}");
            }
            Err(error) => panic!("failed to wait for LOVE process: {error}"),
        }
    }
}
