use full_moon::ast;
use full_moon::node::Node;
use full_moon::tokenizer::TokenReference;

use crate::types::{TableField, Type};
use crate::{ParsedSource, SourcePosition, SourceSpan};

pub(crate) fn config_type() -> Type {
    Type::table([
        ("audio", TableField::optional(audio_type())),
        ("console", TableField::optional(Type::boolean())),
        ("externalstorage", TableField::optional(Type::boolean())),
        ("gammacorrect", TableField::optional(Type::boolean())),
        ("identity", TableField::optional(Type::string())),
        ("modules", TableField::optional(modules_type())),
        ("version", TableField::optional(Type::string())),
        ("window", TableField::optional(window_type())),
    ])
}

pub(crate) fn parameter_name_at(parsed: &ParsedSource, position: SourcePosition) -> Option<String> {
    for stmt in parsed.ast.nodes().stmts() {
        let ast::Stmt::FunctionDeclaration(function) = stmt else {
            continue;
        };

        let Some(body_span) = node_span(function.body()) else {
            continue;
        };
        if !span_contains(body_span, position) {
            continue;
        }

        return parameter_name_for_function(function);
    }

    None
}

pub(crate) fn parameter_type_for_function(
    function: &ast::FunctionDeclaration,
) -> Option<(String, Type)> {
    parameter_name_for_function(function).map(|name| (name, config_type()))
}

pub(crate) fn parameter_type_at_position(
    parsed: &ParsedSource,
    name: &str,
    position: SourcePosition,
) -> Option<Type> {
    (parameter_name_at(parsed, position).as_deref() == Some(name)).then(config_type)
}

fn audio_type() -> Type {
    Type::table([
        ("mic", TableField::optional(Type::boolean())),
        ("mixwithsystem", TableField::optional(Type::boolean())),
    ])
}

fn modules_type() -> Type {
    Type::table([
        ("audio", TableField::optional(Type::boolean())),
        ("data", TableField::optional(Type::boolean())),
        ("event", TableField::optional(Type::boolean())),
        ("font", TableField::optional(Type::boolean())),
        ("graphics", TableField::optional(Type::boolean())),
        ("image", TableField::optional(Type::boolean())),
        ("joystick", TableField::optional(Type::boolean())),
        ("keyboard", TableField::optional(Type::boolean())),
        ("math", TableField::optional(Type::boolean())),
        ("mouse", TableField::optional(Type::boolean())),
        ("physics", TableField::optional(Type::boolean())),
        ("sound", TableField::optional(Type::boolean())),
        ("system", TableField::optional(Type::boolean())),
        ("thread", TableField::optional(Type::boolean())),
        ("timer", TableField::optional(Type::boolean())),
        ("touch", TableField::optional(Type::boolean())),
        ("video", TableField::optional(Type::boolean())),
        ("window", TableField::optional(Type::boolean())),
    ])
}

fn window_type() -> Type {
    Type::table([
        ("borderless", TableField::optional(Type::boolean())),
        ("centered", TableField::optional(Type::boolean())),
        ("depth", TableField::optional(Type::number())),
        ("display", TableField::optional(Type::number())),
        ("displayindex", TableField::optional(Type::number())),
        ("fullscreen", TableField::optional(Type::boolean())),
        (
            "fullscreentype",
            TableField::optional(Type::union([
                Type::string_literal("desktop"),
                Type::string_literal("exclusive"),
            ])),
        ),
        ("height", TableField::optional(Type::number())),
        ("highdpi", TableField::optional(Type::boolean())),
        ("icon", TableField::optional(Type::string())),
        ("minheight", TableField::optional(Type::number())),
        ("minwidth", TableField::optional(Type::number())),
        ("msaa", TableField::optional(Type::number())),
        ("resizable", TableField::optional(Type::boolean())),
        ("stencil", TableField::optional(Type::number())),
        ("title", TableField::optional(Type::string())),
        ("usedpiscale", TableField::optional(Type::boolean())),
        ("vsync", TableField::optional(Type::number())),
        ("width", TableField::optional(Type::number())),
        ("x", TableField::optional(Type::number())),
        ("y", TableField::optional(Type::number())),
    ])
}

fn parameter_name_for_function(function: &ast::FunctionDeclaration) -> Option<String> {
    let name = function
        .name()
        .to_string()
        .split_whitespace()
        .collect::<String>();
    if name != "love.conf" {
        return None;
    }

    let Some(ast::Parameter::Name(name)) = function.body().parameters().iter().next() else {
        return None;
    };

    Some(token_text(name))
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn span_contains(span: SourceSpan, position: SourcePosition) -> bool {
    let start = (span.start.line, span.start.column);
    let end = (span.end.line, span.end.column);
    let position = (position.line, position.column);

    start <= position && position < end
}

fn token_text(token: &TokenReference) -> String {
    token.to_string().trim().to_owned()
}
