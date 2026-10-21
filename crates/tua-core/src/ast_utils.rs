use full_moon::ast::{
    self, Call, Expression, FunctionArgs, FunctionCall, Index, Prefix, Suffix, Var,
};
use full_moon::tokenizer::TokenReference;

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
        .map(|expression| (expression, index - fixed_expression_count))
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
