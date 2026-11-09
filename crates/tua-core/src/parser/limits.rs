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
    validate_native_array_depth(path, source)?;
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
            "(" | "[" | "[]" | "{" | "function" | "if" | "do" | "repeat" => nesting += 1,
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
            return Err(vec![complexity_diagnostic(
                path,
                position.line(),
                position.character(),
            )]);
        }
        previous_line = position.line();
        previous_operator = operator;
    }
    Ok(())
}

fn validate_native_array_depth(path: &Path, source: &str) -> Result<(), Vec<Diagnostic>> {
    if !source.contains("[]") {
        return Ok(());
    }
    let chars = source.chars().collect::<Vec<_>>();
    let protected = super::normalize::protected_syntax_characters(&chars);
    let mut index = 0;

    while index + 1 < chars.len() {
        if chars[index] != '['
            || chars[index + 1] != ']'
            || protected[index]
            || protected[index + 1]
        {
            index += 1;
            continue;
        }

        let mut depth = 0;
        while index + 1 < chars.len()
            && chars[index] == '['
            && chars[index + 1] == ']'
            && !protected[index]
            && !protected[index + 1]
        {
            depth += 1;
            if depth > MAX_NESTING {
                let prefix = chars[..index].iter().collect::<String>();
                let line = prefix
                    .chars()
                    .filter(|character| *character == '\n')
                    .count()
                    + 1;
                let column = prefix
                    .rsplit('\n')
                    .next()
                    .map_or(1, |line| line.chars().count() + 1);
                return Err(vec![complexity_diagnostic(path, line, column)]);
            }
            index += 2;
        }
    }

    Ok(())
}

fn complexity_diagnostic(path: &Path, line: usize, column: usize) -> Diagnostic {
    Diagnostic::error(
        path,
        line,
        column,
        "source exceeds analysis complexity limit (64 nested constructs or 128 operators per expression segment); split the expression or function",
    )
    .with_code(DiagnosticCode::SYNTAX_PARSE)
}
