use full_moon::ast::{Call, Expression, FunctionArgs, FunctionCall, Prefix, Suffix, Var};

use crate::PrimitiveType;

pub(crate) const IS_CHECK_HELPER: &str = "__tua_is";
pub(crate) const LUAJIT_TYPE_NAMES: &[&str] = &[
    "nil", "boolean", "userdata", "number", "string", "table", "function", "thread", "cdata",
];

pub(crate) fn is_luajit_type_name(name: &str) -> bool {
    LUAJIT_TYPE_NAMES.contains(&name)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeTypeCheckCall {
    pub value_name: String,
    pub target: String,
}

pub(crate) fn is_check_call(function_call: &FunctionCall) -> bool {
    let Prefix::Name(name) = function_call.prefix() else {
        return false;
    };

    token_text(name) == IS_CHECK_HELPER
}

pub(crate) fn is_check_call_parts(function_call: &FunctionCall) -> Option<RuntimeTypeCheckCall> {
    if !is_check_call(function_call) {
        return None;
    }

    let Some(Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }))) =
        function_call.suffixes().next()
    else {
        return None;
    };

    let mut arguments = arguments.iter();
    let value = arguments.next()?;
    let target = arguments.next()?;

    if arguments.next().is_some() {
        return None;
    }

    let Expression::Var(Var::Name(value_name)) = value else {
        return None;
    };

    Some(RuntimeTypeCheckCall {
        value_name: token_text(value_name),
        target: string_literal_value(target)?,
    })
}

pub(crate) fn target_lua_type_name(target: &str) -> Option<&'static str> {
    match target.trim() {
        "boolean" => Some("boolean"),
        "nil" => Some("nil"),
        "number" => Some("number"),
        "string" => Some("string"),
        "table" => Some("table"),
        _ => None,
    }
}

pub(crate) fn target_primitive(target: &str) -> Option<PrimitiveType> {
    match target.trim() {
        "boolean" => Some(PrimitiveType::Boolean),
        "nil" => Some(PrimitiveType::Nil),
        "number" => Some(PrimitiveType::Number),
        "string" => Some(PrimitiveType::String),
        _ => None,
    }
}

pub(crate) fn is_table_target(target: &str) -> bool {
    target.trim() == "table"
}

pub(crate) fn is_identifier(target: &str) -> bool {
    let mut chars = target.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn string_literal_value(expression: &Expression) -> Option<String> {
    let Expression::String(_) = expression else {
        return None;
    };

    let text = expression.to_string();
    let text = text.trim();
    let quote = text.chars().next()?;

    if quote != '"' && quote != '\'' {
        return None;
    }

    let inner = text.strip_prefix(quote)?.strip_suffix(quote)?;
    let mut value = String::new();
    let mut escaped = false;

    for character in inner.chars() {
        if escaped {
            value.push(match character {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                _ => character,
            });
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        value.push(character);
    }

    if escaped {
        value.push('\\');
    }

    Some(value)
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}
