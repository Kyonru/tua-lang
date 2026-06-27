use super::*;

const SOURCE: &str = r#"
type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 10 }
end

local function damage(target: Player, amount: number): Player
  target.hp -= amount
  return target
end

local player = makePlayer()
local hp = player.hp
local total = 10
local named: Player = makePlayer()
local unknown = missing()

damage(player, 10)
"#;

#[test]
fn hints_unannotated_local_types_from_known_expressions() {
    let hints = inlay_hints();
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(type_hints, vec![": Player", ": number", ": number"]);
    assert_hint_position(&hints, ": Player", 13, "local player".len() + 1);
    assert_hint_position(&hints, ": number", 14, "local hp".len() + 1);
    assert_hint_position(&hints, ": number", 15, "local total".len() + 1);
}

#[test]
fn hints_anonymous_function_valued_table_fields() {
    let source = r#"
local lookupObject = {
  count = 0,
  y = function(self:any, amount:number)
    self.count = self.count + amount

    return self.count
  end,
}
"#;
    let hints = inlay_hints_tua_source("src/methods.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        type_hints,
        vec![": { count: number, y: function(any, number): any }"]
    );
}

#[test]
fn hints_loop_variable_types_from_numeric_ipairs_and_pairs_loops() {
    let source = r#"
local numbers = { 1, 2, 3 }
local player = { name = "Ada", score = 10 }

for i = 1, #numbers do
  local indexed = numbers[i]
end

for _, value in ipairs(numbers) do
  local doubled = value + 1
end

for key, value in pairs(player) do
  local seen = value
end
"#;
    let hints = inlay_hints_tua_source("src/loops.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert!(type_hints.contains(&": number[]"));
    assert!(type_hints.contains(&": { name: string, score: number }"));
    assert!(type_hints.contains(&": number"));
    assert!(type_hints.contains(&": \"name\" | \"score\""));
    assert!(type_hints.contains(&": string | number"));
}

#[test]
fn hints_loop_variable_types_from_iterator_metamethods() {
    let source = r#"
---@param value any
---@return ["name" | "score", number]
local function iterPairs(value)
end

---@param value any
---@return [number, string]
local function iterIpairs(value)
end

local typedMap = {}
setmetatable(typedMap, { __pairs = iterPairs })

local typedArray = {}
setmetatable(typedArray, { __ipairs = iterIpairs })

for key, value in pairs(typedMap) do
  local seenKey = key
  local seenValue = value
end

for index, value in ipairs(typedArray) do
  local seenIndex = index
  local seenItem = value
end
"#;
    let hints = inlay_hints_tua_source("src/iterators.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();
    assert!(type_hints.contains(&": \"name\" | \"score\""));
    assert!(type_hints.contains(&": number"));
    assert!(type_hints.contains(&": string"));
}

#[test]
fn skips_annotated_unknown_and_any_local_type_hints() {
    let hints = inlay_hints();

    assert!(!hints_at_line(&hints, 16).any(|hint| hint.kind == InlayHintKind::Type));
    assert!(!hints_at_line(&hints, 17).any(|hint| hint.kind == InlayHintKind::Type));
}

#[test]
fn hints_parameter_names_for_known_calls() {
    let hints = inlay_hints();
    let parameter_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Parameter)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(parameter_hints, vec!["target:", "amount:"]);
    assert_hint_position(&hints, "target:", 19, "damage(".len() + 1);
    assert_hint_position(&hints, "amount:", 19, "damage(player, ".len() + 1);
}

#[test]
fn hints_colon_method_parameters_from_self_index_prototype() {
    let source = r#"
type Account = {
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:deposit(amount: number): number
  self.balance = self.balance + amount

  return self.balance
end

local account = setmetatable({ balance = 50 }, Account)
account:deposit(25)
"#;
    let hints = inlay_hints_tua_source("src/account.tua", source).expect("source should parse");
    let parameter_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Parameter)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert!(parameter_hints.contains(&"amount:"));
    assert!(!parameter_hints.contains(&"self:"));
    assert_hint_position(&hints, "amount:", 16, "account:deposit(".len() + 1);
}

#[test]
fn hints_love_asset_loader_local_types() {
    let source = r#"
local player = image "assets/player.png"
local jump = sound "assets/jump.wav"
"#;
    let hints = inlay_hints_tua_source("src/assets.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(type_hints, vec![": Image", ": Source"]);
}

#[test]
fn hints_direct_love_constructor_local_types() {
    let source = r#"
local player = love.graphics.newImage("assets/player.png")
local canvas = love.graphics.newCanvas()
local font = love.graphics.newFont("assets/sniglet.otf", 16)
local data = love.image.newImageData(16, 16)
local world = love.physics.newWorld()
local body = love.physics.newBody(world, 0, 0)
"#;
    let hints = inlay_hints_tua_source("src/assets.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        type_hints,
        vec![
            ": Image",
            ": Canvas",
            ": Font",
            ": ImageData",
            ": World",
            ": Body"
        ]
    );
}

#[test]
fn hints_love_object_method_return_types_from_inferred_asset_loader_returns() {
    let source = r#"
local player = image "assets/hero.png"
local w = player:getWidth()
local a, b = player.getDimensions()
local minFilter, magFilter, anisotropy = player:getFilter()
"#;
    let hints = inlay_hints_tua_source("src/assets.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        type_hints,
        vec![
            ": Image",
            ": number",
            ": number",
            ": number",
            ": FilterMode",
            ": FilterMode",
            ": number"
        ]
    );
}

#[test]
fn hints_love_method_return_type_through_local_function_body() {
    let source = r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local player = loadPlayerImage()

local function azul()
  return player.getDimensions()
end

local c = azul()
local z = c + 1
local literal = 1 + 3
local label = "width: " .. z
"#;
    let hints = inlay_hints_tua_source("src/assets.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        type_hints,
        vec![": Image", ": number", ": number", ": number", ": string"]
    );
}

fn inlay_hints() -> Vec<InlayHint> {
    inlay_hints_tua_source("src/player.tua", SOURCE).expect("source should parse")
}

fn hints_at_line(hints: &[InlayHint], line: usize) -> impl Iterator<Item = &InlayHint> {
    hints.iter().filter(move |hint| hint.position.line == line)
}

fn assert_hint_position(hints: &[InlayHint], label: &str, line: usize, column: usize) {
    let hint = hints
        .iter()
        .find(|hint| hint.label == label && hint.position.line == line)
        .unwrap_or_else(|| panic!("expected hint `{label}` on line {line}"));

    assert_eq!(hint.position.column, column);
}
