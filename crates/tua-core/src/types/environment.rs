use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use full_moon::ast::{self, Expression, Field, FunctionCall, Index, LastStmt, Prefix, Suffix, Var};
use full_moon::node::Node;

use crate::module_index::required_module_export_type_for_fields;
use crate::parser::{
    FunctionAnnotation as ParsedFunctionAnnotation, LuaDocAnnotation, LuaDocAnnotationKind,
    LuaDocParam, LuaDocReturnType, ParsedSource, SourceSpan,
    TableFieldType as ParsedTableFieldType, TuaType,
};
use crate::{Diagnostic, DiagnosticCode};

use super::model::{TableField, Type, TypeAlias};
use super::TypeContext;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeEnvironment {
    path: PathBuf,
    aliases: BTreeMap<String, Type>,
    alias_docs: BTreeMap<String, String>,
    locals: BTreeMap<String, Type>,
    local_docs: BTreeMap<String, String>,
    functions: Vec<TypedFunction>,
}

impl TypeEnvironment {
    pub fn from_parsed(parsed: &ParsedSource) -> Result<Self, Vec<Diagnostic>> {
        build_type_environment(parsed)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn alias(&self, name: &str) -> Option<&Type> {
        self.aliases.get(name)
    }

    pub fn alias_documentation(&self, name: &str) -> Option<&str> {
        self.alias_docs.get(name).map(String::as_str)
    }

    pub fn aliases(&self) -> impl Iterator<Item = (&str, &Type)> {
        self.aliases.iter().map(|(name, ty)| (name.as_str(), ty))
    }

    pub fn local(&self, name: &str) -> Option<&Type> {
        self.locals.get(name)
    }

    pub fn local_documentation(&self, name: &str) -> Option<&str> {
        self.local_docs.get(name).map(String::as_str)
    }

    pub fn locals(&self) -> impl Iterator<Item = (&str, &Type)> {
        self.locals.iter().map(|(name, ty)| (name.as_str(), ty))
    }

    pub fn functions(&self) -> &[TypedFunction] {
        &self.functions
    }

    pub fn function(&self, name: &str) -> Option<&TypedFunction> {
        self.functions
            .iter()
            .find(|function| function.name.as_deref() == Some(name))
    }

    pub fn to_type_context(&self) -> TypeContext {
        self.aliases
            .iter()
            .fold(TypeContext::new(), |context, (name, ty)| {
                context.with_alias(TypeAlias::new(name.clone(), ty.clone()))
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedFunction {
    pub name: Option<String>,
    pub parameters: Vec<TypedParameter>,
    pub parameter_count: usize,
    pub has_varargs: bool,
    pub return_type: Option<Type>,
    pub documentation: Option<String>,
}

impl TypedFunction {
    pub fn has_fully_annotated_parameters(&self) -> bool {
        !self.has_varargs && self.parameters.len() == self.parameter_count
    }

    pub fn return_type_or_any(&self) -> Type {
        self.return_type.clone().unwrap_or(Type::Any)
    }

    pub fn as_type(&self) -> Option<Type> {
        self.has_fully_annotated_parameters().then(|| {
            Type::function(
                self.parameters.iter().map(|parameter| parameter.ty.clone()),
                self.return_type_or_any(),
            )
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedParameter {
    pub name: String,
    pub ty: Type,
    pub documentation: Option<String>,
}

pub fn build_type_environment(parsed: &ParsedSource) -> Result<TypeEnvironment, Vec<Diagnostic>> {
    let (environment, diagnostics) = build_type_environment_with_diagnostics(parsed);

    if diagnostics.is_empty() {
        Ok(environment)
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn build_type_environment_with_diagnostics(
    parsed: &ParsedSource,
) -> (TypeEnvironment, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let alias_names = collect_alias_names(parsed, &mut diagnostics);
    let doc_aliases = lua_doc_alias_declarations(parsed, &alias_names, &mut diagnostics);
    let alias_names = alias_names
        .into_iter()
        .chain(doc_aliases.iter().map(|alias| alias.name.clone()))
        .collect::<BTreeSet<_>>();
    let mut aliases = BTreeMap::new();
    let mut alias_docs = BTreeMap::new();
    let mut locals = BTreeMap::new();
    let mut local_docs = BTreeMap::new();
    let mut functions = Vec::new();

    for alias in &parsed.type_aliases {
        if aliases.contains_key(&alias.name) {
            continue;
        }

        let ty = resolve_tua_type(
            &parsed.path,
            &alias.ty,
            alias.span,
            &alias_names,
            &mut diagnostics,
        );
        aliases.insert(alias.name.clone(), ty);
    }

    for alias in &doc_aliases {
        if aliases.contains_key(&alias.name) {
            continue;
        }

        let ty = resolve_tua_type(
            &parsed.path,
            &alias.ty,
            Some(alias.span),
            &alias_names,
            &mut diagnostics,
        );

        aliases.insert(alias.name.clone(), ty);

        if let Some(documentation) = &alias.documentation {
            alias_docs.insert(alias.name.clone(), documentation.clone());
        }
    }

    for local in &parsed.local_annotations {
        let ty = resolve_tua_type(
            &parsed.path,
            &local.ty,
            local.span,
            &alias_names,
            &mut diagnostics,
        );
        locals.insert(local.name.clone(), ty);
    }

    for function in &parsed.function_annotations {
        functions.push(resolve_function_annotation(
            &parsed.path,
            function,
            &alias_names,
            &mut diagnostics,
        ));
    }

    let mut lua_doc_state = LuaDocLoweringState {
        locals: &mut locals,
        local_docs: &mut local_docs,
        functions: &mut functions,
        diagnostics: &mut diagnostics,
    };
    lower_lua_doc_annotations_into_environment(parsed, &alias_names, &mut lua_doc_state);

    let inferred_returns = infer_function_return_types(parsed, &aliases, &locals, &functions);

    for function in &mut functions {
        if function.return_type.is_some() {
            continue;
        }

        let Some(name) = &function.name else {
            continue;
        };

        function.return_type = inferred_returns.get(name).cloned();
    }

    (
        TypeEnvironment {
            path: parsed.path.clone(),
            aliases,
            alias_docs,
            locals,
            local_docs,
            functions,
        },
        diagnostics,
    )
}

fn collect_alias_names(
    parsed: &ParsedSource,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for alias in &parsed.type_aliases {
        if !names.insert(alias.name.clone()) {
            diagnostics.push(type_diagnostic(
                &parsed.path,
                alias.span,
                format!("duplicate type alias `{}`", alias.name),
                DiagnosticCode::TYPE_DUPLICATE_ALIAS,
            ));
        }
    }

    names
}

fn resolve_function_annotation(
    path: &Path,
    function: &ParsedFunctionAnnotation,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> TypedFunction {
    let parameters = function
        .parameters
        .iter()
        .map(|parameter| TypedParameter {
            name: parameter.name.clone(),
            ty: resolve_tua_type(
                path,
                &parameter.ty,
                parameter.span,
                alias_names,
                diagnostics,
            ),
            documentation: None,
        })
        .collect();
    let return_type = function
        .return_type
        .as_ref()
        .map(|ty| resolve_tua_type(path, ty, function.span, alias_names, diagnostics));

    TypedFunction {
        name: function.name.clone(),
        parameters,
        parameter_count: function.parameter_count,
        has_varargs: function.has_varargs,
        return_type,
        documentation: None,
    }
}

#[derive(Clone, Debug)]
struct LuaDocAliasDeclaration {
    name: String,
    ty: TuaType,
    span: SourceSpan,
    documentation: Option<String>,
}

fn lua_doc_alias_declarations(
    parsed: &ParsedSource,
    tua_alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<LuaDocAliasDeclaration> {
    let mut declarations = Vec::new();
    let mut names = tua_alias_names.clone();
    let mut current_class: Option<LuaDocClassBuilder> = None;

    for annotation in &parsed.lua_doc_annotations {
        match &annotation.kind {
            LuaDocAnnotationKind::Class(class) => {
                flush_lua_doc_class(
                    parsed,
                    &mut names,
                    &mut current_class,
                    &mut declarations,
                    diagnostics,
                );
                current_class = Some(LuaDocClassBuilder {
                    name: class.name.clone(),
                    span: annotation.span,
                    documentation: annotation.documentation.clone(),
                    fields: Vec::new(),
                });
            }
            LuaDocAnnotationKind::Field(field) => {
                if let Some(class) = &mut current_class {
                    class.fields.push(field.clone());
                }
            }
            LuaDocAnnotationKind::Alias(alias) => {
                flush_lua_doc_class(
                    parsed,
                    &mut names,
                    &mut current_class,
                    &mut declarations,
                    diagnostics,
                );

                if !names.insert(alias.name.clone()) {
                    diagnostics.push(type_diagnostic(
                        &parsed.path,
                        Some(annotation.span),
                        format!("duplicate type alias `{}`", alias.name),
                        DiagnosticCode::TYPE_DUPLICATE_ALIAS,
                    ));
                    continue;
                }

                declarations.push(LuaDocAliasDeclaration {
                    name: alias.name.clone(),
                    ty: alias.ty.clone(),
                    span: annotation.span,
                    documentation: annotation.documentation.clone(),
                });
            }
            _ => {
                flush_lua_doc_class(
                    parsed,
                    &mut names,
                    &mut current_class,
                    &mut declarations,
                    diagnostics,
                );
            }
        }
    }

    flush_lua_doc_class(
        parsed,
        &mut names,
        &mut current_class,
        &mut declarations,
        diagnostics,
    );

    declarations
}

#[derive(Clone, Debug)]
struct LuaDocClassBuilder {
    name: String,
    span: SourceSpan,
    documentation: Option<String>,
    fields: Vec<crate::LuaDocField>,
}

fn flush_lua_doc_class(
    parsed: &ParsedSource,
    names: &mut BTreeSet<String>,
    current: &mut Option<LuaDocClassBuilder>,
    declarations: &mut Vec<LuaDocAliasDeclaration>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(class) = current.take() else {
        return;
    };

    if !names.insert(class.name.clone()) {
        diagnostics.push(type_diagnostic(
            &parsed.path,
            Some(class.span),
            format!("duplicate type alias `{}`", class.name),
            DiagnosticCode::TYPE_DUPLICATE_ALIAS,
        ));
        return;
    }

    declarations.push(LuaDocAliasDeclaration {
        name: class.name,
        ty: TuaType::Table(
            class
                .fields
                .into_iter()
                .map(|field| ParsedTableFieldType {
                    name: field.name,
                    optional: field.optional,
                    ty: field.ty,
                    name_span: Some(field.name_span),
                    span: Some(field.type_span),
                })
                .collect(),
        ),
        span: class.span,
        documentation: class.documentation,
    });
}

struct LuaDocLoweringState<'a> {
    locals: &'a mut BTreeMap<String, Type>,
    local_docs: &'a mut BTreeMap<String, String>,
    functions: &'a mut Vec<TypedFunction>,
    diagnostics: &'a mut Vec<Diagnostic>,
}

fn lower_lua_doc_annotations_into_environment(
    parsed: &ParsedSource,
    alias_names: &BTreeSet<String>,
    state: &mut LuaDocLoweringState<'_>,
) {
    for stmt in parsed.ast.nodes().stmts() {
        let Some(stmt_span) = stmt_code_span(parsed, stmt).or_else(|| node_span(stmt)) else {
            continue;
        };
        let docs = leading_lua_doc_annotations(parsed, stmt_span.start.line);

        if docs.is_empty() {
            collect_nested_lua_doc_annotations(parsed, stmt, alias_names, state);
            continue;
        }

        lower_lua_doc_annotations_for_stmt(parsed, stmt, stmt_span, &docs, alias_names, state);
        collect_nested_lua_doc_annotations(parsed, stmt, alias_names, state);
    }

    lower_line_based_lua_doc_annotations_into_environment(parsed, alias_names, state);
}

fn collect_nested_lua_doc_annotations(
    parsed: &ParsedSource,
    stmt: &ast::Stmt,
    alias_names: &BTreeSet<String>,
    state: &mut LuaDocLoweringState<'_>,
) {
    match stmt {
        ast::Stmt::LocalFunction(function) => {
            lower_lua_doc_annotations_in_block(parsed, function.body().block(), alias_names, state);
        }
        ast::Stmt::FunctionDeclaration(function) => {
            lower_lua_doc_annotations_in_block(parsed, function.body().block(), alias_names, state);
        }
        ast::Stmt::Do(block) => {
            lower_lua_doc_annotations_in_block(parsed, block.block(), alias_names, state);
        }
        ast::Stmt::If(if_stmt) => {
            lower_lua_doc_annotations_in_block(parsed, if_stmt.block(), alias_names, state);

            if let Some(else_ifs) = if_stmt.else_if() {
                for else_if in else_ifs {
                    lower_lua_doc_annotations_in_block(parsed, else_if.block(), alias_names, state);
                }
            }

            if let Some(else_block) = if_stmt.else_block() {
                lower_lua_doc_annotations_in_block(parsed, else_block, alias_names, state);
            }
        }
        ast::Stmt::GenericFor(for_stmt) => {
            lower_lua_doc_annotations_in_block(parsed, for_stmt.block(), alias_names, state);
        }
        ast::Stmt::NumericFor(for_stmt) => {
            lower_lua_doc_annotations_in_block(parsed, for_stmt.block(), alias_names, state);
        }
        ast::Stmt::Repeat(repeat) => {
            lower_lua_doc_annotations_in_block(parsed, repeat.block(), alias_names, state);
        }
        ast::Stmt::While(while_stmt) => {
            lower_lua_doc_annotations_in_block(parsed, while_stmt.block(), alias_names, state);
        }
        _ => {}
    }
}

fn lower_lua_doc_annotations_in_block(
    parsed: &ParsedSource,
    block: &ast::Block,
    alias_names: &BTreeSet<String>,
    state: &mut LuaDocLoweringState<'_>,
) {
    for stmt in block.stmts() {
        let Some(stmt_span) = stmt_code_span(parsed, stmt).or_else(|| node_span(stmt)) else {
            continue;
        };
        let docs = leading_lua_doc_annotations(parsed, stmt_span.start.line);

        if !docs.is_empty() {
            lower_lua_doc_annotations_for_stmt(parsed, stmt, stmt_span, &docs, alias_names, state);
        }

        collect_nested_lua_doc_annotations(parsed, stmt, alias_names, state);
    }
}

fn lower_lua_doc_annotations_for_stmt(
    parsed: &ParsedSource,
    stmt: &ast::Stmt,
    stmt_span: SourceSpan,
    docs: &[&LuaDocAnnotation],
    alias_names: &BTreeSet<String>,
    state: &mut LuaDocLoweringState<'_>,
) {
    if let Some((name, ty, documentation)) =
        lua_doc_local_for_stmt(parsed, stmt, docs, alias_names, state.diagnostics)
    {
        merge_doc_local(parsed, &name, ty, documentation, stmt_span, state);
    }

    if let Some(function) =
        lua_doc_function_for_stmt(parsed, stmt, docs, alias_names, state.diagnostics)
    {
        merge_doc_function(parsed, function, stmt_span, state);
    }
}

fn lower_line_based_lua_doc_annotations_into_environment(
    parsed: &ParsedSource,
    alias_names: &BTreeSet<String>,
    state: &mut LuaDocLoweringState<'_>,
) {
    for group in lua_doc_groups(parsed) {
        if let Some((name, ty, documentation)) =
            lua_doc_local_for_code_line(parsed, &group, alias_names, state.diagnostics)
        {
            merge_doc_local(parsed, &name, ty, documentation, group.span, state);
        }

        if let Some(function) =
            lua_doc_function_for_code_line(parsed, &group, alias_names, state.diagnostics)
        {
            merge_doc_function(parsed, function, group.span, state);
        }
    }
}

struct LuaDocGroup<'a> {
    annotations: Vec<&'a LuaDocAnnotation>,
    code: &'a str,
    span: SourceSpan,
}

fn lua_doc_groups(parsed: &ParsedSource) -> Vec<LuaDocGroup<'_>> {
    let mut by_line = BTreeMap::new();

    for annotation in &parsed.lua_doc_annotations {
        by_line.insert(annotation.span.start.line, annotation);
    }

    let mut groups = Vec::new();
    let mut pending = Vec::new();
    let mut pending_span = None;

    for (index, line) in parsed.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim_start();

        if let Some(annotation) = by_line.get(&line_number) {
            pending_span.get_or_insert(annotation.span);
            pending.push(*annotation);
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("--") {
            pending.clear();
            pending_span = None;
            continue;
        }

        if !pending.is_empty() {
            let leading = line.len() - trimmed.len();
            groups.push(LuaDocGroup {
                annotations: std::mem::take(&mut pending),
                code: trimmed,
                span: pending_span.unwrap_or(SourceSpan {
                    start: crate::SourcePosition {
                        line: line_number,
                        column: leading + 1,
                    },
                    end: crate::SourcePosition {
                        line: line_number,
                        column: line.len() + 1,
                    },
                }),
            });
            pending_span = None;
        }
    }

    groups
}

fn lua_doc_local_for_code_line(
    parsed: &ParsedSource,
    group: &LuaDocGroup<'_>,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(String, Type, Option<String>)> {
    let type_annotation =
        group
            .annotations
            .iter()
            .find_map(|annotation| match &annotation.kind {
                LuaDocAnnotationKind::Type(type_annotation) => Some((*annotation, type_annotation)),
                _ => None,
            })?;
    let name = annotated_value_name(group.code)?;
    let ty = resolve_tua_type(
        &parsed.path,
        &type_annotation.1.ty,
        Some(type_annotation.1.type_span),
        alias_names,
        diagnostics,
    );

    Some((name, ty, type_annotation.0.documentation.clone()))
}

fn lua_doc_function_for_code_line(
    parsed: &ParsedSource,
    group: &LuaDocGroup<'_>,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<TypedFunction> {
    if !group.annotations.iter().any(|annotation| {
        matches!(
            annotation.kind,
            LuaDocAnnotationKind::Param(_) | LuaDocAnnotationKind::Return(_)
        )
    }) {
        return None;
    }

    let name = annotated_function_name(group.code)?;
    let parameter_names = group
        .annotations
        .iter()
        .filter_map(|annotation| match &annotation.kind {
            LuaDocAnnotationKind::Param(param) => Some(param.name.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let param_annotations = lua_doc_param_annotations(&group.annotations);
    let parameters = parameter_names
        .into_iter()
        .map(|name| {
            let doc = param_annotations.get(&name);
            let ty = doc
                .map(|annotation| {
                    resolve_tua_type(
                        &parsed.path,
                        &annotation.0.ty,
                        Some(annotation.0.type_span),
                        alias_names,
                        diagnostics,
                    )
                })
                .unwrap_or(Type::Any);

            TypedParameter {
                name,
                ty,
                documentation: doc.and_then(|annotation| annotation.1.clone()),
            }
        })
        .collect::<Vec<_>>();
    let return_type = group
        .annotations
        .iter()
        .find_map(|annotation| match &annotation.kind {
            LuaDocAnnotationKind::Return(return_annotation) => return_annotation
                .returns
                .first()
                .map(|ty| resolve_lua_doc_return_type(&parsed.path, ty, alias_names, diagnostics)),
            _ => None,
        });

    Some(TypedFunction {
        name: Some(name),
        parameter_count: parameters.len(),
        parameters,
        has_varargs: false,
        return_type,
        documentation: lua_doc_documentation(&group.annotations),
    })
}

fn annotated_value_name(code: &str) -> Option<String> {
    let code = code.strip_prefix("local ").unwrap_or(code).trim_start();
    let end = code
        .char_indices()
        .find_map(|(index, ch)| (!is_name_char(ch) && ch != '.').then_some(index))
        .unwrap_or(code.len());

    (end > 0).then(|| code[..end].trim().to_owned())
}

fn annotated_function_name(code: &str) -> Option<String> {
    let code = code.trim_start();

    if let Some(rest) = code.strip_prefix("local function ") {
        return take_name(rest);
    }

    if let Some(rest) = code.strip_prefix("function ") {
        return take_name(rest);
    }

    if let Some(rest) = code.strip_prefix("local ") {
        let (name, value) = rest.split_once('=')?;

        if value.trim_start().starts_with("function") {
            return Some(name.trim().to_owned());
        }
    }

    let (name, value) = code.split_once('=')?;

    value
        .trim_start()
        .starts_with("function")
        .then(|| name.trim().to_owned())
}

fn take_name(rest: &str) -> Option<String> {
    let end = rest
        .char_indices()
        .find_map(|(index, ch)| (!is_name_char(ch) && ch != '.').then_some(index))
        .unwrap_or(rest.len());

    (end > 0).then(|| rest[..end].trim().to_owned())
}

fn is_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn lua_doc_local_for_stmt(
    parsed: &ParsedSource,
    stmt: &ast::Stmt,
    docs: &[&LuaDocAnnotation],
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(String, Type, Option<String>)> {
    let type_annotation = docs.iter().find_map(|annotation| match &annotation.kind {
        LuaDocAnnotationKind::Type(type_annotation) => Some((annotation, type_annotation)),
        _ => None,
    })?;
    let name = match stmt {
        ast::Stmt::LocalAssignment(local) => local.names().iter().next().map(token_text),
        ast::Stmt::Assignment(assignment) => assignment
            .variables()
            .iter()
            .next()
            .and_then(plain_field_chain_from_var)
            .map(|parts| parts.join(".")),
        _ => None,
    }?;
    let ty = resolve_tua_type(
        &parsed.path,
        &type_annotation.1.ty,
        Some(type_annotation.1.type_span),
        alias_names,
        diagnostics,
    );

    Some((name, ty, type_annotation.0.documentation.clone()))
}

fn lua_doc_function_for_stmt(
    parsed: &ParsedSource,
    stmt: &ast::Stmt,
    docs: &[&LuaDocAnnotation],
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<TypedFunction> {
    match stmt {
        ast::Stmt::LocalFunction(function) => typed_function_from_lua_doc_body(
            parsed,
            Some(token_text(function.name())),
            function.body(),
            docs,
            alias_names,
            diagnostics,
        ),
        ast::Stmt::FunctionDeclaration(function) => typed_function_from_lua_doc_body(
            parsed,
            Some(function.name().to_string()),
            function.body(),
            docs,
            alias_names,
            diagnostics,
        ),
        ast::Stmt::LocalAssignment(local) => {
            let name = local.names().iter().next().map(token_text)?;
            let body = local
                .expressions()
                .iter()
                .next()
                .and_then(function_body_from_expression)?;

            typed_function_from_lua_doc_body(
                parsed,
                Some(name),
                body,
                docs,
                alias_names,
                diagnostics,
            )
        }
        ast::Stmt::Assignment(assignment) => {
            let name = assignment
                .variables()
                .iter()
                .next()
                .and_then(plain_field_chain_from_var)
                .map(|parts| parts.join("."))?;
            let body = assignment
                .expressions()
                .iter()
                .next()
                .and_then(function_body_from_expression)?;

            typed_function_from_lua_doc_body(
                parsed,
                Some(name),
                body,
                docs,
                alias_names,
                diagnostics,
            )
        }
        _ => None,
    }
}

fn function_body_from_expression(expression: &Expression) -> Option<&ast::FunctionBody> {
    match expression {
        Expression::Function(function) => Some(function.body()),
        Expression::Parentheses { expression, .. } => function_body_from_expression(expression),
        _ => None,
    }
}

pub(crate) fn typed_function_from_lua_doc_body(
    parsed: &ParsedSource,
    name: Option<String>,
    body: &ast::FunctionBody,
    docs: &[&LuaDocAnnotation],
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<TypedFunction> {
    let param_annotations = lua_doc_param_annotations(docs);
    let return_annotation = docs.iter().find_map(|annotation| match &annotation.kind {
        LuaDocAnnotationKind::Return(return_annotation) => return_annotation
            .returns
            .first()
            .map(|ty| (*annotation, ty)),
        _ => None,
    });

    if param_annotations.is_empty() && return_annotation.is_none() {
        return None;
    }

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
        .filter_map(|parameter| {
            let ast::Parameter::Name(name) = parameter else {
                return None;
            };
            let name = token_text(name);
            let doc = param_annotations.get(&name);
            let ty = doc
                .map(|annotation| {
                    resolve_tua_type(
                        &parsed.path,
                        &annotation.0.ty,
                        Some(annotation.0.type_span),
                        alias_names,
                        diagnostics,
                    )
                })
                .unwrap_or(Type::Any);

            Some(TypedParameter {
                name,
                ty,
                documentation: doc.and_then(|annotation| annotation.1.clone()),
            })
        })
        .collect::<Vec<_>>();
    let return_type = return_annotation
        .map(|(_, ty)| resolve_lua_doc_return_type(&parsed.path, ty, alias_names, diagnostics));
    let documentation = lua_doc_documentation(docs);

    Some(TypedFunction {
        name,
        parameters,
        parameter_count,
        has_varargs,
        return_type,
        documentation,
    })
}

fn lua_doc_param_annotations<'a>(
    docs: &'a [&'a LuaDocAnnotation],
) -> BTreeMap<String, (&'a LuaDocParam, Option<String>)> {
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

fn resolve_lua_doc_return_type(
    path: &Path,
    ty: &LuaDocReturnType,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Type {
    resolve_tua_type(path, &ty.ty, Some(ty.type_span), alias_names, diagnostics)
}

fn lua_doc_documentation(docs: &[&LuaDocAnnotation]) -> Option<String> {
    let documentation = docs
        .iter()
        .filter_map(|annotation| annotation.documentation.as_deref())
        .collect::<Vec<_>>();

    (!documentation.is_empty()).then(|| documentation.join("\n"))
}

fn merge_doc_local(
    parsed: &ParsedSource,
    name: &str,
    ty: Type,
    documentation: Option<String>,
    span: SourceSpan,
    state: &mut LuaDocLoweringState<'_>,
) {
    if let Some(existing) = state.locals.get(name) {
        if existing != &ty {
            push_doc_conflict(
                state.diagnostics,
                doc_conflict_diagnostic(
                    &parsed.path,
                    span,
                    format!("conflicting annotations for `{name}`: `{existing}` vs `{ty}`"),
                ),
            );
        }
    } else {
        state.locals.insert(name.to_owned(), ty);
    }

    if let Some(documentation) = documentation {
        state
            .local_docs
            .entry(name.to_owned())
            .or_insert(documentation);
    }
}

fn merge_doc_function(
    parsed: &ParsedSource,
    function: TypedFunction,
    span: SourceSpan,
    state: &mut LuaDocLoweringState<'_>,
) {
    let Some(name) = function.name.as_deref() else {
        state.functions.push(function);
        return;
    };
    let Some(existing) = state
        .functions
        .iter_mut()
        .find(|existing| existing.name.as_deref() == Some(name))
    else {
        state.functions.push(function);
        return;
    };

    if existing.parameter_count != function.parameter_count {
        push_doc_conflict(
            state.diagnostics,
            doc_conflict_diagnostic(
                &parsed.path,
                span,
                format!("conflicting annotations for `{name}`: parameter count differs"),
            ),
        );
    }

    for (index, parameter) in function.parameters.into_iter().enumerate() {
        let Some(existing_parameter) = existing.parameters.get_mut(index) else {
            existing.parameters.push(parameter);
            continue;
        };

        if existing_parameter.ty != Type::Any
            && parameter.ty != Type::Any
            && existing_parameter.ty != parameter.ty
        {
            push_doc_conflict(
                state.diagnostics,
                doc_conflict_diagnostic(
                    &parsed.path,
                    span,
                    format!(
                        "conflicting annotations for `{name}` parameter `{}`: `{}` vs `{}`",
                        existing_parameter.name, existing_parameter.ty, parameter.ty
                    ),
                ),
            );
            continue;
        }

        if existing_parameter.ty == Type::Any {
            existing_parameter.ty = parameter.ty;
        }

        if existing_parameter.documentation.is_none() {
            existing_parameter.documentation = parameter.documentation;
        }
    }

    if let Some(return_type) = function.return_type {
        match &existing.return_type {
            Some(existing_return) if existing_return != &return_type => {
                push_doc_conflict(
                    state.diagnostics,
                    doc_conflict_diagnostic(
                        &parsed.path,
                        span,
                        format!(
                            "conflicting annotations for `{name}` return type: `{existing_return}` vs `{return_type}`"
                        ),
                    ),
                );
            }
            Some(_) => {}
            None => existing.return_type = Some(return_type),
        }
    }

    if existing.documentation.is_none() {
        existing.documentation = function.documentation;
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

fn stmt_code_span(parsed: &ParsedSource, stmt: &ast::Stmt) -> Option<SourceSpan> {
    if let Some(span) = node_span(stmt).and_then(|span| first_code_line_span(parsed, span)) {
        return Some(span);
    }

    let span = match stmt {
        ast::Stmt::LocalAssignment(local) => local.names().iter().next().and_then(node_span),
        ast::Stmt::Assignment(assignment) => {
            assignment.variables().iter().next().and_then(node_span)
        }
        ast::Stmt::LocalFunction(function) => node_span(function.name()),
        ast::Stmt::FunctionDeclaration(function) => node_span(function.name()),
        ast::Stmt::CompoundAssignment(assignment) => node_span(assignment.lhs()),
        _ => node_span(stmt),
    }?;

    first_code_line_span(parsed, span).or(Some(span))
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

fn plain_field_chain_from_var(var: &Var) -> Option<Vec<String>> {
    match var {
        Var::Name(name) => Some(vec![token_text(name)]),
        Var::Expression(expression) => plain_field_chain(expression),
        _ => None,
    }
}

fn doc_conflict_diagnostic(
    path: &Path,
    span: SourceSpan,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(path, span.start.line, span.start.column, message)
        .with_code(DiagnosticCode::TYPE_DOC_CONFLICT)
}

fn push_doc_conflict(diagnostics: &mut Vec<Diagnostic>, diagnostic: Diagnostic) {
    if diagnostics.iter().any(|existing| {
        existing.file == diagnostic.file
            && existing.code == diagnostic.code
            && existing.message == diagnostic.message
    }) {
        return;
    }

    diagnostics.push(diagnostic);
}

fn infer_function_return_types(
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    functions: &[TypedFunction],
) -> BTreeMap<String, Type> {
    let mut returns = BTreeMap::new();
    let parameters = function_parameters_by_name(functions);
    let function_types = function_types_by_name(functions);

    collect_function_return_types(
        parsed.ast.nodes(),
        parsed,
        aliases,
        locals,
        &function_types,
        &parameters,
        &mut returns,
    );

    returns
}

fn function_parameters_by_name(
    functions: &[TypedFunction],
) -> BTreeMap<String, Vec<TypedParameter>> {
    functions
        .iter()
        .filter_map(|function| {
            let name = function.name.as_ref()?;

            Some((name.clone(), function.parameters.clone()))
        })
        .collect()
}

fn function_types_by_name(functions: &[TypedFunction]) -> BTreeMap<String, Type> {
    functions
        .iter()
        .filter_map(|function| Some((function.name.as_ref()?.clone(), function.as_type()?)))
        .collect()
}

fn collect_function_return_types(
    block: &ast::Block,
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    parameters: &BTreeMap<String, Vec<TypedParameter>>,
    returns: &mut BTreeMap<String, Type>,
) {
    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::LocalFunction(function) => {
                let name = token_text(function.name());
                let function_locals = locals_with_function_parameters(locals, parameters, &name);

                if let Some(return_type) = infer_block_return_type(
                    function.body().block(),
                    parsed,
                    aliases,
                    &function_locals,
                    function_types,
                    &name,
                ) {
                    returns.insert(name, return_type);
                }
                collect_function_return_types(
                    function.body().block(),
                    parsed,
                    aliases,
                    &function_locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            ast::Stmt::FunctionDeclaration(function) => {
                let name = function.name().to_string();
                let function_locals = locals_with_function_parameters(locals, parameters, &name);

                if let Some(return_type) = infer_block_return_type(
                    function.body().block(),
                    parsed,
                    aliases,
                    &function_locals,
                    function_types,
                    &name,
                ) {
                    returns.insert(name, return_type);
                }
                collect_function_return_types(
                    function.body().block(),
                    parsed,
                    aliases,
                    &function_locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            ast::Stmt::Do(block) => {
                collect_function_return_types(
                    block.block(),
                    parsed,
                    aliases,
                    locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            ast::Stmt::GenericFor(for_stmt) => {
                collect_function_return_types(
                    for_stmt.block(),
                    parsed,
                    aliases,
                    locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            ast::Stmt::If(if_stmt) => {
                collect_function_return_types(
                    if_stmt.block(),
                    parsed,
                    aliases,
                    locals,
                    function_types,
                    parameters,
                    returns,
                );

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        collect_function_return_types(
                            else_if.block(),
                            parsed,
                            aliases,
                            locals,
                            function_types,
                            parameters,
                            returns,
                        );
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    collect_function_return_types(
                        else_block,
                        parsed,
                        aliases,
                        locals,
                        function_types,
                        parameters,
                        returns,
                    );
                }
            }
            ast::Stmt::NumericFor(for_stmt) => {
                collect_function_return_types(
                    for_stmt.block(),
                    parsed,
                    aliases,
                    locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            ast::Stmt::Repeat(repeat) => {
                collect_function_return_types(
                    repeat.block(),
                    parsed,
                    aliases,
                    locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            ast::Stmt::While(while_stmt) => {
                collect_function_return_types(
                    while_stmt.block(),
                    parsed,
                    aliases,
                    locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            _ => {}
        }
    }
}

fn locals_with_function_parameters(
    locals: &BTreeMap<String, Type>,
    parameters: &BTreeMap<String, Vec<TypedParameter>>,
    function_name: &str,
) -> BTreeMap<String, Type> {
    let mut scoped = locals.clone();

    if let Some(parameters) = parameters.get(function_name) {
        for parameter in parameters {
            scoped.insert(parameter.name.clone(), parameter.ty.clone());
        }
    }

    scoped
}

fn infer_block_return_type(
    block: &ast::Block,
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
) -> Option<Type> {
    let mut returns = Vec::new();

    collect_return_types_in_block(
        block,
        parsed,
        aliases,
        locals,
        function_types,
        current_function,
        &mut returns,
    );

    if returns.is_empty() {
        return None;
    }

    if block_may_fall_through(block) {
        push_unique_type(&mut returns, Type::nil());
    }

    type_from_return_types(returns)
}

fn collect_return_types_in_block(
    block: &ast::Block,
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
    returns: &mut Vec<Type>,
) {
    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::Do(block) => collect_return_types_in_block(
                block.block(),
                parsed,
                aliases,
                locals,
                function_types,
                current_function,
                returns,
            ),
            ast::Stmt::GenericFor(for_stmt) => collect_return_types_in_block(
                for_stmt.block(),
                parsed,
                aliases,
                locals,
                function_types,
                current_function,
                returns,
            ),
            ast::Stmt::If(if_stmt) => {
                collect_return_types_in_block(
                    if_stmt.block(),
                    parsed,
                    aliases,
                    locals,
                    function_types,
                    current_function,
                    returns,
                );

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        collect_return_types_in_block(
                            else_if.block(),
                            parsed,
                            aliases,
                            locals,
                            function_types,
                            current_function,
                            returns,
                        );
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    collect_return_types_in_block(
                        else_block,
                        parsed,
                        aliases,
                        locals,
                        function_types,
                        current_function,
                        returns,
                    );
                }
            }
            ast::Stmt::NumericFor(for_stmt) => collect_return_types_in_block(
                for_stmt.block(),
                parsed,
                aliases,
                locals,
                function_types,
                current_function,
                returns,
            ),
            ast::Stmt::Repeat(repeat) => collect_return_types_in_block(
                repeat.block(),
                parsed,
                aliases,
                locals,
                function_types,
                current_function,
                returns,
            ),
            ast::Stmt::While(while_stmt) => collect_return_types_in_block(
                while_stmt.block(),
                parsed,
                aliases,
                locals,
                function_types,
                current_function,
                returns,
            ),
            ast::Stmt::LocalFunction(_) | ast::Stmt::FunctionDeclaration(_) => {}
            _ => {}
        }
    }

    if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
        let return_type = match return_stmt.returns().iter().next() {
            Some(expression) => infer_expression_type(
                parsed,
                expression,
                aliases,
                locals,
                function_types,
                current_function,
            ),
            None => Some(Type::nil()),
        };

        if let Some(return_type) = return_type {
            push_unique_type(returns, return_type);
        }
    }
}

fn block_may_fall_through(block: &ast::Block) -> bool {
    if matches!(block.last_stmt(), Some(LastStmt::Return(_))) {
        return false;
    }

    match block.stmts().last() {
        Some(ast::Stmt::If(if_stmt)) => !if_guarantees_return(if_stmt),
        _ => true,
    }
}

fn if_guarantees_return(if_stmt: &ast::If) -> bool {
    if block_may_fall_through(if_stmt.block()) {
        return false;
    }

    if let Some(else_ifs) = if_stmt.else_if() {
        if else_ifs
            .iter()
            .any(|else_if| block_may_fall_through(else_if.block()))
        {
            return false;
        }
    }

    if_stmt
        .else_block()
        .is_some_and(|else_block| !block_may_fall_through(else_block))
}

fn type_from_return_types(types: Vec<Type>) -> Option<Type> {
    match types.len() {
        0 => None,
        1 => types.into_iter().next(),
        _ => Some(Type::union(types)),
    }
}

fn push_unique_type(types: &mut Vec<Type>, ty: Type) {
    if !types.contains(&ty) {
        types.push(ty);
    }
}

fn infer_expression_type(
    parsed: &ParsedSource,
    expression: &Expression,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
) -> Option<Type> {
    match expression {
        Expression::Number(_) => Some(Type::number()),
        Expression::String(_) => Some(Type::string()),
        Expression::Symbol(symbol) => match token_text(symbol).as_str() {
            "true" | "false" => Some(Type::boolean()),
            "nil" => Some(Type::nil()),
            _ => None,
        },
        Expression::TableConstructor(table) => {
            Some(Type::table(table.fields().iter().filter_map(|field| {
                let Field::NameKey { key, value, .. } = field else {
                    return None;
                };
                let ty = infer_expression_type(
                    parsed,
                    value,
                    aliases,
                    locals,
                    function_types,
                    current_function,
                )
                .unwrap_or(Type::Any);

                Some((token_text(key), TableField::required(ty)))
            })))
        }
        Expression::Parentheses { expression, .. } => infer_expression_type(
            parsed,
            expression,
            aliases,
            locals,
            function_types,
            current_function,
        ),
        Expression::TypeAssertion { expression, .. } => infer_expression_type(
            parsed,
            expression,
            aliases,
            locals,
            function_types,
            current_function,
        ),
        Expression::Function(function) => Some(function_type_from_body(function.body())),
        Expression::FunctionCall(function_call) => infer_function_call_return_type(
            parsed,
            function_call,
            aliases,
            locals,
            function_types,
            current_function,
        ),
        Expression::Var(var) => infer_var_type(parsed, var, aliases, locals, function_types),
        _ => None,
    }
}

fn infer_var_type(
    parsed: &ParsedSource,
    var: &Var,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
) -> Option<Type> {
    match var {
        Var::Name(name) => {
            let name = token_text(name);

            locals
                .get(&name)
                .or_else(|| function_types.get(&name))
                .map(|ty| resolve_aliases_from_map(aliases, ty, 0))
        }
        Var::Expression(expression) => {
            infer_field_chain_type(parsed, aliases, locals, &plain_field_chain(expression)?)
        }
        _ => None,
    }
}

fn infer_function_call_return_type(
    parsed: &ParsedSource,
    function_call: &FunctionCall,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
) -> Option<Type> {
    let parts = field_chain_before_first_call(function_call)?;

    if parts.len() == 1 && parts.first().is_some_and(|name| name == current_function) {
        return None;
    }

    let function_type = if parts.len() == 1 {
        let name = parts.first()?;

        function_types
            .get(name)
            .or_else(|| locals.get(name))
            .map(|ty| resolve_aliases_from_map(aliases, ty, 0))
    } else {
        infer_field_chain_type(parsed, aliases, locals, &parts)
    }?;

    function_return_type(&function_type)
}

fn function_return_type(ty: &Type) -> Option<Type> {
    match ty {
        Type::Function(function) => Some((*function.return_type).clone()),
        Type::Any => Some(Type::Any),
        Type::Union(types) => {
            let returns = types
                .iter()
                .filter_map(function_return_type)
                .collect::<Vec<_>>();

            match returns.len() {
                0 => None,
                1 => returns.into_iter().next(),
                _ => Some(Type::union(returns)),
            }
        }
        Type::Alias(_) | Type::Primitive(_) | Type::Table(_) => None,
    }
}

fn infer_field_chain_type(
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    parts: &[String],
) -> Option<Type> {
    resolve_field_chain_type_from_map(aliases, locals, parts).or_else(|| {
        let (root, fields) = parts.split_first()?;

        required_module_export_type_for_fields(parsed, root, fields)
            .map(|ty| resolve_aliases_from_map(aliases, &ty, 0))
    })
}

fn resolve_field_chain_type_from_map(
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    parts: &[String],
) -> Option<Type> {
    let (root, fields) = parts.split_first()?;
    let mut current = locals
        .get(root)
        .map(|ty| resolve_aliases_from_map(aliases, ty, 0))?;

    for field in fields {
        current = resolve_table_field_type_from_map(aliases, &current, field)?;
    }

    Some(current)
}

fn resolve_table_field_type_from_map(
    aliases: &BTreeMap<String, Type>,
    ty: &Type,
    field: &str,
) -> Option<Type> {
    match resolve_aliases_from_map(aliases, ty, 0) {
        Type::Table(table) => table
            .field(field)
            .map(|field| resolve_aliases_from_map(aliases, &field.ty, 0)),
        Type::Union(types) => {
            let mut resolved = Vec::new();

            for ty in types {
                if let Some(field_type) = resolve_table_field_type_from_map(aliases, &ty, field) {
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

fn resolve_aliases_from_map(aliases: &BTreeMap<String, Type>, ty: &Type, depth: usize) -> Type {
    if depth >= 64 {
        return ty.clone();
    }

    match ty {
        Type::Alias(name) => aliases
            .get(name)
            .map(|resolved| resolve_aliases_from_map(aliases, resolved, depth + 1))
            .unwrap_or_else(|| ty.clone()),
        _ => ty.clone(),
    }
}

fn plain_field_chain(expression: &ast::VarExpression) -> Option<Vec<String>> {
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

fn function_type_from_body(body: &ast::FunctionBody) -> Type {
    let parameter_count = body
        .parameters()
        .iter()
        .filter(|parameter| matches!(parameter, ast::Parameter::Name(_)))
        .count();

    Type::function(vec![Type::Any; parameter_count], Type::Any)
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn resolve_tua_type(
    path: &Path,
    ty: &TuaType,
    span: Option<SourceSpan>,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Type {
    match ty {
        TuaType::Named(name) => resolve_named_type(path, name, span, alias_names, diagnostics),
        TuaType::Union(types) => Type::union(
            types
                .iter()
                .map(|ty| resolve_tua_type(path, ty, span, alias_names, diagnostics)),
        ),
        TuaType::Table(fields) => Type::table(fields.iter().map(|field| {
            (
                field.name.clone(),
                TableField {
                    ty: resolve_table_field_type(path, field, alias_names, diagnostics),
                    optional: field.optional,
                },
            )
        })),
    }
}

fn resolve_table_field_type(
    path: &Path,
    field: &ParsedTableFieldType,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Type {
    resolve_tua_type(path, &field.ty, field.span, alias_names, diagnostics)
}

fn resolve_named_type(
    path: &Path,
    name: &str,
    span: Option<SourceSpan>,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Type {
    match name {
        "number" => Type::number(),
        "string" => Type::string(),
        "boolean" => Type::boolean(),
        "nil" => Type::nil(),
        "any" => Type::Any,
        _ if alias_names.contains(name) => Type::alias(name),
        _ => {
            diagnostics.push(type_diagnostic(
                path,
                span,
                format!("unknown type `{name}`"),
                DiagnosticCode::TYPE_UNKNOWN_NAME,
            ));
            Type::Any
        }
    }
}

fn type_diagnostic(
    path: &Path,
    span: Option<SourceSpan>,
    message: impl Into<String>,
    code: DiagnosticCode,
) -> Diagnostic {
    let position = span
        .map(|span| span.start)
        .unwrap_or(crate::SourcePosition { line: 1, column: 1 });

    Diagnostic::error(path, position.line, position.column, message).with_code(code)
}
