//! Cheap lexical limits run before recursive normalization, parsing and AST walks.
use std::path::Path;

use full_moon::ast::LuaVersion;
use full_moon::tokenizer::{Lexer, LexerResult, TokenType};

use crate::{Diagnostic, DiagnosticCode};

const MAX_NESTING: usize = 64;
const MAX_OPERATORS: usize = 128;

pub(super) fn validate(path: &Path, source: &str) -> Result<(), Vec<Diagnostic>> {
    if source.len() > crate::input_limits::MAX_SOURCE_BYTES {
        return Err(vec![Diagnostic::error(
            path,
            1,
            1,
            "source exceeds 8 MiB input limit",
        )
        .with_code(DiagnosticCode::SYNTAX_PARSE)]);
    }
    let tokens = match Lexer::new(source, LuaVersion::luau().with_luajit()).collect() {
        LexerResult::Ok(tokens) | LexerResult::Recovered(tokens, _) => tokens,
        LexerResult::Fatal(_) => return Ok(()), // The parser reports lexical errors.
    };
    let mut nesting = 0usize;
    let mut operators = 0usize;
    let mut previous_line = 0;
    let mut previous_operator = false;
    for token in tokens {
        if !matches!(
            token.token_type(),
            TokenType::Symbol { .. }
                | TokenType::Identifier { .. }
                | TokenType::Number { .. }
                | TokenType::StringLiteral { .. }
        ) {
            continue;
        }
        let text = token.to_string();
        let operator = matches!(
            text.as_str(),
            "+" | "-"
                | "*"
                | "/"
                | "%"
                | "^"
                | ".."
                | "and"
                | "or"
                | "not"
                | "#"
                | "."
                | ":"
                | "|"
                | "&"
                | "->"
                | "=="
                | "~="
                | "<"
                | ">"
                | "<="
                | ">="
        );
        let position = token.start_position();
        if position.line() != previous_line && !operator && !previous_operator {
            operators = 0;
        }
        match text.as_str() {
            "(" | "[" | "{" | "function" | "if" | "do" | "repeat" => nesting += 1,
            ")" | "]" | "}" | "end" | "until" => nesting = nesting.saturating_sub(1),
            _ => {}
        }
        if matches!(
            text.as_str(),
            "," | ";" | "=" | "local" | "return" | "then" | "do" | "end"
        ) {
            operators = 0;
        } else if operator {
            operators += 1;
        }
        if nesting > MAX_NESTING || operators > MAX_OPERATORS {
            return Err(vec![Diagnostic::error(path, position.line(), position.character(),
                "source exceeds analysis complexity limit (64 nested constructs or 128 operators per expression segment); split the expression or function")
                .with_code(DiagnosticCode::SYNTAX_PARSE)]);
        }
        previous_line = position.line();
        previous_operator = operator;
    }
    Ok(())
}
