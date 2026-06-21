use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use full_moon::ast::{self, Expression, Field, FunctionCall, Index, LastStmt, Prefix, Suffix, Var};

use crate::module_index::required_module_export_type_for_fields;
use crate::parser::{
    FunctionAnnotation as ParsedFunctionAnnotation, ParsedSource, SourceSpan,
    TableFieldType as ParsedTableFieldType, TuaType,
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

        let ty = resolve_tua_type(
            &parsed.path,
            &alias.ty,
            alias.span,
            &alias_names,
            &mut diagnostics,
        );
        aliases.insert(alias.name.clone(), ty);
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
            ty: resolve_tua_type(
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
        .map(|ty| resolve_tua_type(path, ty, function.span, alias_names, diagnostics));

    TypedFunction {
        name: function.name.clone(),
        parameters,
        parameter_count: function.parameter_count,
        has_varargs: function.has_varargs,
        return_type,
    }
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
