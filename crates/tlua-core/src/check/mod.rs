use std::fmt;
use std::path::Path;

use full_moon::ast::{self, Expression, Field, Index, Prefix, Suffix, TableConstructor, Var};
use full_moon::node::Node;

use crate::parser::{parse_tlua_source, ParsedSource, SourcePosition};
use crate::types::{build_type_environment, TableField, Type, TypeContext, TypeEnvironment};
use crate::{Diagnostic, DiagnosticCode};

const MAX_TYPE_RESOLUTION_DEPTH: usize = 64;

pub fn check_tlua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<(), Vec<Diagnostic>> {
    let parsed = parse_tlua_source(path, source)?;

    check_parsed_source(&parsed)
}

pub fn check_parsed_source(parsed: &ParsedSource) -> Result<(), Vec<Diagnostic>> {
    let environment = build_type_environment(parsed)?;
    let mut checker = Checker::new(parsed, &environment);

    checker.check_block(parsed.ast.nodes());

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

    fn check_block(&mut self, block: &ast::Block) {
        for stmt in block.stmts() {
            self.check_stmt(stmt);
        }
    }

    fn check_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Assignment(assignment) => self.check_assignment(assignment),
            ast::Stmt::LocalAssignment(local) => self.check_local_assignment(local),
            ast::Stmt::Do(block) => self.check_block(block.block()),
            ast::Stmt::If(if_stmt) => {
                self.check_block(if_stmt.block());
                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        self.check_block(else_if.block());
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    self.check_block(else_block);
                }
            }
            ast::Stmt::GenericFor(for_stmt) => self.check_block(for_stmt.block()),
            ast::Stmt::LocalFunction(function) => self.check_block(function.body().block()),
            ast::Stmt::FunctionDeclaration(function) => self.check_block(function.body().block()),
            ast::Stmt::NumericFor(for_stmt) => self.check_block(for_stmt.block()),
            ast::Stmt::Repeat(repeat) => self.check_block(repeat.block()),
            ast::Stmt::While(while_stmt) => self.check_block(while_stmt.block()),
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

    fn infer_expression(&self, expression: &Expression) -> Option<Type> {
        match expression {
            Expression::Number(_) => Some(Type::number()),
            Expression::String(_) => Some(Type::string()),
            Expression::Symbol(symbol) => infer_symbol_type(&token_text(symbol)),
            Expression::TableConstructor(table) => Some(self.infer_table_constructor(table)),
            Expression::Parentheses { expression, .. } => self.infer_expression(expression),
            Expression::TypeAssertion { expression, .. } => self.infer_expression(expression),
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
            Var::Name(name) => self.environment.local(&token_text(name)).cloned(),
            Var::Expression(expression) => {
                self.resolve_field_chain_type(&plain_field_chain_from_var_expression(expression)?)
            }
            _ => None,
        }
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

impl fmt::Display for Type {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Primitive(primitive) => write!(formatter, "{primitive}"),
            Type::Any => write!(formatter, "any"),
            Type::Alias(name) => write!(formatter, "{name}"),
            Type::Union(types) => {
                for (index, ty) in types.iter().enumerate() {
                    if index > 0 {
                        write!(formatter, " | ")?;
                    }
                    write!(formatter, "{ty}")?;
                }
                Ok(())
            }
            Type::Function(function) => {
                write!(formatter, "function(")?;
                for (index, parameter) in function.parameters.iter().enumerate() {
                    if index > 0 {
                        write!(formatter, ", ")?;
                    }
                    write!(formatter, "{parameter}")?;
                }
                write!(formatter, "): {}", function.return_type)
            }
            Type::Table(table) => {
                write!(formatter, "{{ ")?;
                for (index, (name, field)) in table.fields().enumerate() {
                    if index > 0 {
                        write!(formatter, ", ")?;
                    }
                    if field.optional {
                        write!(formatter, "{name}?: {}", field.ty)?;
                    } else {
                        write!(formatter, "{name}: {}", field.ty)?;
                    }
                }
                write!(formatter, " }}")
            }
        }
    }
}

impl fmt::Display for crate::PrimitiveType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number => write!(formatter, "number"),
            Self::String => write!(formatter, "string"),
            Self::Boolean => write!(formatter, "boolean"),
            Self::Nil => write!(formatter, "nil"),
        }
    }
}

#[cfg(test)]
mod tests;
