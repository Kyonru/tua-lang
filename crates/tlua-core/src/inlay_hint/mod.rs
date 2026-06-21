use std::path::Path;

use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, Index, LastStmt, Prefix, Suffix,
    TableConstructor, Var,
};
use full_moon::node::Node;

use crate::semantic::resolve_field_chain_type;
use crate::types::{
    build_type_environment, FunctionType, TableField, Type, TypeEnvironment, TypedFunction,
};
use crate::{parse_tlua_source, Diagnostic, ParsedSource, SourcePosition};

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

pub fn inlay_hints_tlua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<Vec<InlayHint>, Vec<Diagnostic>> {
    let parsed = parse_tlua_source(path, source)?;

    inlay_hints_parsed_source(&parsed)
}

pub fn inlay_hints_parsed_source(parsed: &ParsedSource) -> Result<Vec<InlayHint>, Vec<Diagnostic>> {
    let environment = build_type_environment(parsed)?;
    let mut collector = InlayHintCollector {
        environment: &environment,
        hints: Vec::new(),
    };

    collector.collect_block(parsed.ast.nodes());
    collector
        .hints
        .sort_by_key(|hint| position_key(hint.position));

    Ok(collector.hints)
}

struct InlayHintCollector<'a> {
    environment: &'a TypeEnvironment,
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
            ast::Stmt::GenericFor(for_stmt) => self.collect_block(for_stmt.block()),
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
            ast::Stmt::NumericFor(for_stmt) => self.collect_block(for_stmt.block()),
            ast::Stmt::Repeat(repeat) => self.collect_block(repeat.block()),
            ast::Stmt::While(while_stmt) => self.collect_block(while_stmt.block()),
            _ => {}
        }
    }

    fn collect_local_assignment(&mut self, local: &ast::LocalAssignment) {
        let specifiers = local.type_specifiers().collect::<Vec<_>>();

        for (index, (name, expression)) in local
            .names()
            .iter()
            .zip(local.expressions().iter())
            .enumerate()
        {
            self.collect_expression(expression);

            if specifiers
                .get(index)
                .and_then(|specifier| *specifier)
                .is_some()
            {
                continue;
            }

            let Some(ty) = self.infer_expression(expression) else {
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
        let Some(function) = self.typed_function_for_call(function_call) else {
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

    fn infer_expression(&self, expression: &Expression) -> Option<Type> {
        match expression {
            Expression::Number(_) => Some(Type::number()),
            Expression::String(_) => Some(Type::string()),
            Expression::Symbol(symbol) => infer_symbol_type(&token_text(symbol)),
            Expression::TableConstructor(table) => Some(self.infer_table_constructor(table)),
            Expression::Parentheses { expression, .. } => self.infer_expression(expression),
            Expression::TypeAssertion { expression, .. } => self.infer_expression(expression),
            Expression::FunctionCall(function_call) => self
                .typed_function_for_call(function_call)
                .map(|function| function.return_type_or_any()),
            Expression::Var(var) => self.infer_var_value(var),
            _ => None,
        }
    }

    fn infer_table_constructor(&self, table: &TableConstructor) -> Type {
        Type::table(table.fields().iter().filter_map(|field| match field {
            Field::NameKey { key, value, .. } => {
                let ty = self.infer_expression(value).unwrap_or(Type::Any);

                Some((token_text(key), TableField::required(ty)))
            }
            _ => None,
        }))
    }

    fn infer_var_value(&self, var: &Var) -> Option<Type> {
        match var {
            Var::Name(name) => self.infer_named_value(&token_text(name)),
            Var::Expression(expression) => resolve_field_chain_type(
                self.environment,
                &plain_field_chain_from_var_expression(expression)?,
            ),
            _ => None,
        }
    }

    fn infer_named_value(&self, name: &str) -> Option<Type> {
        self.environment.local(name).cloned().or_else(|| {
            self.environment
                .function(name)
                .and_then(TypedFunction::as_type)
        })
    }

    fn typed_function_for_call(&self, function_call: &FunctionCall) -> Option<TypedFunction> {
        let parts = field_chain_before_first_call(function_call)?;
        let (root, fields) = parts.split_first()?;

        if fields.is_empty() {
            return self.environment.function(root).cloned().filter(|function| {
                function.has_fully_annotated_parameters() && function.return_type.is_some()
            });
        }

        let callee_type = resolve_field_chain_type(self.environment, &parts)?;
        let Type::Function(function) = callee_type else {
            return None;
        };

        Some(typed_function_from_function_type(function))
    }
}

fn typed_function_from_function_type(function: FunctionType) -> TypedFunction {
    TypedFunction {
        name: None,
        parameters: function
            .parameters
            .into_iter()
            .map(|ty| crate::TypedParameter {
                name: String::new(),
                ty,
            })
            .collect(),
        parameter_count: 0,
        has_varargs: false,
        return_type: Some(*function.return_type),
    }
}

fn should_hint_type(ty: &Type) -> bool {
    !matches!(ty, Type::Any)
}

fn infer_symbol_type(symbol: &str) -> Option<Type> {
    match symbol {
        "true" | "false" => Some(Type::boolean()),
        "nil" => Some(Type::nil()),
        _ => None,
    }
}

fn plain_field_chain_from_var_expression(expression: &ast::VarExpression) -> Option<Vec<String>> {
    let Prefix::Name(name) = expression.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in expression.suffixes() {
        let Suffix::Index(index) = suffix else {
            return None;
        };

        let Index::Dot { name, .. } = index else {
            return None;
        };

        parts.push(token_text(name));
    }

    Some(parts)
}

fn field_chain_before_first_call(function_call: &FunctionCall) -> Option<Vec<String>> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in function_call.suffixes() {
        match suffix {
            Suffix::Index(Index::Dot { name, .. }) => parts.push(token_text(name)),
            Suffix::Call(_) => return Some(parts),
            _ => return None,
        }
    }

    None
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

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests;
