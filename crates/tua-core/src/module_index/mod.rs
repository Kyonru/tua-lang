use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use full_moon::ast::{
    self, Expression, Field, FunctionArgs, LastStmt, Prefix, Suffix, TableConstructor, Var,
};
use full_moon::node::Node;

use crate::ast_utils::plain_field_chain_from_var;
use crate::deprecation::{deprecated_declarations, Deprecation};
use crate::module_link::ModuleSpecifierKind;
use crate::parser::{LuaDocAnnotation, LuaDocAnnotationKind};
use crate::project_model::ProjectModel;
use crate::semantic::SemanticModel;
use crate::types::{
    build_type_environment, deduplicate_inferred_types, function_type_from_tua_signature,
    FunctionParameter, FunctionType, TableField, Type, TypeEnvironment, TypedFunction,
};
use crate::{parse_lua_source, parse_tua_source, ParsedSource, SourceSpan};

thread_local! {
    static REQUIRED_MODULE_EXPORT_CACHE: RefCell<BTreeMap<PathBuf, CachedRequiredModuleExports>> =
        const { RefCell::new(BTreeMap::new()) };
}

#[derive(Clone, Debug)]
struct CachedRequiredModuleExports {
    modified: Option<SystemTime>,
    len: Option<u64>,
    exports: Option<ModuleExports>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleExports {
    pub path: PathBuf,
    pub fields: BTreeMap<String, ModuleExportField>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleExportField {
    pub ty: Type,
    pub documentation: Option<String>,
    pub deprecation: Option<Deprecation>,
    pub parameter_names: Vec<String>,
    pub parameter_documentation: Vec<Option<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequiredModule {
    pub module: String,
    pub kind: ModuleSpecifierKind,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequireBinding {
    pub alias: String,
    pub module: String,
    pub kind: ModuleSpecifierKind,
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
    require_bindings(parsed).into_values().collect()
}

pub fn required_module_for_receiver(
    parsed: &ParsedSource,
    receiver: &str,
) -> Option<RequiredModule> {
    let module = require_bindings(parsed).remove(receiver)?;
    let path = resolve_module_specifier(&parsed.path, &module.module, module.kind)?;

    Some(RequiredModule {
        module: module.module,
        kind: module.kind,
        path,
    })
}

pub fn required_module_exports_for_receiver(
    parsed: &ParsedSource,
    receiver: &str,
) -> Option<ModuleExports> {
    let required = required_module_for_receiver(parsed, receiver)?;

    required_module_exports_for_path(&required.path)
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
        deprecation: None,
        parameter_names: Vec::new(),
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

pub fn resolve_module_specifier(
    current_path: &Path,
    module: &str,
    kind: ModuleSpecifierKind,
) -> Option<PathBuf> {
    ProjectModel::load_for_path(current_path)
        .ok()
        .and_then(|model| match kind {
            ModuleSpecifierKind::Require => model.resolve_required_module(current_path, module),
            ModuleSpecifierKind::Import => model.resolve_imported_module(current_path, module),
        })
}

pub(crate) fn resolve_type_module_specifier(current_path: &Path, module: &str) -> Option<PathBuf> {
    ProjectModel::load_for_path(current_path)
        .ok()
        .and_then(|model| model.resolve_type_imported_module(current_path, module))
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
    let environment = build_type_environment(parsed).ok();

    infer_module_exports_with_optional_environment(parsed, environment.as_ref())
}

pub fn infer_module_exports_with_environment(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
) -> Option<ModuleExports> {
    infer_module_exports_with_optional_environment(parsed, Some(environment))
}

pub(crate) fn infer_module_exports_with_optional_environment(
    parsed: &ParsedSource,
    environment: Option<&TypeEnvironment>,
) -> Option<ModuleExports> {
    let mut inferer = ExportInferer::new(parsed, environment);

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

fn required_module_exports_for_path(path: &Path) -> Option<ModuleExports> {
    let path = path.to_path_buf();
    let metadata = fs::metadata(&path).ok();
    let modified = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok());
    let len = metadata.as_ref().map(fs::Metadata::len);

    if let Some(cached) = REQUIRED_MODULE_EXPORT_CACHE.with(|cache| {
        cache
            .borrow()
            .get(&path)
            .filter(|cached| cached.modified == modified && cached.len == len)
            .cloned()
    }) {
        return cached.exports;
    }

    let exports = fs::read_to_string(&path)
        .ok()
        .and_then(|source| parse_module_source(&path, source).ok())
        .and_then(|parsed| infer_module_exports(&parsed));

    REQUIRED_MODULE_EXPORT_CACHE.with(|cache| {
        cache.borrow_mut().insert(
            path,
            CachedRequiredModuleExports {
                modified,
                len,
                exports: exports.clone(),
            },
        );
    });

    exports
}

fn require_bindings(parsed: &ParsedSource) -> BTreeMap<String, RequireBinding> {
    let mut bindings = BTreeMap::new();

    for stmt in parsed.ast.nodes().stmts() {
        let ast::Stmt::LocalAssignment(local) = stmt else {
            continue;
        };

        let names = local.names();
        let expressions = local.expressions().into_iter().collect::<Vec<_>>();

        for (name, expression) in names.iter().zip(expressions.iter()) {
            let Some((module, kind)) =
                module_specifier_from_expression(expression, names.len(), expressions.len())
            else {
                continue;
            };

            let alias = token_text(name);

            bindings.insert(
                alias.clone(),
                RequireBinding {
                    alias,
                    module,
                    kind,
                },
            );
        }
    }

    bindings
}

fn module_specifier_from_expression(
    expression: &Expression,
    name_count: usize,
    expression_count: usize,
) -> Option<(String, ModuleSpecifierKind)> {
    let Expression::FunctionCall(call) = expression else {
        return None;
    };
    let Prefix::Name(name) = call.prefix() else {
        return None;
    };

    let kind = match token_text(name).as_str() {
        "require" => ModuleSpecifierKind::Require,
        "import" if name_count == 1 && expression_count == 1 => ModuleSpecifierKind::Import,
        _ => return None,
    };

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

    let module = unquote_string(&module.to_string());

    if kind == ModuleSpecifierKind::Import && !is_static_import_specifier(&module) {
        return None;
    }

    Some((module, kind))
}

fn is_static_import_specifier(module: &str) -> bool {
    is_child_relative_import_specifier(module) || is_project_import_specifier(module)
}

fn is_child_relative_import_specifier(module: &str) -> bool {
    module.starts_with("./") && !has_parent_path_component(module)
}

fn is_project_import_specifier(module: &str) -> bool {
    !module.is_empty() && !module.starts_with('.') && !has_parent_path_component(module)
}

fn has_parent_path_component(module: &str) -> bool {
    Path::new(module)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
}

struct ExportInferer<'a> {
    parsed: &'a ParsedSource,
    environment: Option<&'a TypeEnvironment>,
    code_field_docs: BTreeMap<String, Vec<&'a LuaDocAnnotation>>,
    locals: BTreeMap<String, Type>,
    function_docs: BTreeMap<String, String>,
    deprecations: BTreeMap<String, Deprecation>,
    function_parameter_names: BTreeMap<String, Vec<String>>,
    function_parameter_docs: BTreeMap<String, Vec<Option<String>>>,
    table_members: BTreeMap<String, BTreeMap<String, ModuleExportField>>,
    return_expr: Option<&'a Expression>,
}

impl<'a> ExportInferer<'a> {
    fn new(parsed: &'a ParsedSource, environment: Option<&'a TypeEnvironment>) -> Self {
        let deprecations = deprecated_declarations(parsed)
            .into_iter()
            .filter(|declaration| {
                matches!(
                    declaration.kind,
                    crate::hir::HirSymbolKind::Local | crate::hir::HirSymbolKind::Function
                )
            })
            .map(|declaration| (declaration.path.join("."), declaration.deprecation))
            .collect();

        Self {
            parsed,
            environment,
            code_field_docs: lua_doc_code_field_annotations(parsed),
            locals: BTreeMap::new(),
            function_docs: BTreeMap::new(),
            deprecations,
            function_parameter_names: BTreeMap::new(),
            function_parameter_docs: BTreeMap::new(),
            table_members: BTreeMap::new(),
            return_expr: None,
        }
    }

    fn collect_top_level(&mut self) {
        if let Some(environment) = &self.environment {
            for function in environment.functions() {
                let Some(name) = &function.name else {
                    continue;
                };

                if let Some((table, field)) = name.split_once('.') {
                    let ty = self.resolve_export_type(&typed_function_type(function));

                    self.table_members
                        .entry(table.trim().to_owned())
                        .or_default()
                        .insert(
                            field.trim().to_owned(),
                            ModuleExportField {
                                ty,
                                documentation: function.documentation.clone(),
                                deprecation: self.deprecations.get(name).cloned(),
                                parameter_names: parameter_names(function),
                                parameter_documentation: parameter_documentation(function),
                            },
                        );
                } else {
                    if let Some(documentation) = &function.documentation {
                        self.function_docs
                            .entry(name.clone())
                            .or_insert_with(|| documentation.clone());
                    }

                    self.function_parameter_docs
                        .entry(name.clone())
                        .or_insert_with(|| parameter_documentation(function));
                    self.function_parameter_names
                        .entry(name.clone())
                        .or_insert_with(|| parameter_names(function));
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
                            deprecation: self
                                .deprecations
                                .get(&function.name().to_string().replace(':', "."))
                                .cloned(),
                            parameter_names: parameter_names_from_body(function.body()),
                            parameter_documentation: Vec::new(),
                        });
                }
            }
            ast::Stmt::LocalAssignment(local) => {
                for (name, expression) in local.names().iter().zip(local.expressions()) {
                    let name = token_text(name);
                    if self
                        .environment
                        .is_some_and(|environment| environment.local(&name).is_some())
                    {
                        continue;
                    }

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
                deprecation: self.deprecations.get(&parts.join(".")).cloned(),
                parameter_names: Vec::new(),
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

        if let Some(ty) = self.local_type(name) {
            collect_table_fields(&ty, &mut fields);
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
                let lua_doc_export = self.lua_doc_function_export_for_field(field);
                let documentation = self.field_documentation(field).or_else(|| {
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
                let parameter_names = lua_doc_export
                    .as_ref()
                    .map(|export| export.parameter_names.clone())
                    .or_else(|| {
                        let Expression::Function(function) = value else {
                            return None;
                        };

                        Some(parameter_names_from_body(function.body()))
                    })
                    .or_else(|| {
                        let Expression::Var(Var::Name(name)) = value else {
                            return None;
                        };

                        self.function_parameter_names
                            .get(&token_text(name))
                            .cloned()
                    })
                    .unwrap_or_default();
                let ty = lua_doc_export
                    .map(|export| export.ty)
                    .unwrap_or_else(|| self.infer_expression(value));
                let deprecation = self.field_deprecation(field).or_else(|| {
                    let Expression::Var(Var::Name(name)) = value else {
                        return None;
                    };

                    self.deprecations.get(&token_text(name)).cloned()
                });

                Some((
                    name,
                    ModuleExportField {
                        ty,
                        documentation,
                        deprecation,
                        parameter_names,
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
            Expression::TableConstructor(table) => self.infer_table_type(table),
            Expression::Function(function) => function_type_from_body(function.body()),
            Expression::FunctionCall(_) => self
                .environment
                .and_then(|environment| {
                    let position = node_span(expression)?.start;
                    SemanticModel::new(self.parsed, environment)
                        .type_of_expression_return(expression, position, 0)
                })
                .map(|ty| self.resolve_export_type(&ty))
                .unwrap_or(Type::Any),
            Expression::Parentheses { expression, .. } => self.infer_expression(expression),
            Expression::Var(Var::Name(name)) => {
                self.local_type(&token_text(name)).unwrap_or(Type::Any)
            }
            _ => Type::Any,
        }
    }

    fn infer_table_type(&mut self, table: &TableConstructor) -> Type {
        let mut fields = Vec::new();
        let mut array_items = Vec::new();

        for field in table.fields() {
            match field {
                Field::NameKey { key, value, .. } => {
                    fields.push((
                        token_text(key),
                        TableField::required(self.infer_expression(value)),
                    ));
                }
                Field::ExpressionKey { key, value, .. } => {
                    let ty = self.infer_expression(value);

                    if let Some(key) = string_literal_expression(key) {
                        fields.push((key, TableField::required(ty)));
                    } else if expression_is_number(key) {
                        array_items.push(ty);
                    }
                }
                Field::NoKey(value) => {
                    array_items.push(self.infer_expression(value));
                }
                _ => {}
            }
        }

        table_or_array_type(fields, array_items)
    }

    fn local_type(&mut self, name: &str) -> Option<Type> {
        if let Some(ty) = self
            .environment
            .and_then(|environment| environment.class_value_type(name))
        {
            let ty = self.resolve_export_type(&ty);
            self.locals.insert(name.to_owned(), ty.clone());

            return Some(ty);
        }

        if let Some(function) = self
            .environment
            .and_then(|environment| environment.function(name))
        {
            let ty = self.resolve_export_type(&typed_function_type(function));

            self.locals.insert(name.to_owned(), ty.clone());

            return Some(ty);
        }

        if let Some(ty) = self.locals.get(name).cloned() {
            return Some(ty);
        }

        let ty = self
            .environment
            .and_then(|environment| environment.local(name))
            .map(|ty| self.resolve_export_type(ty))?;

        self.locals.insert(name.to_owned(), ty.clone());

        Some(ty)
    }

    fn lua_doc_function_export_for_field(&self, field: &Field) -> Option<ModuleExportField> {
        let Field::NameKey { key, value, .. } = field else {
            return None;
        };
        let body = function_body_from_expression(value)?;
        let docs = self.lua_doc_annotations_for_field(&token_text(key), field);

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
            documentation: self.field_documentation(field),
            deprecation: self.field_deprecation(field),
            parameter_names: parameter_names_from_body(body),
            parameter_documentation,
        })
    }

    fn field_documentation(&self, field: &Field) -> Option<String> {
        let Field::NameKey { key, .. } = field else {
            return None;
        };
        let docs = self.lua_doc_annotations_for_field(&token_text(key), field);
        let documentation = docs
            .iter()
            .filter(|annotation| !matches!(annotation.kind, LuaDocAnnotationKind::Deprecated(_)))
            .filter_map(|annotation| annotation.documentation.as_deref())
            .collect::<Vec<_>>();

        (!documentation.is_empty()).then(|| documentation.join("\n"))
    }

    fn field_deprecation(&self, field: &Field) -> Option<Deprecation> {
        let Field::NameKey { key, .. } = field else {
            return None;
        };
        let docs = self.lua_doc_annotations_for_field(&token_text(key), field);

        Deprecation::from_annotations(&docs)
    }

    fn lua_doc_annotations_for_field(
        &self,
        name: &str,
        field: &Field,
    ) -> Vec<&'a LuaDocAnnotation> {
        let docs = node_span(field)
            .and_then(|span| first_code_line_span(self.parsed, span))
            .map(|span| leading_lua_doc_annotations(self.parsed, span.start.line))
            .unwrap_or_default();

        if !docs.is_empty() {
            return docs;
        }

        self.code_field_docs.get(name).cloned().unwrap_or_default()
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
                        deprecation: None,
                        parameter_names: Vec::new(),
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
    resolve_export_aliases_inner(
        environment,
        ty,
        true,
        &mut std::collections::BTreeSet::new(),
    )
}

fn resolve_export_aliases_inner(
    environment: &TypeEnvironment,
    ty: &Type,
    expand_non_class_alias: bool,
    visiting: &mut std::collections::BTreeSet<String>,
) -> Type {
    match ty {
        Type::Alias(name) => {
            if !environment.is_class(name) && !expand_non_class_alias {
                return ty.clone();
            }
            if !visiting.insert(name.clone()) {
                return nominal_alias_stub(environment, name).unwrap_or(Type::Any);
            }

            let resolved = if environment.is_class(name) {
                environment.alias(name).cloned()
            } else {
                Some(environment.resolve_aliases(ty))
            };
            let result = resolved
                .filter(|resolved| resolved != ty)
                .map(|resolved| {
                    resolve_export_aliases_inner(environment, &resolved, false, visiting)
                })
                .unwrap_or_else(|| ty.clone());
            visiting.remove(name);

            result
        }
        Type::Function(function) => {
            let parameters = function
                .parameters
                .iter()
                .map(|parameter| FunctionParameter {
                    name: parameter.name.clone(),
                    ty: resolve_export_aliases_inner(environment, &parameter.ty, false, visiting),
                })
                .collect::<Vec<_>>();
            let vararg_type = function
                .vararg_type
                .as_deref()
                .map(|ty| resolve_export_aliases_inner(environment, ty, false, visiting));
            let return_types = function
                .return_types_or_any()
                .iter()
                .map(|ty| resolve_export_aliases_inner(environment, ty, false, visiting))
                .collect::<Vec<_>>();

            Type::Function(FunctionType::new_signature(
                parameters,
                function.required_parameter_count,
                vararg_type,
                return_types,
            ))
        }
        Type::Table(table) => {
            let mut resolved =
                crate::TableShape::from_fields(table.fields().map(|(name, field)| {
                    (
                        name.to_owned(),
                        TableField {
                            ty: resolve_export_aliases_inner(
                                environment,
                                &field.ty,
                                false,
                                visiting,
                            ),
                            optional: field.optional,
                        },
                    )
                }));
            resolved.copy_semantic_metadata_from(table);

            Type::Table(resolved)
        }
        Type::Union(types) => Type::union(types.iter().map(|ty| {
            resolve_export_aliases_inner(environment, ty, expand_non_class_alias, visiting)
        })),
        Type::Array(item) => Type::array(resolve_export_aliases_inner(
            environment,
            item,
            expand_non_class_alias,
            visiting,
        )),
        Type::Tuple(items) => Type::tuple(items.iter().map(|ty| {
            resolve_export_aliases_inner(environment, ty, expand_non_class_alias, visiting)
        })),
        Type::Dictionary { key, value } => Type::dictionary(
            resolve_export_aliases_inner(environment, key, expand_non_class_alias, visiting),
            resolve_export_aliases_inner(environment, value, expand_non_class_alias, visiting),
        ),
        Type::Any | Type::Primitive(_) | Type::StringLiteral(_) => ty.clone(),
    }
}

fn nominal_alias_stub(environment: &TypeEnvironment, name: &str) -> Option<Type> {
    let Type::Table(table) = environment.alias(name)? else {
        return None;
    };
    table.nominal_identity()?;

    let mut stub = crate::TableShape::new();
    stub.copy_semantic_metadata_from(table);

    Some(Type::Table(stub))
}

fn lua_doc_code_field_annotations(
    parsed: &ParsedSource,
) -> BTreeMap<String, Vec<&LuaDocAnnotation>> {
    let mut by_line = BTreeMap::new();

    for annotation in &parsed.lua_doc_annotations {
        by_line.insert(annotation.span.start.line, annotation);
    }

    let mut pending = Vec::new();
    let mut docs_by_field = BTreeMap::new();

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

        if let Some(field_name) = field_name_from_code_line(trimmed) {
            docs_by_field
                .entry(field_name)
                .or_insert_with(|| pending.clone());
        }

        pending.clear();
    }

    docs_by_field
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

fn string_literal_expression(expression: &Expression) -> Option<String> {
    match expression {
        Expression::String(value) => Some(unquote_string(&value.to_string())),
        Expression::Parentheses { expression, .. } => string_literal_expression(expression),
        _ => None,
    }
}

fn expression_is_number(expression: &Expression) -> bool {
    match expression {
        Expression::Number(_) => true,
        Expression::Parentheses { expression, .. } => expression_is_number(expression),
        _ => false,
    }
}

fn table_or_array_type(fields: Vec<(String, TableField)>, array_items: Vec<Type>) -> Type {
    let table = (!fields.is_empty()).then(|| Type::table(fields));
    let array = (!array_items.is_empty()).then(|| Type::array(union_types(array_items)));

    match (table, array) {
        (Some(table), Some(array)) => Type::union([table, array]),
        (Some(table), None) => table,
        (None, Some(array)) => array,
        (None, None) => Type::empty_table(),
    }
}

fn union_types(types: Vec<Type>) -> Type {
    let unique = deduplicate_inferred_types(types);

    match unique.len() {
        0 => Type::Any,
        1 => unique.into_iter().next().unwrap_or(Type::Any),
        _ => Type::union(unique),
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
        crate::TuaType::Named(name) if name == "identifier" => Type::string(),
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
        } => Type::Function(function_type_from_tua_signature(
            parameters,
            returns,
            |ty| Some(type_from_lua_doc_type(ty)),
        )),
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
            .map(|parameter| FunctionParameter::named(&parameter.name, parameter.ty.clone()))
            .collect::<Vec<_>>()
    } else {
        (0..function.parameter_count)
            .map(|index| FunctionParameter::named(format!("arg{}", index + 1), Type::Any))
            .collect()
    };

    Type::Function(FunctionType::new_signature(
        parameters,
        function.required_parameter_count,
        function.vararg_type.clone(),
        function.return_types_or_any(),
    ))
}

fn parameter_documentation(function: &TypedFunction) -> Vec<Option<String>> {
    function
        .parameters
        .iter()
        .map(|parameter| parameter.documentation.clone())
        .collect()
}

fn parameter_names(function: &TypedFunction) -> Vec<String> {
    function
        .parameters
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect()
}

fn function_type_from_body(body: &ast::FunctionBody) -> Type {
    crate::types::function_type_from_body(body)
}

fn parameter_names_from_body(body: &ast::FunctionBody) -> Vec<String> {
    body.parameters()
        .iter()
        .filter_map(|parameter| {
            let ast::Parameter::Name(name) = parameter else {
                return None;
            };

            Some(token_text(name))
        })
        .collect()
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

fn unquote_string(value: &str) -> String {
    value.trim().trim_matches('"').trim_matches('\'').to_owned()
}

#[cfg(test)]
mod tests;
