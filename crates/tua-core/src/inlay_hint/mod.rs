use std::path::Path;

use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, LastStmt, Suffix, TableConstructor,
};
use full_moon::node::Node;

use crate::semantic::SemanticModel;
use crate::types::{build_type_environment, Type};
use crate::{parse_tua_source, Diagnostic, ParsedSource, SourcePosition};

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
    let environment = build_type_environment(parsed)?;
    let mut collector = InlayHintCollector {
        semantic: SemanticModel::new(parsed, &environment),
        hints: Vec::new(),
    };

    collector.collect_block(parsed.ast.nodes());
    collector
        .hints
        .sort_by_key(|hint| position_key(hint.position));

    Ok(collector.hints)
}

struct InlayHintCollector<'a> {
    semantic: SemanticModel<'a>,
    hints: Vec<InlayHint>,
}

impl<'a> InlayHintCollector<'a> {
    fn collect_block(&mut self, block: &ast::Block) {
        for stmt in block.stmts() {
            self.collect_stmt(stmt);
        }

        if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
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
                self.hints.push(InlayHint {
                    position: node_end(name),
                    label: format!(": {ty}"),
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
                self.hints.push(InlayHint {
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

        self.hints.push(InlayHint {
            position: node_end(for_stmt.index_variable()),
            label: ": number".to_owned(),
            kind: InlayHintKind::Type,
        });

        self.collect_block(for_stmt.block());
    }

    fn collect_expression(&mut self, expression: &Expression) {
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

        for (argument, parameter) in arguments.iter().zip(&function.parameters) {
            self.collect_expression(argument);

            if !parameter.name.is_empty() {
                self.hints.push(InlayHint {
                    position: node_start(*argument),
                    label: format!("{}:", parameter.name),
                    kind: InlayHintKind::Parameter,
                });
            }
        }

        for argument in arguments.iter().skip(function.parameters.len()) {
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
}

fn assignment_expression_for_local<'a>(
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

fn should_hint_type(ty: &Type) -> bool {
    !matches!(ty, Type::Any)
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

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

#[cfg(test)]
mod tests;
