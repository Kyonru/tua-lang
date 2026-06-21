use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::parser::{
    FunctionAnnotation as ParsedFunctionAnnotation, ParsedSource, SourceSpan,
    TableFieldType as ParsedTableFieldType, TluaType,
};
use crate::{Diagnostic, DiagnosticCode};

use super::model::{TableField, Type, TypeAlias};
use super::TypeContext;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeEnvironment {
    path: PathBuf,
    aliases: BTreeMap<String, Type>,
    locals: BTreeMap<String, Type>,
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

    pub fn aliases(&self) -> impl Iterator<Item = (&str, &Type)> {
        self.aliases.iter().map(|(name, ty)| (name.as_str(), ty))
    }

    pub fn local(&self, name: &str) -> Option<&Type> {
        self.locals.get(name)
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
    let mut aliases = BTreeMap::new();
    let mut locals = BTreeMap::new();
    let mut functions = Vec::new();

    for alias in &parsed.type_aliases {
        if aliases.contains_key(&alias.name) {
            continue;
        }

        let ty = resolve_tlua_type(
            &parsed.path,
            &alias.ty,
            alias.span,
            &alias_names,
            &mut diagnostics,
        );
        aliases.insert(alias.name.clone(), ty);
    }

    for local in &parsed.local_annotations {
        let ty = resolve_tlua_type(
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

    (
        TypeEnvironment {
            path: parsed.path.clone(),
            aliases,
            locals,
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
            ty: resolve_tlua_type(
                path,
                &parameter.ty,
                parameter.span,
                alias_names,
                diagnostics,
            ),
        })
        .collect();
    let return_type = function
        .return_type
        .as_ref()
        .map(|ty| resolve_tlua_type(path, ty, function.span, alias_names, diagnostics));

    TypedFunction {
        name: function.name.clone(),
        parameters,
        parameter_count: function.parameter_count,
        has_varargs: function.has_varargs,
        return_type,
    }
}

fn resolve_tlua_type(
    path: &Path,
    ty: &TluaType,
    span: Option<SourceSpan>,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Type {
    match ty {
        TluaType::Named(name) => resolve_named_type(path, name, span, alias_names, diagnostics),
        TluaType::Union(types) => Type::union(
            types
                .iter()
                .map(|ty| resolve_tlua_type(path, ty, span, alias_names, diagnostics)),
        ),
        TluaType::Table(fields) => Type::table(fields.iter().map(|field| {
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
    resolve_tlua_type(path, &field.ty, field.span, alias_names, diagnostics)
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
