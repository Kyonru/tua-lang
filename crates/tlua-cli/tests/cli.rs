use std::process::Command;

fn tlua() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tlua"))
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
