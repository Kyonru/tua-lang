use std::path::Path;

use full_moon::ast;
use full_moon::ast::span::ContainedSpan;
use full_moon::tokenizer::TokenReference;

use crate::{parse_tua_source, Diagnostic, DiagnosticCode, ParsedSource};

pub fn emit_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<String, Vec<Diagnostic>> {
    let parsed =
        parse_tua_source(path, source).map_err(reclassify_compound_assignment_diagnostics)?;

    emit_lua(&parsed)
}

pub fn emit_lua(parsed: &ParsedSource) -> Result<String, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let ast = erase_ast(&parsed.path, parsed.ast.clone(), &mut diagnostics);

    if diagnostics.is_empty() {
        Ok(lower_compound_assignments_text(&normalize_emitted_lua(
            ast.to_string(),
        )))
    } else {
        Err(diagnostics)
    }
}

fn erase_ast(path: &Path, ast: ast::Ast, diagnostics: &mut Vec<Diagnostic>) -> ast::Ast {
    let nodes = erase_block(path, ast.nodes().clone(), diagnostics);

    ast.with_nodes(nodes)
}

fn erase_block(path: &Path, block: ast::Block, diagnostics: &mut Vec<Diagnostic>) -> ast::Block {
    let stmts = block
        .stmts_with_semicolon()
        .cloned()
        .filter_map(|(stmt, semicolon)| {
            erase_stmt(path, stmt, diagnostics).map(|stmt| (stmt, semicolon))
        })
        .collect();
    let last_stmt = block.last_stmt_with_semicolon().cloned();

    block.with_stmts(stmts).with_last_stmt(last_stmt)
}

fn erase_stmt(
    path: &Path,
    stmt: ast::Stmt,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ast::Stmt> {
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
            let body = erase_function_body(path, function.body().clone(), diagnostics);

            Some(ast::Stmt::LocalFunction(function.with_body(body)))
        }
        ast::Stmt::FunctionDeclaration(function) => {
            let body = erase_function_body(path, function.body().clone(), diagnostics);

            Some(ast::Stmt::FunctionDeclaration(function.with_body(body)))
        }
        ast::Stmt::Do(block) => {
            let erased = erase_block(path, block.block().clone(), diagnostics);

            Some(ast::Stmt::Do(block.with_block(erased)))
        }
        ast::Stmt::If(if_stmt) => {
            let block = erase_block(path, if_stmt.block().clone(), diagnostics);
            let else_if = if_stmt.else_if().map(|items| {
                items
                    .iter()
                    .cloned()
                    .map(|else_if| erase_else_if(path, else_if, diagnostics))
                    .collect()
            });
            let else_block = if_stmt
                .else_block()
                .cloned()
                .map(|block| erase_block(path, block, diagnostics));

            Some(ast::Stmt::If(
                if_stmt
                    .with_block(block)
                    .with_else_if(else_if)
                    .with_else(else_block),
            ))
        }
        ast::Stmt::GenericFor(for_stmt) => {
            let block = erase_block(path, for_stmt.block().clone(), diagnostics);
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
            let block = erase_block(path, for_stmt.block().clone(), diagnostics);
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
            let block = erase_block(path, repeat.block().clone(), diagnostics);

            Some(ast::Stmt::Repeat(repeat.with_block(block)))
        }
        ast::Stmt::While(while_stmt) => {
            let block = erase_block(path, while_stmt.block().clone(), diagnostics);

            Some(ast::Stmt::While(while_stmt.with_block(block)))
        }
        _ => Some(stmt),
    }
}

fn erase_else_if(
    path: &Path,
    else_if: ast::ElseIf,
    diagnostics: &mut Vec<Diagnostic>,
) -> ast::ElseIf {
    let block = erase_block(path, else_if.block().clone(), diagnostics);

    else_if.with_block(block)
}

fn erase_function_body(
    path: &Path,
    body: ast::FunctionBody,
    diagnostics: &mut Vec<Diagnostic>,
) -> ast::FunctionBody {
    let block = erase_block(path, body.block().clone(), diagnostics);
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

fn lower_compound_assignments_text(output: &str) -> String {
    let mut lowered = output
        .lines()
        .map(lower_compound_assignment_line)
        .collect::<Vec<_>>()
        .join("\n");

    if output.ends_with('\n') {
        lowered.push('\n');
    }

    lowered
}

fn lower_compound_assignment_line(line: &str) -> String {
    let trimmed = line.trim_start_matches(char::is_whitespace);
    let indent = &line[..line.len() - trimmed.len()];
    let Some((lhs, rest)) = split_plain_field_chain_prefix(trimmed) else {
        return line.to_owned();
    };
    let rest = rest.trim_start();
    let Some((binary_operator, rhs)) = split_compound_operator(rest) else {
        return line.to_owned();
    };

    format!(
        "{indent}{lhs} = {lhs} {binary_operator} {}",
        rhs.trim_start()
    )
}

fn split_plain_field_chain_prefix(input: &str) -> Option<(&str, &str)> {
    let mut end = consume_identifier(input)?;

    loop {
        let rest = &input[end..];

        if !rest.starts_with('.') {
            break;
        }

        let field_end = consume_identifier(&rest[1..])?;
        end += 1 + field_end;
    }

    Some((&input[..end], &input[end..]))
}

fn consume_identifier(input: &str) -> Option<usize> {
    let mut chars = input.char_indices();
    let (_, first) = chars.next()?;

    if !is_identifier_start(first) {
        return None;
    }

    let mut end = first.len_utf8();

    for (index, character) in chars {
        if !is_identifier_part(character) {
            return Some(index);
        }

        end = index + character.len_utf8();
    }

    Some(end)
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_identifier_part(character: char) -> bool {
    is_identifier_start(character) || character.is_ascii_digit()
}

fn split_compound_operator(input: &str) -> Option<(&'static str, &str)> {
    [
        ("+=", "+"),
        ("-=", "-"),
        ("*=", "*"),
        ("/=", "/"),
        ("%=", "%"),
    ]
    .into_iter()
    .find_map(|(sugar, binary)| input.strip_prefix(sugar).map(|rhs| (binary, rhs)))
}

fn reclassify_compound_assignment_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|mut diagnostic| {
            if diagnostic.code == Some(DiagnosticCode::SYNTAX_COMPOUND_ASSIGNMENT) {
                diagnostic.code = Some(DiagnosticCode::LOWERING_COMPOUND_ASSIGNMENT);
            }

            diagnostic
        })
        .collect()
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
mod tests;
