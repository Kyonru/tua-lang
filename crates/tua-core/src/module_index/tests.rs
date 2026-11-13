use super::*;
use crate::{parse_lua_source, parse_tua_source};
use std::fs;
use std::path::PathBuf;

#[test]
fn module_cache_detects_same_size_same_timestamp_edits() {
    let project = TempProject::new("cache_content");
    project.write("module.lua", "return { value = 123 }");
    let path = project.path.join("module.lua");
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(
        required_module_exports_for_path(&path)
            .unwrap()
            .field("value"),
        Some(&Type::number())
    );
    project.write("module.lua", "return { value = 'x' }");
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert_eq!(
        required_module_exports_for_path(&path)
            .unwrap()
            .field("value"),
        Some(&Type::string())
    );
}

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
    let Some(Type::Function(play)) = exports.field("play") else {
        panic!("play should be exported as a function");
    };
    assert_eq!(play.parameters[0].ty, Type::Any);
    assert_eq!(play.parameters[0].name.as_deref(), Some("source"));
    assert_eq!(play.return_types_or_any(), &[Type::nil()]);
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
fn returned_local_preserves_initializer_fields_after_method_assignments() {
    let parsed = parse_tua_source(
        "src/world.tua",
        "local WIDTH = 2400\nlocal HEIGHT = 1600\nlocal M = { width = WIDTH, height = HEIGHT, stars = {}, meteors = {}, spawnTimer = 0 }\nfunction M.reset()\n  M.stars = {}\n  M.meteors = {}\n  M.spawnTimer = 0\nend\nfunction M.update(dt: number)\n  M.spawnTimer -= dt\nend\nreturn M\n",
    )
    .expect("module should parse");
    let exports = infer_module_exports(&parsed).expect("module should have exports");
    for name in ["width", "height", "stars", "reset"] {
        assert!(
            exports.field(name).is_some(),
            "{name} missing from {exports:#?}"
        );
    }
    let (environment, _) = crate::types::build_type_environment_with_domain_registry(
        &parsed,
        crate::semantic_domains::BuiltinDomainRegistry::with_ecs(true),
    );
    let project_exports = infer_module_exports_with_environment(&parsed, &environment)
        .expect("project module should have exports");
    for name in ["width", "height", "stars", "reset"] {
        assert!(
            project_exports.field(name).is_some(),
            "{name} missing from {project_exports:#?}"
        );
    }
}

#[test]
fn infers_root_function_modules_and_anonymous_function_returns() {
    let function_module = parse_lua_source(
        "src/logger.lua",
        r#"
return function(message)
  return message, true
end
"#,
    )
    .expect("Lua source should parse");
    let exports = infer_module_exports(&function_module).expect("function module should infer");

    assert_eq!(
        exports.value_type,
        Type::function_returns([Type::Any], [Type::Any, Type::boolean()])
    );
    assert!(exports.fields.is_empty());

    let table_module = parse_lua_source(
        "src/factory.lua",
        r#"
return {
  create = function(name)
    return { name = name, active = true, score = 0 }
  end,
}
"#,
    )
    .expect("Lua source should parse");
    let exports = infer_module_exports(&table_module).expect("table module should infer");
    let Type::Function(create) = exports.field("create").expect("create should be exported") else {
        panic!("create should be a function");
    };
    let Type::Table(created) = create.return_type.as_ref() else {
        panic!("create should return a table");
    };

    assert_eq!(
        created.field("name").map(|field| &field.ty),
        Some(&Type::Any)
    );
    assert_eq!(
        created.field("active").map(|field| &field.ty),
        Some(&Type::boolean())
    );
    assert_eq!(
        created.field("score").map(|field| &field.ty),
        Some(&Type::number())
    );
}

#[test]
fn follows_safe_module_aliases_and_nested_field_assignments() {
    let parsed = parse_lua_source(
        "src/settings.lua",
        r#"
local Module = {}
local Public = Module
Public.settings = {}
Public.settings.video = {}
Public.settings.video.fullscreen = false
Public.settings.video.scale = 2
return Module
"#,
    )
    .expect("Lua source should parse");
    let exports = infer_module_exports(&parsed).expect("aliased module should infer");
    let Type::Table(settings) = exports
        .field("settings")
        .expect("settings should be exported")
    else {
        panic!("settings should be a table");
    };
    let Some(Type::Table(video)) = settings.field("video").map(|field| &field.ty) else {
        panic!("video should be a table");
    };

    assert_eq!(
        video.field("fullscreen").map(|field| &field.ty),
        Some(&Type::boolean())
    );
    assert_eq!(
        video.field("scale").map(|field| &field.ty),
        Some(&Type::number())
    );
}

#[test]
fn stops_following_an_alias_after_its_target_is_reassigned() {
    let parsed = parse_lua_source(
        "src/reassigned.lua",
        r#"
local Module = {}
local OldModule = Module
Module = {}
OldModule.oldField = true
Module.newField = 1
return Module
"#,
    )
    .expect("Lua source should parse");
    let exports = infer_module_exports(&parsed).expect("reassigned module should infer");

    assert_eq!(exports.field("newField"), Some(&Type::number()));
    assert_eq!(exports.field("oldField"), None);
}

#[test]
fn preserves_common_lua_metatable_constructor_shapes() {
    let parsed = parse_lua_source(
        "src/player.lua",
        r#"
local Player = {}
Player.__index = Player

function Player.new(name)
  return setmetatable({ name = name, x = 0, active = true }, Player)
end

function Player:update(dt)
  self.x = self.x + dt
end

function Player.serialize(value)
  return tostring(value.x)
end

return Player
"#,
    )
    .expect("Lua source should parse");
    let exports = infer_module_exports(&parsed).expect("metatable module should infer");
    let Type::Function(constructor) = exports.field("new").expect("new should be exported") else {
        panic!("new should be a function");
    };
    let Type::Table(instance) = constructor.return_type.as_ref() else {
        panic!("new should return the instance shape: {constructor:#?}");
    };
    let fields = instance.semantic_fields();
    let Type::Function(update) = &fields.get("update").unwrap().ty else {
        panic!("method");
    };
    assert_eq!(
        update.parameters.len(),
        2,
        "colon methods include the implicit receiver"
    );
    assert_eq!(update.required_parameter_count, 2);

    assert_eq!(fields.get("name").map(|field| &field.ty), Some(&Type::Any));
    assert_eq!(
        fields.get("x").map(|field| &field.ty),
        Some(&Type::number())
    );
    assert_eq!(
        fields.get("active").map(|field| &field.ty),
        Some(&Type::boolean())
    );
    assert!(
        matches!(
            fields.get("update").map(|field| &field.ty),
            Some(Type::Function(_))
        ),
        "exports: {exports:#?}, instance fields: {fields:#?}"
    );
    assert!(!fields.contains_key("serialize"));
    assert!(matches!(
        exports.field("serialize"),
        Some(Type::Function(_))
    ));
}

#[test]
fn merges_conditional_lua_exports_without_claiming_missing_fields() {
    let parsed = parse_lua_source(
        "src/platform.lua",
        r#"
local Module = {}
if isWindows then
  Module.separator = "\\"
  Module.debugName = "windows"
else
  Module.separator = "/"
end
return Module
"#,
    )
    .expect("Lua source should parse");
    let exports = infer_module_exports(&parsed).expect("conditional module should infer");

    assert_eq!(exports.field("separator"), Some(&Type::string()));
    assert_eq!(
        exports.field("debugName"),
        Some(&Type::union([Type::string(), Type::nil()]))
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
    let Some(Type::Function(add)) = exports.field("add") else {
        panic!("add should be exported as a function");
    };
    assert_eq!(add.parameters[0].ty, Type::number());
    assert_eq!(add.parameters[0].name.as_deref(), Some("amount"));
    assert_eq!(add.return_types_or_any(), &[Type::nil()]);
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
fn recognizes_lua_string_call_require_bindings_for_module_properties() {
    let temp = TempProject::new("string-call-require-binding");
    temp.write("tua.toml", "source = \"src\"\nout = \"game\"\n");
    temp.write("src/events.tua", "return { enemies = signal() }\n");
    let main = temp.path.join("src/main.tua");
    let parsed = parse_tua_source(&main, "local Events = require \"events\"\n")
        .expect("string-call require should parse");

    let required = required_module_for_receiver(&parsed, "Events")
        .expect("string-call require should bind the module receiver");
    assert_eq!(required.path, temp.path.join("src/events.tua"));
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

    let Some(Type::Function(play)) = exports.field("play") else {
        panic!("play should be exported as a function");
    };
    assert_eq!(play.parameters[0].ty, Type::Any);
    assert_eq!(play.parameters[0].name.as_deref(), Some("source"));
    assert_eq!(play.return_types_or_any(), &[Type::nil()]);
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

#[test]
fn infers_exports_from_lua_modules_that_use_luajit_goto() {
    let source = r#"
local M = {}

function M.sumOdd(items)
  local total = 0
  for _, value in ipairs(items) do
    if value % 2 == 0 then goto continue end
    total = total + value
    ::continue::
  end
  return total
end

return M
"#;
    let parsed = parse_lua_source("src/lib.lua", source).expect("LuaJIT goto should parse");

    let exports = infer_module_exports(&parsed).expect("exports should be inferred");

    assert!(exports.field("sumOdd").is_some(), "{exports:#?}");
    assert!(
        parse_tua_source("src/lib.tua", source).is_err(),
        "Tua source keeps rejecting goto"
    );
}

#[test]
fn lua_exported_wrappers_preserve_untyped_named_and_variadic_parameters() {
    let mut host = crate::AnalysisHost::new();
    host.set_source_roots([std::path::PathBuf::from("src")]);
    host.set_file("src/wrapper.lua", "return { call = function(...) return select('#', ...) end, named = function(first, ...) return first end }\n");
    let source = "local Wrapper = require('wrapper')\nlocal count = Wrapper.call(1, 2, 3)\nlocal first = Wrapper.named('one', 2, 3)\n";
    host.set_file("src/main.tua", source);
    let snapshot = host.snapshot();
    assert!(snapshot
        .diagnostics("src/main.tua")
        .iter()
        .all(|d| d.severity != crate::DiagnosticSeverity::Error));
    let parsed = crate::parse_lua_source(
        "src/wrapper.lua",
        "return function(first, ...) return first end\n",
    )
    .unwrap();
    let function = match parsed.ast.nodes().last_stmt().unwrap() {
        full_moon::ast::LastStmt::Return(stmt) => match stmt.returns().iter().next().unwrap() {
            full_moon::ast::Expression::Function(function) => function,
            _ => panic!("expected function"),
        },
        _ => panic!("expected return"),
    };
    let crate::Type::Function(ty) = crate::types::function_type_from_body(function.body()) else {
        panic!("expected callable")
    };
    assert_eq!(ty.parameters.len(), 1);
    assert_eq!(ty.parameters[0].name.as_deref(), Some("first"));
    assert_eq!(ty.vararg_type.as_deref(), Some(&crate::Type::Any));
}

#[test]
fn nested_callable_module_exports_preserve_declared_functions() {
    for returned in [
        "{ smooth = camera.smooth }",
        "setmetatable({ smooth = camera.smooth }, { __call = function() return {} end })",
    ] {
        let source = format!(
            r#"
local camera = {{}}
camera.__index = camera
camera.smooth = {{}}
---@param speed number
---@return number
function camera.smooth.damped(speed) return speed end
return {returned}
"#
        );
        let parsed = parse_lua_source("src/vendor.lua", source).unwrap();
        let exports = infer_module_exports(&parsed).unwrap();
        let Some(Type::Table(smooth)) = exports.field("smooth") else {
            panic!("nested table missing: {exports:?}");
        };
        let Some(Type::Function(damped)) = smooth.field("damped").map(|field| &field.ty) else {
            panic!("nested function missing: {smooth:?}");
        };
        assert_eq!(damped.parameters[0].ty, Type::number());
        assert_eq!(damped.return_types_or_any(), vec![Type::number()]);
    }
}

#[test]
fn imported_lua_overloads_preserve_all_call_contracts() {
    let project = TempProject::new("overloads");
    project.write("tua.toml", "source = \"src\"\n");
    project.write("src/legacy.lua", "---@overload fun(value: string): string\n---@overload fun(value: number): number\nlocal function identity(value) return value end\nreturn {identity=identity}\n");
    let path = project.path.join("src/main.tua");
    let source =
        "local Legacy = require('legacy')\nlocal value = Legacy.identity(true)\nprint(value)\n";
    project.write("src/main.tua", source);
    let parsed = parse_tua_source(&path, source).unwrap();
    let exports = required_module_exports_for_receiver(&parsed, "Legacy").unwrap();
    let Type::Function(function) = exports.field("identity").unwrap() else {
        panic!("function");
    };
    assert_eq!(function.overloads.len(), 2);
    assert!(crate::check_tua_source(&path, source)
        .unwrap_err()
        .iter()
        .any(|d| d.code == Some(crate::DiagnosticCode::TYPE_MISMATCH)));
    crate::check_tua_source(&path, source.replace("true", "42")).unwrap();
    let parsed = parse_tua_source(&path, source.replace("true", "42")).unwrap();
    let environment = crate::build_type_environment(&parsed).unwrap();
    let model = crate::semantic::SemanticModel::new(&parsed, &environment);
    assert_eq!(
        model.type_of_name("value", crate::SourcePosition { line: 3, column: 7 }),
        Some(Type::number())
    );
}

#[test]
fn final_review_transitive_module_cache_tracks_dependency_edits() {
    let project = TempProject::new("review_transitive_cache");
    project.write("tua.toml", "source = \"src\"\n");
    project.write("src/b.tua", "return { value = 123 }\n");
    project.write(
        "src/a.tua",
        "local value = require('b').value\nreturn { value = value }\n",
    );
    let path = project.path.join("src/a.tua");
    assert_eq!(
        required_module_exports_for_path(&path)
            .unwrap()
            .field("value"),
        Some(&Type::number())
    );
    project.write("src/b.tua", "return { value = 'changed' }\n");
    assert_eq!(
        required_module_exports_for_path(&path)
            .unwrap()
            .field("value"),
        Some(&Type::string())
    );
}

#[test]
fn many_imported_expressions_keep_distinct_export_types_after_an_edit() {
    let project = TempProject::new("many_imported_expressions");
    project.write("tua.toml", "source = \"src\"\n");
    let mut source = String::new();
    for index in 0..64 {
        project.write(
            &format!("src/m{index}.tua"),
            &format!("return {{ value = {index} }}\n"),
        );
        source.push_str(&format!("local M{index} = require('m{index}')\n"));
    }
    source.push_str("return { first = M0.value, last = M63.value }\n");
    project.write("src/entry.tua", &source);
    let entry = project.path.join("src/entry.tua");

    let exports = required_module_exports_for_path(&entry).expect("entry exports");
    assert_eq!(exports.field("first"), Some(&Type::number()));
    assert_eq!(exports.field("last"), Some(&Type::number()));

    project.write("src/m63.tua", "return { value = 'updated' }\n");
    let updated = required_module_exports_for_path(&entry).expect("updated exports");
    assert_eq!(updated.field("first"), Some(&Type::number()));
    assert_eq!(updated.field("last"), Some(&Type::string()));
}

#[test]
fn callable_import_remains_referenced_when_other_imports_are_unused() {
    let project = TempProject::new("callable_import_with_unused_imports");
    project.write("tua.toml", "source = \"src\"\n");
    project.write("src/unused.tua", "return { value = 'unused' }\n");
    project.write(
        "src/factory.tua",
        "return function() return { value = 7 } end\n",
    );
    project.write(
        "src/entry.tua",
        "local Unused = require('unused')\nlocal Make = require('factory')\nlocal item = Make()\nreturn { value = item.value }\n",
    );
    let exports = required_module_exports_for_path(&project.path.join("src/entry.tua"))
        .expect("entry exports");
    assert_eq!(exports.field("value"), Some(&Type::number()));
}

#[test]
fn dependency_walk_reuses_exports_without_retaining_transitive_results() {
    let project = TempProject::new("memoized_import_walk");
    project.write("tua.toml", "source = \"src\"\n");
    const DEPTH: usize = 10;
    for index in 0..DEPTH {
        project.write(
            &format!("src/m{index}.tua"),
            &format!(
                "local child = require('m{}')\nreturn {{ value = child.value }}\n",
                index + 1
            ),
        );
    }
    project.write("src/m10.tua", "return { value = 123 }\n");
    MODULE_EXPORT_INFERENCE_COUNT.with(|count| count.set(0));
    let root = project.path.join("src/m0.tua");
    assert_eq!(
        required_module_exports_for_path(&root)
            .unwrap()
            .field("value"),
        Some(&Type::number())
    );
    let count = MODULE_EXPORT_INFERENCE_COUNT.with(|count| count.get());
    assert!(
        count <= DEPTH + 1,
        "dependency walk repeated inference {count} times"
    );
    MODULE_EXPORT_WALK_CACHE.with(|cache| assert!(cache.borrow().is_empty()));
    assert_eq!(
        required_module_exports_for_path(&root)
            .unwrap()
            .field("value"),
        Some(&Type::number())
    );
    assert_eq!(
        MODULE_EXPORT_INFERENCE_COUNT.with(|count| count.get()),
        count,
        "an unchanged dependency graph must reuse inference across requests"
    );
    project.write("src/m10.tua", "return { value = 'changed' }\n");
    assert_eq!(
        required_module_exports_for_path(&root)
            .unwrap()
            .field("value"),
        Some(&Type::string())
    );
}

#[test]
fn dependency_cache_tracks_new_deleted_and_preferred_modules() {
    let project = TempProject::new("dependency_resolution_changes");
    project.write("tua.toml", "source = \"src\"\n");
    project.write(
        "src/main.tua",
        "local value = require('provider').value\nreturn { value = value }\n",
    );
    let path = project.path.join("src/main.tua");
    let value = || {
        required_module_exports_for_path(&path)
            .unwrap()
            .field("value")
            .cloned()
    };
    assert_eq!(value(), Some(Type::Any));
    project.write("src/provider.lua", "return { value = 123 }\n");
    assert_eq!(value(), Some(Type::number()));
    project.write("src/provider.tua", "return { value = 'preferred' }\n");
    assert_eq!(value(), Some(Type::string()));
    fs::remove_file(project.path.join("src/provider.tua")).unwrap();
    assert_eq!(value(), Some(Type::number()));
    fs::remove_file(project.path.join("src/provider.lua")).unwrap();
    assert_eq!(value(), Some(Type::Any));
}

#[test]
fn dependency_cache_does_not_reuse_cycle_truncated_exports() {
    let project = TempProject::new("dependency_cycle_context");
    project.write("tua.toml", "source = \"src\"\n");
    project.write(
        "src/a.tua",
        "local b = require('b')\nreturn { value = 123, other = b.value }\n",
    );
    project.write(
        "src/b.tua",
        "local a = require('a')\nreturn { value = 'b', other = a.value }\n",
    );
    for _ in 0..2 {
        let a = required_module_exports_for_path(&project.path.join("src/a.tua")).unwrap();
        assert_eq!(a.field("other"), Some(&Type::string()));
        let b = required_module_exports_for_path(&project.path.join("src/b.tua")).unwrap();
        assert_eq!(b.field("other"), Some(&Type::number()));
    }
}

#[test]
fn shared_dependency_graph_reuses_inference_and_invalidates_every_branch() {
    let project = TempProject::new("shared_dependency_graph");
    project.write("tua.toml", "source = \"src\"\n");
    // Each layer fans out and rejoins: disabling transitive caching makes this
    // small graph repeat work exponentially, like a game with shared services.
    const DEPTH: usize = 6;
    project.write("src/leaf.tua", "return { value = 123 }\n");
    for index in (0..DEPTH).rev() {
        let child = if index + 1 == DEPTH {
            "leaf".to_owned()
        } else {
            format!("layer{}", index + 1)
        };
        for side in ["left", "right"] {
            project.write(
                &format!("src/{side}{index}.tua"),
                &format!("local child = require('{child}')\nreturn {{ value = child.value }}\n"),
            );
        }
        project.write(
            &format!("src/layer{index}.tua"),
            &format!("local a = require('left{index}')\nlocal b = require('right{index}')\nreturn {{ value = a.value, other = b.value }}\n"),
        );
    }
    let root = project.path.join("src/layer0.tua");
    for (source, expected) in [
        ("return { value = 123 }\n", Type::number()),
        ("return { value = 'changed' }\n", Type::string()),
    ] {
        project.write("src/leaf.tua", source);
        MODULE_EXPORT_INFERENCE_COUNT.with(|count| count.set(0));
        let exports = required_module_exports_for_path(&root).unwrap();
        assert_eq!(exports.field("value"), Some(&expected));
        assert_eq!(exports.field("other"), Some(&expected));
        let count = MODULE_EXPORT_INFERENCE_COUNT.with(|count| count.get());
        assert!(count <= DEPTH * 3 + 1, "repeated inference: {count}");
        required_module_exports_for_path(&root).unwrap();
        assert_eq!(
            MODULE_EXPORT_INFERENCE_COUNT.with(|count| count.get()),
            count
        );
    }
}
