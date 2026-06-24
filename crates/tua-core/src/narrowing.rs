use std::collections::BTreeMap;

use full_moon::ast::{self, Call, Expression, FunctionArgs, FunctionCall, Prefix, Suffix, Var};
use full_moon::node::Node;

use crate::semantic::plain_field_chain_from_var_expression;
use crate::{ParsedSource, PrimitiveType, SourcePosition, SourceSpan, Type};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeNarrowing {
    pub name: String,
    pub kind: TypeNarrowingKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TypeNarrowingKind {
    NonNil,
    Nil,
    Primitive(PrimitiveType),
}

pub(crate) fn narrowed_locals_for_condition(
    condition: &Expression,
    aliases: &BTreeMap<String, Type>,
    lookup: impl Fn(&str) -> Option<Type>,
) -> BTreeMap<String, Type> {
    let mut locals = BTreeMap::new();

    for narrowing in condition_narrowings(condition) {
        let Some(base) = lookup(&narrowing.name) else {
            continue;
        };
        let Some(narrowed) = apply_narrowing(aliases, &base, &narrowing.kind) else {
            continue;
        };

        locals.insert(narrowing.name, narrowed);
    }

    locals
}

pub(crate) fn narrowed_type_at_position(
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    name: &str,
    position: SourcePosition,
    lookup: impl Fn(&str) -> Option<Type> + Copy,
) -> Option<Type> {
    let mut narrowed = None;

    collect_narrowed_type_at_position(
        parsed.ast.nodes(),
        aliases,
        name,
        position,
        lookup,
        &mut narrowed,
    );

    narrowed
}

pub(crate) fn assigned_name(var: &Var) -> Option<String> {
    match var {
        Var::Name(name) => Some(token_text(name)),
        Var::Expression(expression) => plain_field_chain_from_var_expression(expression)?
            .into_iter()
            .next(),
        _ => None,
    }
}

pub(crate) fn directly_assigned_name(var: &Var) -> Option<String> {
    match var {
        Var::Name(name) => Some(token_text(name)),
        _ => None,
    }
}

fn collect_narrowed_type_at_position(
    block: &ast::Block,
    aliases: &BTreeMap<String, Type>,
    name: &str,
    position: SourcePosition,
    lookup: impl Fn(&str) -> Option<Type> + Copy,
    narrowed: &mut Option<Type>,
) {
    for stmt in block.stmts() {
        let Some(stmt_span) = node_span(stmt) else {
            continue;
        };

        if !contains_position(stmt_span, position) {
            continue;
        }

        match stmt {
            ast::Stmt::Do(block) => collect_narrowed_type_at_position(
                block.block(),
                aliases,
                name,
                position,
                lookup,
                narrowed,
            ),
            ast::Stmt::GenericFor(for_stmt) => collect_narrowed_type_at_position(
                for_stmt.block(),
                aliases,
                name,
                position,
                lookup,
                narrowed,
            ),
            ast::Stmt::FunctionDeclaration(function) => collect_narrowed_type_at_position(
                function.body().block(),
                aliases,
                name,
                position,
                lookup,
                narrowed,
            ),
            ast::Stmt::If(if_stmt) => {
                collect_if_branch_narrowing(
                    if_stmt.condition(),
                    if_stmt.block(),
                    aliases,
                    name,
                    position,
                    lookup,
                    narrowed,
                );

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        collect_if_branch_narrowing(
                            else_if.condition(),
                            else_if.block(),
                            aliases,
                            name,
                            position,
                            lookup,
                            narrowed,
                        );
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    collect_narrowed_type_at_position(
                        else_block, aliases, name, position, lookup, narrowed,
                    );
                }
            }
            ast::Stmt::LocalFunction(function) => collect_narrowed_type_at_position(
                function.body().block(),
                aliases,
                name,
                position,
                lookup,
                narrowed,
            ),
            ast::Stmt::NumericFor(for_stmt) => collect_narrowed_type_at_position(
                for_stmt.block(),
                aliases,
                name,
                position,
                lookup,
                narrowed,
            ),
            ast::Stmt::Repeat(repeat) => collect_narrowed_type_at_position(
                repeat.block(),
                aliases,
                name,
                position,
                lookup,
                narrowed,
            ),
            ast::Stmt::While(while_stmt) => collect_narrowed_type_at_position(
                while_stmt.block(),
                aliases,
                name,
                position,
                lookup,
                narrowed,
            ),
            _ => {}
        }
    }
}

fn collect_if_branch_narrowing(
    condition: &Expression,
    block: &ast::Block,
    aliases: &BTreeMap<String, Type>,
    name: &str,
    position: SourcePosition,
    lookup: impl Fn(&str) -> Option<Type> + Copy,
    narrowed: &mut Option<Type>,
) {
    let Some(block_span) = node_span(block) else {
        return;
    };

    if !contains_position(block_span, position) {
        return;
    }

    if !block_mutates_name_before(block, name, position) {
        let branch_narrowing = condition_narrowings(condition)
            .into_iter()
            .find(|narrowing| narrowing.name == name)
            .and_then(|narrowing| {
                let base = lookup(name)?;

                apply_narrowing(aliases, &base, &narrowing.kind)
            });

        if let Some(branch_narrowing) = branch_narrowing {
            *narrowed = Some(branch_narrowing);
        }
    }

    collect_narrowed_type_at_position(block, aliases, name, position, lookup, narrowed);
}

fn block_mutates_name_before(block: &ast::Block, name: &str, position: SourcePosition) -> bool {
    block
        .stmts()
        .any(|stmt| stmt_mutates_name_before(stmt, name, position))
}

fn stmt_mutates_name_before(stmt: &ast::Stmt, name: &str, position: SourcePosition) -> bool {
    if node_start(stmt).is_some_and(|start| !starts_before(start, position)) {
        return false;
    }

    match stmt {
        ast::Stmt::Assignment(assignment) => assignment
            .variables()
            .iter()
            .any(|var| assigned_name(var).as_deref() == Some(name)),
        ast::Stmt::LocalAssignment(local) => local
            .names()
            .iter()
            .any(|local_name| token_text(local_name) == name),
        ast::Stmt::CompoundAssignment(assignment) => {
            assigned_name(assignment.lhs()).as_deref() == Some(name)
        }
        ast::Stmt::Do(block) => block_mutates_name_before(block.block(), name, position),
        ast::Stmt::GenericFor(for_stmt) => {
            block_mutates_name_before(for_stmt.block(), name, position)
        }
        ast::Stmt::If(if_stmt) => {
            block_mutates_name_before(if_stmt.block(), name, position)
                || if_stmt.else_if().is_some_and(|else_ifs| {
                    else_ifs
                        .iter()
                        .any(|else_if| block_mutates_name_before(else_if.block(), name, position))
                })
                || if_stmt
                    .else_block()
                    .is_some_and(|else_block| block_mutates_name_before(else_block, name, position))
        }
        ast::Stmt::NumericFor(for_stmt) => {
            block_mutates_name_before(for_stmt.block(), name, position)
        }
        ast::Stmt::Repeat(repeat) => block_mutates_name_before(repeat.block(), name, position),
        ast::Stmt::While(while_stmt) => {
            block_mutates_name_before(while_stmt.block(), name, position)
        }
        _ => false,
    }
}

fn condition_narrowings(condition: &Expression) -> Vec<TypeNarrowing> {
    match condition {
        Expression::BinaryOperator { lhs, binop, rhs } if is_equality_operator(binop) => {
            equality_narrowing(lhs, binop, rhs)
                .into_iter()
                .chain(equality_narrowing(rhs, binop, lhs))
                .collect()
        }
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => condition_narrowings(expression),
        _ => Vec::new(),
    }
}

fn equality_narrowing(
    lhs: &Expression,
    binop: &ast::BinOp,
    rhs: &Expression,
) -> Option<TypeNarrowing> {
    if matches!(binop, ast::BinOp::TwoEqual(_)) {
        if let Some(name) = type_call_argument_name(lhs) {
            let kind = type_call_string_target(rhs)?;

            return Some(TypeNarrowing { name, kind });
        }
    }

    let name = expression_name(lhs)?;

    if matches!(binop, ast::BinOp::TildeEqual(_)) {
        if matches!(primitive_literal_type(rhs), Some(PrimitiveType::Nil)) {
            return Some(TypeNarrowing {
                name,
                kind: TypeNarrowingKind::NonNil,
            });
        }

        return None;
    }

    let kind = primitive_literal_type(rhs).map(|primitive| match primitive {
        PrimitiveType::Nil => TypeNarrowingKind::Nil,
        primitive => TypeNarrowingKind::Primitive(primitive),
    })?;

    Some(TypeNarrowing { name, kind })
}

fn type_call_string_target(expression: &Expression) -> Option<TypeNarrowingKind> {
    let primitive = string_type_literal(expression)?;

    Some(match primitive.as_str() {
        "boolean" => TypeNarrowingKind::Primitive(PrimitiveType::Boolean),
        "nil" => TypeNarrowingKind::Nil,
        "number" => TypeNarrowingKind::Primitive(PrimitiveType::Number),
        "string" => TypeNarrowingKind::Primitive(PrimitiveType::String),
        _ => return None,
    })
}

fn expression_name(expression: &Expression) -> Option<String> {
    match expression {
        Expression::Var(Var::Name(name)) => Some(token_text(name)),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => expression_name(expression),
        _ => None,
    }
}

fn type_call_argument_name(expression: &Expression) -> Option<String> {
    let Expression::FunctionCall(function_call) = expression else {
        return None;
    };
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    if token_text(name) != "type" {
        return None;
    }

    let arguments = first_parenthesized_call_arguments(function_call)?;
    let [Expression::Var(Var::Name(name))] = arguments.as_slice() else {
        return None;
    };

    Some(token_text(name))
}

fn first_parenthesized_call_arguments(function_call: &FunctionCall) -> Option<Vec<Expression>> {
    function_call.suffixes().find_map(|suffix| {
        let Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })) = suffix
        else {
            return None;
        };

        Some(arguments.iter().cloned().collect())
    })
}

fn primitive_literal_type(expression: &Expression) -> Option<PrimitiveType> {
    match expression {
        Expression::Number(_) => Some(PrimitiveType::Number),
        Expression::String(_) => Some(PrimitiveType::String),
        Expression::Symbol(symbol) => match token_text(symbol).as_str() {
            "true" | "false" => Some(PrimitiveType::Boolean),
            "nil" => Some(PrimitiveType::Nil),
            _ => None,
        },
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => primitive_literal_type(expression),
        _ => None,
    }
}

fn string_type_literal(expression: &Expression) -> Option<String> {
    let Expression::String(_) = expression else {
        return None;
    };

    let text = expression.to_string();
    let text = text.trim();

    text.strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            text.strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .map(str::to_owned)
}

fn apply_narrowing(
    aliases: &BTreeMap<String, Type>,
    base: &Type,
    kind: &TypeNarrowingKind,
) -> Option<Type> {
    let resolved = resolve_aliases_from_map(aliases, base, 0);

    match kind {
        TypeNarrowingKind::NonNil => Some(remove_nil(&resolved)),
        TypeNarrowingKind::Nil => type_contains_primitive(&resolved, PrimitiveType::Nil)
            .then(Type::nil)
            .or_else(|| matches!(resolved, Type::Any).then_some(Type::nil())),
        TypeNarrowingKind::Primitive(primitive) => type_contains_primitive(&resolved, *primitive)
            .then_some(Type::Primitive(*primitive))
            .or_else(|| matches!(resolved, Type::Any).then_some(Type::Primitive(*primitive))),
    }
}

fn remove_nil(ty: &Type) -> Type {
    match ty {
        Type::Union(types) => {
            let without_nil = types
                .iter()
                .filter(|ty| !matches!(ty, Type::Primitive(PrimitiveType::Nil)))
                .cloned()
                .collect::<Vec<_>>();

            match without_nil.len() {
                0 => ty.clone(),
                1 => without_nil.into_iter().next().unwrap_or_else(|| ty.clone()),
                _ => Type::union(without_nil),
            }
        }
        _ => ty.clone(),
    }
}

fn type_contains_primitive(ty: &Type, primitive: PrimitiveType) -> bool {
    match ty {
        Type::Primitive(current) => *current == primitive,
        Type::Union(types) => types
            .iter()
            .any(|ty| type_contains_primitive(ty, primitive)),
        Type::Any => true,
        Type::Alias(_)
        | Type::Function(_)
        | Type::Table(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => false,
    }
}

fn resolve_aliases_from_map(aliases: &BTreeMap<String, Type>, ty: &Type, depth: usize) -> Type {
    if depth >= 64 {
        return ty.clone();
    }

    match ty {
        Type::Alias(name) => aliases
            .get(name)
            .map(|resolved| resolve_aliases_from_map(aliases, resolved, depth + 1))
            .unwrap_or_else(|| ty.clone()),
        Type::Union(types) => Type::union(
            types
                .iter()
                .map(|ty| resolve_aliases_from_map(aliases, ty, depth + 1)),
        ),
        _ => ty.clone(),
    }
}

fn is_equality_operator(binop: &ast::BinOp) -> bool {
    matches!(binop, ast::BinOp::TwoEqual(_) | ast::BinOp::TildeEqual(_))
}

fn contains_position(span: SourceSpan, position: SourcePosition) -> bool {
    let start = position_key(span.start);
    let end = position_key(span.end);
    let position = position_key(position);

    start <= position && position < end
}

fn starts_before(left: SourcePosition, right: SourcePosition) -> bool {
    position_key(left) < position_key(right)
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn node_start(node: &impl Node) -> Option<SourcePosition> {
    node.tokens()
        .next()
        .and_then(|token| token.start_position())
        .map(SourcePosition::from_full_moon)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}
