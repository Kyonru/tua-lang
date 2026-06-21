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
fn classifies_primitive_types_and_unresolved_identifiers() {
    let tokens = semantic_tokens();

    assert_token(&tokens, "number", SemanticTokenKind::BuiltinType, false);
    assert_token(&tokens, "string", SemanticTokenKind::BuiltinType, false);
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
fn falls_back_to_lossy_tokens_when_source_does_not_parse() {
    let source = r#"type Player = {
  hp: number,
}

local player: Player =
function player.damage(amount: number)
  player.
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
        .find(|token| token_text_from_source(source, token) == text && token.kind == kind)
        .unwrap_or_else(|| panic!("expected {kind:?} token `{text}`"));

    assert_eq!(
        token
            .modifiers
            .contains(&SemanticTokenModifier::Declaration),
        declaration
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
        .unwrap_or_else(|| panic!("expected {kind:?} token `{text}` on line {line}"));

    assert_eq!(
        token
            .modifiers
            .contains(&SemanticTokenModifier::Declaration),
        declaration
    );
}

fn token_text_from_source<'a>(source: &'a str, token: &SemanticToken) -> &'a str {
    let line = line_at(source, token.span.start.line).expect("token line should exist");
    let start = byte_for_column(line, token.span.start.column);
    let end = byte_for_column(line, token.span.end.column);

    &line[start..end]
}
