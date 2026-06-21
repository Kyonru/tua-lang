use std::path::Path;

use full_moon::ast;
use full_moon::ast::span::ContainedSpan;
use full_moon::tokenizer::TokenReference;

use crate::{parse_tlua_source, Diagnostic, ParsedSource};

pub fn emit_tlua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<String, Vec<Diagnostic>> {
    let parsed = parse_tlua_source(path, source)?;

    Ok(emit_lua(&parsed))
}

pub fn emit_lua(parsed: &ParsedSource) -> String {
    normalize_emitted_lua(erase_ast(parsed.ast.clone()).to_string())
}

fn erase_ast(ast: ast::Ast) -> ast::Ast {
    let nodes = erase_block(ast.nodes().clone());

    ast.with_nodes(nodes)
}

fn erase_block(block: ast::Block) -> ast::Block {
    let stmts = block
        .stmts_with_semicolon()
        .cloned()
        .filter_map(|(stmt, semicolon)| erase_stmt(stmt).map(|stmt| (stmt, semicolon)))
        .collect();
    let last_stmt = block.last_stmt_with_semicolon().cloned();

    block.with_stmts(stmts).with_last_stmt(last_stmt)
}

fn erase_stmt(stmt: ast::Stmt) -> Option<ast::Stmt> {
    match stmt {
        ast::Stmt::TypeDeclaration(_) => None,
        ast::Stmt::ExportedTypeDeclaration(_) => None,
        ast::Stmt::LocalAssignment(local) => {
            let had_type_specifier = local.type_specifiers().any(|specifier| specifier.is_some());
            let equal_token = if had_type_specifier && local.equal_token().is_some() {
                Some(spaced_symbol("="))
            } else {
                local.equal_token().cloned()
            };

            Some(ast::Stmt::LocalAssignment(
                local
                    .with_type_specifiers(Vec::new())
                    .with_equal_token(equal_token),
            ))
        }
        ast::Stmt::LocalFunction(function) => {
            let body = erase_function_body(function.body().clone());

            Some(ast::Stmt::LocalFunction(function.with_body(body)))
        }
        ast::Stmt::FunctionDeclaration(function) => {
            let body = erase_function_body(function.body().clone());

            Some(ast::Stmt::FunctionDeclaration(function.with_body(body)))
        }
        ast::Stmt::Do(block) => {
            let erased = erase_block(block.block().clone());

            Some(ast::Stmt::Do(block.with_block(erased)))
        }
        ast::Stmt::If(if_stmt) => {
            let block = erase_block(if_stmt.block().clone());
            let else_if = if_stmt
                .else_if()
                .map(|items| items.iter().cloned().map(erase_else_if).collect());
            let else_block = if_stmt.else_block().cloned().map(erase_block);

            Some(ast::Stmt::If(
                if_stmt
                    .with_block(block)
                    .with_else_if(else_if)
                    .with_else(else_block),
            ))
        }
        ast::Stmt::GenericFor(for_stmt) => {
            let block = erase_block(for_stmt.block().clone());
            let had_type_specifier = for_stmt
                .type_specifiers()
                .any(|specifier| specifier.is_some());
            let for_stmt = if had_type_specifier {
                for_stmt.with_in_token(spaced_symbol("in"))
            } else {
                for_stmt
            };

            Some(ast::Stmt::GenericFor(
                for_stmt.with_type_specifiers(Vec::new()).with_block(block),
            ))
        }
        ast::Stmt::NumericFor(for_stmt) => {
            let block = erase_block(for_stmt.block().clone());
            let had_type_specifier = for_stmt.type_specifier().is_some();
            let for_stmt = if had_type_specifier {
                for_stmt.with_equal_token(spaced_symbol("="))
            } else {
                for_stmt
            };

            Some(ast::Stmt::NumericFor(
                for_stmt.with_type_specifier(None).with_block(block),
            ))
        }
        ast::Stmt::Repeat(repeat) => {
            let block = erase_block(repeat.block().clone());

            Some(ast::Stmt::Repeat(repeat.with_block(block)))
        }
        ast::Stmt::While(while_stmt) => {
            let block = erase_block(while_stmt.block().clone());

            Some(ast::Stmt::While(while_stmt.with_block(block)))
        }
        _ => Some(stmt),
    }
}

fn erase_else_if(else_if: ast::ElseIf) -> ast::ElseIf {
    let block = erase_block(else_if.block().clone());

    else_if.with_block(block)
}

fn erase_function_body(body: ast::FunctionBody) -> ast::FunctionBody {
    let block = erase_block(body.block().clone());
    let parameters_parentheses = parameters_parentheses_without_return_type(&body);

    body.with_generics(None)
        .with_type_specifiers(Vec::new())
        .with_return_type(None)
        .with_parameters_parentheses(parameters_parentheses)
        .with_block(block)
}

fn parameters_parentheses_without_return_type(body: &ast::FunctionBody) -> ContainedSpan {
    let Some(return_type) = body.return_type() else {
        return body.parameters_parentheses().clone();
    };

    let (open, close) = body.parameters_parentheses().tokens();
    let return_type_text = return_type.to_string();
    let trailing = trailing_whitespace(&return_type_text);

    ContainedSpan::new(open.clone(), symbol_with_trailing(close, trailing))
}

fn symbol_with_trailing(token: &TokenReference, trailing: &str) -> TokenReference {
    TokenReference::symbol(&format!("{}{trailing}", token.to_string().trim()))
        .expect("existing syntax token should remain a valid symbol")
}

fn spaced_symbol(symbol: &str) -> TokenReference {
    TokenReference::symbol(&format!(" {symbol} ")).expect("spaced syntax token should be valid")
}

fn trailing_whitespace(text: &str) -> &str {
    let index = text.trim_end_matches(char::is_whitespace).len();

    &text[index..]
}

fn normalize_emitted_lua(output: String) -> String {
    let trimmed = output.trim_matches(char::is_whitespace);

    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_type_aliases_emit_no_runtime_code() {
        let emitted = emit_tlua_source(
            "src/types.tlua",
            r#"
type Vec2 = {
  x: number,
  y: number,
}

type MaybeNumber = number | nil
"#,
        )
        .expect("type-only source should emit");

        assert_eq!(emitted, "");
    }

    #[test]
    fn snapshot_erases_local_annotations() {
        let emitted = emit_tlua_source(
            "src/locals.tlua",
            r#"
type Player = {
  hp: number,
}

local score: number = 0
local title: string = "Arena"
local current: Player | nil = nil

return score
"#,
        )
        .expect("annotated locals should emit");

        assert_eq!(
            emitted,
            r#"local score = 0
local title = "Arena"
local current = nil

return score
"#
        );
    }

    #[test]
    fn snapshot_erases_function_annotations() {
        let emitted = emit_tlua_source(
            "src/functions.tlua",
            r#"
type Player = {
  hp: number,
}

local function damage(player: Player, amount: number): Player
  player.hp = player.hp - amount
  return player
end

function heal(player: Player, amount: number): Player
  player.hp = player.hp + amount
  return player
end
"#,
        )
        .expect("annotated functions should emit");

        assert_eq!(
            emitted,
            r#"local function damage(player, amount)
  player.hp = player.hp - amount
  return player
end

function heal(player, amount)
  player.hp = player.hp + amount
  return player
end
"#
        );
    }

    #[test]
    fn snapshot_preserves_ordinary_lua_statements_and_expressions() {
        let input = r#"
local Player = require("player")
local player = {
  hp = 10,
  alive = true,
}

if player.hp > 0 then
  print(Player, player.hp)
end
"#;
        let emitted = emit_tlua_source("src/plain.tlua", input).expect("plain Lua should emit");

        assert_eq!(
            emitted,
            r#"local Player = require("player")
local player = {
  hp = 10,
  alive = true,
}

if player.hp > 0 then
  print(Player, player.hp)
end
"#
        );
    }

    #[test]
    fn snapshot_erases_annotations_in_nested_blocks() {
        let emitted = emit_tlua_source(
            "src/nested.tlua",
            r#"
if ready then
  type LocalState = {
    ok: boolean,
  }
  local state: LocalState = { ok = true }
  for index: number = 1, 3 do
    local value: number = index
    print(value)
  end
end
"#,
        )
        .expect("nested annotations should emit");

        assert_eq!(
            emitted,
            r#"if ready then
  local state = { ok = true }
  for index = 1, 3 do
    local value = index
    print(value)
  end
end
"#
        );
    }
}
