mod support;

use std::fs;
use std::process::Command;

use support::{lua_executable, tua, TempDir};

const BOX_SOURCE: &str = r#"final class Box<T> {
  value: T
  init(value: T) { self.value = value }
  get(): T { return self.value }
  pair(): (T, T) { return self.value, self.value }
}
export type BoxInstance<T> = Box<T>
return { Box = Box }
"#;

const MAIN_SOURCE: &str = r#"import type { BoxInstance } from "./box"
local BoxModule = require("box")
local box: BoxInstance<number> = BoxModule.Box.new(3.5)
local function identity(value: number): number return value end
local function add(first: number, second: number): number return first + second end
local whole: number = math.floor(box:get())
local same: number = identity(box:get())
local doubled: number = add(box:pair())
assert(whole == 3 and same == 3.5 and doubled == 7)
"#;

#[test]
fn imported_class_method_preserves_final_argument_returns() {
    let temp = TempDir::new("imported-nested-call");
    fs::write(
        temp.path().join("tua.toml"),
        "source = \"src\"\nout = \"build\"\n",
    )
    .unwrap();
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src/box.tua"), BOX_SOURCE).unwrap();
    fs::write(temp.path().join("src/main.tua"), MAIN_SOURCE).unwrap();

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

    if let Some(lua) = lua_executable() {
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

    fs::write(
        temp.path().join("src/main.tua"),
        MAIN_SOURCE.replace(
            "local whole: number = math.floor(box:get())",
            "local whole: number = math.floor(BoxModule.Box.new('wrong'):get())",
        ),
    )
    .unwrap();
    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostics.contains("expected number, found string"),
        "{diagnostics}"
    );
    assert!(
        !diagnostics.contains("argument count mismatch"),
        "{diagnostics}"
    );
}
