use full_moon::ast::{
    self, Call, Expression, FunctionArgs, FunctionCall, Index, Prefix, Suffix, Var,
};
use full_moon::tokenizer::{StringLiteralQuoteType, TokenReference, TokenType};

pub(crate) fn token_text(token: &TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

pub(crate) fn assignment_expression_for_local<'a>(
    expressions: &[&'a Expression],
    index: usize,
) -> Option<(&'a Expression, usize)> {
    if expressions.is_empty() {
        return None;
    }

    let fixed_expression_count = expressions.len().saturating_sub(1);

    if index < fixed_expression_count {
        return expressions
            .get(index)
            .copied()
            .map(|expression| (expression, 0));
    }

    expressions
        .last()
        .copied()
        .filter(|expression| {
            index == fixed_expression_count
                || matches!(expression, Expression::FunctionCall(_))
                || matches!(expression, Expression::Symbol(symbol) if token_text(symbol) == "...")
        })
        .map(|expression| (expression, index - fixed_expression_count))
}

/// Split a value chain at its final call so all inference consumers can use
/// their normal call resolver before resolving the remaining property/index.
pub(crate) fn var_expression_call_prefix(
    expression: &ast::VarExpression,
) -> Option<(FunctionCall, Vec<&Suffix>)> {
    let suffixes = expression.suffixes().collect::<Vec<_>>();
    let last_call = suffixes
        .iter()
        .rposition(|suffix| matches!(suffix, Suffix::Call(_)))?;
    let call = FunctionCall::new(expression.prefix().clone()).with_suffixes(
        suffixes[..=last_call]
            .iter()
            .map(|suffix| (*suffix).clone())
            .collect(),
    );
    Some((call, suffixes[last_call + 1..].to_vec()))
}

pub(crate) fn plain_field_chain_from_var(var: &Var) -> Option<Vec<String>> {
    match var {
        Var::Name(name) => Some(vec![token_text(name)]),
        Var::Expression(expression) => plain_field_chain_from_var_expression(expression),
        _ => None,
    }
}

pub(crate) fn plain_field_chain_from_var_expression(
    expression: &ast::VarExpression,
) -> Option<Vec<String>> {
    let Prefix::Name(name) = expression.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in expression.suffixes() {
        let Suffix::Index(Index::Dot { name, .. }) = suffix else {
            return None;
        };

        parts.push(token_text(name));
    }

    Some(parts)
}

pub(crate) fn plain_field_chain_from_expression(expression: &Expression) -> Option<Vec<String>> {
    match expression {
        Expression::Var(var) => plain_field_chain_from_var(var),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => {
            plain_field_chain_from_expression(expression)
        }
        _ => None,
    }
}

pub(crate) fn field_chain_before_first_call(function_call: &FunctionCall) -> Option<Vec<String>> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    let mut parts = vec![bare_token_text(name)];

    for suffix in function_call.suffixes() {
        match suffix {
            Suffix::Index(Index::Dot { name, .. }) => parts.push(bare_token_text(name)),
            Suffix::Call(Call::MethodCall(method)) => {
                parts.push(bare_token_text(method.name()));
                return Some(parts);
            }
            Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { .. }))
            | Suffix::Call(Call::AnonymousCall(FunctionArgs::String(_)))
            | Suffix::Call(Call::AnonymousCall(FunctionArgs::TableConstructor(_))) => {
                return Some(parts);
            }
            _ => return None,
        }
    }

    None
}

fn bare_token_text(token: &TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

/// Decode static Lua string contents without changing their source token spans.
pub(crate) fn string_literal_runtime_bytes(token: &TokenReference) -> Option<Vec<u8>> {
    let TokenType::StringLiteral {
        literal,
        quote_type,
        ..
    } = token.token().token_type()
    else {
        return None;
    };

    match quote_type {
        StringLiteralQuoteType::Brackets => Some(long_string_runtime_bytes(literal.as_bytes())),
        StringLiteralQuoteType::Double | StringLiteralQuoteType::Single => {
            decode_short_string_runtime_bytes(literal.as_bytes())
        }
        _ => None,
    }
}

fn long_string_runtime_bytes(literal: &[u8]) -> Vec<u8> {
    let literal = if literal.starts_with(b"\r\n") {
        &literal[2..]
    } else if literal.starts_with(b"\n") || literal.starts_with(b"\r") {
        &literal[1..]
    } else {
        literal
    };
    let mut output = Vec::with_capacity(literal.len());
    let mut index = 0;

    while index < literal.len() {
        match literal[index] {
            b'\r' => {
                if literal.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                output.push(b'\n');
            }
            byte => output.push(byte),
        }

        index += 1;
    }

    output
}

pub(crate) fn decode_short_string_runtime_bytes(literal: &[u8]) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(literal.len());
    let mut index = 0;

    while index < literal.len() {
        if literal[index] != b'\\' {
            output.push(literal[index]);
            index += 1;
            continue;
        }

        index += 1;
        let escaped = *literal.get(index)?;

        if escaped.is_ascii_digit() {
            let start = index;
            let end = (index + 3).min(literal.len());

            while index < end && literal[index].is_ascii_digit() {
                index += 1;
            }

            let value = std::str::from_utf8(&literal[start..index])
                .ok()?
                .parse::<u16>()
                .ok()?;
            output.push(u8::try_from(value).ok()?);
            continue;
        }

        match escaped {
            b'a' => output.push(0x07),
            b'b' => output.push(0x08),
            b'f' => output.push(0x0c),
            b'n' => output.push(b'\n'),
            b'r' => output.push(b'\r'),
            b't' => output.push(b'\t'),
            b'v' => output.push(0x0b),
            b'\\' => output.push(b'\\'),
            b'"' => output.push(b'"'),
            b'\'' => output.push(b'\''),
            b'\n' => output.push(b'\n'),
            b'\r' => {
                if literal.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                output.push(b'\n');
            }
            b'x' => {
                let high = *literal.get(index + 1)?;
                let low = *literal.get(index + 2)?;
                output.push(hex_digit(high)? * 16 + hex_digit(low)?);
                index += 2;
            }
            b'z' => {
                while literal.get(index + 1).is_some_and(u8::is_ascii_whitespace) {
                    index += 1;
                }
            }
            _ => return None,
        }

        index += 1;
    }

    Some(output)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(crate) fn parse_lua_number(text: &str) -> Option<f64> {
    let normalized = text.replace('_', "");

    if let Some(hex) = normalized
        .strip_prefix("0x")
        .or_else(|| normalized.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).ok().map(|value| value as f64)
    } else {
        normalized.parse().ok()
    }
}


/// A numeric literal, optionally parenthesized or negated; never evaluates user code.
pub(crate) fn numeric_literal(expression: &Expression) -> Option<f64> {
    match expression {
        Expression::Number(token) => parse_lua_number(&token_text(token)),
        Expression::UnaryOperator { unop: ast::UnOp::Minus(_), expression } => numeric_literal(expression).map(|value| -value),
        Expression::Parentheses { expression, .. } | Expression::TypeAssertion { expression, .. } => numeric_literal(expression),
        _ => None,
    }
}
