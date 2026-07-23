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
    assert_eq!(exports.fields["play"].parameter_names, vec!["source"]);

    let Some(Type::Table(nested)) = exports.field("nested") else {
        panic!("nested export should be a table");
    };

    assert_eq!(
        nested.field("x").map(|field| &field.ty),
        Some(&Type::number())
    );
}

#[test]
fn infers_exports_from_returned_local_tables_with_string_literal_keys() {
    let parsed = parse_lua_source(
        "src/scale.lua",
        r#"
local SCALE_BY_VALUE = {
  ['--^---^'] = 'major',
  ['-^---^-'] = 'dorian',
}

return {
  SCALE_BY_VALUE = SCALE_BY_VALUE,
}
"#,
    )
    .expect("Lua source should parse");

    let exports = infer_module_exports(&parsed).expect("exports should be inferred");

    let Some(Type::Table(scale_by_value)) = exports.field("SCALE_BY_VALUE") else {
        panic!("SCALE_BY_VALUE export should be a table");
    };

    assert_eq!(
        scale_by_value.field("--^---^").map(|field| &field.ty),
        Some(&Type::string())
    );
    assert_eq!(
        scale_by_value.field("-^---^-").map(|field| &field.ty),
        Some(&Type::string())
    );
}

#[test]
fn infers_typed_anonymous_function_exports_from_returned_tua_table() {
    let parsed = parse_tua_source(
        "src/audio.tua",
        r#"
return {
  play = function(source:string, volume:number): boolean
    return true
  end,
}
"#,
    )
    .expect("Tua source should parse");

    let exports = infer_module_exports(&parsed).expect("exports should be inferred");

    assert_eq!(
        exports.field("play"),
        Some(&Type::function(
            [Type::string(), Type::number()],
            Type::boolean()
        ))
    );
    assert_eq!(
        exports.fields["play"].parameter_names,
        vec!["source", "volume"]
    );
}

#[test]
fn preserves_nominal_class_types_in_static_module_exports() {
    let parsed = parse_tua_source(
        "src/classes.tua",
        r#"
final class A {
  value: number = 1
}

final class B {
  value: number = 1
}

local function acceptA(value: A): A
  return value
end

return {
  A = A,
  B = B,
  acceptA = acceptA,
}
"#,
    )
    .expect("Tua source should parse");
    let exports = infer_module_exports(&parsed).expect("exports should be inferred");

    let Some(Type::Function(accept_a)) = exports.field("acceptA") else {
        panic!("acceptA should retain its function type");
    };
    let Type::Table(parameter) = &accept_a.parameters[0].ty else {
        panic!("acceptA parameter should resolve to the nominal class type");
    };
    assert_eq!(parameter.nominal_name(), Some("A"));

    let Some(Type::Table(class_b)) = exports.field("B") else {
        panic!("B should export its class value");
    };
    let Some(Type::Function(new_b)) = class_b.field("new").map(|field| &field.ty) else {
        panic!("B.new should retain its constructor type");
    };
    let Type::Table(instance_b) = new_b.return_type.as_ref() else {
        panic!("B.new should return the nominal class type");
    };
    assert_eq!(instance_b.nominal_name(), Some("B"));
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
    assert_eq!(exports.fields["play"].parameter_names, vec!["source"]);
}

#[test]
fn resolves_import_binding_exports_for_receiver() {
    let temp = TempProject::new("import-binding");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/main.tua",
        r#"
local Player = import("./player")
local made = Player.makePlayer("Ada")
"#,
    );
    temp.write(
        "src/player.tua",
        r#"
local function makePlayer(name: string)
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#,
    );

    let main = temp.path.join("src/main.tua");
    let source = fs::read_to_string(&main).expect("main source should be readable");
    let parsed = parse_tua_source(&main, source).expect("main source should parse");
    let exports = required_module_exports_for_receiver(&parsed, "Player")
        .expect("imported module exports should resolve");

    assert_eq!(
        exports.field("makePlayer"),
        Some(&Type::function(
            [Type::string()],
            Type::table([("name", crate::TableField::required(Type::string()))])
        ))
    );
}

#[test]
fn annotated_lua_modules_provide_typed_function_exports() {
    let temp = TempProject::new("annotated-lua-module");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/main.tua",
        r#"
local Audio = require("audio")
local ok = Audio.play("theme")
"#,
    );
    temp.write(
        "src/audio.lua",
        r#"---@param source string audio source
---@return boolean success flag
local function play(source)
  return true
end

return {
  play = play,
}
"#,
    );

    let main = temp.path.join("src/main.tua");
    let source = fs::read_to_string(&main).expect("main source should be readable");
    let parsed = parse_tua_source(&main, source).expect("main source should parse");
    let exports = required_module_exports_for_receiver(&parsed, "Audio")
        .expect("required module exports should resolve");

    assert_eq!(
        exports.field("play"),
        Some(&Type::function([Type::string()], Type::boolean()))
    );
    assert_eq!(
        exports.field_documentation("play"),
        Some("audio source\nsuccess flag")
    );
    assert_eq!(
        exports.fields["play"].parameter_documentation,
        vec![Some("audio source".to_owned())]
    );
    assert_eq!(exports.fields["play"].parameter_names, vec!["source"]);
}

#[test]
fn returned_table_function_fields_use_lua_doc_annotations() {
    let parsed = parse_lua_source(
        "src/audio.lua",
        r#"
return {
  ---@param source string inline source
  ---@return boolean inline success
  play = function(source)
    return true
  end,
}
"#,
    )
    .expect("Lua source should parse");
    let exports = infer_module_exports(&parsed).expect("exports should be inferred");

    assert_eq!(
        exports.field("play"),
        Some(&Type::function([Type::string()], Type::boolean()))
    );
    assert_eq!(
        exports.field_documentation("play"),
        Some("inline source\ninline success")
    );
    assert_eq!(
        exports.fields["play"].parameter_documentation,
        vec![Some("inline source".to_owned())]
    );
}

#[test]
fn module_exports_preserve_deprecation_metadata() {
    let source = r#"
---@deprecated use play
local function oldPlay() end

return {
  oldPlay = oldPlay,
  ---@deprecated use play
  oldInline = function() end,
}
"#;

    for parsed in [
        parse_lua_source("src/audio.lua", source).expect("Lua source should parse"),
        parse_tua_source("src/audio.tua", source).expect("Tua source should parse"),
    ] {
        let exports = infer_module_exports(&parsed).expect("exports should be inferred");

        assert_eq!(
            exports.fields["oldPlay"]
                .deprecation
                .as_ref()
                .and_then(|deprecation| deprecation.guidance.as_deref()),
            Some("use play")
        );
        assert_eq!(
            exports.fields["oldInline"]
                .deprecation
                .as_ref()
                .and_then(|deprecation| deprecation.guidance.as_deref()),
            Some("use play")
        );
    }
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
