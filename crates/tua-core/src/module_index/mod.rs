use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use full_moon::ast::{
    self, Expression, Field, FunctionArgs, Index, LastStmt, Prefix, Suffix, TableConstructor, Var,
};
use full_moon::node::Node;

use crate::parser::{LuaDocAnnotation, LuaDocAnnotationKind};
use crate::project_model::ProjectModel;
use crate::semantic::resolve_aliases;
use crate::types::{build_type_environment, TableField, Type, TypeEnvironment, TypedFunction};
use crate::{parse_lua_source, parse_tua_source, ParsedSource, SourceSpan};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleExports {
    pub path: PathBuf,
    pub fields: BTreeMap<String, ModuleExportField>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleExportField {
    pub ty: Type,
    pub documentation: Option<String>,
    pub parameter_documentation: Vec<Option<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequiredModule {
    pub module: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequireBinding {
    pub alias: String,
    pub module: String,
}

impl ModuleExports {
    pub fn field(&self, name: &str) -> Option<&Type> {
        self.fields.get(name).map(|field| &field.ty)
    }

    pub fn field_documentation(&self, name: &str) -> Option<&str> {
        self.fields
            .get(name)
            .and_then(|field| field.documentation.as_deref())
    }
}

pub fn require_bindings_for_parsed_source(parsed: &ParsedSource) -> Vec<RequireBinding> {
    require_bindings(parsed)
        .into_iter()
        .map(|(alias, module)| RequireBinding { alias, module })
        .collect()
}

pub fn required_module_for_receiver(
    parsed: &ParsedSource,
    receiver: &str,
) -> Option<RequiredModule> {
    let module = require_bindings(parsed).remove(receiver)?;
    let path = resolve_required_module(&parsed.path, &module)?;

    Some(RequiredModule { module, path })
}

pub fn required_module_exports_for_receiver(
    parsed: &ParsedSource,
    receiver: &str,
) -> Option<ModuleExports> {
    let required = required_module_for_receiver(parsed, receiver)?;
    let source = fs::read_to_string(&required.path).ok()?;
    let parsed = parse_module_source(&required.path, source).ok()?;

    infer_module_exports(&parsed)
}

pub fn required_module_export_type_for_fields(
    parsed: &ParsedSource,
    receiver: &str,
    fields: &[String],
) -> Option<Type> {
    required_module_export_for_fields(parsed, receiver, fields).map(|field| field.ty)
}

pub fn required_module_export_for_fields(
    parsed: &ParsedSource,
    receiver: &str,
    fields: &[String],
) -> Option<ModuleExportField> {
    let exports = required_module_exports_for_receiver(parsed, receiver)?;

    exported_field_for_fields(&exports, fields)
}

pub fn exported_field_for_fields(
    exports: &ModuleExports,
    fields: &[String],
) -> Option<ModuleExportField> {
    let (first, rest) = fields.split_first()?;
    let field = exports.fields.get(first)?;

    if rest.is_empty() {
        return Some(field.clone());
    }

    export_field_type(&field.ty, rest).map(|ty| ModuleExportField {
        ty,
        documentation: None,
        parameter_documentation: Vec::new(),
    })
}

pub fn exported_type_for_fields(exports: &ModuleExports, fields: &[String]) -> Option<Type> {
    exported_field_for_fields(exports, fields).map(|field| field.ty)
}

pub fn resolve_required_module(current_path: &Path, module: &str) -> Option<PathBuf> {
    ProjectModel::load_for_path(current_path)
        .ok()
        .and_then(|model| model.resolve_required_module(current_path, module))
}

fn export_field_type(ty: &Type, fields: &[String]) -> Option<Type> {
    let mut current = ty.clone();

    for field in fields {
        current = export_table_field_type(&current, field)?;
    }

    Some(current)
}

fn export_table_field_type(ty: &Type, field: &str) -> Option<Type> {
    match ty {
        Type::Table(table) => table.semantic_field(field).map(|field| field.ty.clone()),
        Type::Union(types) => {
            let mut resolved = Vec::new();

            for ty in types {
                if let Some(field_ty) = export_table_field_type(ty, field) {
                    resolved.push(field_ty);
                }
            }

            match resolved.len() {
                0 => None,
                1 => resolved.into_iter().next(),
                _ => Some(Type::union(resolved)),
            }
        }
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => None,
    }
}

pub fn infer_module_exports(parsed: &ParsedSource) -> Option<ModuleExports> {
    let mut inferer = ExportInferer::new(parsed);

    inferer.collect_top_level();
    inferer.exports()
}

fn parse_module_source(
    path: &Path,
    source: String,
) -> Result<ParsedSource, Vec<crate::Diagnostic>> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("lua") => parse_lua_source(path, source),
        _ => parse_tua_source(path, source),
    }
}

fn require_bindings(parsed: &ParsedSource) -> BTreeMap<String, String> {
    let mut bindings = BTreeMap::new();

    for stmt in parsed.ast.nodes().stmts() {
        let ast::Stmt::LocalAssignment(local) = stmt else {
            continue;
        };

        for (name, expression) in local.names().iter().zip(local.expressions()) {
            let Some(module) = require_module_from_expression(expression) else {
                continue;
            };

            bindings.insert(token_text(name), module);
        }
    }

    bindings
}

fn require_module_from_expression(expression: &Expression) -> Option<String> {
    let Expression::FunctionCall(call) = expression else {
        return None;
    };
    let Prefix::Name(name) = call.prefix() else {
        return None;
    };

    if token_text(name) != "require" {
        return None;
    }

    let first_call = call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;
    let ast::Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) = first_call else {
        return None;
    };
    let Expression::String(module) = arguments.iter().next()? else {
        return None;
    };

    Some(unquote_string(&module.to_string()))
}

struct ExportInferer<'a> {
    parsed: &'a ParsedSource,
    environment: Option<TypeEnvironment>,
    locals: BTreeMap<String, Type>,
    function_docs: BTreeMap<String, String>,
    function_parameter_docs: BTreeMap<String, Vec<Option<String>>>,
    table_members: BTreeMap<String, BTreeMap<String, ModuleExportField>>,
    return_expr: Option<&'a Expression>,
}

impl<'a> ExportInferer<'a> {
    fn new(parsed: &'a ParsedSource) -> Self {
        Self {
            parsed,
            environment: build_type_environment(parsed).ok(),
            locals: BTreeMap::new(),
            function_docs: BTreeMap::new(),
            function_parameter_docs: BTreeMap::new(),
            table_members: BTreeMap::new(),
            return_expr: None,
        }
    }

    fn collect_top_level(&mut self) {
        if let Some(environment) = &self.environment {
            for (name, ty) in environment.locals() {
                self.locals
                    .insert(name.to_owned(), self.resolve_export_type(ty));
            }

            for function in environment.functions() {
                let Some(name) = &function.name else {
                    continue;
                };
                let ty = typed_function_type(function);

                if let Some((table, field)) = name.split_once('.') {
                    self.table_members
                        .entry(table.trim().to_owned())
                        .or_default()
                        .insert(
                            field.trim().to_owned(),
                            ModuleExportField {
                                ty,
                                documentation: function.documentation.clone(),
                                parameter_documentation: parameter_documentation(function),
                            },
                        );
                } else {
                    self.locals.entry(name.clone()).or_insert(ty);

                    if let Some(documentation) = &function.documentation {
                        self.function_docs
                            .entry(name.clone())
                            .or_insert_with(|| documentation.clone());
                    }

                    self.function_parameter_docs
                        .entry(name.clone())
                        .or_insert_with(|| parameter_documentation(function));
                }
            }
        }

        for stmt in self.parsed.ast.nodes().stmts() {
            self.collect_stmt(stmt);
        }

        if let Some(LastStmt::Return(return_stmt)) = self.parsed.ast.nodes().last_stmt() {
            self.return_expr = return_stmt.returns().iter().next();
        }
    }

    fn collect_stmt(&mut self, stmt: &'a ast::Stmt) {
        match stmt {
            ast::Stmt::Assignment(assignment) => {
                for (var, expression) in assignment.variables().iter().zip(assignment.expressions())
                {
                    let Some(parts) = plain_field_chain_from_var(var) else {
                        continue;
                    };
                    self.collect_table_member_assignment(&parts, expression);
                }
            }
            ast::Stmt::FunctionDeclaration(function) => {
                let parts = function
                    .name()
                    .to_string()
                    .split('.')
                    .map(|part| part.trim().to_owned())
                    .collect::<Vec<_>>();

                if let [table, field] = parts.as_slice() {
                    self.table_members
                        .entry(table.clone())
                        .or_default()
                        .entry(field.clone())
                        .or_insert_with(|| ModuleExportField {
                            ty: function_type_from_body(function.body()),
                            documentation: None,
                            parameter_documentation: Vec::new(),
                        });
                }
            }
            ast::Stmt::LocalAssignment(local) => {
                for (name, expression) in local.names().iter().zip(local.expressions()) {
                    let name = token_text(name);

                    let inferred = self.infer_expression(expression);

                    self.locals.entry(name).or_insert(inferred);
                }
            }
            ast::Stmt::LocalFunction(function) => {
                self.locals
                    .entry(token_text(function.name()))
                    .or_insert_with(|| function_type_from_body(function.body()));
            }
            _ => {}
        }
    }

    fn collect_table_member_assignment(&mut self, parts: &[String], expression: &Expression) {
        let [table, field] = parts else {
            return;
        };
        let ty = self.infer_expression(expression);

        self.table_members.entry(table.clone()).or_default().insert(
            field.clone(),
            ModuleExportField {
                ty,
                documentation: None,
                parameter_documentation: Vec::new(),
            },
        );
    }

    fn exports(&mut self) -> Option<ModuleExports> {
        let expression = self.return_expr?;
        let fields = match expression {
            Expression::TableConstructor(table) => self.infer_table_fields(table),
            Expression::Var(Var::Name(name)) => self.fields_for_returned_local(&token_text(name)),
            Expression::Parentheses { expression, .. } => match expression.as_ref() {
                Expression::TableConstructor(table) => self.infer_table_fields(table),
                Expression::Var(Var::Name(name)) => {
                    self.fields_for_returned_local(&token_text(name))
                }
                _ => BTreeMap::new(),
            },
            _ => BTreeMap::new(),
        };

        (!fields.is_empty()).then(|| ModuleExports {
            path: self.parsed.path.clone(),
            fields,
        })
    }

    fn fields_for_returned_local(&mut self, name: &str) -> BTreeMap<String, ModuleExportField> {
        let mut fields = BTreeMap::new();

        if let Some(ty) = self.locals.get(name).cloned() {
            collect_table_fields(&self.resolve_export_type(&ty), &mut fields);
        }

        if let Some(members) = self.table_members.get(name) {
            fields.extend(members.clone());
        }

        fields
    }

    fn infer_table_fields(
        &mut self,
        table: &TableConstructor,
    ) -> BTreeMap<String, ModuleExportField> {
        table
            .fields()
            .iter()
            .filter_map(|field| {
                let Field::NameKey { key, value, .. } = field else {
                    return None;
                };
                let name = token_text(key);
                let lua_doc_export = lua_doc_function_export_for_field(self.parsed, field);
                let documentation = field_documentation(self.parsed, field).or_else(|| {
                    let Expression::Var(Var::Name(name)) = value else {
                        return None;
                    };

                    self.function_docs.get(&token_text(name)).cloned()
                });
                let parameter_documentation = lua_doc_export
                    .as_ref()
                    .map(|export| export.parameter_documentation.clone())
                    .or_else(|| {
                        let Expression::Var(Var::Name(name)) = value else {
                            return None;
                        };

                        self.function_parameter_docs.get(&token_text(name)).cloned()
                    })
                    .unwrap_or_default();
                let ty = lua_doc_export
                    .map(|export| export.ty)
                    .unwrap_or_else(|| self.infer_expression(value));

                Some((
                    name,
                    ModuleExportField {
                        ty,
                        documentation,
                        parameter_documentation,
                    },
                ))
            })
            .collect()
    }

    fn infer_expression(&mut self, expression: &Expression) -> Type {
        match expression {
            Expression::Number(_) => Type::number(),
            Expression::String(_) => Type::string(),
            Expression::Symbol(symbol) => match token_text(symbol).as_str() {
                "true" | "false" => Type::boolean(),
                "nil" => Type::nil(),
                _ => Type::Any,
            },
            Expression::TableConstructor(table) => Type::table(
                self.infer_table_fields(table)
                    .into_iter()
                    .map(|(name, field)| (name, TableField::required(field.ty))),
            ),
            Expression::Function(function) => function_type_from_body(function.body()),
            Expression::Parentheses { expression, .. } => self.infer_expression(expression),
            Expression::Var(Var::Name(name)) => self
                .locals
                .get(&token_text(name))
                .cloned()
                .map(|ty| self.resolve_export_type(&ty))
                .unwrap_or(Type::Any),
            _ => Type::Any,
        }
    }

    fn resolve_export_type(&self, ty: &Type) -> Type {
        let Some(environment) = &self.environment else {
            return ty.clone();
        };

        resolve_export_aliases(environment, ty)
    }
}

fn collect_table_fields(ty: &Type, fields: &mut BTreeMap<String, ModuleExportField>) {
    match ty {
        Type::Table(table) => {
            fields.extend(table.fields().map(|(name, field)| {
                (
                    name.to_owned(),
                    ModuleExportField {
                        ty: field.ty.clone(),
                        documentation: None,
                        parameter_documentation: Vec::new(),
                    },
                )
            }));
        }
        Type::Union(types) => {
            for ty in types {
                collect_table_fields(ty, fields);
            }
        }
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => {}
    }
}

fn resolve_export_aliases(environment: &TypeEnvironment, ty: &Type) -> Type {
    match resolve_aliases(environment, ty, 0) {
        Type::Table(table) => Type::table(table.fields().map(|(name, field)| {
            (
                name.to_owned(),
                TableField {
                    ty: resolve_export_aliases(environment, &field.ty),
                    optional: field.optional,
                },
            )
        })),
        Type::Union(types) => Type::union(
            types
                .iter()
                .map(|ty| resolve_export_aliases(environment, ty)),
        ),
        resolved => resolved,
    }
}

fn lua_doc_function_export_for_field(
    parsed: &ParsedSource,
    field: &Field,
) -> Option<ModuleExportField> {
    let Field::NameKey { key, value, .. } = field else {
        return None;
    };
    let body = function_body_from_expression(value)?;
    let docs = lua_doc_annotations_for_field(parsed, &token_text(key), field);

    if !docs.iter().any(|annotation| {
        matches!(
            annotation.kind,
            LuaDocAnnotationKind::Param(_) | LuaDocAnnotationKind::Return(_)
        )
    }) {
        return None;
    }

    let param_annotations = lua_doc_param_annotations(&docs);
    let parameters = body
        .parameters()
        .iter()
        .filter_map(|parameter| {
            let ast::Parameter::Name(name) = parameter else {
                return None;
            };

            let name = token_text(name);
            Some(
                param_annotations
                    .get(&name)
                    .map(|annotation| type_from_lua_doc_type(&annotation.0.ty))
                    .unwrap_or(Type::Any),
            )
        })
        .collect::<Vec<_>>();
    let parameter_documentation = body
        .parameters()
        .iter()
        .filter_map(|parameter| {
            let ast::Parameter::Name(name) = parameter else {
                return None;
            };

            Some(
                param_annotations
                    .get(&token_text(name))
                    .and_then(|annotation| annotation.1.clone()),
            )
        })
        .collect::<Vec<_>>();
    let return_types = docs
        .iter()
        .find_map(|annotation| match &annotation.kind {
            LuaDocAnnotationKind::Return(return_annotation) => Some(
                return_annotation
                    .returns
                    .iter()
                    .map(|return_type| type_from_lua_doc_type(&return_type.ty))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .filter(|returns| !returns.is_empty())
        .unwrap_or_else(|| vec![Type::Any]);

    Some(ModuleExportField {
        ty: Type::function_returns(parameters, return_types),
        documentation: field_documentation(parsed, field),
        parameter_documentation,
    })
}

fn field_documentation(parsed: &ParsedSource, field: &Field) -> Option<String> {
    let Field::NameKey { key, .. } = field else {
        return None;
    };
    let docs = lua_doc_annotations_for_field(parsed, &token_text(key), field);
    let documentation = docs
        .iter()
        .filter_map(|annotation| annotation.documentation.as_deref())
        .collect::<Vec<_>>();

    (!documentation.is_empty()).then(|| documentation.join("\n"))
}

fn lua_doc_annotations_for_field<'a>(
    parsed: &'a ParsedSource,
    name: &str,
    field: &Field,
) -> Vec<&'a LuaDocAnnotation> {
    let docs = node_span(field)
        .and_then(|span| first_code_line_span(parsed, span))
        .map(|span| leading_lua_doc_annotations(parsed, span.start.line))
        .unwrap_or_default();

    if !docs.is_empty() {
        return docs;
    }

    lua_doc_annotations_for_code_field(parsed, name)
}

fn lua_doc_annotations_for_code_field<'a>(
    parsed: &'a ParsedSource,
    name: &str,
) -> Vec<&'a LuaDocAnnotation> {
    let mut by_line = BTreeMap::new();

    for annotation in &parsed.lua_doc_annotations {
        by_line.insert(annotation.span.start.line, annotation);
    }

    let mut pending = Vec::new();

    for (index, line) in parsed.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim_start();

        if let Some(annotation) = by_line.get(&line_number) {
            pending.push(*annotation);
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("--") {
            pending.clear();
            continue;
        }

        if pending.is_empty() {
            continue;
        }

        let matched = field_name_from_code_line(trimmed)
            .as_deref()
            .is_some_and(|field_name| field_name == name);

        if matched {
            return pending;
        }

        pending.clear();
    }

    Vec::new()
}

fn field_name_from_code_line(code: &str) -> Option<String> {
    let end = code
        .char_indices()
        .find_map(|(index, ch)| (!is_lua_name_char(ch)).then_some(index))
        .unwrap_or(code.len());

    if end == 0 {
        return None;
    }

    let rest = code[end..].trim_start();

    rest.starts_with('=').then(|| code[..end].to_owned())
}

fn is_lua_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn function_body_from_expression(expression: &Expression) -> Option<&ast::FunctionBody> {
    match expression {
        Expression::Function(function) => Some(function.body()),
        Expression::Parentheses { expression, .. } => function_body_from_expression(expression),
        _ => None,
    }
}

fn lua_doc_param_annotations<'a>(
    docs: &[&'a LuaDocAnnotation],
) -> BTreeMap<String, (&'a crate::LuaDocParam, Option<String>)> {
    docs.iter()
        .filter_map(|annotation| match &annotation.kind {
            LuaDocAnnotationKind::Param(param) => Some((
                param.name.clone(),
                (param, annotation.documentation.clone()),
            )),
            _ => None,
        })
        .collect()
}

fn type_from_lua_doc_type(ty: &crate::TuaType) -> Type {
    match ty {
        crate::TuaType::Named(name) => Type::named(name),
        crate::TuaType::StringLiteral(value) => Type::string_literal(value),
        crate::TuaType::Union(types) => Type::union(types.iter().map(type_from_lua_doc_type)),
        crate::TuaType::Table(fields) => Type::table(fields.iter().map(|field| {
            (
                field.name.clone(),
                TableField {
                    ty: type_from_lua_doc_type(&field.ty),
                    optional: field.optional,
                },
            )
        })),
        crate::TuaType::Array(item) => Type::array(type_from_lua_doc_type(item)),
        crate::TuaType::Tuple(items) => Type::tuple(items.iter().map(type_from_lua_doc_type)),
        crate::TuaType::Dictionary { key, value } => {
            Type::dictionary(type_from_lua_doc_type(key), type_from_lua_doc_type(value))
        }
        crate::TuaType::Function {
            parameters,
            returns,
        } => Type::function_returns(
            parameters.iter().map(|parameter| {
                let ty = type_from_lua_doc_type(&parameter.ty);

                if parameter.optional {
                    Type::union([ty, Type::nil()])
                } else {
                    ty
                }
            }),
            returns.iter().map(type_from_lua_doc_type),
        ),
    }
}

fn leading_lua_doc_annotations(parsed: &ParsedSource, line: usize) -> Vec<&LuaDocAnnotation> {
    if line <= 1 {
        return Vec::new();
    }

    let lines = parsed.source.lines().collect::<Vec<_>>();
    let mut current = line - 1;
    let mut first_doc_line = None;

    while current > 0 {
        let text = lines.get(current - 1).copied().unwrap_or("").trim_start();

        if text.is_empty() {
            current -= 1;
            continue;
        }

        if text.starts_with("---") {
            first_doc_line = Some(current);
            current -= 1;
            continue;
        }

        break;
    }

    let first = first_doc_line.unwrap_or(line);

    parsed
        .lua_doc_annotations
        .iter()
        .filter(|annotation| annotation.span.start.line >= first && annotation.span.end.line < line)
        .collect()
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn first_code_line_span(parsed: &ParsedSource, span: SourceSpan) -> Option<SourceSpan> {
    for line in span.start.line..=span.end.line.max(span.start.line) {
        let text = parsed.source.lines().nth(line.saturating_sub(1))?;
        let trimmed = text.trim_start();

        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }

        let leading = text.len() - trimmed.len();

        return Some(SourceSpan {
            start: crate::SourcePosition {
                line,
                column: leading + 1,
            },
            end: crate::SourcePosition {
                line,
                column: text.len() + 1,
            },
        });
    }

    None
}

fn typed_function_type(function: &TypedFunction) -> Type {
    let parameters = if function.has_fully_annotated_parameters() {
        function
            .parameters
            .iter()
            .map(|parameter| parameter.ty.clone())
            .collect::<Vec<_>>()
    } else {
        vec![Type::Any; function.parameter_count]
    };

    Type::function_returns(parameters, function.return_types_or_any())
}

fn parameter_documentation(function: &TypedFunction) -> Vec<Option<String>> {
    function
        .parameters
        .iter()
        .map(|parameter| parameter.documentation.clone())
        .collect()
}

fn function_type_from_body(body: &ast::FunctionBody) -> Type {
    let parameter_count = body
        .parameters()
        .iter()
        .filter(|parameter| matches!(parameter, ast::Parameter::Name(_)))
        .count();

    Type::function(vec![Type::Any; parameter_count], Type::Any)
}

fn plain_field_chain_from_var(var: &Var) -> Option<Vec<String>> {
    match var {
        Var::Expression(expression) => plain_field_chain_from_var_expression(expression),
        Var::Name(name) => Some(vec![token_text(name)]),
        _ => None,
    }
}

fn plain_field_chain_from_var_expression(expression: &ast::VarExpression) -> Option<Vec<String>> {
    let Prefix::Name(name) = expression.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in expression.suffixes() {
        let Suffix::Index(Index::Dot { name, .. }) = suffix else {
            return None;
        };

        parts.push(token_text(name));
    }

    Some(parts)
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

fn unquote_string(value: &str) -> String {
    value.trim().trim_matches('"').trim_matches('\'').to_owned()
}

#[cfg(test)]
mod tests;
