use super::*;
use crate::{parse_lua_source, parse_tua_source};
use std::fs;
use std::path::PathBuf;

#[test]
fn infers_exports_from_returned_table_literal() {
    let parsed = parse_lua_source(
        "src/audio.lua",
        r#"
return {
  enabled = true,
  volume = 1,
  name = "music",
  nested = { x = 1 },
  play = function(source) end,
}
"#,
    )
    .expect("Lua source should parse");

    let exports = infer_module_exports(&parsed).expect("exports should be inferred");

    assert_eq!(exports.field("enabled"), Some(&Type::boolean()));
    assert_eq!(exports.field("volume"), Some(&Type::number()));
    assert_eq!(exports.field("name"), Some(&Type::string()));
    assert_eq!(
        exports.field("play"),
        Some(&Type::function([Type::Any], Type::Any))
    );

    let Some(Type::Table(nested)) = exports.field("nested") else {
        panic!("nested export should be a table");
    };

    assert_eq!(
        nested.field("x").map(|field| &field.ty),
        Some(&Type::number())
    );
}

#[test]
fn infers_exports_from_returned_tua_module_table() {
    let parsed = parse_tua_source(
        "src/score.tua",
        r#"
type ScoreModule = {
  score?: number,
}

local score: ScoreModule = {}

function score.get(): number
  return score.score
end

function score.add(amount: number)
  score.score += amount
end

return score
"#,
    )
    .expect("Tua source should parse");

    let exports = infer_module_exports(&parsed).expect("exports should be inferred");

    assert_eq!(exports.field("score"), Some(&Type::number()));
    assert_eq!(
        exports.field("get"),
        Some(&Type::function([], Type::number()))
    );
    assert_eq!(
        exports.field("add"),
        Some(&Type::function([Type::number()], Type::Any))
    );
}

#[test]
fn resolves_required_modules_from_configured_source_and_out_dirs() {
    let temp = TempProject::new("module-resolution");

    temp.write("tua.toml", "source = \"src\"\nout = \"game\"\n");
    temp.write("src/main.tua", "local player = require(\"player\")\n");
    temp.write("src/player.tua", "return { hp = 10 }\n");
    temp.write("game/utils.lua", "return { clamp = function(value) end }\n");

    let main = temp.path.join("src/main.tua");

    assert_eq!(
        resolve_required_module(&main, "player"),
        Some(temp.path.join("src/player.tua"))
    );
    assert_eq!(
        resolve_required_module(&main, "utils"),
        Some(temp.path.join("game/utils.lua"))
    );
}

#[test]
fn resolves_require_binding_exports_for_receiver() {
    let temp = TempProject::new("require-binding");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/main.tua",
        r#"
local audio = require("audio")
local value = audio.play
"#,
    );
    temp.write(
        "src/audio.lua",
        r#"
return {
  play = function(source) end,
}
"#,
    );

    let main = temp.path.join("src/main.tua");
    let source = fs::read_to_string(&main).expect("main source should be readable");
    let parsed = parse_tua_source(&main, source).expect("main source should parse");
    let exports = required_module_exports_for_receiver(&parsed, "audio")
        .expect("required module exports should resolve");

    assert_eq!(
        exports.field("play"),
        Some(&Type::function([Type::Any], Type::Any))
    );
}

#[test]
fn unknown_requires_and_dynamic_exports_are_quiet() {
    let parsed = parse_lua_source(
        "src/dynamic.lua",
        r#"
local key = "play"
return {
  [key] = function() end,
  ["string_key"] = 1,
}
"#,
    )
    .expect("Lua source should parse");

    assert_eq!(infer_module_exports(&parsed), None);
}

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("tua_{name}_{}", std::process::id()));

        if path.exists() {
            fs::remove_dir_all(&path).expect("stale temp project should be removed");
        }

        fs::create_dir_all(&path).expect("temp project should be created");

        Self { path }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir should be created");
        }

        fs::write(path, contents).expect("file should be written");
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
