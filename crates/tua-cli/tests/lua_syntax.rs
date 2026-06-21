use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod support;

use support::{copy_dir_all, example_root, tua, TempDir};

const LUAJIT_BIN_ENV: &str = "TUA_LUAJIT_BIN";
const LUA_BIN_ENV: &str = "TUA_LUA_BIN";
const REQUIRE_LUAJIT_ENV: &str = "TUA_REQUIRE_LUAJIT";

struct Fixture {
    name: &'static str,
    out: &'static str,
}

#[derive(Debug)]
struct LuaValidator {
    program: OsString,
    runtime: LuaRuntime,
    version: String,
}

#[derive(Debug)]
enum LuaRuntime {
    LuaJit,
    Lua,
}

#[test]
fn emitted_example_lua_passes_syntax_validation() {
    let Some(validator) = discover_lua_validator() else {
        eprintln!("skipping emitted Lua syntax validation; no Lua executable was found");
        return;
    };

    eprintln!(
        "validating emitted Lua syntax with {} ({})",
        validator.label(),
        validator.version
    );

    for fixture in [
        Fixture {
            name: "love-minimal",
            out: "build",
        },
        Fixture {
            name: "love-full",
            out: "game",
        },
        Fixture {
            name: "features",
            out: "build",
        },
    ] {
        let temp = TempDir::new(&format!("lua-syntax-{}", fixture.name));
        copy_dir_all(&example_root(fixture.name), temp.path())
            .unwrap_or_else(|source| panic!("failed to copy {}: {source}", fixture.name));

        let build_output = tua()
            .arg("build")
            .current_dir(temp.path())
            .output()
            .unwrap_or_else(|source| {
                panic!("failed to run tua build for {}: {source}", fixture.name)
            });

        assert!(
            build_output.status.success(),
            "tua build failed for {}\nstdout:\n{}\nstderr:\n{}",
            fixture.name,
            String::from_utf8_lossy(&build_output.stdout),
            String::from_utf8_lossy(&build_output.stderr)
        );

        let emitted_files = emitted_lua_files(temp.path(), fixture.out);
        assert!(
            !emitted_files.is_empty(),
            "expected emitted Lua files for {}",
            fixture.name
        );

        for file in emitted_files {
            validate_lua_syntax(&validator, &file);
        }
    }
}

impl LuaValidator {
    fn label(&self) -> &'static str {
        match self.runtime {
            LuaRuntime::LuaJit => "LuaJIT",
            LuaRuntime::Lua => "Lua",
        }
    }
}

fn discover_lua_validator() -> Option<LuaValidator> {
    if let Some(program) = env::var_os(LUAJIT_BIN_ENV) {
        if let Some(version) = lua_version(&program) {
            return Some(LuaValidator {
                program,
                runtime: LuaRuntime::LuaJit,
                version,
            });
        }
    }

    if let Some(validator) = first_working_luajit() {
        return Some(validator);
    }

    if env_truthy(REQUIRE_LUAJIT_ENV) {
        panic!(
            "LuaJIT syntax validation is required by {REQUIRE_LUAJIT_ENV}=1, but no LuaJIT executable was found"
        );
    }

    eprintln!("skipping LuaJIT syntax validation; no LuaJIT executable was found");

    if let Some(program) = env::var_os(LUA_BIN_ENV) {
        if let Some(version) = lua_version(&program) {
            return Some(LuaValidator {
                program,
                runtime: LuaRuntime::Lua,
                version,
            });
        }
    }

    first_working_lua()
}

fn first_working_luajit() -> Option<LuaValidator> {
    [
        "luajit",
        "/opt/homebrew/bin/luajit",
        "/usr/local/bin/luajit",
    ]
    .into_iter()
    .find_map(|candidate| {
        let program = OsString::from(candidate);
        lua_version(&program).map(|version| LuaValidator {
            program,
            runtime: LuaRuntime::LuaJit,
            version,
        })
    })
}

fn first_working_lua() -> Option<LuaValidator> {
    ["lua5.1", "lua", "lua5.4", "lua5.3", "lua5.2"]
        .into_iter()
        .find_map(|candidate| {
            let program = OsString::from(candidate);
            lua_version(&program).map(|version| LuaValidator {
                program,
                runtime: LuaRuntime::Lua,
                version,
            })
        })
}

fn lua_version(program: &OsString) -> Option<String> {
    let output = Command::new(program).arg("-v").output().ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let version = format!("{stdout}{stderr}").trim().to_owned();

    (!version.is_empty()).then_some(version)
}

fn emitted_lua_files(project: &Path, out: &str) -> Vec<PathBuf> {
    let source_dir = project.join("src");
    let out_dir = project.join(out);
    let mut sources = Vec::new();

    discover_tua_files(&source_dir, &mut sources);

    let mut emitted = sources
        .into_iter()
        .map(|source| {
            let relative = source
                .strip_prefix(&source_dir)
                .expect("discovered source should be below source directory");
            out_dir.join(relative).with_extension("lua")
        })
        .collect::<Vec<_>>();

    emitted.sort();

    for file in &emitted {
        assert!(
            file.is_file(),
            "expected emitted Lua file {}",
            file.display()
        );
    }

    emitted
}

fn discover_tua_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|source| panic!("failed to read {}: {source}", dir.display()));

    for entry in entries {
        let entry =
            entry.unwrap_or_else(|source| panic!("failed to read directory entry: {source}"));
        let path = entry.path();
        let file_type = entry.file_type().unwrap_or_else(|source| {
            panic!("failed to read {} metadata: {source}", path.display())
        });

        if file_type.is_dir() {
            discover_tua_files(&path, files);
        } else if file_type.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension == "tua")
        {
            files.push(path);
        }
    }
}

fn validate_lua_syntax(validator: &LuaValidator, file: &Path) {
    let output = Command::new(&validator.program)
        .arg("-e")
        .arg(
            "local path = os.getenv('TUA_SYNTAX_FILE'); assert(path and path ~= '', 'missing TUA_SYNTAX_FILE'); assert(loadfile(path))",
        )
        .env("TUA_SYNTAX_FILE", file)
        .output()
        .unwrap_or_else(|source| {
            panic!(
                "failed to run {} for {}: {source}",
                validator.label(),
                file.display()
            )
        });

    assert!(
        output.status.success(),
        "{} syntax validation failed for {}\nstdout:\n{}\nstderr:\n{}",
        validator.label(),
        file.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn env_truthy(name: &str) -> bool {
    env::var(name)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}
