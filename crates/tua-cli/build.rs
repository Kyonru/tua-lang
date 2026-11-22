use std::{env, path::PathBuf, process::Command};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|output| output.trim().to_owned())
    };
    // Archives without Git remain buildable and explicitly report unknown provenance.
    let checkout = git(&["rev-parse", "--show-toplevel"])
        .and_then(|path| PathBuf::from(path).canonicalize().ok())
        == root.canonicalize().ok();
    let revision = if checkout {
        git(&["rev-parse", "HEAD"]).unwrap_or_default()
    } else {
        String::new()
    };
    let dirty = checkout
        .then(|| git(&["status", "--porcelain", "--untracked-files=normal"]))
        .flatten()
        .map(|status| (!status.is_empty()).to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=TUA_BUILD_REVISION={revision}");
    println!("cargo:rustc-env=TUA_BUILD_DIRTY={dirty}");
    println!(
        "cargo:rustc-env=TUA_BUILD_TARGET={}",
        env::var("TARGET").unwrap()
    );
    println!(
        "cargo:rustc-env=TUA_BUILD_PROFILE={}",
        env::var("PROFILE").unwrap()
    );
    println!("cargo:rerun-if-changed=build.rs");
    // Follow worktree Git paths as well as ordinary checkouts. Source changes
    // must refresh the dirty marker even when the version does not change.
    if !checkout {
        return;
    }
    for name in ["HEAD", "index", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", name]) {
            let path = root.join(path);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"]) {
        if let Some(path) = git(&["rev-parse", "--git-path", &reference]) {
            let path = root.join(path);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
    if let Some(files) = git(&["ls-files"]) {
        for file in files.lines() {
            println!("cargo:rerun-if-changed={}", root.join(file).display());
        }
    }
}
