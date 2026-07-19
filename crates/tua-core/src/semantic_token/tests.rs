use super::*;
use crate::source_text::{byte_for_column, line_at};

const SOURCE: &str = r#"
type Player = {
  hp: number,
  name: string,
  alive?: boolean,
}

local function makePlayer(): Player
  return { hp = 10, alive = true }
end

local function damage(player: Player, amount: number): Player
  player.hp -= amount
  return player
end

local hero = makePlayer()
damage(hero, 10)
missing(hero)
require("player")
"#;

#[test]
fn classifies_aliases_functions_variables_parameters_and_properties() {
    let tokens = semantic_tokens();

    assert_token(&tokens, "Player", SemanticTokenKind::TypeAlias, true);
    assert_token(&tokens, "makePlayer", SemanticTokenKind::Function, true);
    assert_token(&tokens, "damage", SemanticTokenKind::Function, true);
    assert_token(&tokens, "hero", SemanticTokenKind::Variable, true);
    assert_token(&tokens, "player", SemanticTokenKind::Parameter, true);
    assert_token(&tokens, "hp", SemanticTokenKind::Property, true);

    assert_token_at_line(&tokens, "Player", 8, SemanticTokenKind::TypeAlias, false);
    assert_token_at_line(
        &tokens,
        "makePlayer",
        17,
        SemanticTokenKind::Function,
        false,
    );
    assert_token_at_line(&tokens, "damage", 18, SemanticTokenKind::Function, false);
    assert_token_at_line(&tokens, "hero", 18, SemanticTokenKind::Variable, false);
    assert_token_at_line(&tokens, "hp", 13, SemanticTokenKind::Property, false);
}

#[test]
fn classifies_object_pool_as_a_builtin_function() {
    let source = r#"local pool = objectPool({
  create = function()
    return {}
  end,
})
"#;
    let tokens = semantic_tokens_tua_source("src/pool.tua", source)
        .expect("object pool source should tokenize");

    assert_token_in_source(
        &tokens,
        source,
        "objectPool",
        SemanticTokenKind::Function,
        false,
    );
    assert_token_modifier(
        &tokens,
        source,
        "objectPool",
        SemanticTokenKind::Function,
        SemanticTokenModifier::DefaultLibrary,
    );
}

#[test]
fn classifies_store_as_a_builtin_function() {
    let source = "local game = store({ score = 0 })\n";
    let tokens =
        semantic_tokens_tua_source("src/store.tua", source).expect("store source should tokenize");

    assert_token_in_source(&tokens, source, "store", SemanticTokenKind::Function, false);
    assert_token_modifier(
        &tokens,
        source,
        "store",
        SemanticTokenKind::Function,
        SemanticTokenModifier::DefaultLibrary,
    );
}

#[test]
fn classifies_each_member_in_an_access_chain() {
    let source = r#"local object = {}
local value = object.branch.leaf.position.x
object.branch.leaf:refresh()
"#;
    let tokens = semantic_tokens_tua_source("src/member-chain.tua", source)
        .expect("member access chain should tokenize");

    assert_token_at_source_line(
        &tokens,
        source,
        "object",
        2,
        SemanticTokenKind::Variable,
        false,
    );
    for property in ["branch", "leaf", "position", "x"] {
        assert_token_at_source_line(
            &tokens,
            source,
            property,
            2,
            SemanticTokenKind::Property,
            false,
        );
    }
    assert_token_at_source_line(
        &tokens,
        source,
        "branch",
        3,
        SemanticTokenKind::Property,
        false,
    );
    assert_token_at_source_line(
        &tokens,
        source,
        "leaf",
        3,
        SemanticTokenKind::Property,
        false,
    );
    assert_token_at_source_line(
        &tokens,
        source,
        "refresh",
        3,
        SemanticTokenKind::Method,
        false,
    );
}

#[test]
fn classifies_primitive_types_and_unresolved_identifiers() {
    let tokens = semantic_tokens();

    assert_token(&tokens, "number", SemanticTokenKind::BuiltinType, false);
    assert_token(&tokens, "string", SemanticTokenKind::BuiltinType, false);
    assert_token(&tokens, "require", SemanticTokenKind::Function, false);
    assert_token(&tokens, "player", SemanticTokenKind::Module, false);
    assert_token_at_line(
        &tokens,
        "missing",
        19,
        SemanticTokenKind::UnresolvedReference,
        false,
    );
    assert!(!tokens
        .iter()
        .any(|token| token_text_from_source(SOURCE, token) == "require"
            && token.kind == SemanticTokenKind::UnresolvedReference));
}

#[test]
fn classifies_annotations_globals_modules_methods_exports_and_modifiers() {
    let source = r#"---@class Player playable character
---@field hp number current health
---@alias PlayerId string|number stable id
---@param player Player target player
---@deprecated
local function LEGACY(player: Player)
  print(player.hp)
  player:heal()
end

function Player:heal()
end

local MAX_HP = 10

require("systems.player")
unknownGlobal()

return {
  makePlayer = LEGACY,
}
"#;
    let tokens = semantic_tokens_tua_source("src/player.tua", source).expect("source should parse");

    assert_token_in_source(
        &tokens,
        source,
        "@class",
        SemanticTokenKind::AnnotationTag,
        false,
    );
    assert_token_in_source(
        &tokens,
        source,
        "@field",
        SemanticTokenKind::AnnotationTag,
        false,
    );
    assert_token_in_source(
        &tokens,
        source,
        "@alias",
        SemanticTokenKind::AnnotationTag,
        false,
    );
    assert_token_in_source(
        &tokens,
        source,
        "@param",
        SemanticTokenKind::AnnotationTag,
        false,
    );
    assert_token_in_source(
        &tokens,
        source,
        "@deprecated",
        SemanticTokenKind::AnnotationTag,
        false,
    );
    assert_token_in_source(
        &tokens,
        source,
        "Player",
        SemanticTokenKind::TypeAlias,
        true,
    );
    assert_token_in_source(
        &tokens,
        source,
        "PlayerId",
        SemanticTokenKind::TypeAlias,
        true,
    );
    assert_token_in_source(&tokens, source, "hp", SemanticTokenKind::Property, true);
    assert_token_in_source(
        &tokens,
        source,
        "player",
        SemanticTokenKind::Parameter,
        true,
    );
    assert_token_in_source(&tokens, source, "print", SemanticTokenKind::Function, false);
    assert_token_in_source(
        &tokens,
        source,
        "systems.player",
        SemanticTokenKind::Module,
        false,
    );
    assert_token_in_source(&tokens, source, "heal", SemanticTokenKind::Method, true);
    assert_token_in_source(&tokens, source, "heal", SemanticTokenKind::Method, false);
    assert_token_in_source(
        &tokens,
        source,
        "makePlayer",
        SemanticTokenKind::Property,
        true,
    );
    assert_token_in_source(
        &tokens,
        source,
        "unknownGlobal",
        SemanticTokenKind::UnresolvedReference,
        false,
    );
    assert_token_modifier(
        &tokens,
        source,
        "LEGACY",
        SemanticTokenKind::Function,
        SemanticTokenModifier::Readonly,
    );
    assert_token_modifier(
        &tokens,
        source,
        "LEGACY",
        SemanticTokenKind::Function,
        SemanticTokenModifier::Deprecated,
    );
    assert_token_modifier(
        &tokens,
        source,
        "MAX_HP",
        SemanticTokenKind::Variable,
        SemanticTokenModifier::Readonly,
    );
    assert_token_modifier(
        &tokens,
        source,
        "@deprecated",
        SemanticTokenKind::AnnotationTag,
        SemanticTokenModifier::Deprecated,
    );
}

#[test]
fn classifies_tua_classes_enums_domains_events_labels_and_modifications() {
    let source = r#"enum PlayerState
  "idle"
  "running"
end

open class Entity {
  x: number = 0

  init(name: string) {
    self.name = name
  }
}

final class Player extends Entity {
  final update(dt: number) {
    self.x += dt
  }
}

local events = signal()
events:on("player.hit", function(player: Player, damage: number)
  player.x -= damage
end)
events:emit("player.hit", Player.new())

local state: PlayerState = "idle"
local texture: Image = image("assets/player.png")
local sheet = spriteSheet({ image = "assets/player.png" })
local pool = objectPool({ create = function(): Player return Player.new() end })
local world = ecs.world()
love.graphics.draw(texture)
local PlayerModule = import("game.player")
"#;
    let parsed = parse_tua_source("src/features.tua", source).expect("feature source should parse");
    let tokens = semantic_tokens_parsed_source(&parsed).expect("feature source should tokenize");

    assert_token_in_source(
        &tokens,
        source,
        "PlayerState",
        SemanticTokenKind::Enum,
        true,
    );
    assert_token_in_source(&tokens, source, "idle", SemanticTokenKind::EnumMember, true);
    assert_token_modifier(
        &tokens,
        source,
        "idle",
        SemanticTokenKind::EnumMember,
        SemanticTokenModifier::Readonly,
    );
    assert_token_in_source(&tokens, source, "Entity", SemanticTokenKind::Class, true);
    assert_token_at_source_line(
        &tokens,
        source,
        "Entity",
        14,
        SemanticTokenKind::Class,
        false,
    );
    assert_token_in_source(&tokens, source, "Player", SemanticTokenKind::Class, true);
    assert_token_in_source(&tokens, source, "init", SemanticTokenKind::Method, true);
    assert_token_in_source(&tokens, source, "update", SemanticTokenKind::Method, true);
    assert_token_at_source_line(
        &tokens,
        source,
        "name",
        9,
        SemanticTokenKind::Parameter,
        true,
    );
    assert_token_at_source_line(&tokens, source, "x", 16, SemanticTokenKind::Property, false);
    assert_token_modifier_at_line(
        &tokens,
        source,
        "x",
        16,
        SemanticTokenKind::Property,
        SemanticTokenModifier::Modification,
    );
    assert_token_at_source_line(
        &tokens,
        source,
        "signal",
        20,
        SemanticTokenKind::Function,
        false,
    );
    assert_token_at_source_line(
        &tokens,
        source,
        "player.hit",
        21,
        SemanticTokenKind::Event,
        true,
    );
    assert_token_at_source_line(
        &tokens,
        source,
        "player.hit",
        24,
        SemanticTokenKind::Event,
        false,
    );
    assert_token_at_source_line(
        &tokens,
        source,
        "Image",
        27,
        SemanticTokenKind::BuiltinType,
        false,
    );
    assert_token_modifier_at_line(
        &tokens,
        source,
        "Image",
        27,
        SemanticTokenKind::BuiltinType,
        SemanticTokenModifier::DefaultLibrary,
    );
    for (line, name) in [(27, "image"), (28, "spriteSheet"), (29, "objectPool")] {
        assert_token_at_source_line(
            &tokens,
            source,
            name,
            line,
            SemanticTokenKind::Function,
            false,
        );
        assert_token_modifier_at_line(
            &tokens,
            source,
            name,
            line,
            SemanticTokenKind::Function,
            SemanticTokenModifier::DefaultLibrary,
        );
    }
    assert_token_at_source_line(
        &tokens,
        source,
        "ecs",
        30,
        SemanticTokenKind::Namespace,
        false,
    );
    assert_token_at_source_line(
        &tokens,
        source,
        "world",
        30,
        SemanticTokenKind::Method,
        false,
    );
    assert_token_at_source_line(
        &tokens,
        source,
        "love",
        31,
        SemanticTokenKind::Namespace,
        false,
    );
    assert_token_at_source_line(
        &tokens,
        source,
        "graphics",
        31,
        SemanticTokenKind::Property,
        false,
    );
    assert_token_at_source_line(
        &tokens,
        source,
        "draw",
        31,
        SemanticTokenKind::Method,
        false,
    );
    for (line, name, kind) in [
        (30, "ecs", SemanticTokenKind::Namespace),
        (30, "world", SemanticTokenKind::Method),
        (31, "love", SemanticTokenKind::Namespace),
        (31, "graphics", SemanticTokenKind::Property),
        (31, "draw", SemanticTokenKind::Method),
    ] {
        assert_token_modifier_at_line(
            &tokens,
            source,
            name,
            line,
            kind,
            SemanticTokenModifier::DefaultLibrary,
        );
    }
    assert_token_at_source_line(
        &tokens,
        source,
        "game.player",
        32,
        SemanticTokenKind::Module,
        false,
    );
}

#[test]
fn falls_back_to_lossy_tokens_when_source_does_not_parse() {
    let source = r#"type Player = {
  hp: number,
}

local player: Player =
function player.damage(amount: number)
  player.
events:on("player.hit",
::retry::
goto retry
"#;
    let tokens = semantic_tokens_tua_source("src/player.tua", source)
        .expect("lossy semantic tokens should not fail on parse errors");

    assert_token_in_source(
        &tokens,
        source,
        "Player",
        SemanticTokenKind::TypeAlias,
        true,
    );
    assert_token_in_source(&tokens, source, "hp", SemanticTokenKind::Property, false);
    assert_token_in_source(
        &tokens,
        source,
        "number",
        SemanticTokenKind::BuiltinType,
        false,
    );
    assert_token_in_source(&tokens, source, "player", SemanticTokenKind::Variable, true);
    assert_token_in_source(&tokens, source, "damage", SemanticTokenKind::Function, true);
    assert_token_at_source_line(&tokens, source, "on", 8, SemanticTokenKind::Method, false);
    assert_token_at_source_line(
        &tokens,
        source,
        "player.hit",
        8,
        SemanticTokenKind::Event,
        true,
    );
    assert_token_at_source_line(&tokens, source, "retry", 9, SemanticTokenKind::Label, true);
    assert_token_at_source_line(
        &tokens,
        source,
        "retry",
        10,
        SemanticTokenKind::Label,
        false,
    );
}

#[test]
fn returns_tokens_in_source_order() {
    let tokens = semantic_tokens();
    let mut sorted = tokens.clone();

    sorted.sort_by_key(|token| position_key(token.span.start));

    assert_eq!(tokens, sorted);
}

fn semantic_tokens() -> Vec<SemanticToken> {
    semantic_tokens_tua_source("src/player.tua", SOURCE).expect("source should parse")
}

fn assert_token(tokens: &[SemanticToken], text: &str, kind: SemanticTokenKind, declaration: bool) {
    assert_token_in_source(tokens, SOURCE, text, kind, declaration);
}

fn assert_token_in_source(
    tokens: &[SemanticToken],
    source: &str,
    text: &str,
    kind: SemanticTokenKind,
    declaration: bool,
) {
    let token = tokens
        .iter()
        .find(|token| {
            token_text_from_source(source, token) == text
                && token.kind == kind
                && token
                    .modifiers
                    .contains(&SemanticTokenModifier::Declaration)
                    == declaration
        })
        .unwrap_or_else(|| panic!("expected {kind:?} token `{text}`"));

    assert_eq!(
        token
            .modifiers
            .contains(&SemanticTokenModifier::Declaration),
        declaration,
        "expected {kind:?} token `{text}` declaration={declaration}, got {:?}",
        token.modifiers
    );
}

fn assert_token_at_line(
    tokens: &[SemanticToken],
    text: &str,
    line: usize,
    kind: SemanticTokenKind,
    declaration: bool,
) {
    let token = tokens
        .iter()
        .find(|token| {
            token.span.start.line == line
                && token_text_from_source(SOURCE, token) == text
                && token.kind == kind
        })
        .unwrap_or_else(|| {
            let actual = tokens
                .iter()
                .filter(|token| token.span.start.line == line)
                .map(|token| {
                    (
                        token_text_from_source(SOURCE, token),
                        token.kind,
                        &token.modifiers,
                    )
                })
                .collect::<Vec<_>>();
            panic!("expected {kind:?} token `{text}` on line {line}; got {actual:?}")
        });

    assert_eq!(
        token
            .modifiers
            .contains(&SemanticTokenModifier::Declaration),
        declaration,
        "expected {kind:?} token `{text}` on line {line} declaration={declaration}, got {:?}",
        token.modifiers
    );
}

fn assert_token_at_source_line(
    tokens: &[SemanticToken],
    source: &str,
    text: &str,
    line: usize,
    kind: SemanticTokenKind,
    declaration: bool,
) {
    let token = tokens
        .iter()
        .find(|token| {
            token.span.start.line == line
                && token_text_from_source(source, token) == text
                && token.kind == kind
        })
        .unwrap_or_else(|| {
            let actual = tokens
                .iter()
                .filter(|token| token.span.start.line == line)
                .map(|token| {
                    (
                        token_text_from_source(source, token),
                        token.kind,
                        &token.modifiers,
                    )
                })
                .collect::<Vec<_>>();
            panic!("expected {kind:?} token `{text}` on line {line}; got {actual:?}")
        });

    assert_eq!(
        token
            .modifiers
            .contains(&SemanticTokenModifier::Declaration),
        declaration,
        "expected {kind:?} token `{text}` on line {line} declaration={declaration}, got {:?}",
        token.modifiers
    );
}

fn assert_token_modifier_at_line(
    tokens: &[SemanticToken],
    source: &str,
    text: &str,
    line: usize,
    kind: SemanticTokenKind,
    modifier: SemanticTokenModifier,
) {
    let token = tokens
        .iter()
        .find(|token| {
            token.span.start.line == line
                && token_text_from_source(source, token) == text
                && token.kind == kind
        })
        .unwrap_or_else(|| panic!("expected {kind:?} token `{text}` on line {line}"));

    assert!(
        token.modifiers.contains(&modifier),
        "expected {kind:?} token `{text}` on line {line} to have {modifier:?}, got {:?}",
        token.modifiers
    );
}

fn assert_token_modifier(
    tokens: &[SemanticToken],
    source: &str,
    text: &str,
    kind: SemanticTokenKind,
    modifier: SemanticTokenModifier,
) {
    let token = tokens
        .iter()
        .find(|token| token_text_from_source(source, token) == text && token.kind == kind)
        .unwrap_or_else(|| panic!("expected {kind:?} token `{text}`"));

    assert!(
        token.modifiers.contains(&modifier),
        "expected {kind:?} token `{text}` to have {modifier:?}, got {:?}",
        token.modifiers
    );
}

fn token_text_from_source<'a>(source: &'a str, token: &SemanticToken) -> &'a str {
    let line = line_at(source, token.span.start.line).expect("token line should exist");
    let start = byte_for_column(line, token.span.start.column);
    let end = byte_for_column(line, token.span.end.column);

    &line[start..end]
}
