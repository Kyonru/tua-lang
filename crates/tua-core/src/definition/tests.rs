use super::*;
use crate::source_text::{byte_for_column, line_at, position_at};
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

const SOURCE: &str = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

local player: Player = {
  pos = { x = 0, y = 0 },
  hp = 10,
}

local function damage(target: Player, amount: number): Player
  target.hp -= amount
  return target
end

damage(player, 2)
player.hp = player.hp - 1
local payload: any = {}
payload.value = 1
missing.value = 2
"#;

#[test]
fn resolves_type_alias_reference() {
    assert_definition(
        "player: Player",
        "player: ".len(),
        DefinitionKind::TypeAlias,
        "Player",
    );
}

#[test]
fn type_annotations_prefer_aliases_over_runtime_values() {
    let source = r#"type Player = { hp: number }
local Player: any = nil
local player: Player = nil
"#;
    let mut position = position_at(source, "player: Player");

    position.column += "player: ".len();

    let definition = definition_tua_source("src/conflict.tua", source, position)
        .expect("source should parse")
        .expect("alias definition should resolve");

    assert_eq!(definition.kind, DefinitionKind::TypeAlias);
    assert_eq!(source_text_from(source, definition.span), "Player");
}

#[test]
fn resolves_function_call() {
    assert_definition("damage(player", 0, DefinitionKind::Function, "damage");
}

#[test]
fn resolves_global_function_call() {
    let source = r#"function test(a: boolean)
  return ""
end

test(false)
"#;
    let position = position_at(source, "test(false");
    let definition = definition_tua_source("src/main.tua", source, position)
        .expect("source should parse")
        .expect("global function call should resolve");

    assert_eq!(definition.kind, DefinitionKind::Function);
    assert_eq!(source_text_from(source, definition.span), "test");
}

#[test]
fn resolves_definition_when_an_unrelated_line_is_incomplete() {
    let source = r#"type Player = { hp: number }
local player: Player = { hp = 10 }
player.

function test(a: boolean)
  return ""
end

test(false)
"#;
    let position = position_at(source, "test(false");
    let definition = definition_tua_source("src/main.tua", source, position)
        .expect("definition should recover from one incomplete line")
        .expect("global function call should resolve");

    assert_eq!(definition.kind, DefinitionKind::Function);
    assert_eq!(source_text_from(source, definition.span), "test");
}

#[test]
fn resolves_local_usage() {
    assert_definition(
        "damage(player",
        "damage(".len(),
        DefinitionKind::Local,
        "player",
    );
}

#[test]
fn resolves_unannotated_local_values_returned_from_table() {
    let source = r#"
local Functions = require("functions")
local Compound = require("compound")
local Legacy = require("legacy")

local player = Functions.makePlayer("Lua interop")
local clamped = Legacy.clamp(Compound.stats.score, 0, 100)
local message = Legacy.describe("score", clamped)
local built_in = tostring(1)

return {
  player = player,
  clamped = clamped,
  message = message,
  built_in = built_in,
}
"#;

    assert_definition_in_source(
        source,
        "player = player",
        "player = ".len(),
        DefinitionKind::Local,
        "player",
    );
    assert_definition_in_source(
        source,
        "clamped = clamped",
        "clamped = ".len(),
        DefinitionKind::Local,
        "clamped",
    );
    assert_definition_in_source(
        source,
        "message = message",
        "message = ".len(),
        DefinitionKind::Local,
        "message",
    );
    assert_definition_in_source(
        source,
        "built_in = built_in",
        "built_in = ".len(),
        DefinitionKind::Local,
        "built_in",
    );
}

#[test]
fn resolves_parameter_usage() {
    assert_definition(
        "return target",
        "return ".len(),
        DefinitionKind::Parameter,
        "target",
    );
}

#[test]
fn resolves_table_field_to_type_shape_field() {
    assert_definition(
        "player.hp =",
        "player.".len(),
        DefinitionKind::TableField,
        "hp",
    );
}

#[test]
fn resolves_parameter_table_field_to_type_shape_field() {
    assert_definition(
        "target.hp -=",
        "target.".len(),
        DefinitionKind::TableField,
        "hp",
    );
}

#[test]
fn unknown_symbols_and_any_fields_have_no_definition() {
    assert_no_definition("missing.value", 0);
    assert_no_definition("payload.value", "payload.".len());
}

#[test]
fn resolves_require_string_to_module_file() {
    let temp = TempProject::new("require-module-definition");
    let source = r#"local Player = require("player")
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write(
        "src/player.tua",
        "local function makePlayer() end\nreturn { makePlayer = makePlayer }\n",
    );

    let mut position = position_at(source, "player\")");
    let definition = definition_tua_source(temp.path.join("src/main.tua"), source, position)
        .expect("source should parse")
        .expect("module definition should resolve");

    assert_eq!(definition.kind, DefinitionKind::Module);
    assert_eq!(definition.path, temp.path.join("src/player.tua"));
    assert_eq!(
        source_text_owned(
            &fs::read_to_string(&definition.path).unwrap(),
            definition.span
        ),
        "makePlayer"
    );

    position.column += 1;
    assert!(
        definition_tua_source(temp.path.join("src/main.tua"), source, position)
            .expect("source should parse")
            .is_some()
    );
}

#[test]
fn resolves_required_tua_module_export_function() {
    let temp = TempProject::new("required-tua-export-definition");
    let source = r#"local Player = require("player")
local player = Player.makePlayer()
"#;
    let module = r#"local function makePlayer()
  return { hp = 10 }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/player.tua", module);

    let mut position = position_at(source, "Player.makePlayer");
    position.column += "Player.".len();
    let definition = definition_tua_source(temp.path.join("src/main.tua"), source, position)
        .expect("source should parse")
        .expect("module export definition should resolve");

    assert_eq!(definition.kind, DefinitionKind::Function);
    assert_eq!(definition.path, temp.path.join("src/player.tua"));
    assert_eq!(source_text_owned(module, definition.span), "makePlayer");
}

#[test]
fn resolves_required_lua_module_export_key() {
    let temp = TempProject::new("required-lua-export-definition");
    let source = r#"local Legacy = require("legacy")
local value = Legacy.clamp(10, 0, 20)
"#;
    let module = r#"return {
  clamp = function(value, min, max)
    return value
  end,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/legacy.lua", module);

    let mut position = position_at(source, "Legacy.clamp");
    position.column += "Legacy.".len();
    let definition = definition_tua_source(temp.path.join("src/main.tua"), source, position)
        .expect("source should parse")
        .expect("Lua module export definition should resolve");

    assert_eq!(definition.kind, DefinitionKind::TableField);
    assert_eq!(definition.path, temp.path.join("src/legacy.lua"));
    assert_eq!(source_text_owned(module, definition.span), "clamp");
}

fn assert_definition(needle: &str, offset: usize, kind: DefinitionKind, expected_text: &str) {
    let definition = definition_at(needle, offset).expect("definition should resolve");

    assert_eq!(definition.kind, kind);
    assert_eq!(source_text(definition.span), expected_text);
}

fn assert_definition_in_source(
    source: &'static str,
    needle: &str,
    offset: usize,
    kind: DefinitionKind,
    expected_text: &str,
) {
    let mut position = position_at(source, needle);

    position.column += offset;

    let definition = definition_tua_source("src/main.tua", source, position)
        .expect("source should parse")
        .expect("definition should resolve");

    assert_eq!(definition.kind, kind);
    assert_eq!(source_text_from(source, definition.span), expected_text);
}

fn assert_no_definition(needle: &str, offset: usize) {
    assert_eq!(definition_at(needle, offset), None);
}

fn definition_at(needle: &str, offset: usize) -> Option<Definition> {
    let mut position = position_at(SOURCE, needle);

    position.column += offset;

    definition_tua_source("src/main.tua", SOURCE, position).expect("source should parse")
}

fn source_text(span: SourceSpan) -> &'static str {
    source_text_from(SOURCE, span)
}

fn source_text_from(source: &'static str, span: SourceSpan) -> &'static str {
    let line = line_at(source, span.start.line).expect("definition line should exist");
    let start = byte_for_column(line, span.start.column);
    let end = byte_for_column(line, span.end.column);

    &line[start..end]
}

fn source_text_owned(source: &str, span: SourceSpan) -> String {
    let line = line_at(source, span.start.line).expect("definition line should exist");
    let start = byte_for_column(line, span.start.column);
    let end = byte_for_column(line, span.end.column);

    line[start..end].to_owned()
}

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tua-core-{name}-{}-{unique}", process::id()));

        fs::create_dir_all(&path).expect("temp project should be created");

        Self { path }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory should be created");
        }

        fs::write(path, contents).expect("fixture should be written");
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
