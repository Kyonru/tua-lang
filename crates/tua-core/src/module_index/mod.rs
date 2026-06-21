use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use full_moon::ast::{
    self, Expression, Field, FunctionArgs, Index, LastStmt, Prefix, Suffix, TableConstructor, Var,
};

use crate::config::{load_config_from_dir, CONFIG_FILE};
use crate::semantic::resolve_aliases;
use crate::types::{build_type_environment, TableField, Type, TypeEnvironment, TypedFunction};
use crate::{parse_lua_source, parse_tua_source, ParsedSource};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleExports {
    pub path: PathBuf,
    pub fields: BTreeMap<String, Type>,
}

impl ModuleExports {
    pub fn field(&self, name: &str) -> Option<&Type> {
        self.fields.get(name)
    }
}

pub fn required_module_exports_for_receiver(
    parsed: &ParsedSource,
    receiver: &str,
) -> Option<ModuleExports> {
    let module = require_bindings(parsed).remove(receiver)?;
    let path = resolve_required_module(&parsed.path, &module)?;
    let source = fs::read_to_string(&path).ok()?;
    let parsed = parse_module_source(&path, source).ok()?;

    infer_module_exports(&parsed)
}

pub fn resolve_required_module(current_path: &Path, module: &str) -> Option<PathBuf> {
    let roots = module_roots(current_path);
    let relative = module.replace('.', "/");

    for root in roots {
        for candidate in module_candidates(&root, &relative) {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
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

fn module_roots(current_path: &Path) -> Vec<PathBuf> {
    let current_dir = current_path.parent().unwrap_or_else(|| Path::new("."));
    let mut roots = Vec::new();

    if let Some(project_root) = project_root_for(current_dir) {
        if let Ok(loaded) = load_config_from_dir(&project_root) {
            push_unique(&mut roots, project_root.join(loaded.config.source));
            push_unique(&mut roots, project_root.join(loaded.config.out));
        }
    }

    push_unique(&mut roots, current_dir.to_path_buf());

    roots
}

fn project_root_for(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(CONFIG_FILE).is_file())
        .map(Path::to_path_buf)
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn module_candidates(root: &Path, relative: &str) -> Vec<PathBuf> {
    let base = root.join(relative);

    vec![
        base.with_extension("tua"),
        base.with_extension("lua"),
        base.join("init.tua"),
        base.join("init.lua"),
    ]
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
    table_members: BTreeMap<String, BTreeMap<String, Type>>,
    return_expr: Option<&'a Expression>,
}

impl<'a> ExportInferer<'a> {
    fn new(parsed: &'a ParsedSource) -> Self {
        Self {
            parsed,
            environment: build_type_environment(parsed).ok(),
            locals: BTreeMap::new(),
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
                        .insert(field.trim().to_owned(), ty);
                } else {
                    self.locals.entry(name.clone()).or_insert(ty);
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
                        .or_insert_with(|| function_type_from_body(function.body()));
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

        self.table_members
            .entry(table.clone())
            .or_default()
            .insert(field.clone(), ty);
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

    fn fields_for_returned_local(&mut self, name: &str) -> BTreeMap<String, Type> {
        let mut fields = BTreeMap::new();

        if let Some(ty) = self.locals.get(name).cloned() {
            collect_table_fields(&self.resolve_export_type(&ty), &mut fields);
        }

        if let Some(members) = self.table_members.get(name) {
            fields.extend(members.clone());
        }

        fields
    }

    fn infer_table_fields(&mut self, table: &TableConstructor) -> BTreeMap<String, Type> {
        table
            .fields()
            .iter()
            .filter_map(|field| {
                let Field::NameKey { key, value, .. } = field else {
                    return None;
                };

                Some((token_text(key), self.infer_expression(value)))
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
                    .map(|(name, ty)| (name, TableField::required(ty))),
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

fn collect_table_fields(ty: &Type, fields: &mut BTreeMap<String, Type>) {
    match ty {
        Type::Table(table) => {
            fields.extend(
                table
                    .fields()
                    .map(|(name, field)| (name.to_owned(), field.ty.clone())),
            );
        }
        Type::Union(types) => {
            for ty in types {
                collect_table_fields(ty, fields);
            }
        }
        Type::Any | Type::Alias(_) | Type::Function(_) | Type::Primitive(_) => {}
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

    Type::function(parameters, function.return_type_or_any())
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
    token.to_string().trim().to_owned()
}

fn unquote_string(value: &str) -> String {
    value.trim().trim_matches('"').trim_matches('\'').to_owned()
}

#[cfg(test)]
mod tests;
