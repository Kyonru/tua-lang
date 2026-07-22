use super::*;
use crate::source_text::position_at;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

const SOURCE: &str = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number }

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
"#;

#[test]
fn hovers_known_type_alias() {
    let hover = hover_at("Vec2").expect("alias should hover");

    assert_eq!(hover.contents, "type Vec2 = { x: number, y: number }");
}

#[test]
fn hover_pretty_prints_long_type_aliases() {
    let source = r#"type Feedback = {
  animation: string,
  callback: (event: string, previous: number, state: boolean) -> boolean,
  duration: number,
  event: string,
  payload: { channel: string, cue: string, volume: number },
  restart: boolean,
  target: string,
}
"#;
    let hover = hover_tua_source("src/main.tua", source, position_at(source, "Feedback"))
        .expect("source should parse")
        .expect("alias should hover");

    assert!(hover.contents.starts_with("type Feedback = {\n"));
    assert!(hover.contents.contains("\n  callback: "));
    assert!(hover.contents.contains("\n  payload: "));
    assert!(hover.contents.ends_with("\n}"));
}

#[test]
fn hovers_known_local() {
    let hover = hover_at("player:").expect("local should hover");

    assert_eq!(hover.contents, "local player: Player");
}

#[test]
fn hovers_builtin_ecs_module_and_documented_functions() {
    let source = "local world = ecs.world()\n";
    let module = hover_tua_source("src/main.tua", source, position_at(source, "ecs"))
        .expect("ECS module hover should succeed")
        .expect("ECS module should hover");
    let world = hover_tua_source("src/main.tua", source, position_at(source, "world()"))
        .expect("ecs.world hover should succeed")
        .expect("ecs.world should hover");

    assert_eq!(module.contents, "(built-in module) ecs");
    assert!(module
        .documentation
        .as_deref()
        .is_some_and(|documentation| documentation.contains("data-oriented ECS helpers")));
    assert_eq!(
        world.documentation.as_deref(),
        Some("Create an empty ECS world that owns entities and component storage.")
    );
}

#[test]
fn hovers_function_parameters() {
    let source = r#"local function recursive(a: string)
  if a == tostring(1) then
    return 1
  end

  if a == tostring(2) then
    return "value"
  end

  return recursive(a + 1)
end
"#;

    assert_hover_in_source(source, "a: string", 0, "parameter a: string");
    assert_hover_in_source(source, "a == tostring", 0, "parameter a: string");
    assert_hover_in_source(source, "a + 1", 0, "parameter a: string");
}

#[test]
fn hovers_innermost_matching_parameter() {
    let source = r#"local function outer(value: string)
  local function inner(value: number)
    return value
  end

  return value
end
"#;

    assert_hover_in_source(
        source,
        "return value",
        "return ".len(),
        "parameter value: number",
    );
    assert_hover_in_source(
        source,
        "return value\nend",
        "return ".len(),
        "parameter value: string",
    );
}

#[test]
fn hovers_flow_sensitive_narrowed_types_inside_guards() {
    let source = r#"local function read(value: number | nil, tagged: string | number)
  if value ~= nil then
    local copy = value
  end

  if type(tagged) == "string" then
    local label = tagged
  end
end
"#;

    assert_hover_in_source(
        source,
        "local copy = value",
        "local copy = ".len(),
        "parameter value: number",
    );
    assert_hover_in_source(
        source,
        "local label = tagged",
        "local label = ".len(),
        "parameter tagged: string",
    );
}

#[test]
fn hover_uses_assigned_type_after_mutation_in_narrowed_block() {
    let source = r#"local function read(value: number | nil)
  if value ~= nil then
    value = nil
    local copy = value
  end
end
"#;

    assert_hover_in_source(
        source,
        "local copy = value",
        "local copy = ".len(),
        "parameter value: number | nil",
    );
}

#[test]
fn hover_keeps_the_inferred_local_declaration_contract_after_reassignment() {
    let source = r#"local value = 1
value = "invalid"
local copy = value
"#;

    assert_hover_in_source(
        source,
        "local copy = value",
        "local copy = ".len(),
        "local value: number",
    );
}

#[test]
fn parameter_hover_follows_definition_identity() {
    let source = r#"local value: number = 1

local function before_shadow(value: string)
  local result = value
  local value: boolean = true
  return value
end
"#;

    assert_hover_in_source(
        source,
        "local result = value",
        "local result = ".len(),
        "parameter value: string",
    );
    assert_hover_in_source(
        source,
        "return value",
        "return ".len(),
        "local value: boolean",
    );
}

#[test]
fn hovers_unannotated_local_values_returned_from_table() {
    let source = r#"
type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 10 }
end

local player = makePlayer()
local clamped = 10
local message = tostring(clamped)
local built_in = tostring(1)

return {
  player = player,
  clamped = clamped,
  message = message,
  built_in = built_in,
}
"#;

    assert_hover_in_source(
        source,
        "player = player",
        "player = ".len(),
        "local player: Player",
    );
    assert_hover_in_source(
        source,
        "clamped = clamped",
        "clamped = ".len(),
        "local clamped: number",
    );
    assert_hover_in_source(
        source,
        "message = message",
        "message = ".len(),
        "local message: string",
    );
    assert_hover_in_source(
        source,
        "built_in = built_in",
        "built_in = ".len(),
        "local built_in: string",
    );
}

#[test]
fn hovers_known_function_signature() {
    let hover = hover_at("damage(player").expect("function should hover");

    assert_eq!(
        hover.contents,
        "function damage(target: Player, amount: number): Player"
    );
}

#[test]
fn hovers_annotated_function_docs_and_declaration_context() {
    let source = r#"type Player = { hp: number }

--- Applies damage.
---@param target Player affected player
---@return Player updated player
local function damage(target)
  return target
end

damage({ hp = 10 })
"#;
    let hover = hover_tua_source("src/main.tua", source, position_at(source, "damage({"))
        .expect("hover should succeed")
        .expect("annotated function should hover");

    assert_eq!(hover.contents, "function damage(target: Player): Player");
    assert_eq!(
        hover.documentation.as_deref(),
        Some("Applies damage.\naffected player\nupdated player")
    );
    assert!(hover.context.contains(&"kind: function".to_owned()));
    assert!(hover
        .context
        .iter()
        .any(|item| item.contains("declared: src/main.tua:")));
}

#[test]
fn hover_labels_deprecated_function_guidance() {
    let source = r#"--- Old entrypoint.
---@deprecated use newApi
local function oldApi() end

oldApi()
"#;
    let hover = hover_tua_source("src/main.tua", source, position_at(source, "oldApi()"))
        .expect("hover should succeed")
        .expect("deprecated function should hover");

    assert_eq!(
        hover.documentation.as_deref(),
        Some("**Deprecated.** use newApi\nOld entrypoint.")
    );
}

#[test]
fn hovers_lua_doc_overload_signatures() {
    let source = r#"---@overload fun(path: string): Image
---@overload fun(path: string, mode: "stream"): Source
local function load(path, mode)
  return nil
end

load("assets/player.png")
"#;
    let hover = hover_tua_source("src/main.tua", source, position_at(source, "load(\""))
        .expect("hover should succeed")
        .expect("overloaded function should hover");

    assert_eq!(
        hover.contents,
        "function load(path: string): Image\nfunction load(path: string, mode: \"stream\"): Source"
    );
}

#[test]
fn hovers_native_function_aliases_and_callback_fields() {
    let source = r#"type Handler = (value: number, ...string) -> (boolean, string)
type Hooks = {
  onValue: (value: number, ...string) -> (boolean, string),
}

local hooks: Hooks = nil
local selected = hooks.onValue
"#;
    let alias = hover_tua_source("src/main.tua", source, position_at(source, "Handler ="))
        .expect("hover should succeed")
        .expect("function alias should hover");
    let field = hover_tua_source("src/main.tua", source, position_at(source, "onValue\n"))
        .expect("hover should succeed")
        .expect("callback field should hover");

    assert_eq!(
        alias.contents,
        "type Handler = (value: number, ...string) -> (boolean, string)"
    );
    assert_eq!(
        field.contents,
        "(field) onValue: (value: number, ...string) -> (boolean, string)"
    );
}

#[test]
fn hovers_inferred_function_return_type_on_definition_and_call() {
    let source = r#"function test(a: boolean)
  return ""
end

test(false)
"#;
    let definition = hover_tua_source("src/main.tua", source, position_at(source, "test(a"))
        .expect("hover should succeed")
        .expect("function definition should hover");
    let call = hover_tua_source("src/main.tua", source, position_at(source, "test(false"))
        .expect("hover should succeed")
        .expect("function call should hover");

    assert_eq!(definition.contents, "function test(a: boolean): string");
    assert_eq!(call.contents, "function test(a: boolean): string");
}

#[test]
fn hovers_local_function_return_type_inferred_from_parameter() {
    let source = r#"local function incomplete(value: string)
  return value
end

incomplete("ok")
"#;
    let definition = hover_tua_source(
        "src/main.tua",
        source,
        position_at(source, "incomplete(value"),
    )
    .expect("hover should succeed")
    .expect("function definition should hover");
    let call = hover_tua_source(
        "src/main.tua",
        source,
        position_at(source, "incomplete(\"ok\")"),
    )
    .expect("hover should succeed")
    .expect("function call should hover");

    assert_eq!(
        definition.contents,
        "function incomplete(value: string): string"
    );
    assert_eq!(call.contents, "function incomplete(value: string): string");
}

#[test]
fn hovers_local_function_return_types_inferred_from_required_module_exports() {
    let temp = TempProject::new("required-module-return-hover");
    let source = r#"local Functions = require("functions")

local function macarrons()
  return Functions.makePlayer("lua interop")
end

local function make_function()
  return Functions.makePlayer
end

macarrons()
make_function()
"#;
    let functions = r#"type Player = {
  name: string,
  hp: number,
}

local function makePlayer(name: string): Player
  return {
    name = name,
    hp = 100,
  }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/functions.tua", functions);

    let main_path = temp.path.join("src/main.tua");
    let macarrons = hover_tua_source(&main_path, source, position_at(source, "macarrons()"))
        .expect("hover should succeed")
        .expect("module call return should hover");
    let make_function =
        hover_tua_source(&main_path, source, position_at(source, "make_function()"))
            .expect("hover should succeed")
            .expect("module function value return should hover");

    assert_eq!(macarrons.contents, "function macarrons(): Player");
    assert_eq!(
        make_function.contents,
        "function make_function(): (name: string) -> Player"
    );
}

#[test]
fn hovers_union_return_type_from_conditional_returns() {
    let source = r#"local function recursive(a: string)
  if a == 1 then
    return 1
  end

  if a == 2 then
    return "value"
  end
end

recursive("1")
"#;
    let hover = hover_tua_source(
        "src/main.tua",
        source,
        position_at(source, "recursive(\"1\")"),
    )
    .expect("hover should succeed")
    .expect("recursive function should hover");

    assert_eq!(
        hover.contents,
        "function recursive(a: string): number | string | nil"
    );
}

#[test]
fn hovers_union_return_type_from_recursive_conditional_returns() {
    let source = r#"local function recursive(a: string)
  if a == 1 then
    return 1
  end

  if a == 2 then
    return "value"
  end

  return recursive(a + 1)
end

recursive("1")
"#;
    let hover = hover_tua_source(
        "src/main.tua",
        source,
        position_at(source, "recursive(\"1\")"),
    )
    .expect("hover should succeed")
    .expect("recursive function should hover");

    assert_eq!(
        hover.contents,
        "function recursive(a: string): number | string"
    );
}

#[test]
fn hovers_known_table_field() {
    let hover = hover_at("hp = player").expect("table field should hover");

    assert_eq!(hover.contents, "(field) hp: number");
}

#[test]
fn hovers_anonymous_function_valued_table_fields() {
    let source = r#"
local lookupObject = {
  count = 0,
  y = function(self:any, amount:number)
    self.count = self.count + amount

    return self.count
  end,
}

local method = lookupObject.y
"#;
    let object_hover = hover_tua_source(
        "src/methods.tua",
        source,
        position_at(source, "lookupObject ="),
    )
    .expect("hover should not fail")
    .expect("lookupObject should hover");
    let field_hover = hover_tua_source("src/methods.tua", source, position_at(source, "y\n"))
        .expect("hover should not fail")
        .expect("function-valued field should hover");

    assert_eq!(
        object_hover.contents,
        "local lookupObject: { count: number, y: (self: any, amount: number) -> any }"
    );
    assert_eq!(
        field_hover.contents,
        "(field) y: (self: any, amount: number) -> any"
    );
}

#[test]
fn hovers_array_index_and_loop_variable_types() {
    let source = r#"
local numbers = { 1, 2, 3 }
local first = numbers[1]

for i = 1, #numbers do
  local loopIndex = i
  local indexed = numbers[i]
end

for _, value in ipairs(numbers) do
  local doubled = value + 1
end
"#;

    let first = hover_tua_source("src/loops.tua", source, position_at(source, "first ="))
        .expect("hover should not fail")
        .expect("indexed array local should hover");
    let indexed = hover_tua_source("src/loops.tua", source, position_at(source, "indexed ="))
        .expect("hover should not fail")
        .expect("loop-indexed array local should hover");
    let index = hover_tua_source("src/loops.tua", source, position_at(source, "loopIndex ="))
        .expect("hover should not fail")
        .expect("numeric loop variable should hover");
    let value = hover_tua_source("src/loops.tua", source, position_at(source, "doubled ="))
        .expect("hover should not fail")
        .expect("ipairs value should hover");

    assert_eq!(first.contents, "local first: number");
    assert_eq!(indexed.contents, "local indexed: number");
    assert_eq!(index.contents, "local loopIndex: number");
    assert_eq!(value.contents, "local doubled: number");
}

#[test]
fn hovers_record_index_and_pairs_variable_types() {
    let source = r#"
local player = { name = "Ada", score = 10 }
local score = player["score"]

for key, value in pairs(player) do
  local knownKey = key
  local seen = value
end
"#;

    let score = hover_tua_source(
        "src/pairs.tua",
        source,
        position_at(source, "score = player"),
    )
    .expect("hover should not fail")
    .expect("record index local should hover");
    let key = hover_tua_source("src/pairs.tua", source, position_at(source, "knownKey ="))
        .expect("hover should not fail")
        .expect("pairs key should hover");
    let value = hover_tua_source("src/pairs.tua", source, position_at(source, "seen ="))
        .expect("hover should not fail")
        .expect("pairs value should hover");

    assert_eq!(score.contents, "local score: number");
    assert_eq!(key.contents, "local knownKey: \"name\" | \"score\"");
    assert_eq!(value.contents, "local seen: string | number");
}

#[test]
fn hovers_self_index_class_style_metatable_methods() {
    let source = r#"
local Account = {}
Account.__index = Account

function Account:withdraw(amount)
end

local account = setmetatable({}, Account)
account:withdraw(10)
"#;
    let hover = hover_tua_source(
        "src/account.tua",
        source,
        position_at(source, "withdraw(10)"),
    )
    .expect("hover should not fail")
    .expect("self-index metatable method should hover");

    assert_eq!(
        hover.contents,
        "(field) withdraw: (any, amount: any) -> any"
    );
}

#[test]
fn hovers_native_class_fields_and_methods_with_exact_types() {
    let source = r#"
final class Counter {
  values: number[]

  init() {
    self.values = {}
  }

  add(amount: number): number {
    return amount
  }
}

local counter = Counter.new()
local first = counter.values[1]
local value = counter.add(1)
"#;

    assert_hover_in_source(source, "values[1]", 0, "(field) values: number[]");
    assert_hover_in_source(
        source,
        "add(1)",
        0,
        "(field) add: (self: Counter, amount: number) -> number",
    );
}

#[test]
fn hovers_self_fields_inside_native_classes() {
    let source = r#"
final class Counter {
  value: number

  read(): number {
    return self.value
  }
}
"#;

    assert_hover_in_source(source, "self.value", 5, "(field) value: number");
}

#[test]
fn hovers_self_index_constructor_instance_methods() {
    let source = r#"
local Account = {}
Account.__index = Account

function Account:new(balance)
  local instance = { balance = balance }
  return setmetatable(instance, self)
end

function Account:withdraw(amount)
end

local account = Account:new(100)
account:withdraw(10)
"#;
    let hover = hover_tua_source(
        "src/account.tua",
        source,
        position_at(source, "withdraw(10)"),
    )
    .expect("hover should not fail")
    .expect("constructor instance method should hover");

    assert_eq!(
        hover.contents,
        "(field) withdraw: (any, amount: any) -> any"
    );
}

#[test]
fn hovers_typed_colon_method_from_self_index_instance() {
    let source = r#"
type Account = {
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:withdraw(amount: number): number
  self.balance -= amount
  return self.balance
end

local account = setmetatable({ balance = 100 }, Account)
account:withdraw(10)
"#;
    let hover = hover_tua_source(
        "src/account.tua",
        source,
        position_at(source, "withdraw(10)"),
    )
    .expect("hover should not fail")
    .expect("typed colon method should hover");

    assert_eq!(
        hover.contents,
        "(field) withdraw: (self: Account, amount: number) -> number"
    );
}

#[test]
fn hovers_receiver_type_from_self_returning_method() {
    let source = r#"
type Account = {
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:deposit(amount: number): Self
  self.balance += amount
  return self
end

local account = setmetatable({ balance = 100 }, Account)
local updated = account:deposit(25)
"#;

    assert_hover_in_source(source, "updated = account", 0, "local updated: Account");
}

#[test]
fn hovers_metatable_index_chain_inherited_methods() {
    let source = r#"
local Base = {}
Base.__index = Base

function Base:describe(): string
  return "base"
end

local Mid = {}
Mid.__index = Mid
setmetatable(Mid, { __index = Base })

function Mid:level(): number
  return 2
end

local Leaf = {}
Leaf.__index = Leaf
setmetatable(Leaf, { __index = Mid })

local leaf = setmetatable({ name = "sprout" }, Leaf)
local description = leaf:describe()
local level = leaf:level()
"#;
    let description = hover_tua_source(
        "src/inheritance.tua",
        source,
        position_at(source, "description ="),
    )
    .expect("hover should not fail")
    .expect("inherited method return should hover");
    let level = hover_tua_source(
        "src/inheritance.tua",
        source,
        position_at(source, "level ="),
    )
    .expect("hover should not fail")
    .expect("inherited mid method return should hover");

    assert_eq!(description.contents, "local description: string");
    assert_eq!(level.contents, "local level: number");
}

#[test]
fn hovers_static_metamethod_operator_and_call_results() {
    let source = r#"
type Vector = {
  x: number,
  y: number,
  length: number,
}

local function vectorLen(value: Vector): number
  return value.length
end

local function vectorCall(value: Vector, scale: number): Vector
  return {
    x = value.x * scale,
    y = value.y * scale,
    length = value.length,
  }
end

local vectorMetatable = {
  __len = vectorLen,
  __call = vectorCall,
}

local vector = { x = 10, y = 20, length = 30 }
setmetatable(vector, vectorMetatable)

local vectorLength = #vector
local scaledVector = vector(2)
"#;
    let length_hover = hover_tua_source(
        "src/vector.tua",
        source,
        position_at(source, "vectorLength"),
    )
    .expect("hover should not fail")
    .expect("length result should hover");
    let scaled_hover = hover_tua_source(
        "src/vector.tua",
        source,
        position_at(source, "scaledVector"),
    )
    .expect("hover should not fail")
    .expect("call result should hover");

    assert_eq!(length_hover.contents, "local vectorLength: number");
    assert_eq!(scaled_hover.contents, "local scaledVector: Vector");
}

#[test]
fn hovers_metatable_index_function_fields_and_binary_operator_expressions() {
    let source = r#"
type Vector = {
  x: number,
  y: number,
}

local function addVector(left: Vector, right: Vector): Vector
  return {
    x = left.x + right.x,
    y = left.y + right.y,
  }
end

local vectorMetatable = {
  __add = addVector,
  __index = function(_, key)
    if key == "length" then
      return 0
    end

    return nil
  end,
}

local vectorA = { x = 10, y = 20 }
local vectorB = { x = 5, y = 15 }
setmetatable(vectorA, vectorMetatable)
setmetatable(vectorB, vectorMetatable)

local vectorLength = vectorA.length
local vectorSum = vectorA + vectorB
"#;
    let length_hover = hover_tua_source(
        "src/vector.tua",
        source,
        position_at(source, "length\nlocal vectorSum"),
    )
    .expect("hover should not fail")
    .expect("__index field should hover");
    let operator_hover =
        hover_tua_source("src/vector.tua", source, position_at(source, "+ vectorB"))
            .expect("hover should not fail")
            .expect("operator expression should hover");

    assert_eq!(length_hover.contents, "(field) length: number");
    assert_eq!(operator_hover.contents, "expression: Vector");
}

#[test]
fn unknown_symbols_do_not_hover() {
    let source = "local result = missing\n";
    let position = position_at(source, "missing");
    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("hover should not fail for unknown values");

    assert_eq!(hover, None);
}

#[test]
fn hovers_lua_standard_library_items() {
    let source = "local value = math.sin(1)\n";
    let position = position_at(source, "sin");
    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("hover should not fail")
        .expect("math.sin should hover");

    assert_eq!(hover.contents, "function math.sin(x: number): number");
}

#[test]
fn hovers_extended_lua_globals() {
    let source = "_ENV.raw = rawlen({})\nwarn(\"careful\")\n";
    let environment = hover_tua_source("src/main.tua", source, position_at(source, "_ENV"))
        .expect("hover should not fail")
        .expect("_ENV should hover");
    let rawlen = hover_tua_source("src/main.tua", source, position_at(source, "rawlen"))
        .expect("hover should not fail")
        .expect("rawlen should hover");
    let warn = hover_tua_source("src/main.tua", source, position_at(source, "warn"))
        .expect("hover should not fail")
        .expect("warn should hover");

    assert_eq!(environment.contents, "global environment table");
    assert_eq!(
        rawlen.contents,
        "function rawlen(v: string | table): number"
    );
    assert_eq!(warn.contents, "function warn(message: string, ...)");
}

#[test]
fn hovers_love_items() {
    let source = "love.graphics.print(\"hi\", 0, 0)\n";
    let position = position_at(source, "print");
    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("hover should not fail")
        .expect("love.graphics.print should hover");

    assert_eq!(
        hover.contents,
        "function love.graphics.print(text: string, x?: number, y?: number, r?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number)"
    );
    assert!(hover
        .documentation
        .as_deref()
        .unwrap_or_default()
        .contains("Draws text on screen"));
    assert!(hover
        .documentation
        .as_deref()
        .unwrap_or_default()
        .contains("[Open LOVE wiki](https://love2d.org/wiki/love.graphics.print)"));
}

#[test]
fn hovers_declared_love_callbacks_from_the_generated_catalog() {
    let source = r#"function love.keypressed(key: string)
end
"#;
    let hover = hover_tua_source("src/main.tua", source, position_at(source, "keypressed"))
        .expect("hover should not fail")
        .expect("declared LOVE callback should hover");

    assert_eq!(
        hover.contents,
        "function love.keypressed(key: KeyConstant, scancode: Scancode, isrepeat: boolean)"
    );
    assert!(hover
        .documentation
        .as_deref()
        .unwrap_or_default()
        .contains("Callback function triggered when a key is pressed"));
}

#[test]
fn hovers_love_asset_loader_items_and_inferred_locals() {
    let source = r#"
local player = image "assets/player.png"
local jump = sound "assets/jump.wav"
local theme = music "assets/theme.ogg"
local mainFont = font("assets/ui.ttf", 18)
local wave = shader "assets/wave.glsl"
local data = imageData "assets/player.png"
local target = canvas(320, 180)
local frame = quad(0, 0, 16, 16, 64, 64)
local batch = spriteBatch(player, 128)
local sparks = particles(player, 64)
"#;
    let image = hover_tua_source("src/main.tua", source, position_at(source, "image"))
        .expect("hover should not fail")
        .expect("image loader should hover");
    let player = hover_tua_source("src/main.tua", source, position_at(source, "player"))
        .expect("hover should not fail")
        .expect("inferred image local should hover");
    let jump = hover_tua_source("src/main.tua", source, position_at(source, "jump"))
        .expect("hover should not fail")
        .expect("inferred sound local should hover");
    let theme = hover_tua_source("src/main.tua", source, position_at(source, "theme"))
        .expect("hover should not fail")
        .expect("inferred music local should hover");
    let font = hover_tua_source("src/main.tua", source, position_at(source, "mainFont"))
        .expect("hover should not fail")
        .expect("inferred font local should hover");
    let shader = hover_tua_source("src/main.tua", source, position_at(source, "wave"))
        .expect("hover should not fail")
        .expect("inferred shader local should hover");
    let image_data = hover_tua_source("src/main.tua", source, position_at(source, "data"))
        .expect("hover should not fail")
        .expect("inferred imageData local should hover");
    let canvas = hover_tua_source("src/main.tua", source, position_at(source, "target"))
        .expect("hover should not fail")
        .expect("inferred canvas local should hover");
    let quad = hover_tua_source("src/main.tua", source, position_at(source, "frame"))
        .expect("hover should not fail")
        .expect("inferred quad local should hover");
    let batch = hover_tua_source("src/main.tua", source, position_at(source, "batch"))
        .expect("hover should not fail")
        .expect("inferred spriteBatch local should hover");
    let particles = hover_tua_source("src/main.tua", source, position_at(source, "sparks"))
        .expect("hover should not fail")
        .expect("inferred particles local should hover");

    assert_eq!(image.contents, "function image(path: string): Image");
    assert_eq!(player.contents, "local player: Image");
    assert_eq!(jump.contents, "local jump: Source");
    assert_eq!(theme.contents, "local theme: Source");
    assert_eq!(font.contents, "local mainFont: Font");
    assert_eq!(shader.contents, "local wave: Shader");
    assert_eq!(image_data.contents, "local data: ImageData");
    assert_eq!(canvas.contents, "local target: Canvas");
    assert_eq!(quad.contents, "local frame: Quad");
    assert_eq!(batch.contents, "local batch: SpriteBatch");
    assert_eq!(particles.contents, "local sparks: ParticleSystem");
}

#[test]
fn hovers_builtin_vec2_constructor_local_and_fields() {
    let source = r#"
local position = vec2(100, 200)
local x = position.x
local speed = position.length
local direction = position.normalized
"#;
    let constructor = hover_tua_source("src/main.tua", source, position_at(source, "vec2"))
        .expect("hover should not fail")
        .expect("vec2 constructor should hover");
    let position = hover_tua_source("src/main.tua", source, position_at(source, "position ="))
        .expect("hover should not fail")
        .expect("inferred Vec2 local should hover");
    let x = hover_tua_source("src/main.tua", source, position_at(source, "x\n"))
        .expect("hover should not fail")
        .expect("Vec2 field should hover");
    let speed = hover_tua_source("src/main.tua", source, position_at(source, "length"))
        .expect("hover should not fail")
        .expect("Vec2 length should hover");
    let direction = hover_tua_source("src/main.tua", source, position_at(source, "normalized"))
        .expect("hover should not fail")
        .expect("Vec2 normalized should hover");

    assert_eq!(
        constructor.contents,
        "function vec2(x: number, y: number): Vec2"
    );
    assert_eq!(position.contents, "local position: Vec2");
    assert_eq!(x.contents, "(field) x: number");
    assert_eq!(speed.contents, "(field) length: number");
    assert_eq!(direction.contents, "(field) normalized: Vec2");
}

#[test]
fn hovers_builtin_signal_constructor_local_and_methods() {
    let source = r#"
local events = signal()
events:on("player.hit", function(player, damage)
  print(player, damage)
end)
events:emit("player.hit", "ada", 4)
"#;
    let constructor = hover_tua_source("src/main.tua", source, position_at(source, "signal"))
        .expect("hover should not fail")
        .expect("signal constructor should hover");
    let events = hover_tua_source("src/main.tua", source, position_at(source, "events ="))
        .expect("hover should not fail")
        .expect("inferred SignalBus local should hover");
    let emit = hover_tua_source("src/main.tua", source, position_at(source, "emit"))
        .expect("hover should not fail")
        .expect("SignalBus emit method should hover");

    assert_eq!(constructor.contents, "function signal(): SignalBus");
    assert_eq!(events.contents, "local events: SignalBus");
    assert_eq!(
        emit.contents,
        "function SignalBus:emit(name: string, ...: any): nil"
    );
    assert!(emit
        .documentation
        .as_deref()
        .is_some_and(|documentation| documentation.contains("Emit a named signal")));
}

#[test]
fn hovers_typed_builtin_object_pool_constructor_items_and_methods() {
    let source = r#"
type Bullet = { active: boolean }
local bullets = objectPool({
  create = function(): Bullet
    return { active = false }
  end,
})
local bullet = bullets:acquire()
bullets:release(bullet)
"#;
    let constructor = hover_tua_source("src/main.tua", source, position_at(source, "objectPool"))
        .expect("hover should not fail")
        .expect("objectPool constructor should hover");
    let pool = hover_tua_source("src/main.tua", source, position_at(source, "bullets ="))
        .expect("hover should not fail")
        .expect("ObjectPool local should hover");
    let bullet = hover_tua_source("src/main.tua", source, position_at(source, "bullet ="))
        .expect("hover should not fail")
        .expect("acquired item should hover");
    let release = hover_tua_source("src/main.tua", source, position_at(source, "release"))
        .expect("hover should not fail")
        .expect("ObjectPool release should hover");

    assert_eq!(
        constructor.contents,
        "function objectPool(options: ObjectPoolOptions): ObjectPool"
    );
    assert_eq!(pool.contents, "local bullets: ObjectPool");
    assert_eq!(
        pool.documentation.as_deref(),
        Some("Acquires and releases values of type `Bullet`.")
    );
    assert_eq!(bullet.contents, "local bullet: Bullet");
    assert_eq!(
        release.contents,
        "function ObjectPool:release(object: Bullet): boolean"
    );
    assert!(release
        .documentation
        .as_deref()
        .is_some_and(|documentation| documentation.contains("duplicate releases")));
}

#[test]
fn hovers_typed_store_constructor_state_and_methods() {
    let source = r#"local game = store({ score = 0, paused = false })
local state = game:get()
game:update(function(state)
  state.score += 1
end)
"#;
    let constructor = hover_tua_source("src/store.tua", source, position_at(source, "store"))
        .expect("hover should not fail")
        .expect("store constructor should hover");
    let game = hover_tua_source("src/store.tua", source, position_at(source, "game ="))
        .expect("hover should not fail")
        .expect("Store local should hover");
    let state = hover_tua_source("src/store.tua", source, position_at(source, "state ="))
        .expect("hover should not fail")
        .expect("Store state should hover");
    let update = hover_tua_source("src/store.tua", source, position_at(source, "update"))
        .expect("hover should not fail")
        .expect("Store update should hover");

    assert_eq!(
        constructor.contents,
        "function store(initial: StoreState): Store"
    );
    assert_eq!(game.contents, "local game: Store");
    assert_eq!(
        game.documentation.as_deref(),
        Some("Holds runtime state of type `{ paused: boolean, score: number }`.")
    );
    assert_eq!(
        state.contents,
        "local state: { paused: boolean, score: number }"
    );
    assert_eq!(
        update.contents,
        "function Store:update(\n  mutator: (state: { paused: boolean, score: number }) -> nil\n): { paused: boolean, score: number }"
    );
}

#[test]
fn hovers_typed_builtin_state_machine_constructor_local_and_event() {
    let source = r#"local light = stateMachine({
  initial = "green",
  events = {
    { name = "warn", from = "green", to = "yellow" },
    { name = "panic", from = "yellow", to = "red" },
  },
})
light:warn()
"#;
    let constructor = hover_tua_source(
        "src/state_machine.tua",
        source,
        position_at(source, "stateMachine"),
    )
    .expect("hover should not fail")
    .expect("stateMachine constructor should hover");
    let machine = hover_tua_source(
        "src/state_machine.tua",
        source,
        position_at(source, "light ="),
    )
    .expect("hover should not fail")
    .expect("StateMachine local should hover");
    let warn = hover_tua_source(
        "src/state_machine.tua",
        source,
        position_at(source, "warn()"),
    )
    .expect("hover should not fail")
    .expect("derived event should hover");

    assert_eq!(
        constructor.contents,
        "function stateMachine(options: StateMachineOptions): StateMachine"
    );
    assert_eq!(machine.contents, "local light: StateMachine");
    assert_eq!(
        machine.documentation.as_deref(),
        Some("States: `\"green\" | \"red\" | \"yellow\"`. Events: `\"panic\" | \"warn\"`.")
    );
    assert_eq!(
        warn.contents,
        "function StateMachine:warn(...: any): boolean"
    );
}

#[test]
fn hovers_typed_baton_input_constructor_local_and_methods() {
    let source = r#"local controls = input({
  controls = {
    left = { "key:left" },
    right = { "key:right" },
    up = { "key:up" },
    down = { "key:down" },
    confirm = { "key:return" },
  },
  pairs = { move = { "left", "right", "up", "down" } },
})
controls:get("move")
"#;
    let constructor = hover_tua_source("src/input.tua", source, position_at(source, "input"))
        .expect("hover should not fail")
        .expect("input constructor should hover");
    let controls = hover_tua_source("src/input.tua", source, position_at(source, "controls ="))
        .expect("hover should not fail")
        .expect("Input local should hover");
    let get = hover_tua_source("src/input.tua", source, position_at(source, "get(\"move"))
        .expect("hover should not fail")
        .expect("Input:get should hover");

    assert_eq!(
        constructor.contents,
        "function input(config: InputConfig): Input"
    );
    assert_eq!(controls.contents, "local controls: Input");
    assert_eq!(
        controls.documentation.as_deref(),
        Some("Controls: `confirm, down, left, right, up`. Pairs: `move`.")
    );
    assert!(get.contents.contains("name: \"move\""));
    assert!(get.contents.ends_with(": number, number"));
}

#[test]
fn hovers_love_object_methods_from_inferred_asset_loader_returns() {
    let source = r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local player = loadPlayerImage()

player:setFilter()
"#;
    let hover = hover_tua_source("src/main.tua", source, position_at(source, "setFilter"))
        .expect("hover should not fail")
        .expect("Image method should hover");

    assert_eq!(
        hover.contents,
        "function Image:setFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)"
    );
    assert!(hover
        .documentation
        .as_deref()
        .unwrap_or_default()
        .contains("filter mode"));
    assert!(hover
        .documentation
        .as_deref()
        .unwrap_or_default()
        .contains("[Open LOVE wiki](https://love2d.org/wiki/Image:setFilter)"));
}

#[test]
fn hovers_love_object_methods_from_direct_constructor_locals() {
    let source = r#"
local player = love.graphics.newImage("assets/player.png")

player:setFilter()
"#;
    let hover = hover_tua_source("src/main.tua", source, position_at(source, "setFilter"))
        .expect("hover should not fail")
        .expect("Image method should hover");

    assert_eq!(
        hover.contents,
        "function Image:setFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)"
    );
}

#[test]
fn hovers_every_generated_love_object_method() {
    for (type_name, methods) in love_object_methods_by_type() {
        let source = love_object_method_source(type_name, &methods, ":");
        let parsed = parse_tua_source("src/main.tua", source).unwrap_or_else(|diagnostics| {
            panic!("{type_name} generated LOVE method hover source should parse: {diagnostics:#?}")
        });
        let environment =
            crate::types::build_type_environment(&parsed).unwrap_or_else(|diagnostics| {
                panic!(
                    "{type_name} generated LOVE method hover source should type: {diagnostics:#?}"
                )
            });

        for method in methods {
            let hover = hover_parsed_source_with_environment(
                &parsed,
                love_method_position(&parsed.source, ":", method),
                &environment,
            )
            .unwrap_or_else(|| panic!("{type_name}:{method} should hover"));

            assert!(
                hover
                    .contents
                    .starts_with(&format!("function {type_name}:{method}(")),
                "{type_name}:{method} should use generated method hover, got {:?}",
                hover.contents
            );
        }
    }
}

#[test]
fn hovers_love_object_method_return_types_from_inferred_asset_loader_returns() {
    let source = r#"
local player = image "assets/hero.png"
local w = player:getWidth()
local a, b = player.getDimensions()
local minFilter, magFilter, anisotropy = player:getFilter()
local copied = anisotropy
"#;
    let width = hover_tua_source("src/main.tua", source, position_at(source, "w ="))
        .expect("hover should not fail")
        .expect("width local should hover");
    let first_dimension = hover_tua_source("src/main.tua", source, position_at(source, "a, b"))
        .expect("hover should not fail")
        .expect("first dimension should hover");
    let second_dimension = hover_tua_source("src/main.tua", source, position_at(source, "b ="))
        .expect("hover should not fail")
        .expect("second dimension should hover");
    let copied = hover_tua_source("src/main.tua", source, position_at(source, "copied ="))
        .expect("hover should not fail")
        .expect("copied return should hover");

    assert_eq!(width.contents, "local w: number");
    assert_eq!(first_dimension.contents, "local a: number");
    assert_eq!(second_dimension.contents, "local b: number");
    assert_eq!(copied.contents, "local copied: number");
}

#[test]
fn hovers_love_method_return_type_through_local_function_body() {
    let source = r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local function loadJumpSound()
  return sound "assets/jump.wav"
end

local player = loadPlayerImage()

local a, b = player.getDimensions()

local function azul()
  return player.getDimensions()
end

local c = azul()
local z = c + b
local literal = 1 + 3
local label = "width: " .. z

return {
  loadPlayerImage = loadPlayerImage,
  loadJumpSound = loadJumpSound,
}
"#;
    let function = hover_tua_source("src/main.tua", source, position_at(source, "azul()"))
        .expect("hover should not fail")
        .expect("azul should hover");
    let local = hover_tua_source("src/main.tua", source, position_at(source, "c ="))
        .expect("hover should not fail")
        .expect("c should hover");
    let sum = hover_tua_source("src/main.tua", source, position_at(source, "z ="))
        .expect("hover should not fail")
        .expect("z should hover");
    let literal = hover_tua_source("src/main.tua", source, position_at(source, "literal ="))
        .expect("hover should not fail")
        .expect("literal should hover");
    let label = hover_tua_source("src/main.tua", source, position_at(source, "label ="))
        .expect("hover should not fail")
        .expect("label should hover");

    assert_eq!(function.contents, "function azul(): number, number");
    assert_eq!(local.contents, "local c: number");
    assert_eq!(sum.contents, "local z: number");
    assert_eq!(literal.contents, "local literal: number");
    assert_eq!(label.contents, "local label: string");
}

#[test]
fn local_symbols_shadow_lua_global_hover() {
    let source = "local math: any = {}\nlocal value = math.sin\n";
    let position = position_at(source, "sin");
    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("hover should not fail for shadowed values");

    assert_eq!(hover, None);
}

#[test]
fn hovers_required_module_literal_binding_and_tua_exports() {
    let temp = TempProject::new("required-module-hover");
    let source = r#"local Player = require("player")

local player = Player.makePlayer("Ada")
local hp = Player.defaultHp
"#;
    let player = r#"type Player = { name: string, hp: number }

local function makePlayer(name: string): Player
  return { name = name, hp = 10 }
end

return {
  makePlayer = makePlayer,
  defaultHp = 10,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/player.tua", player);

    let main_path = temp.path.join("src/main.tua");
    let literal = hover_tua_source(&main_path, source, position_at(source, "player\")"))
        .expect("hover should not fail")
        .expect("require literal should hover");
    let binding = hover_tua_source(&main_path, source, position_at(source, "Player.makePlayer"))
        .expect("hover should not fail")
        .expect("module binding should hover");
    let function = hover_tua_source(&main_path, source, position_at(source, "makePlayer("))
        .expect("hover should not fail")
        .expect("module function should hover");
    let field = hover_tua_source(&main_path, source, position_at(source, "defaultHp"))
        .expect("hover should not fail")
        .expect("module field should hover");

    assert!(literal.contents.contains("(module) player ->"));
    assert!(literal.contents.contains("player.tua"));
    assert!(binding
        .contents
        .contains("(module) Player = require(\"player\") ->"));
    assert_eq!(
        function.contents,
        "function Player.makePlayer(name: string): Player"
    );
    assert_eq!(field.contents, "(field) Player.defaultHp: number");
}

#[test]
fn hovers_imported_module_literal_binding_and_tua_exports() {
    let temp = TempProject::new("imported-module-hover");
    let source = r#"local Player = import("./player")

local player = Player.makePlayer("Ada")
"#;
    let player = r#"type Player = { name: string }

local function makePlayer(name: string): Player
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/player.tua", player);

    let main_path = temp.path.join("src/main.tua");
    let literal = hover_tua_source(&main_path, source, position_at(source, "player\")"))
        .expect("hover should not fail")
        .expect("import literal should hover");
    let binding = hover_tua_source(&main_path, source, position_at(source, "Player.makePlayer"))
        .expect("hover should not fail")
        .expect("module binding should hover");
    let function = hover_tua_source(&main_path, source, position_at(source, "makePlayer("))
        .expect("hover should not fail")
        .expect("module function should hover");

    assert!(literal.contents.contains("(module) ./player ->"));
    assert!(literal.contents.contains("player.tua"));
    assert!(binding
        .contents
        .contains("(module) Player = import(\"./player\") ->"));
    assert_eq!(
        function.contents,
        "function Player.makePlayer(name: string): Player"
    );
}

#[test]
fn hovers_required_lua_module_exports() {
    let temp = TempProject::new("required-lua-module-hover");
    let source = r#"local Legacy = require("legacy")

local label = Legacy.describe("hp", 10)
"#;
    let legacy = r#"return {
  describe = function(label, value)
    return label
  end,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/legacy.lua", legacy);

    let main_path = temp.path.join("src/main.tua");
    let hover = hover_tua_source(&main_path, source, position_at(source, "describe("))
        .expect("hover should not fail")
        .expect("Lua module function should hover");

    assert_eq!(
        hover.contents,
        "function Legacy.describe(label: any, value: any): any"
    );
}

#[test]
fn hovers_annotated_lua_module_export_docs() {
    let temp = TempProject::new("annotated-required-lua-module-hover");
    let source = r#"local Audio = require("audio")
local ok = Audio.play("theme")
"#;
    let audio = r#"---@param source string audio source
---@return boolean success flag
local function play(source)
  return true
end

return {
  play = play,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/audio.lua", audio);
    let main_path = temp.path.join("src/main.tua");

    let hover = hover_tua_source(&main_path, source, position_at(source, "play("))
        .expect("hover should not fail")
        .expect("annotated Lua module function should hover");

    assert_eq!(
        hover.contents,
        "function Audio.play(source: string): boolean"
    );
    assert_eq!(
        hover.documentation.as_deref(),
        Some("audio source\nsuccess flag")
    );
    assert!(hover.context.iter().any(|item| item.contains("audio.lua")));
    assert!(hover.context.contains(&"kind: function".to_owned()));
}

#[test]
fn hovers_module_export_plain_doc_comment_and_origin_context() {
    let temp = TempProject::new("module-export-doc-hover");
    let source = r#"local Audio = require("audio")
local ok = Audio.play("theme")
"#;
    let audio = r#"--- Plays a source.
---@param source string audio source
---@return boolean success flag
local function play(source)
  return true
end

return {
  play = play,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/audio.lua", audio);

    let hover = hover_tua_source(
        temp.path.join("src/main.tua"),
        source,
        position_at(source, "play("),
    )
    .expect("hover should not fail")
    .expect("module export should hover");

    assert_eq!(
        hover.contents,
        "function Audio.play(source: string): boolean"
    );
    assert_eq!(
        hover.documentation.as_deref(),
        Some("Plays a source.\naudio source\nsuccess flag")
    );
    assert!(hover.context.contains(&"kind: function".to_owned()));
    assert!(hover.context.iter().any(|item| item.contains("module:")));
    assert!(hover.context.iter().any(|item| item.contains("declared:")));
}

#[test]
fn unresolved_required_module_fields_do_not_hover() {
    let temp = TempProject::new("missing-required-module-hover");
    let source = r#"local Missing = require("missing")

local value = Missing.nope
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);

    let hover = hover_tua_source(
        temp.path.join("src/main.tua"),
        source,
        position_at(source, "nope"),
    )
    .expect("hover should not fail");

    assert_eq!(hover, None);
}

fn hover_at(needle: &str) -> Option<Hover> {
    let position = position_at(SOURCE, needle);

    hover_tua_source("src/main.tua", SOURCE, position).expect("source should parse and type")
}

fn assert_hover_in_source(source: &str, needle: &str, offset: usize, expected: &str) {
    let mut position = position_at(source, needle);

    position.column += offset;

    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("source should parse")
        .expect("hover should resolve");

    assert_eq!(hover.contents, expected);
}

fn love_object_methods_by_type() -> std::collections::BTreeMap<&'static str, Vec<&'static str>> {
    let mut cases: std::collections::BTreeMap<&'static str, Vec<&'static str>> =
        std::collections::BTreeMap::new();

    for case in crate::love_api_signatures::object_method_cases() {
        let methods = cases.entry(case.type_name).or_default();

        if !methods.contains(&case.method_name) {
            methods.push(case.method_name);
        }
    }

    cases
}

fn love_object_method_source(type_name: &str, methods: &[&str], separator: &str) -> String {
    let mut source = format!("local receiver: {type_name} | nil = nil\n");

    for method in methods {
        source.push_str(&format!("receiver{separator}{method}()\n"));
    }

    source
}

fn love_method_position(source: &str, separator: &str, method: &str) -> SourcePosition {
    let needle = format!("receiver{separator}{method}");
    let mut position = position_at(source, &needle);

    position.column += format!("receiver{separator}").chars().count();
    position
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
        let path =
            std::env::temp_dir().join(format!("tua-core-hover-{name}-{}-{unique}", process::id()));

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
