use std::path::Path;

use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, LastStmt, Suffix, TableConstructor,
};
use full_moon::node::Node;

use crate::ast_utils::assignment_expression_for_local;
use crate::semantic::SemanticModel;
use crate::types::{build_type_environment, Type, TypeEnvironment};
use crate::{
    ecs, input, object_pool, parse_tua_source, state_machine, Diagnostic, ParsedSource,
    SourcePosition, SourceSpan,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlayHint {
    pub position: SourcePosition,
    pub label: String,
    pub kind: InlayHintKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InlayHintKind {
    Type,
    Parameter,
}

pub fn inlay_hints_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<Vec<InlayHint>, Vec<Diagnostic>> {
    let parsed = parse_tua_source(path, source)?;

    inlay_hints_parsed_source(&parsed)
}

pub fn inlay_hints_parsed_source(parsed: &ParsedSource) -> Result<Vec<InlayHint>, Vec<Diagnostic>> {
    let environment = ecs::augment_environment(build_type_environment(parsed)?);

    Ok(inlay_hints_parsed_source_with_environment(
        parsed,
        &environment,
    ))
}

pub(crate) fn inlay_hints_parsed_source_with_environment(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
) -> Vec<InlayHint> {
    inlay_hints_parsed_source_with_environment_in_range(parsed, environment, None)
}

pub(crate) fn inlay_hints_parsed_source_with_environment_in_range(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    range: Option<SourceSpan>,
) -> Vec<InlayHint> {
    let mut collector = InlayHintCollector {
        parsed,
        semantic: SemanticModel::new(parsed, environment),
        environment,
        range,
        hints: Vec::new(),
    };

    collector.collect_block(parsed.ast.nodes());
    collector
        .hints
        .sort_by_key(|hint| position_key(hint.position));

    collector.hints
}

struct InlayHintCollector<'a> {
    parsed: &'a ParsedSource,
    semantic: SemanticModel<'a>,
    environment: &'a TypeEnvironment,
    range: Option<SourceSpan>,
    hints: Vec<InlayHint>,
}

impl<'a> InlayHintCollector<'a> {
    fn collect_block(&mut self, block: &ast::Block) {
        for stmt in block.stmts() {
            if !self.node_overlaps_range(stmt) {
                continue;
            }

            self.collect_stmt(stmt);
        }

        if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
            if !self.node_overlaps_range(return_stmt) {
                return;
            }

            for expression in return_stmt.returns() {
                self.collect_expression(expression);
            }
        }
    }

    fn collect_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Assignment(assignment) => {
                for expression in assignment.expressions() {
                    self.collect_expression(expression);
                }
            }
            ast::Stmt::Do(block) => self.collect_block(block.block()),
            ast::Stmt::FunctionCall(function_call) => self.collect_function_call(function_call),
            ast::Stmt::FunctionDeclaration(function) => self.collect_block(function.body().block()),
            ast::Stmt::GenericFor(for_stmt) => self.collect_generic_for(for_stmt),
            ast::Stmt::If(if_stmt) => {
                self.collect_block(if_stmt.block());

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        self.collect_block(else_if.block());
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    self.collect_block(else_block);
                }
            }
            ast::Stmt::LocalAssignment(local) => self.collect_local_assignment(local),
            ast::Stmt::LocalFunction(function) => self.collect_block(function.body().block()),
            ast::Stmt::NumericFor(for_stmt) => self.collect_numeric_for(for_stmt),
            ast::Stmt::Repeat(repeat) => self.collect_block(repeat.block()),
            ast::Stmt::While(while_stmt) => self.collect_block(while_stmt.block()),
            _ => {}
        }
    }

    fn collect_local_assignment(&mut self, local: &ast::LocalAssignment) {
        let specifiers = local.type_specifiers().collect::<Vec<_>>();
        let expressions = local.expressions().iter().collect::<Vec<_>>();

        for expression in &expressions {
            self.collect_expression(expression);
        }

        for (index, name) in local.names().iter().enumerate() {
            if specifiers
                .get(index)
                .and_then(|specifier| *specifier)
                .is_some()
            {
                continue;
            }

            let Some((expression, return_index)) =
                assignment_expression_for_local(&expressions, index)
            else {
                continue;
            };
            let Some(ty) =
                self.semantic
                    .type_of_expression_return(expression, node_start(name), return_index)
            else {
                continue;
            };

            if should_hint_type(&ty) {
                let hint_type = if object_pool::pool_item_type(self.environment, &ty).is_some() {
                    Type::alias(object_pool::TYPE_NAME)
                } else if state_machine::type_is_state_machine(self.environment, &ty) {
                    Type::alias(state_machine::TYPE_NAME)
                } else if input::type_is_input(self.environment, &ty) {
                    Type::alias(input::TYPE_NAME)
                } else {
                    ty
                };

                self.push_hint(InlayHint {
                    position: node_end(name),
                    label: format!(": {hint_type}"),
                    kind: InlayHintKind::Type,
                });
            }
        }
    }

    fn collect_generic_for(&mut self, for_stmt: &ast::GenericFor) {
        for expression in for_stmt.expressions() {
            self.collect_expression(expression);
        }

        for name in for_stmt.names() {
            let position = node_start(name);
            let Some(ty) = self.semantic.type_of_name(&token_text(name), position) else {
                continue;
            };

            if should_hint_type(&ty) {
                self.push_hint(InlayHint {
                    position: node_end(name),
                    label: format!(": {ty}"),
                    kind: InlayHintKind::Type,
                });
            }
        }

        self.collect_block(for_stmt.block());
    }

    fn collect_numeric_for(&mut self, for_stmt: &ast::NumericFor) {
        self.collect_expression(for_stmt.start());
        self.collect_expression(for_stmt.end());

        if let Some(step) = for_stmt.step() {
            self.collect_expression(step);
        }

        self.push_hint(InlayHint {
            position: node_end(for_stmt.index_variable()),
            label: ": number".to_owned(),
            kind: InlayHintKind::Type,
        });

        self.collect_block(for_stmt.block());
    }

    fn collect_expression(&mut self, expression: &Expression) {
        if !self.node_overlaps_range(expression) {
            return;
        }

        match expression {
            Expression::BinaryOperator { lhs, rhs, .. } => {
                self.collect_expression(lhs);
                self.collect_expression(rhs);
            }
            Expression::Function(function) => self.collect_block(function.body().block()),
            Expression::FunctionCall(function_call) => self.collect_function_call(function_call),
            Expression::Parentheses { expression, .. } => self.collect_expression(expression),
            Expression::TableConstructor(table) => self.collect_table_constructor(table),
            Expression::TypeAssertion { expression, .. } => self.collect_expression(expression),
            Expression::UnaryOperator { expression, .. } => self.collect_expression(expression),
            _ => {}
        }
    }

    fn collect_table_constructor(&mut self, table: &TableConstructor) {
        for field in table.fields() {
            match field {
                Field::ExpressionKey { key, value, .. } => {
                    self.collect_expression(key);
                    self.collect_expression(value);
                }
                Field::NameKey { value, .. } | Field::NoKey(value) => {
                    self.collect_expression(value);
                }
                _ => {}
            }
        }
    }

    fn collect_function_call(&mut self, function_call: &FunctionCall) {
        let Some(function) = self
            .semantic
            .typed_function_for_call(function_call, node_start(function_call))
        else {
            self.collect_call_arguments(function_call);
            return;
        };
        let arguments = parenthesized_arguments(function_call).unwrap_or_default();
        let parameters = call_visible_parameters(function_call, &function.parameters);

        for (argument, parameter) in arguments.iter().zip(parameters) {
            self.collect_expression(argument);

            if !parameter.name.is_empty() {
                self.push_hint(InlayHint {
                    position: node_start(*argument),
                    label: format!("{}:", parameter.name),
                    kind: InlayHintKind::Parameter,
                });
            }
        }

        for argument in arguments.iter().skip(parameters.len()) {
            self.collect_expression(argument);
        }
    }

    fn collect_call_arguments(&mut self, function_call: &FunctionCall) {
        if let Some(arguments) = parenthesized_arguments(function_call) {
            for argument in arguments {
                self.collect_expression(argument);
            }
        }
    }

    fn push_hint(&mut self, mut hint: InlayHint) {
        let Some(position) = self.source_position(hint.position) else {
            return;
        };
        hint.position = position;

        if self
            .range
            .is_none_or(|range| range_contains_position(range, hint.position))
        {
            self.hints.push(hint);
        }
    }

    fn node_overlaps_range(&self, node: &impl Node) -> bool {
        let Some(span) = node_span(node) else {
            return self.range.is_none();
        };
        let Some(span) = self.source_span(span) else {
            return false;
        };

        self.range
            .is_none_or(|range| span_overlaps_range(span, range))
    }

    fn source_span(&self, parser_span: SourceSpan) -> Option<SourceSpan> {
        Some(SourceSpan {
            start: self.source_position(parser_span.start)?,
            end: self.source_position(parser_span.end)?,
        })
    }

    fn source_position(&self, parser_position: SourcePosition) -> Option<SourcePosition> {
        let provenance = self
            .parsed
            .raw
            .parser_provenance
            .get(parser_position.line.saturating_sub(1))?;
        if provenance.class_generated {
            return None;
        }

        Some(SourcePosition {
            line: provenance.source_span.start.line,
            column: provenance
                .source_span
                .start
                .column
                .saturating_add(parser_position.column.saturating_sub(1)),
        })
    }
}

fn should_hint_type(ty: &Type) -> bool {
    !matches!(ty, Type::Any) && type_is_reasonably_shallow(ty, 0)
}

// External LuaCATS libraries may define recursive/very deeply nested table
// unions (for example Feel's recursive step sequences).  Inlay labels are
// presentation-only; avoid expanding such types indefinitely while formatting
// a hint, which can otherwise overflow the language server stack.
fn type_is_reasonably_shallow(ty: &Type, depth: usize) -> bool {
    const MAX_DEPTH: usize = 64;
    if depth >= MAX_DEPTH {
        return false;
    }
    match ty {
        Type::Union(types) | Type::Tuple(types) => types
            .iter()
            .all(|ty| type_is_reasonably_shallow(ty, depth + 1)),
        Type::Function(function) => {
            function
                .parameters
                .iter()
                .all(|parameter| type_is_reasonably_shallow(&parameter.ty, depth + 1))
                && function
                    .vararg_type
                    .as_deref()
                    .is_none_or(|ty| type_is_reasonably_shallow(ty, depth + 1))
                && function
                    .return_types_or_any()
                    .iter()
                    .all(|ty| type_is_reasonably_shallow(ty, depth + 1))
        }
        Type::Table(table) => table
            .fields()
            .all(|(_, field)| type_is_reasonably_shallow(&field.ty, depth + 1)),
        Type::Array(item) => type_is_reasonably_shallow(item, depth + 1),
        Type::Dictionary { key, value } => {
            type_is_reasonably_shallow(key, depth + 1)
                && type_is_reasonably_shallow(value, depth + 1)
        }
        Type::Primitive(_) | Type::Any | Type::Alias(_) | Type::StringLiteral(_) => true,
    }
}

fn call_visible_parameters<'a>(
    function_call: &FunctionCall,
    parameters: &'a [crate::TypedParameter],
) -> &'a [crate::TypedParameter] {
    if is_method_call(function_call) && parameters.first().is_some_and(is_synthetic_method_receiver)
    {
        &parameters[1..]
    } else {
        parameters
    }
}

fn is_synthetic_method_receiver(parameter: &crate::TypedParameter) -> bool {
    parameter.name == "self" || (parameter.name.is_empty() && parameter.ty == Type::Any)
}

fn is_method_call(function_call: &FunctionCall) -> bool {
    function_call
        .suffixes()
        .any(|suffix| matches!(suffix, Suffix::Call(Call::MethodCall(_))))
}

fn parenthesized_arguments(function_call: &FunctionCall) -> Option<Vec<&Expression>> {
    let call = function_call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;

    match call {
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            Some(arguments.iter().collect())
        }
        Call::MethodCall(method) => match method.args() {
            FunctionArgs::Parentheses { arguments, .. } => Some(arguments.iter().collect()),
            _ => None,
        },
        _ => None,
    }
}

fn node_start(node: &impl Node) -> SourcePosition {
    node.range()
        .map(|(start, _)| SourcePosition::from_full_moon(start))
        .unwrap_or(SourcePosition { line: 1, column: 1 })
}

fn node_end(node: &impl Node) -> SourcePosition {
    node.range()
        .map(|(_, end)| SourcePosition::from_full_moon(end))
        .unwrap_or(SourcePosition { line: 1, column: 1 })
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn span_overlaps_range(span: SourceSpan, range: SourceSpan) -> bool {
    position_key(span.start) <= position_key(range.end)
        && position_key(range.start) <= position_key(span.end)
}

fn range_contains_position(range: SourceSpan, position: SourcePosition) -> bool {
    position_key(range.start) <= position_key(position)
        && position_key(position) <= position_key(range.end)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

#[cfg(test)]
mod tests;
