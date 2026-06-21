use std::path::Path;

use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, Index, LastStmt, Prefix, Return,
    Suffix, TableConstructor, Var,
};
use full_moon::node::Node;

use crate::parser::{parse_tua_source, ParsedSource, SourcePosition};
use crate::types::{
    build_type_environment, FunctionType, TableField, Type, TypeContext, TypeEnvironment,
    TypedFunction,
};
use crate::{Diagnostic, DiagnosticCode};

const MAX_TYPE_RESOLUTION_DEPTH: usize = 64;

pub fn check_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<(), Vec<Diagnostic>> {
    let parsed = parse_tua_source(path, source)?;

    check_parsed_source(&parsed)
}

pub fn check_parsed_source(parsed: &ParsedSource) -> Result<(), Vec<Diagnostic>> {
    let environment = build_type_environment(parsed)?;
    let mut checker = Checker::new(parsed, &environment);

    checker.check_block(parsed.ast.nodes(), None);

    if checker.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(checker.diagnostics)
    }
}

struct Checker<'a> {
    parsed: &'a ParsedSource,
    environment: &'a TypeEnvironment,
    context: TypeContext,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Checker<'a> {
    fn new(parsed: &'a ParsedSource, environment: &'a TypeEnvironment) -> Self {
        Self {
            parsed,
            environment,
            context: environment.to_type_context(),
            diagnostics: Vec::new(),
        }
    }

    fn check_block(&mut self, block: &ast::Block, current_return: Option<&Type>) {
        for stmt in block.stmts() {
            self.check_stmt(stmt, current_return);
        }

        if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
            self.check_return(return_stmt, current_return);
        }
    }

    fn check_stmt(&mut self, stmt: &ast::Stmt, current_return: Option<&Type>) {
        match stmt {
            ast::Stmt::Assignment(assignment) => self.check_assignment(assignment),
            ast::Stmt::FunctionCall(function_call) => {
                self.check_function_call(function_call);
            }
            ast::Stmt::LocalAssignment(local) => self.check_local_assignment(local),
            ast::Stmt::Do(block) => self.check_block(block.block(), current_return),
            ast::Stmt::If(if_stmt) => {
                self.check_block(if_stmt.block(), current_return);
                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        self.check_block(else_if.block(), current_return);
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    self.check_block(else_block, current_return);
                }
            }
            ast::Stmt::GenericFor(for_stmt) => self.check_block(for_stmt.block(), current_return),
            ast::Stmt::LocalFunction(function) => {
                let return_type = self.function_return_type(&token_text(function.name()));

                self.check_block(function.body().block(), return_type.as_ref());
            }
            ast::Stmt::FunctionDeclaration(function) => {
                let return_type = self.function_return_type(&function.name().to_string());

                self.check_block(function.body().block(), return_type.as_ref());
            }
            ast::Stmt::NumericFor(for_stmt) => self.check_block(for_stmt.block(), current_return),
            ast::Stmt::Repeat(repeat) => self.check_block(repeat.block(), current_return),
            ast::Stmt::While(while_stmt) => self.check_block(while_stmt.block(), current_return),
            _ => {}
        }
    }

    fn check_local_assignment(&mut self, local: &ast::LocalAssignment) {
        for ((name, specifier), expression) in local
            .names()
            .iter()
            .zip(local.type_specifiers())
            .zip(local.expressions().iter())
        {
            if specifier.is_none() {
                continue;
            }

            let name = token_text(name);
            let Some(target) = self.environment.local(&name).cloned() else {
                continue;
            };

            self.check_expression_assignable(expression, &target);
        }
    }

    fn check_assignment(&mut self, assignment: &ast::Assignment) {
        for (variable, expression) in assignment.variables().iter().zip(assignment.expressions()) {
            let Some(target) = self.infer_var_assignment_target(variable) else {
                continue;
            };

            self.check_expression_assignable(expression, &target);
        }
    }

    fn check_expression_assignable(&mut self, expression: &Expression, target: &Type) {
        let Some(source) = self.infer_expression(expression) else {
            return;
        };

        if !self.context.is_assignable(&source, target) {
            self.diagnostics.push(mismatch_diagnostic(
                &self.parsed.path,
                expression,
                target,
                &source,
            ));
        }
    }

    fn check_return(&mut self, return_stmt: &Return, target: Option<&Type>) {
        let Some(target) = target else {
            return;
        };

        let Some(expression) = return_stmt.returns().iter().next() else {
            if !self.context.is_assignable(&Type::nil(), target) {
                self.diagnostics.push(mismatch_diagnostic(
                    &self.parsed.path,
                    return_stmt,
                    target,
                    &Type::nil(),
                ));
            }
            return;
        };

        self.check_expression_assignable(expression, target);
    }

    fn infer_expression(&mut self, expression: &Expression) -> Option<Type> {
        match expression {
            Expression::Number(_) => Some(Type::number()),
            Expression::String(_) => Some(Type::string()),
            Expression::Symbol(symbol) => infer_symbol_type(&token_text(symbol)),
            Expression::TableConstructor(table) => Some(self.infer_table_constructor(table)),
            Expression::Parentheses { expression, .. } => self.infer_expression(expression),
            Expression::TypeAssertion { expression, .. } => self.infer_expression(expression),
            Expression::FunctionCall(function_call) => self.check_function_call(function_call),
            Expression::Var(var) => self.infer_var_value(var),
            _ => None,
        }
    }

    fn infer_table_constructor(&mut self, table: &TableConstructor) -> Type {
        Type::table(table.fields().iter().filter_map(|field| match field {
            Field::NameKey { key, value, .. } => {
                let ty = self.infer_expression(value).unwrap_or(Type::Any);

                Some((token_text(key), TableField::required(ty)))
            }
            _ => None,
        }))
    }

    fn infer_var_value(&mut self, var: &Var) -> Option<Type> {
        match var {
            Var::Name(name) => self.infer_named_value(&token_text(name)),
            Var::Expression(expression) => {
                self.resolve_field_chain_type(&plain_field_chain_from_var_expression(expression)?)
            }
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

    fn check_function_call(&mut self, function_call: &FunctionCall) -> Option<Type> {
        let callee = self.infer_call_target(function_call)?;

        match callee {
            Callable::Any => Some(Type::Any),
            Callable::Function(function) => {
                self.check_call_arguments(function_call, &function);

                Some(*function.return_type)
            }
        }
    }

    fn infer_call_target(&self, function_call: &FunctionCall) -> Option<Callable> {
        let parts = field_chain_before_first_call(function_call)?;
        let (root, fields) = parts.split_first()?;

        if fields.is_empty() {
            if let Some(local) = self.environment.local(root) {
                return callable_from_type(local, self.environment);
            }

            if let Some(function) = self.environment.function(root) {
                return Some(Callable::from_typed_function(function));
            }
        }

        let callee_type = self.resolve_field_chain_type(&parts)?;

        callable_from_type(&callee_type, self.environment)
    }

    fn check_call_arguments(&mut self, function_call: &FunctionCall, function: &FunctionType) {
        let arguments = parenthesized_arguments(function_call).unwrap_or_default();

        if arguments.len() != function.parameters.len() {
            self.diagnostics.push(argument_count_diagnostic(
                &self.parsed.path,
                function_call,
                function.parameters.len(),
                arguments.len(),
            ));
            return;
        }

        for (argument, parameter) in arguments.into_iter().zip(&function.parameters) {
            self.check_expression_assignable(argument, parameter);
        }
    }

    fn function_return_type(&self, name: &str) -> Option<Type> {
        self.environment
            .function(name.trim())
            .and_then(|function| function.return_type.clone())
    }

    fn infer_var_assignment_target(&self, var: &Var) -> Option<Type> {
        match var {
            Var::Name(name) => self.environment.local(&token_text(name)).cloned(),
            Var::Expression(expression) => {
                self.resolve_field_chain_type(&plain_field_chain_from_var_expression(expression)?)
            }
            _ => None,
        }
    }

    fn resolve_field_chain_type(&self, parts: &[String]) -> Option<Type> {
        let (root, fields) = parts.split_first()?;
        let mut current = self.environment.local(root)?.clone();

        for field in fields {
            current = self.resolve_table_field_type(&current, field)?;
        }

        Some(current)
    }

    fn resolve_table_field_type(&self, ty: &Type, field: &str) -> Option<Type> {
        match resolve_aliases(ty, self.environment, 0) {
            Type::Table(table) => table.field(field).map(|field| field.ty.clone()),
            Type::Union(types) => {
                let mut resolved = Vec::new();

                for ty in types {
                    if let Some(field_type) = self.resolve_table_field_type(&ty, field) {
                        resolved.push(field_type);
                    }
                }

                match resolved.len() {
                    0 => None,
                    1 => resolved.into_iter().next(),
                    _ => Some(Type::union(resolved)),
                }
            }
            _ => None,
        }
    }
}

enum Callable {
    Any,
    Function(FunctionType),
}

impl Callable {
    fn from_typed_function(function: &TypedFunction) -> Self {
        if function.has_fully_annotated_parameters() {
            Self::Function(FunctionType::new(
                function
                    .parameters
                    .iter()
                    .map(|parameter| parameter.ty.clone()),
                function.return_type_or_any(),
            ))
        } else {
            Self::Any
        }
    }
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

fn callable_from_type(ty: &Type, environment: &TypeEnvironment) -> Option<Callable> {
    match resolve_aliases(ty, environment, 0) {
        Type::Any => Some(Callable::Any),
        Type::Function(function) => Some(Callable::Function(function)),
        _ => None,
    }
}

fn resolve_aliases(ty: &Type, environment: &TypeEnvironment, depth: usize) -> Type {
    if depth >= MAX_TYPE_RESOLUTION_DEPTH {
        return ty.clone();
    }

    match ty {
        Type::Alias(name) => environment
            .alias(name)
            .map(|resolved| resolve_aliases(resolved, environment, depth + 1))
            .unwrap_or_else(|| ty.clone()),
        _ => ty.clone(),
    }
}

fn argument_count_diagnostic(
    path: &Path,
    node: &impl Node,
    expected: usize,
    found: usize,
) -> Diagnostic {
    let position = node_position(node);

    Diagnostic::error(
        path,
        position.line,
        position.column,
        format!("argument count mismatch: expected {expected}, found {found}"),
    )
    .with_code(DiagnosticCode::TYPE_ARGUMENT_COUNT)
}

fn mismatch_diagnostic(path: &Path, node: &impl Node, expected: &Type, found: &Type) -> Diagnostic {
    let position = node_position(node);

    Diagnostic::error(
        path,
        position.line,
        position.column,
        format!("type mismatch: expected {expected}, found {found}"),
    )
    .with_code(DiagnosticCode::TYPE_MISMATCH)
}

fn node_position(node: &impl Node) -> SourcePosition {
    node.range()
        .map(|(start, _)| SourcePosition::from_full_moon(start))
        .unwrap_or(SourcePosition { line: 1, column: 1 })
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

#[cfg(test)]
mod tests;
