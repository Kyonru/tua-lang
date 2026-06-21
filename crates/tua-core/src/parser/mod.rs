mod model;
mod normalize;

use std::path::Path;

use full_moon::ast::luau::{TypeDeclaration, TypeField, TypeFieldKey, TypeInfo};
use full_moon::ast::{self, LuaVersion};
use full_moon::node::Node;

use crate::{Diagnostic, DiagnosticCode};
pub use model::{
    CompoundAssignment, CompoundOperator, FunctionAnnotation, LocalAnnotation, ParameterAnnotation,
    ParsedSource, SourcePosition, SourceSpan, TableFieldType, TuaType, TypeAlias,
};
use normalize::normalize_tua_source_for_luau_parser;

pub fn parse_lua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    parse_source_with_version(path, source, LuaVersion::lua51(), false)
}

pub fn parse_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    parse_source_with_version(path, source, LuaVersion::luau(), true)
}

fn parse_source_with_version(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    lua_version: LuaVersion,
    collect_types: bool,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    let path = path.as_ref().to_path_buf();
    let source = source.into();

    let parser_source = if collect_types {
        normalize_tua_source_for_luau_parser(&source)
    } else {
        source.clone()
    };

    match full_moon::parse_fallible(&parser_source, lua_version).into_result() {
        Ok(ast) => {
            let span = ast
                .range()
                .map(|(start, end)| SourceSpan::from_range(start, end));
            let syntax = if collect_types {
                collect_tua_syntax(&path, &ast)
            } else {
                TuaSyntax::default()
            };

            if !syntax.diagnostics.is_empty() {
                return Err(syntax.diagnostics);
            }

            Ok(ParsedSource {
                path,
                source,
                ast,
                span,
                type_aliases: syntax.type_aliases,
                local_annotations: syntax.local_annotations,
                function_annotations: syntax.function_annotations,
                compound_assignments: syntax.compound_assignments,
            })
        }
        Err(errors) => Err(errors
            .into_iter()
            .map(|error| parser_error_to_diagnostic(&path, error))
            .collect()),
    }
}

#[derive(Default)]
struct TuaSyntax {
    type_aliases: Vec<TypeAlias>,
    local_annotations: Vec<LocalAnnotation>,
    function_annotations: Vec<FunctionAnnotation>,
    compound_assignments: Vec<CompoundAssignment>,
    diagnostics: Vec<Diagnostic>,
}

fn collect_tua_syntax(path: &Path, ast: &ast::Ast) -> TuaSyntax {
    let mut syntax = TuaSyntax::default();

    collect_block_tua_syntax(path, ast.nodes(), &mut syntax);

    syntax
}

fn collect_block_tua_syntax(path: &Path, block: &ast::Block, syntax: &mut TuaSyntax) {
    for stmt in block.stmts() {
        collect_stmt_tua_syntax(path, stmt, syntax);
    }
}

fn collect_stmt_tua_syntax(path: &Path, stmt: &ast::Stmt, syntax: &mut TuaSyntax) {
    match stmt {
        ast::Stmt::TypeDeclaration(declaration) => {
            syntax
                .type_aliases
                .push(type_alias_from_declaration(declaration));
        }
        ast::Stmt::LocalAssignment(local) => {
            for (name, specifier) in local.names().iter().zip(local.type_specifiers()) {
                let Some(specifier) = specifier else {
                    continue;
                };

                syntax.local_annotations.push(LocalAnnotation {
                    name: token_text(name),
                    ty: tua_type_from_type_info(specifier.type_info()),
                    name_span: node_span(name),
                    span: node_span(specifier),
                });
            }
        }
        ast::Stmt::LocalFunction(function) => {
            syntax.function_annotations.push(function_annotation(
                Some(token_text(function.name())),
                node_span(function.name()),
                function.body(),
            ));
            collect_block_tua_syntax(path, function.body().block(), syntax);
        }
        ast::Stmt::FunctionDeclaration(function) => {
            syntax.function_annotations.push(function_annotation(
                Some(function.name().to_string()),
                node_span(function.name()),
                function.body(),
            ));
            collect_block_tua_syntax(path, function.body().block(), syntax);
        }
        ast::Stmt::CompoundAssignment(assignment) => {
            match compound_assignment_from_ast(path, assignment) {
                Ok(assignment) => syntax.compound_assignments.push(assignment),
                Err(diagnostic) => syntax.diagnostics.push(diagnostic),
            }
        }
        ast::Stmt::Do(block) => collect_block_tua_syntax(path, block.block(), syntax),
        ast::Stmt::If(if_stmt) => {
            collect_block_tua_syntax(path, if_stmt.block(), syntax);
            if let Some(else_ifs) = if_stmt.else_if() {
                for else_if in else_ifs {
                    collect_block_tua_syntax(path, else_if.block(), syntax);
                }
            }
            if let Some(else_block) = if_stmt.else_block() {
                collect_block_tua_syntax(path, else_block, syntax);
            }
        }
        ast::Stmt::GenericFor(for_stmt) => {
            collect_block_tua_syntax(path, for_stmt.block(), syntax);
        }
        ast::Stmt::NumericFor(for_stmt) => {
            collect_block_tua_syntax(path, for_stmt.block(), syntax);
        }
        ast::Stmt::Repeat(repeat) => collect_block_tua_syntax(path, repeat.block(), syntax),
        ast::Stmt::While(while_stmt) => {
            collect_block_tua_syntax(path, while_stmt.block(), syntax);
        }
        _ => {}
    }
}

fn type_alias_from_declaration(declaration: &TypeDeclaration) -> TypeAlias {
    TypeAlias {
        name: token_text(declaration.type_name()),
        ty: tua_type_from_type_info(declaration.type_definition()),
        name_span: node_span(declaration.type_name()),
        span: node_span(declaration),
    }
}

fn function_annotation(
    name: Option<String>,
    name_span: Option<SourceSpan>,
    body: &ast::FunctionBody,
) -> FunctionAnnotation {
    let parameter_count = body
        .parameters()
        .iter()
        .filter(|parameter| matches!(parameter, ast::Parameter::Name(_)))
        .count();
    let has_varargs = body
        .parameters()
        .iter()
        .any(|parameter| matches!(parameter, ast::Parameter::Ellipsis(_)));
    let parameters = body
        .parameters()
        .iter()
        .zip(body.type_specifiers())
        .filter_map(|(parameter, specifier)| {
            let specifier = specifier?;
            let ast::Parameter::Name(name) = parameter else {
                return None;
            };

            Some(ParameterAnnotation {
                name: token_text(name),
                ty: tua_type_from_type_info(specifier.type_info()),
                name_span: node_span(name),
                span: node_span(specifier),
            })
        })
        .collect();

    FunctionAnnotation {
        name,
        name_span,
        parameters,
        parameter_count,
        has_varargs,
        return_type: body
            .return_type()
            .map(|specifier| tua_type_from_type_info(specifier.type_info())),
        span: node_span(body),
    }
}

fn compound_assignment_from_ast(
    path: &Path,
    assignment: &ast::CompoundAssignment,
) -> Result<CompoundAssignment, Diagnostic> {
    let operator = compound_operator_from_ast(assignment.compound_operator()).ok_or_else(|| {
        diagnostic_for_node(
            path,
            assignment.compound_operator(),
            "unsupported compound assignment operator: expected one of +=, -=, *=, /=, %=",
        )
    })?;

    let lhs = plain_field_chain_from_var(assignment.lhs()).ok_or_else(|| {
        diagnostic_for_node(
            path,
            assignment.lhs(),
            "invalid compound assignment left-hand side: expected a name or plain field chain",
        )
    })?;

    Ok(CompoundAssignment {
        lhs,
        operator,
        rhs: assignment.rhs().to_string().trim().to_owned(),
        span: node_span(assignment),
    })
}

fn compound_operator_from_ast(operator: &ast::CompoundOp) -> Option<CompoundOperator> {
    match operator {
        ast::CompoundOp::PlusEqual(_) => Some(CompoundOperator::Add),
        ast::CompoundOp::MinusEqual(_) => Some(CompoundOperator::Subtract),
        ast::CompoundOp::StarEqual(_) => Some(CompoundOperator::Multiply),
        ast::CompoundOp::SlashEqual(_) => Some(CompoundOperator::Divide),
        ast::CompoundOp::PercentEqual(_) => Some(CompoundOperator::Modulo),
        _ => None,
    }
}

fn plain_field_chain_from_var(var: &ast::Var) -> Option<Vec<String>> {
    match var {
        ast::Var::Name(name) => Some(vec![token_text(name)]),
        ast::Var::Expression(expression) => plain_field_chain_from_var_expression(expression),
        _ => None,
    }
}

fn plain_field_chain_from_var_expression(expression: &ast::VarExpression) -> Option<Vec<String>> {
    let ast::Prefix::Name(name) = expression.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in expression.suffixes() {
        let ast::Suffix::Index(index) = suffix else {
            return None;
        };

        let ast::Index::Dot { name, .. } = index else {
            return None;
        };

        parts.push(token_text(name));
    }

    Some(parts)
}

fn tua_type_from_type_info(type_info: &TypeInfo) -> TuaType {
    match type_info {
        TypeInfo::Basic(token) => TuaType::Named(token_text(token)),
        TypeInfo::Optional { base, .. } => TuaType::Union(vec![
            tua_type_from_type_info(base),
            TuaType::Named("nil".to_owned()),
        ]),
        TypeInfo::Union(union) => {
            TuaType::Union(union.types().iter().map(tua_type_from_type_info).collect())
        }
        TypeInfo::Table { fields, .. } => TuaType::Table(
            fields
                .iter()
                .filter_map(table_field_type_from_type_field)
                .collect(),
        ),
        other => TuaType::Named(other.to_string()),
    }
}

fn table_field_type_from_type_field(field: &TypeField) -> Option<TableFieldType> {
    let key = match field.key() {
        TypeFieldKey::Name(key) => key,
        _ => return None,
    };

    let mut name = token_text(key);
    let key_optional = name.ends_with('?');

    if key_optional {
        name.pop();
    }

    let (value_optional, ty) = table_field_value_type(field.value());

    Some(TableFieldType {
        name,
        optional: key_optional || value_optional,
        ty,
        name_span: node_span(key),
        span: node_span(field),
    })
}

fn table_field_value_type(type_info: &TypeInfo) -> (bool, TuaType) {
    match type_info {
        TypeInfo::Optional { base, .. } => (true, tua_type_from_type_info(base)),
        _ => (false, tua_type_from_type_info(type_info)),
    }
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn parser_error_to_diagnostic(path: &Path, error: full_moon::Error) -> Diagnostic {
    let (start, _) = error.range();
    let position = SourcePosition::from_full_moon(start);

    Diagnostic::error(
        path,
        position.line,
        position.column,
        error.error_message().into_owned(),
    )
    .with_code(DiagnosticCode::SYNTAX_PARSE)
}

fn diagnostic_for_node(path: &Path, node: &impl Node, message: impl Into<String>) -> Diagnostic {
    let position = node
        .range()
        .map(|(start, _)| SourcePosition::from_full_moon(start))
        .unwrap_or(SourcePosition { line: 1, column: 1 });

    Diagnostic::error(path, position.line, position.column, message)
        .with_code(DiagnosticCode::SYNTAX_COMPOUND_ASSIGNMENT)
}

#[cfg(test)]
mod tests;
