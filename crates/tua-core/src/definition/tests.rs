use super::*;
use crate::source_text::{byte_for_column, line_at, position_at};
use crate::Fixture;
use std::fs;

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
fn resolves_love_builtin_function_to_virtual_stub() {
    let source = r#"local player = love.graphics.newImage("assets/player.png")
"#;
    let position = position_at(source, "newImage");
    let definition = definition_tua_source("src/main.tua", source, position)
        .expect("source should parse")
        .expect("LOVE builtin should resolve");
    let uri = definition.path.to_string_lossy();
    let document = crate::builtin_definition_document(&uri).expect("builtin document should exist");

    assert_eq!(definition.kind, DefinitionKind::Builtin);
    assert_eq!(uri, "tua:/builtins/love.graphics.newImage.tua");
    assert_eq!(source_text_owned(&document, definition.span), "newImage");
    assert!(document.contains("function love.graphics.newImage"));
}

#[test]
fn resolves_love_object_method_to_virtual_stub_from_inferred_receiver() {
    let source = r#"local player = love.graphics.newImage("assets/player.png")
player:setFilter()
"#;
    let position = position_at(source, "setFilter");
    let definition = definition_tua_source("src/main.tua", source, position)
        .expect("source should parse")
        .expect("LOVE method should resolve");
    let uri = definition.path.to_string_lossy();
    let document = crate::builtin_definition_document(&uri).expect("builtin document should exist");

    assert_eq!(definition.kind, DefinitionKind::Builtin);
    assert_eq!(uri, "tua:/builtins/Image.setFilter.tua");
    assert_eq!(source_text_owned(&document, definition.span), "setFilter");
    assert!(document.contains("function Image:setFilter"));
}

#[test]
fn resolves_object_pool_constructor_and_method_to_virtual_stubs() {
    let source = r#"type Bullet = { active: boolean }
local bullets = objectPool({
  create = function(): Bullet
    return { active = false }
  end,
})
local bullet = bullets:acquire()
bullets:release(bullet)
"#;

    for (label, expected_uri) in [
        ("objectPool", "tua:/builtins/objectPool.tua"),
        ("release", "tua:/builtins/ObjectPool.release.tua"),
    ] {
        let definition = definition_tua_source("src/main.tua", source, position_at(source, label))
            .expect("source should parse")
            .expect("object pool builtin should resolve");
        let uri = definition.path.to_string_lossy();
        let document =
            crate::builtin_definition_document(&uri).expect("builtin document should exist");

        assert_eq!(definition.kind, DefinitionKind::Builtin);
        assert_eq!(uri, expected_uri);
        assert_eq!(source_text_owned(&document, definition.span), label);
    }
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
fn resolves_typed_parameter_field_inside_table_constructor() {
    let source = r#"
type MetaVector = {
  x: number,
  y: number,
  length: number,
}

local function combineVector(left: MetaVector, right: MetaVector): MetaVector
  return {
    x = left.x + right.x,
    y = left.y + right.y,
    length = left.length + right.length,
  }
end
"#;

    assert_definition_in_source(
        source,
        "right.y",
        "right.".len(),
        DefinitionKind::TableField,
        "y",
    );
}

#[test]
fn resolves_self_index_instance_method_to_prototype_method() {
    let source = r#"
local Account = {}
Account.__index = Account

function Account:deposit(amount)
  self.balance = self.balance + amount
end

function Account:withdraw(amount)
  self.balance = self.balance - amount
end

local account = setmetatable({ name = "checking", balance = 50 }, Account)
account:deposit(25)
account:withdraw(10)
"#;

    assert_definition_in_source(
        source,
        "deposit(25)",
        0,
        DefinitionKind::Function,
        "Account:deposit",
    );
    assert_definition_in_source(
        source,
        "withdraw(10)",
        0,
        DefinitionKind::Function,
        "Account:withdraw",
    );
}

#[test]
fn resolves_self_index_instance_dot_call_to_prototype_method() {
    let source = r#"
local Account = {}
Account.__index = Account

function Account:deposit(amount)
  self.balance = self.balance + amount
end

local account = setmetatable({ balance = 50 }, Account)
account.deposit(account, 25)
"#;

    assert_definition_in_source(
        source,
        "deposit(account",
        0,
        DefinitionKind::Function,
        "Account:deposit",
    );
}

#[test]
fn resolves_class_constructor_instance_fields_and_methods() {
    let source = r#"
final class BeatManager {
  steps: number[]

  init() {
    self.steps = {}
  }

  add(step: number): number {
    return step
  }
}

local manager = BeatManager.new()
local first = manager.steps[1]
local added = manager.add(manager, 1)
"#;

    assert_definition_in_source(source, "steps[1]", 0, DefinitionKind::TableField, "steps");
    assert_definition_in_source(source, "add(manager", 0, DefinitionKind::Function, "add");
}

#[test]
fn unknown_symbols_and_any_fields_have_no_definition() {
    assert_no_definition("missing.value", 0);
    assert_no_definition("payload.value", "payload.".len());
}

#[test]
fn resolves_require_string_to_module_file() {
    let fixture = Fixture::parse(
        r#"-- file: tua.toml
source = "src"
-- file: src/main.tua
local Player = require("<|module|>player")
-- file: src/player.tua
local function makePlayer() end
return { makePlayer = makePlayer }
"#,
    );
    let temp = fixture
        .write_to_temp("require-module-definition")
        .expect("fixture should write");
    let source = fixture.source("src/main.tua");

    let mut position = fixture.position("module");
    let definition = definition_tua_source(temp.join("src/main.tua"), source, position)
        .expect("source should parse")
        .expect("module definition should resolve");

    assert_eq!(definition.kind, DefinitionKind::Module);
    assert_eq!(definition.path, temp.join("src/player.tua"));
    assert_eq!(
        source_text_owned(
            &fs::read_to_string(&definition.path).unwrap(),
            definition.span
        ),
        "makePlayer"
    );

    position.column += 1;
    assert!(
        definition_tua_source(temp.join("src/main.tua"), source, position)
            .expect("source should parse")
            .is_some()
    );
}

#[test]
fn resolves_required_tua_module_export_function() {
    let fixture = Fixture::parse(
        r#"-- file: tua.toml
source = "src"
-- file: src/main.tua
local Player = require("player")
local player = Player.<|call|>makePlayer()
-- file: src/player.tua
local function makePlayer()
  return { hp = 10 }
end

return {
  makePlayer = makePlayer,
}
"#,
    );
    let temp = fixture
        .write_to_temp("required-tua-export-definition")
        .expect("fixture should write");
    let source = fixture.source("src/main.tua");
    let module = fixture.source("src/player.tua");
    let definition =
        definition_tua_source(temp.join("src/main.tua"), source, fixture.position("call"))
            .expect("source should parse")
            .expect("module export definition should resolve");

    assert_eq!(definition.kind, DefinitionKind::Function);
    assert_eq!(definition.path, temp.join("src/player.tua"));
    assert_eq!(source_text_owned(module, definition.span), "makePlayer");
}

#[test]
fn resolves_required_lua_module_export_key() {
    let fixture = Fixture::parse(
        r#"-- file: tua.toml
source = "src"
-- file: src/main.tua
local Legacy = require("legacy")
local value = Legacy.<|call|>clamp(10, 0, 20)
-- file: src/legacy.lua
return {
  clamp = function(value, min, max)
    return value
  end,
}
"#,
    );
    let temp = fixture
        .write_to_temp("required-lua-export-definition")
        .expect("fixture should write");
    let source = fixture.source("src/main.tua");
    let module = fixture.source("src/legacy.lua");
    let definition =
        definition_tua_source(temp.join("src/main.tua"), source, fixture.position("call"))
            .expect("source should parse")
            .expect("Lua module export definition should resolve");

    assert_eq!(definition.kind, DefinitionKind::TableField);
    assert_eq!(definition.path, temp.join("src/legacy.lua"));
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

fn source_text_from(source: &str, span: SourceSpan) -> &str {
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
