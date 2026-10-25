use std::collections::BTreeMap;

use full_moon::ast::{self, Call, Expression, FunctionArgs, FunctionCall, Prefix, Suffix, Var};
use full_moon::node::Node;
use full_moon::visitors::Visitor;

use crate::ast_utils::plain_field_chain_from_var_expression;
use crate::control_flow::block_fallthrough_position;
use crate::runtime_type;
use crate::types::MetatableFacts;
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
    Table,
    RuntimeType(String),
}

/// A local that received a later value of a call whose first value guards the
/// rest, such as `y` in `local x, y = project(point)`. Once `x` is known not
/// to be `nil`, `y` has the type the non-nil returns give it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReturnGuard {
    pub guard: String,
    pub sibling: String,
    pub guarded: Type,
}

/// The guards created by `local first, ... = call()` when the call has
/// guarded return types.
pub(crate) fn return_guards_for_names(names: &[String], guarded: &[Type]) -> Vec<ReturnGuard> {
    let Some(guard) = names.first() else {
        return Vec::new();
    };

    names
        .iter()
        .zip(guarded)
        .skip(1)
        .filter(|(sibling, _)| *sibling != guard)
        .map(|(sibling, guarded)| ReturnGuard {
            guard: guard.clone(),
            sibling: sibling.clone(),
            guarded: guarded.clone(),
        })
        .collect()
}

/// Narrows the siblings of every guard that `narrowed` proves is not `nil`.
pub(crate) fn extend_with_return_guards<'a>(
    narrowed: &mut BTreeMap<String, Type>,
    guards: impl IntoIterator<Item = &'a ReturnGuard>,
) {
    let mut siblings = Vec::new();

    for guard in guards {
        let Some(guard_type) = narrowed.get(&guard.guard) else {
            continue;
        };
        if type_may_be_nil(guard_type) || narrowed.contains_key(&guard.sibling) {
            continue;
        }
        siblings.push((guard.sibling.clone(), guard.guarded.clone()));
    }

    narrowed.extend(siblings);
}

pub(crate) fn type_may_be_nil(ty: &Type) -> bool {
    match ty {
        Type::Any => true,
        Type::Primitive(PrimitiveType::Nil) => true,
        Type::Union(types) => types.iter().any(type_may_be_nil),
        _ => false,
    }
}

/// Whether any statement in the file assigns or redeclares `name` after
/// `after` and before `before`. Unrelated scopes count too, which keeps the
/// answer conservative.
pub(crate) fn name_assigned_between(
    parsed: &ParsedSource,
    name: &str,
    after: SourcePosition,
    before: SourcePosition,
) -> bool {
    struct AssignmentFinder<'a> {
        name: &'a str,
        after: SourcePosition,
        before: SourcePosition,
        found: bool,
    }

    impl AssignmentFinder<'_> {
        fn record(&mut self, node: &impl Node, assigns: bool) {
            if assigns
                && node_start(node).is_some_and(|start| {
                    starts_before(self.after, start) && starts_before(start, self.before)
                })
            {
                self.found = true;
            }
        }
    }

    impl Visitor for AssignmentFinder<'_> {
        fn visit_assignment(&mut self, node: &ast::Assignment) {
            let assigns = node
                .variables()
                .iter()
                .any(|var| assigned_name(var).as_deref() == Some(self.name));
            self.record(node, assigns);
        }

        fn visit_compound_assignment(&mut self, node: &ast::CompoundAssignment) {
            let assigns = assigned_name(node.lhs()).as_deref() == Some(self.name);
            self.record(node, assigns);
        }

        fn visit_local_assignment(&mut self, node: &ast::LocalAssignment) {
            let assigns = node
                .names()
                .iter()
                .any(|local_name| token_text(local_name) == self.name);
            self.record(node, assigns);
        }
    }

    let mut finder = AssignmentFinder {
        name,
        after,
        before,
        found: false,
    };
    finder.visit_ast(&parsed.ast);
    finder.found
}

/// Drops the guards that an assignment or declaration of `name` invalidates.
pub(crate) fn forget_return_guards(guards: &mut Vec<ReturnGuard>, name: &str) {
    guards.retain(|guard| guard.guard != name && guard.sibling != name);
}

pub(crate) fn narrowed_locals_for_condition(
    condition: &Expression,
    aliases: &BTreeMap<String, Type>,
    lookup: impl Fn(&str) -> Option<Type> + Copy,
) -> BTreeMap<String, Type> {
    let mut locals = BTreeMap::new();

    for narrowing in condition_narrowings(condition) {
        let Some(base) = lookup(&narrowing.name) else {
            continue;
        };
        let Some(narrowed) = apply_narrowing(aliases, &base, &narrowing.kind, lookup) else {
            continue;
        };

        locals.insert(narrowing.name, narrowed);
    }

    locals
}

pub(crate) fn narrowed_locals_after_false_condition(
    condition: &Expression,
    aliases: &BTreeMap<String, Type>,
    lookup: impl Fn(&str) -> Option<Type> + Copy,
) -> BTreeMap<String, Type> {
    let mut locals = BTreeMap::new();

    for narrowing in false_condition_narrowings(condition) {
        let Some(base) = lookup(&narrowing.name) else {
            continue;
        };
        let Some(narrowed) = apply_narrowing(aliases, &base, &narrowing.kind, lookup) else {
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
        let Some(span) = node_span(stmt) else {
            continue;
        };
        if !starts_before(span.end, position) {
            break;
        }
        if stmt_mutates_name_before(stmt, name, position) {
            *narrowed = None;
        }
        if let Some(guarded) = narrowed_type_after_guard(stmt, aliases, name, lookup) {
            *narrowed = Some(guarded);
        }
    }

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
            ast::Stmt::While(while_stmt) => collect_if_branch_narrowing(
                while_stmt.condition(),
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

fn narrowed_type_after_guard(
    statement: &ast::Stmt,
    aliases: &BTreeMap<String, Type>,
    name: &str,
    lookup: impl Fn(&str) -> Option<Type> + Copy,
) -> Option<Type> {
    let ast::Stmt::If(statement) = statement else {
        return None;
    };
    if statement.else_if().is_some() || statement.else_block().is_some() {
        return None;
    }
    let empty_position = node_start(statement)?;
    if block_fallthrough_position(statement.block(), empty_position, &|_| false).is_some() {
        return None;
    }

    narrowed_locals_after_false_condition(statement.condition(), aliases, lookup).remove(name)
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

                apply_narrowing(aliases, &base, &narrowing.kind, lookup)
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
        // Both operands of `and` hold when the whole condition does.
        Expression::BinaryOperator {
            lhs,
            binop: ast::BinOp::And(_),
            rhs,
        } => condition_narrowings(lhs)
            .into_iter()
            .chain(condition_narrowings(rhs))
            .collect(),
        // A truthy value is not nil. It may still be `true`, so booleans stay.
        Expression::Var(Var::Name(name)) => vec![TypeNarrowing {
            name: token_text(name),
            kind: TypeNarrowingKind::NonNil,
        }],
        Expression::FunctionCall(function_call) => runtime_type_check_narrowing(function_call)
            .into_iter()
            .collect(),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => condition_narrowings(expression),
        _ => Vec::new(),
    }
}

fn false_condition_narrowings(condition: &Expression) -> Vec<TypeNarrowing> {
    match condition {
        Expression::UnaryOperator {
            unop: ast::UnOp::Not(_),
            expression,
        } => condition_narrowings(expression),
        Expression::BinaryOperator { lhs, binop, rhs } if is_equality_operator(binop) => {
            equality_narrowing(lhs, binop, rhs)
                .into_iter()
                .chain(equality_narrowing(rhs, binop, lhs))
                .filter_map(|mut narrowing| {
                    narrowing.kind = match narrowing.kind {
                        TypeNarrowingKind::NonNil => TypeNarrowingKind::Nil,
                        TypeNarrowingKind::Nil => TypeNarrowingKind::NonNil,
                        _ => return None,
                    };
                    Some(narrowing)
                })
                .collect()
        }
        Expression::Var(Var::Name(name)) => vec![TypeNarrowing {
            name: token_text(name),
            kind: TypeNarrowingKind::Nil,
        }],
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => false_condition_narrowings(expression),
        _ => Vec::new(),
    }
}

fn runtime_type_check_narrowing(function_call: &FunctionCall) -> Option<TypeNarrowing> {
    let call = runtime_type::is_check_call_parts(function_call)?;

    Some(TypeNarrowing {
        name: call.value_name,
        kind: TypeNarrowingKind::RuntimeType(call.target),
    })
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
        "table" => TypeNarrowingKind::Table,
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
    lookup: impl Fn(&str) -> Option<Type> + Copy,
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
        TypeNarrowingKind::Table => apply_table_narrowing(&resolved),
        TypeNarrowingKind::RuntimeType(target) => {
            apply_runtime_type_narrowing(aliases, &resolved, target, lookup)
        }
    }
}

fn apply_runtime_type_narrowing(
    aliases: &BTreeMap<String, Type>,
    base: &Type,
    target: &str,
    lookup: impl Fn(&str) -> Option<Type> + Copy,
) -> Option<Type> {
    if runtime_type::is_table_target(target) {
        return apply_table_narrowing(base);
    }

    if let Some(primitive) = runtime_type::target_primitive(target) {
        let kind = match primitive {
            PrimitiveType::Nil => TypeNarrowingKind::Nil,
            primitive => TypeNarrowingKind::Primitive(primitive),
        };

        return apply_narrowing(aliases, base, &kind, lookup);
    }

    if let Some(runtime_target) = lookup(target) {
        return metatable_instance_type(aliases, target, &runtime_target);
    }

    let alias = aliases.get(target)?;
    match resolve_aliases_from_map(aliases, alias, 0) {
        Type::Primitive(PrimitiveType::Nil) => {
            apply_narrowing(aliases, base, &TypeNarrowingKind::Nil, lookup)
        }
        Type::Primitive(primitive) => apply_narrowing(
            aliases,
            base,
            &TypeNarrowingKind::Primitive(primitive),
            lookup,
        ),
        _ => None,
    }
}

fn apply_table_narrowing(base: &Type) -> Option<Type> {
    match base {
        Type::Any => Some(Type::empty_table()),
        Type::Table(_) => Some(base.clone()),
        Type::Union(types) => {
            let narrowed = types
                .iter()
                .filter_map(|ty| match ty {
                    Type::Any => Some(Type::empty_table()),
                    Type::Table(_) => Some(ty.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();

            match narrowed.len() {
                0 => None,
                1 => narrowed.into_iter().next(),
                _ => Some(Type::union(narrowed)),
            }
        }
        _ => None,
    }
}

fn metatable_instance_type(
    aliases: &BTreeMap<String, Type>,
    target_name: &str,
    target: &Type,
) -> Option<Type> {
    let resolved = resolve_aliases_from_map(aliases, target, 0);

    if !matches!(resolved, Type::Table(_)) {
        return None;
    }

    let base = aliases
        .get(target_name)
        .map(|alias| resolve_aliases_from_map(aliases, alias, 0))
        .filter(|alias| matches!(alias, Type::Table(_)))
        .unwrap_or_else(Type::empty_table);

    Some(MetatableFacts::from_type(&resolved).apply_to_type(base))
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
        Type::StringLiteral(_) => primitive == PrimitiveType::String,
        Type::Union(types) => types
            .iter()
            .any(|ty| type_contains_primitive(ty, primitive)),
        Type::Any => true,
        Type::Alias(_)
        | Type::Function(_)
        | Type::Table(_)
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
    token.token().to_string().trim().to_owned()
}
