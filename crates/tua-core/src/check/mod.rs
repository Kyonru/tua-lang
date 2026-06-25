use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use full_moon::ast::luau::TypeInfo;
use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, Index, LastStmt, Prefix, Return,
    Suffix, TableConstructor, Var,
};
use full_moon::node::Node;

use crate::config::DiagnosticsConfig;
use crate::inline_ignore::apply_inline_ignores;
use crate::lint::lint_parsed_source;
use crate::module_index::required_module_export_type_for_fields;
use crate::narrowing::{directly_assigned_name, narrowed_locals_for_condition};
use crate::parser::{parse_tua_source, ParsedSource, SourcePosition};
use crate::types::{
    build_type_environment, FunctionType, MetatableFacts, TableField, Type, TypeContext,
    TypeEnvironment, TypedFunction,
};
use crate::{asset, lua};
use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity};

const MAX_TYPE_RESOLUTION_DEPTH: usize = 64;

pub fn check_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<(), Vec<Diagnostic>> {
    let path = path.as_ref();
    let source = source.into();
    let parsed = parse_tua_source(path, source)?;

    check_parsed_source(&parsed)
}

pub fn check_tua_source_with_diagnostics_config(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    diagnostics_config: &DiagnosticsConfig,
) -> Result<(), Vec<Diagnostic>> {
    let path = path.as_ref();
    let source = source.into();
    let parsed = parse_tua_source(path, source.clone()).map_err(|diagnostics| {
        apply_inline_ignores(path, &source, diagnostics, diagnostics_config)
    })?;

    check_parsed_source_with_diagnostics_config(&parsed, diagnostics_config)
}

pub fn check_tua_source_with_asset_roots_and_diagnostics_config(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    asset_roots: &[PathBuf],
    diagnostics_config: &DiagnosticsConfig,
) -> Result<(), Vec<Diagnostic>> {
    let path = path.as_ref();
    let source = source.into();
    let parsed = parse_tua_source(path, source.clone()).map_err(|diagnostics| {
        apply_inline_ignores(path, &source, diagnostics, diagnostics_config)
    })?;

    check_parsed_source_with_asset_roots_and_diagnostics_config(
        &parsed,
        asset_roots,
        diagnostics_config,
    )
}

pub fn check_parsed_source(parsed: &ParsedSource) -> Result<(), Vec<Diagnostic>> {
    check_parsed_source_with_diagnostics_config(parsed, &DiagnosticsConfig::default())
}

pub fn check_parsed_source_with_diagnostics_config(
    parsed: &ParsedSource,
    diagnostics_config: &DiagnosticsConfig,
) -> Result<(), Vec<Diagnostic>> {
    check_parsed_source_with_asset_roots_and_diagnostics_config(
        parsed,
        &default_asset_roots(&parsed.path),
        diagnostics_config,
    )
}

pub fn check_parsed_source_with_asset_roots_and_diagnostics_config(
    parsed: &ParsedSource,
    asset_roots: &[PathBuf],
    diagnostics_config: &DiagnosticsConfig,
) -> Result<(), Vec<Diagnostic>> {
    let environment = build_type_environment(parsed)?;
    let mut checker = Checker::new(parsed, &environment, asset_roots);

    checker.check_block(parsed.ast.nodes(), None);
    checker
        .diagnostics
        .extend(lint_parsed_source(parsed, diagnostics_config));

    if diagnostics_config.doc_annotations {
        checker
            .diagnostics
            .extend(parsed.lua_doc_diagnostics.iter().cloned());
    }

    checker.diagnostics = apply_inline_ignores(
        &parsed.path,
        &parsed.source,
        checker.diagnostics,
        diagnostics_config,
    );

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
    scoped_locals: Vec<BTreeMap<String, Type>>,
    asset_roots: Vec<PathBuf>,
    reported_asset_diagnostics: BTreeSet<(usize, usize, u16, String)>,
    reported_method_style_diagnostics: BTreeSet<(usize, usize, String)>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Checker<'a> {
    fn new(
        parsed: &'a ParsedSource,
        environment: &'a TypeEnvironment,
        asset_roots: &[PathBuf],
    ) -> Self {
        Self {
            parsed,
            environment,
            context: environment.to_type_context(),
            scoped_locals: Vec::new(),
            asset_roots: normalize_asset_roots(asset_roots),
            reported_asset_diagnostics: BTreeSet::new(),
            reported_method_style_diagnostics: BTreeSet::new(),
            diagnostics: Vec::new(),
        }
    }

    fn check_block(&mut self, block: &ast::Block, current_return: Option<&[Type]>) {
        self.check_block_with_locals(block, current_return, BTreeMap::new());
    }

    fn check_block_with_locals(
        &mut self,
        block: &ast::Block,
        current_return: Option<&[Type]>,
        locals: BTreeMap<String, Type>,
    ) {
        self.scoped_locals.push(locals);

        for stmt in block.stmts() {
            self.check_stmt(stmt, current_return);
        }

        if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
            self.check_return(return_stmt, current_return);
        }

        self.scoped_locals.pop();
    }

    fn check_stmt(&mut self, stmt: &ast::Stmt, current_return: Option<&[Type]>) {
        match stmt {
            ast::Stmt::Assignment(assignment) => self.check_assignment(assignment),
            ast::Stmt::FunctionCall(function_call) => {
                self.check_function_call(function_call);
                self.record_setmetatable_call(function_call);
            }
            ast::Stmt::LocalAssignment(local) => self.check_local_assignment(local),
            ast::Stmt::Do(block) => self.check_block(block.block(), current_return),
            ast::Stmt::If(if_stmt) => {
                self.check_condition_expression(if_stmt.condition());
                let narrowed_locals = self.narrowed_locals_for_condition(if_stmt.condition());
                self.check_block_with_locals(if_stmt.block(), current_return, narrowed_locals);
                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        self.check_condition_expression(else_if.condition());
                        let narrowed_locals =
                            self.narrowed_locals_for_condition(else_if.condition());
                        self.check_block_with_locals(
                            else_if.block(),
                            current_return,
                            narrowed_locals,
                        );
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    self.check_block(else_block, current_return);
                }
            }
            ast::Stmt::GenericFor(for_stmt) => self.check_generic_for(for_stmt, current_return),
            ast::Stmt::LocalFunction(function) => {
                let name = token_text(function.name());
                let return_types = self.function_return_types(&name);
                let scoped_locals = self.function_parameter_locals(&name);

                self.scoped_locals.push(scoped_locals);
                self.check_block(function.body().block(), return_types.as_deref());
                self.scoped_locals.pop();
            }
            ast::Stmt::FunctionDeclaration(function) => {
                self.record_function_declaration_type(function);

                let name = function.name().to_string();
                let return_types = self.function_return_types(&name);
                let scoped_locals = self.function_parameter_locals(&name);

                self.scoped_locals.push(scoped_locals);
                self.check_block(function.body().block(), return_types.as_deref());
                self.scoped_locals.pop();
            }
            ast::Stmt::NumericFor(for_stmt) => self.check_numeric_for(for_stmt, current_return),
            ast::Stmt::Repeat(repeat) => {
                self.check_condition_expression(repeat.until());
                self.check_block(repeat.block(), current_return);
            }
            ast::Stmt::While(while_stmt) => {
                self.check_condition_expression(while_stmt.condition());
                self.check_block(while_stmt.block(), current_return);
            }
            _ => {}
        }
    }

    fn check_condition_expression(&mut self, expression: &Expression) {
        match expression {
            Expression::BinaryOperator { lhs, binop, rhs } => {
                if is_equality_operator(binop) {
                    self.check_equality_comparison(lhs, binop, rhs);
                }

                self.check_condition_expression(lhs);
                self.check_condition_expression(rhs);
            }
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. } => {
                self.check_condition_expression(expression);
            }
            Expression::FunctionCall(function_call) => {
                self.check_function_call(function_call);
            }
            Expression::TableConstructor(table) => {
                for field in table.fields() {
                    if let Field::NameKey { value, .. } = field {
                        self.check_condition_expression(value);
                    }
                }
            }
            _ => {}
        }
    }

    fn check_generic_for(&mut self, for_stmt: &ast::GenericFor, current_return: Option<&[Type]>) {
        for expression in for_stmt.expressions() {
            if let Expression::FunctionCall(function_call) = expression {
                self.check_function_call(function_call);
            } else {
                self.infer_expression(expression);
            }
        }

        let locals = for_stmt
            .names()
            .into_iter()
            .zip(self.generic_for_variable_types(for_stmt))
            .map(|(name, ty)| (token_text(name), ty))
            .collect::<BTreeMap<_, _>>();
        self.check_block_with_locals(for_stmt.block(), current_return, locals);
    }

    fn check_numeric_for(&mut self, for_stmt: &ast::NumericFor, current_return: Option<&[Type]>) {
        self.infer_expression(for_stmt.start());
        self.infer_expression(for_stmt.end());

        if let Some(step) = for_stmt.step() {
            self.infer_expression(step);
        }

        self.check_block_with_locals(
            for_stmt.block(),
            current_return,
            BTreeMap::from([(token_text(for_stmt.index_variable()), Type::number())]),
        );
    }

    fn check_equality_comparison(
        &mut self,
        lhs: &Expression,
        binop: &ast::BinOp,
        rhs: &Expression,
    ) {
        let Some(left) = self.infer_expression(lhs) else {
            return;
        };
        let Some(right) = self.infer_expression(rhs) else {
            return;
        };

        if self.types_can_overlap(&left, &right) {
            return;
        }

        self.diagnostics.push(impossible_comparison_diagnostic(
            &self.parsed.path,
            binop,
            &left,
            &right,
        ));
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

        self.record_local_assignment_types(local);
    }

    fn record_local_assignment_types(&mut self, local: &ast::LocalAssignment) {
        let expressions = local.expressions().iter().collect::<Vec<_>>();
        let specifiers = local.type_specifiers().collect::<Vec<_>>();

        for (index, name) in local.names().iter().enumerate() {
            let name = token_text(name);
            let has_annotation = specifiers
                .get(index)
                .and_then(|specifier| *specifier)
                .is_some();
            let ty = specifiers
                .get(index)
                .and_then(|specifier| *specifier)
                .and_then(|_| self.environment.local(&name).cloned())
                .or_else(|| {
                    assignment_expression_for_local(&expressions, index).and_then(
                        |(expression, return_index)| {
                            self.infer_expression_return(expression, return_index)
                        },
                    )
                });

            if let Some(ty) = ty {
                let ty = if has_annotation {
                    ty
                } else {
                    widen_inferred_local_type(ty)
                };
                self.insert_scoped_local(name, ty);
            }
        }
    }

    fn insert_scoped_local(&mut self, name: String, ty: Type) {
        if let Some(scope) = self.scoped_locals.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn check_assignment(&mut self, assignment: &ast::Assignment) {
        for (variable, expression) in assignment.variables().iter().zip(assignment.expressions()) {
            let source = self.infer_expression(expression);

            if let (Some(source), Some(target)) =
                (&source, self.infer_var_assignment_target(variable))
            {
                self.check_type_assignable(expression, source, &target);
            }

            self.record_assignment_type(variable, source, expression);
        }
    }

    fn record_assignment_type(
        &mut self,
        variable: &Var,
        ty: Option<Type>,
        expression: &Expression,
    ) {
        if self.record_table_field_assignment_type(variable, ty.clone(), expression) {
            return;
        }

        let Some(name) = directly_assigned_name(variable) else {
            return;
        };

        match ty {
            Some(ty) => self.insert_scoped_local(name, widen_inferred_local_type(ty)),
            None => self.clear_scoped_local(&name),
        }
    }

    fn clear_scoped_local(&mut self, name: &str) {
        if let Some(scope) = self
            .scoped_locals
            .iter_mut()
            .rev()
            .find(|scope| scope.contains_key(name))
        {
            scope.remove(name);
        }
    }

    fn record_function_declaration_type(&mut self, function: &ast::FunctionDeclaration) {
        let Some((root, field, is_method)) = function_declaration_table_field(function) else {
            return;
        };
        let is_constructor = is_method && function_body_returns_setmetatable_self(function.body());
        let ty = self
            .environment
            .function(function.name().to_string().trim())
            .and_then(TypedFunction::as_type)
            .unwrap_or_else(|| function_type_from_body_with_receiver(function.body(), is_method));

        self.record_known_table_field_type(&root, &field, ty, false, is_constructor);
    }

    fn record_table_field_assignment_type(
        &mut self,
        variable: &Var,
        ty: Option<Type>,
        expression: &Expression,
    ) -> bool {
        let Some(parts) = plain_field_chain_from_var(variable) else {
            return false;
        };
        let [root, field] = parts.as_slice() else {
            return false;
        };

        let is_self_index = field == "__index" && expression_is_name(expression, root);
        let Some(ty) = ty else {
            return true;
        };

        if self
            .infer_named_value(root)
            .is_some_and(|root_type| self.field_assignment_uses_newindex(&root_type, field))
        {
            return true;
        }

        self.record_known_table_field_type(
            root,
            field,
            widen_inferred_local_type(ty),
            is_self_index,
            false,
        );
        true
    }

    fn field_assignment_uses_newindex(&self, root_type: &Type, field: &str) -> bool {
        let Type::Table(table) = resolve_aliases(root_type, self.environment, 0) else {
            return false;
        };

        table.field(field).is_none()
            && table
                .metatable_facts()
                .is_some_and(|facts| facts.newindex.assignment_type_for_key(field).is_some())
    }

    fn record_known_table_field_type(
        &mut self,
        root: &str,
        field: &str,
        ty: Type,
        is_self_index: bool,
        is_constructor: bool,
    ) {
        let Some(mut root_ty) = self.infer_named_value(root) else {
            return;
        };
        let Type::Table(table) = &mut root_ty else {
            return;
        };

        if is_self_index {
            table.mark_self_index();
        }

        if is_constructor {
            table.mark_constructor_method(field);
        }

        table.insert(field.to_owned(), TableField::required(ty));
        self.insert_scoped_local(root.to_owned(), root_ty);
    }

    fn check_expression_assignable(&mut self, expression: &Expression, target: &Type) {
        let Some(source) = self.infer_expression(expression) else {
            return;
        };

        self.check_type_assignable(expression, &source, target);
    }

    fn check_type_assignable(&mut self, node: &impl Node, source: &Type, target: &Type) {
        if !self.context.is_assignable(source, target) {
            let target = self.display_type_for_mismatch(target);
            self.diagnostics.push(mismatch_diagnostic(
                &self.parsed.path,
                node,
                &target,
                source,
            ));
        }
    }

    fn display_type_for_mismatch(&self, target: &Type) -> Type {
        match target {
            Type::Alias(name) => self
                .context
                .alias(name)
                .filter(|ty| type_contains_string_literal(ty))
                .cloned()
                .unwrap_or_else(|| target.clone()),
            _ => target.clone(),
        }
    }

    fn check_return(&mut self, return_stmt: &Return, target: Option<&[Type]>) {
        let Some(target) = target else {
            return;
        };

        let expressions = return_stmt.returns().iter().collect::<Vec<_>>();

        if expressions.is_empty() {
            self.check_type_assignable(
                return_stmt,
                &Type::nil(),
                target.first().unwrap_or(&Type::nil()),
            );
            return;
        }

        for (index, target) in target.iter().enumerate() {
            let source = assignment_expression_for_local(&expressions, index)
                .and_then(|(expression, return_index)| {
                    self.infer_expression_return(expression, return_index)
                })
                .unwrap_or_else(Type::nil);

            self.check_type_assignable(return_stmt, &source, target);
        }
    }

    fn infer_expression(&mut self, expression: &Expression) -> Option<Type> {
        self.infer_expression_return(expression, 0)
    }

    fn infer_expression_return(
        &mut self,
        expression: &Expression,
        return_index: usize,
    ) -> Option<Type> {
        match expression {
            Expression::FunctionCall(function_call) => self
                .check_function_call_returns(function_call)
                .get(return_index)
                .cloned(),
            _ if return_index > 0 => None,
            Expression::BinaryOperator { lhs, binop, rhs } => {
                let left = self.infer_expression(lhs)?;
                let right = self.infer_expression(rhs)?;

                self.infer_binary_operator_type(binop, &left, &right)
            }
            Expression::UnaryOperator { unop, expression } => {
                let operand = self.infer_expression(expression)?;

                self.infer_unary_operator_type(unop, &operand)
            }
            Expression::Number(_) => Some(Type::number()),
            Expression::String(value) => {
                Some(Type::string_literal(unquote_string(&value.to_string())))
            }
            Expression::Symbol(symbol) => infer_symbol_type(&token_text(symbol)),
            Expression::TableConstructor(table) => Some(self.infer_table_constructor(table)),
            Expression::Parentheses { expression, .. } => {
                self.infer_expression_return(expression, return_index)
            }
            Expression::TypeAssertion { expression, .. } => {
                self.infer_expression_return(expression, return_index)
            }
            Expression::Function(function) => Some(function_type_from_body(function.body())),
            Expression::Var(var) => self.infer_var_value(var),
            _ => None,
        }
    }

    fn infer_table_constructor(&mut self, table: &TableConstructor) -> Type {
        let mut fields = Vec::new();
        let mut array_items = Vec::new();

        for field in table.fields() {
            match field {
                Field::ExpressionKey { key, value, .. } => {
                    let ty = self.infer_expression(value).unwrap_or(Type::Any);

                    if let Some(key) = string_literal_expression(key) {
                        fields.push((key, TableField::required(ty)));
                    } else if expression_is_number(key) {
                        array_items.push(ty);
                    }
                }
                Field::NameKey { key, value, .. } => {
                    let key_text = token_text(key);
                    let ty = if key_text == "__newindex" {
                        self.infer_newindex_expression(value)
                    } else {
                        self.infer_expression(value)
                    }
                    .unwrap_or(Type::Any);

                    if key_text == "__index" {
                        fields.extend(index_fields_from_expression(value));
                    }

                    fields.push((key_text, TableField::required(ty)));
                }
                Field::NoKey(value) => {
                    array_items.push(self.infer_expression(value).unwrap_or(Type::Any));
                }
                _ => {}
            }
        }

        table_or_array_type(fields, array_items)
    }

    fn infer_newindex_expression(&mut self, expression: &Expression) -> Option<Type> {
        match expression {
            Expression::TableConstructor(table) => {
                Some(Type::table(table.fields().iter().filter_map(|field| {
                    let Field::NameKey { key, value, .. } = field else {
                        return None;
                    };

                    Some((
                        token_text(key),
                        TableField::required(type_literal_expression(value)?),
                    ))
                })))
            }
            _ => self.infer_expression(expression),
        }
    }

    fn infer_var_value(&mut self, var: &Var) -> Option<Type> {
        match var {
            Var::Name(name) => self.infer_named_value(&token_text(name)),
            Var::Expression(expression) => {
                if let Some(parts) = plain_field_chain_from_var_expression(expression) {
                    if let Some(ty) = self.resolve_field_chain_type(&parts) {
                        return Some(ty);
                    }
                }

                self.infer_var_expression(expression)
            }
            _ => None,
        }
    }

    fn infer_var_expression(&mut self, expression: &ast::VarExpression) -> Option<Type> {
        let mut current = match expression.prefix() {
            Prefix::Name(name) => self.infer_named_value(&token_text(name))?,
            Prefix::Expression(expression) => self.infer_expression(expression)?,
            _ => return None,
        };

        for suffix in expression.suffixes() {
            match suffix {
                Suffix::Index(Index::Dot { name, .. }) => {
                    current = self.resolve_table_field_type(&current, &token_text(name))?;
                }
                Suffix::Index(Index::Brackets { expression, .. }) => {
                    let key = self.index_key_type(expression)?;
                    current = resolve_index_type(self.environment, &current, &key)?;
                }
                _ => return None,
            }
        }

        Some(current)
    }

    fn index_key_type(&mut self, expression: &Expression) -> Option<Type> {
        match expression {
            Expression::Number(_) => Some(Type::number()),
            Expression::String(value) => {
                Some(Type::string_literal(unquote_string(&value.to_string())))
            }
            _ => self.infer_expression(expression),
        }
    }

    fn infer_named_value(&self, name: &str) -> Option<Type> {
        self.scoped_locals
            .iter()
            .rev()
            .find_map(|locals| locals.get(name).cloned())
            .or_else(|| self.environment.local(name).cloned())
            .or_else(|| {
                self.environment
                    .function(name)
                    .and_then(TypedFunction::as_type)
            })
    }

    fn check_function_call(&mut self, function_call: &FunctionCall) -> Option<Type> {
        self.check_function_call_returns(function_call)
            .into_iter()
            .next()
    }

    fn check_function_call_returns(&mut self, function_call: &FunctionCall) -> Vec<Type> {
        self.check_asset_loader_call(function_call);
        self.check_dot_love_method_call(function_call);

        if let Some(ty) = self.setmetatable_return_type(function_call) {
            return vec![ty];
        }

        if is_tostring_call(function_call) {
            return vec![Type::string()];
        }

        let Some(callee) = self.infer_call_target(function_call) else {
            return Vec::new();
        };

        match callee {
            Callable::Any => vec![Type::Any],
            Callable::Function(function) => {
                self.check_call_arguments(function_call, &function);

                if let Some(ty) = self.constructor_call_return_type(function_call) {
                    return vec![ty];
                }

                function.return_types_or_any().to_vec()
            }
            Callable::ExternalFunction(function) => {
                self.check_external_call_arguments(function_call, &function);

                function.return_types
            }
        }
    }

    fn setmetatable_return_type(&mut self, function_call: &FunctionCall) -> Option<Type> {
        let (target, meta) = setmetatable_arguments(function_call)?;
        let base = self.infer_expression(target)?;
        let meta = self.infer_expression(meta)?;

        Some(merge_static_metatable(base, meta))
    }

    fn record_setmetatable_call(&mut self, function_call: &FunctionCall) {
        let Some((target, meta)) = setmetatable_arguments(function_call) else {
            return;
        };
        let Expression::Var(Var::Name(name)) = target else {
            return;
        };
        let Some(base) = self.infer_named_value(&token_text(name)) else {
            return;
        };
        let Some(meta) = self.infer_expression(meta) else {
            return;
        };

        self.insert_scoped_local(token_text(name), merge_static_metatable(base, meta));
    }

    fn check_asset_loader_call(&mut self, function_call: &FunctionCall) {
        let Some(asset_call) = asset_loader_call(function_call) else {
            return;
        };

        if let Some(extension) = Path::new(&asset_call.path)
            .extension()
            .and_then(|extension| extension.to_str())
        {
            let extension = extension.to_ascii_lowercase();

            if !asset_call
                .kind
                .supported_extensions()
                .contains(&extension.as_str())
            {
                self.push_asset_diagnostic(unsupported_asset_extension_diagnostic(
                    &self.parsed.path,
                    function_call,
                    asset_call.kind,
                    &asset_call.path,
                ));
                return;
            }
        } else {
            self.push_asset_diagnostic(unsupported_asset_extension_diagnostic(
                &self.parsed.path,
                function_call,
                asset_call.kind,
                &asset_call.path,
            ));
            return;
        }

        if !self.asset_exists(&asset_call.path) {
            self.push_asset_diagnostic(missing_asset_diagnostic(
                &self.parsed.path,
                function_call,
                asset_call.kind,
                &asset_call.path,
            ));
        }
    }

    fn asset_exists(&self, asset_path: &str) -> bool {
        let path = Path::new(asset_path);

        if path.is_absolute() {
            return path.exists();
        }

        if self.asset_roots.is_empty() {
            return true;
        }

        self.asset_roots.iter().any(|root| root.join(path).exists())
    }

    fn push_asset_diagnostic(&mut self, diagnostic: Diagnostic) {
        let key = (
            diagnostic.line,
            diagnostic.column,
            diagnostic
                .code
                .expect("asset diagnostics should always have codes")
                .as_u16(),
            diagnostic.message.clone(),
        );

        if self.reported_asset_diagnostics.insert(key) {
            self.diagnostics.push(diagnostic);
        }
    }

    fn check_dot_love_method_call(&mut self, function_call: &FunctionCall) {
        let Some(dot_call) = dot_method_call(function_call) else {
            return;
        };

        if self
            .love_object_method_call_target(&dot_call.parts)
            .is_none()
        {
            return;
        }

        let key = (
            dot_call.position.line,
            dot_call.position.column,
            dot_call.method.clone(),
        );

        if self.reported_method_style_diagnostics.insert(key) {
            self.diagnostics.push(dot_method_call_diagnostic(
                &self.parsed.path,
                dot_call.position,
                &dot_call.receiver,
                &dot_call.method,
            ));
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

            if let Some(kind) = asset::loader_kind(root) {
                return Some(Callable::Function(kind.function_type()));
            }
        }

        if !fields.is_empty() {
            if let Some(function) = self.love_object_method_call_target(&parts) {
                return Some(Callable::ExternalFunction(function));
            }

            if let Some(export) = required_module_export_type_for_fields(self.parsed, root, fields)
            {
                return callable_from_type(&export, self.environment);
            }
        }

        let callee_type = self.resolve_field_chain_type(&parts)?;

        callable_from_type(&callee_type, self.environment)
    }

    fn love_object_method_call_target(&self, parts: &[String]) -> Option<ExternalFunctionType> {
        let method = parts.last()?;
        let receiver_parts = parts.get(..parts.len().checked_sub(1)?)?;
        let receiver_type = self.resolve_field_chain_type(receiver_parts)?;
        let item = lua_type_field_item(self.environment, &receiver_type, method)?;

        external_function_type_from_lua_item(item)
    }

    fn constructor_call_return_type(&self, function_call: &FunctionCall) -> Option<Type> {
        let parts = field_chain_before_first_call(function_call)?;
        let (method, receiver_parts) = parts.split_last()?;
        let receiver_type = self.resolve_field_chain_type(receiver_parts)?;
        let Type::Table(table) = resolve_aliases(&receiver_type, self.environment, 0) else {
            return None;
        };

        (table.has_self_index() && table.is_constructor_method(method)).then_some(receiver_type)
    }

    fn generic_for_variable_types(&mut self, for_stmt: &ast::GenericFor) -> Vec<Type> {
        let Some(Expression::FunctionCall(function_call)) = for_stmt.expressions().iter().next()
        else {
            return vec![Type::Any; for_stmt.names().into_iter().count()];
        };
        let Some((iterator, value_expression)) = iterator_call(function_call) else {
            return vec![Type::Any; for_stmt.names().into_iter().count()];
        };
        let value_type = self.infer_expression(value_expression).unwrap_or(Type::Any);

        match iterator.as_str() {
            "ipairs" => iterator_metamethod_pair_types(self.environment, &value_type, "__ipairs")
                .map(|(key, value)| vec![key, value])
                .unwrap_or_else(|| {
                    vec![
                        Type::number(),
                        array_element_type(self.environment, &value_type).unwrap_or(Type::Any),
                    ]
                }),
            "pairs" => iterator_metamethod_pair_types(self.environment, &value_type, "__pairs")
                .or_else(|| pair_key_value_types(self.environment, &value_type))
                .map(|(key, value)| vec![key, value])
                .unwrap_or_else(|| vec![Type::Any, Type::Any]),
            _ => vec![Type::Any; for_stmt.names().into_iter().count()],
        }
    }

    fn check_call_arguments(&mut self, function_call: &FunctionCall, function: &FunctionType) {
        let arguments = self.call_arguments_for_function(function_call);

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
            match argument {
                CallArgument::Expression(argument) => {
                    self.check_expression_assignable(argument, parameter);
                }
                CallArgument::Inferred(argument_type) => {
                    if !self.context.is_assignable(&argument_type, parameter) {
                        self.diagnostics.push(mismatch_diagnostic(
                            &self.parsed.path,
                            function_call,
                            parameter,
                            &argument_type,
                        ));
                    }
                }
            }
        }
    }

    fn call_arguments_for_function<'b>(
        &mut self,
        function_call: &'b FunctionCall,
    ) -> Vec<CallArgument<'b>> {
        let mut arguments = call_arguments(function_call).unwrap_or_default();

        if !is_method_call(function_call) {
            return arguments;
        }

        let Some(parts) = field_chain_before_first_call(function_call) else {
            return arguments;
        };
        let Some(receiver_parts) = parts.get(..parts.len().saturating_sub(1)) else {
            return arguments;
        };
        let Some(receiver_type) = self.resolve_field_chain_type(receiver_parts) else {
            return arguments;
        };

        arguments.insert(0, CallArgument::Inferred(receiver_type));
        arguments
    }

    fn check_external_call_arguments(
        &mut self,
        function_call: &FunctionCall,
        function: &ExternalFunctionType,
    ) {
        let arguments = call_arguments(function_call).unwrap_or_default();
        let required_parameter_count = function.required_parameter_count();

        if arguments.len() < required_parameter_count {
            self.diagnostics.push(argument_count_diagnostic(
                &self.parsed.path,
                function_call,
                required_parameter_count,
                arguments.len(),
            ));
            return;
        }

        if arguments.len() > function.parameters.len() {
            self.diagnostics.push(argument_count_diagnostic(
                &self.parsed.path,
                function_call,
                function.parameters.len(),
                arguments.len(),
            ));
            return;
        }

        for (argument, parameter) in arguments.into_iter().zip(&function.parameters) {
            match argument {
                CallArgument::Expression(argument) => {
                    let Some(argument_type) = self.infer_expression(argument) else {
                        continue;
                    };

                    self.check_external_argument_type(argument, &argument_type, &parameter.ty);
                }
                CallArgument::Inferred(argument_type) => {
                    self.check_external_argument_type(function_call, &argument_type, &parameter.ty);
                }
            }
        }
    }

    fn check_external_argument_type(&mut self, node: &impl Node, source: &Type, target: &Type) {
        if self.external_argument_assignable(source, target) {
            return;
        }

        self.diagnostics
            .push(mismatch_diagnostic(&self.parsed.path, node, target, source));
    }

    fn external_argument_assignable(&self, source: &Type, target: &Type) -> bool {
        self.context.is_assignable(source, target)
            || matches!(
                source,
                Type::Primitive(crate::PrimitiveType::String) | Type::StringLiteral(_)
            ) && matches!(target, Type::Alias(name) if self.context.alias(name).is_none())
    }

    fn function_return_types(&self, name: &str) -> Option<Vec<Type>> {
        self.environment
            .function(name.trim())
            .map(|function| function.return_types_or_any())
    }

    fn infer_var_assignment_target(&self, var: &Var) -> Option<Type> {
        match var {
            Var::Name(name) => self.infer_named_assignment_target(&token_text(name)),
            Var::Expression(expression) => {
                let parts = plain_field_chain_from_var_expression(expression)?;

                self.resolve_field_chain_type(&parts)
                    .or_else(|| self.resolve_newindex_assignment_type(&parts))
            }
            _ => None,
        }
    }

    fn resolve_newindex_assignment_type(&self, parts: &[String]) -> Option<Type> {
        let (field, receiver_parts) = parts.split_last()?;
        let receiver_type = self.resolve_field_chain_type(receiver_parts)?;

        self.resolve_metatable_facts(&receiver_type)?
            .newindex
            .assignment_type_for_key(field)
    }

    fn infer_named_assignment_target(&self, name: &str) -> Option<Type> {
        self.environment.local(name).cloned().or_else(|| {
            self.scoped_locals
                .iter()
                .rev()
                .skip(1)
                .find_map(|locals| locals.get(name).cloned())
        })
    }

    fn resolve_field_chain_type(&self, parts: &[String]) -> Option<Type> {
        let (root, fields) = parts.split_first()?;
        if !fields.is_empty() {
            if let Some(export) = required_module_export_type_for_fields(self.parsed, root, fields)
            {
                return Some(export);
            }
        }

        let mut current = self.infer_named_value(root)?;

        for field in fields {
            current = self.resolve_table_field_type(&current, field)?;
        }

        Some(current)
    }

    fn resolve_table_field_type(&self, ty: &Type, field: &str) -> Option<Type> {
        match resolve_aliases(ty, self.environment, 0) {
            Type::Table(table) => table.semantic_field(field).map(|field| field.ty.clone()),
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

    fn resolve_metatable_facts(&self, ty: &Type) -> Option<MetatableFacts> {
        match resolve_aliases(ty, self.environment, 0) {
            Type::Table(table) => table.metatable_facts().cloned(),
            Type::Union(types) => {
                for ty in types {
                    if let Some(facts) = self.resolve_metatable_facts(&ty) {
                        return Some(facts);
                    }
                }

                None
            }
            _ => None,
        }
    }

    fn function_parameter_locals(&self, name: &str) -> BTreeMap<String, Type> {
        self.environment
            .function(name.trim())
            .map(|function| {
                function
                    .parameters
                    .iter()
                    .map(|parameter| (parameter.name.clone(), parameter.ty.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn types_can_overlap(&self, left: &Type, right: &Type) -> bool {
        types_can_overlap(
            &resolve_aliases(left, self.environment, 0),
            &resolve_aliases(right, self.environment, 0),
        )
    }

    fn narrowed_locals_for_condition(&self, condition: &Expression) -> BTreeMap<String, Type> {
        let aliases = self
            .environment
            .aliases()
            .map(|(name, ty)| (name.to_owned(), ty.clone()))
            .collect::<BTreeMap<_, _>>();

        narrowed_locals_for_condition(condition, &aliases, |name| self.infer_named_value(name))
    }

    fn infer_binary_operator_type(
        &self,
        binop: &ast::BinOp,
        left: &Type,
        right: &Type,
    ) -> Option<Type> {
        if is_numeric_operator(binop) && self.is_number_type(left) && self.is_number_type(right) {
            return Some(Type::number());
        }

        if matches!(binop, ast::BinOp::TwoDots(_))
            && self.is_concat_operand_type(left)
            && self.is_concat_operand_type(right)
        {
            return Some(Type::string());
        }

        if let Some(name) = arithmetic_metamethod_name(binop) {
            if let Some(function) = self.binary_metamethod(left, right, name) {
                return Some(metamethod_return_or(&function, left.clone()));
            }
        }

        if let Some(name) = comparison_metamethod_name(binop) {
            if let Some(function) = self.binary_metamethod(left, right, name) {
                return Some(metamethod_return_or(&function, Type::boolean()));
            }
        }

        if matches!(left, Type::Any) || matches!(right, Type::Any) {
            return Some(Type::Any);
        }

        None
    }

    fn infer_unary_operator_type(&self, unop: &ast::UnOp, operand: &Type) -> Option<Type> {
        match unop {
            ast::UnOp::Hash(_) => {
                if let Some(function) = self
                    .resolve_metatable_facts(operand)
                    .and_then(|facts| facts.len)
                {
                    return Some(metamethod_return_or(&function, Type::number()));
                }

                if matches!(operand, Type::Any) {
                    Some(Type::Any)
                } else {
                    Some(Type::number())
                }
            }
            ast::UnOp::Minus(_) => {
                if self.is_number_type(operand) {
                    Some(Type::number())
                } else if matches!(operand, Type::Any) {
                    Some(Type::Any)
                } else {
                    None
                }
            }
            ast::UnOp::Not(_) => Some(Type::boolean()),
            _ => {
                if matches!(operand, Type::Any) {
                    Some(Type::Any)
                } else {
                    Some(Type::number())
                }
            }
        }
    }

    fn binary_metamethod(&self, left: &Type, right: &Type, name: &str) -> Option<FunctionType> {
        self.resolve_metatable_facts(left)
            .and_then(|facts| facts.metamethod(name).cloned())
            .or_else(|| {
                self.resolve_metatable_facts(right)
                    .and_then(|facts| facts.metamethod(name).cloned())
            })
    }

    fn is_number_type(&self, ty: &Type) -> bool {
        match resolve_aliases(ty, self.environment, 0) {
            Type::Primitive(crate::PrimitiveType::Number) => true,
            Type::Union(types) => types.iter().all(|ty| self.is_number_type(ty)),
            _ => false,
        }
    }

    fn is_concat_operand_type(&self, ty: &Type) -> bool {
        match resolve_aliases(ty, self.environment, 0) {
            Type::Primitive(crate::PrimitiveType::Number | crate::PrimitiveType::String)
            | Type::StringLiteral(_) => true,
            Type::Union(types) => types.iter().all(|ty| self.is_concat_operand_type(ty)),
            _ => false,
        }
    }
}

enum Callable {
    Any,
    Function(FunctionType),
    ExternalFunction(ExternalFunctionType),
}

enum CallArgument<'a> {
    Expression(&'a Expression),
    Inferred(Type),
}

struct AssetLoaderCall {
    kind: asset::AssetLoaderKind,
    path: String,
}

struct DotMethodCall {
    parts: Vec<String>,
    receiver: String,
    method: String,
    position: SourcePosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExternalFunctionType {
    parameters: Vec<ExternalParameter>,
    return_types: Vec<Type>,
}

impl ExternalFunctionType {
    fn required_parameter_count(&self) -> usize {
        self.parameters
            .iter()
            .filter(|parameter| !parameter.optional)
            .count()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExternalParameter {
    ty: Type,
    optional: bool,
}

impl Callable {
    fn from_typed_function(function: &TypedFunction) -> Self {
        if function.has_fully_annotated_parameters() {
            Self::Function(FunctionType::new_returns(
                function
                    .parameters
                    .iter()
                    .map(|parameter| parameter.ty.clone()),
                function.return_types_or_any(),
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

fn is_equality_operator(binop: &ast::BinOp) -> bool {
    matches!(binop, ast::BinOp::TwoEqual(_) | ast::BinOp::TildeEqual(_))
}

fn is_numeric_operator(binop: &ast::BinOp) -> bool {
    matches!(
        binop,
        ast::BinOp::Caret(_)
            | ast::BinOp::Minus(_)
            | ast::BinOp::Percent(_)
            | ast::BinOp::Plus(_)
            | ast::BinOp::Slash(_)
            | ast::BinOp::Star(_)
    )
}

fn types_can_overlap(left: &Type, right: &Type) -> bool {
    match (left, right) {
        (Type::Any, _) | (_, Type::Any) => true,
        (Type::StringLiteral(_), Type::Primitive(crate::PrimitiveType::String))
        | (Type::Primitive(crate::PrimitiveType::String), Type::StringLiteral(_)) => true,
        (Type::StringLiteral(left), Type::StringLiteral(right)) => left == right,
        (Type::Primitive(left), Type::Primitive(right)) => left == right,
        (Type::Union(types), other) | (other, Type::Union(types)) => {
            types.iter().any(|ty| types_can_overlap(ty, other))
        }
        (Type::Function(_), Type::Function(_)) => true,
        (Type::Table(_), Type::Table(_)) => true,
        (Type::Array(_), Type::Array(_)) => true,
        (Type::Tuple(left), Type::Tuple(right)) => left.len() == right.len(),
        (Type::Dictionary { .. }, Type::Dictionary { .. }) => true,
        (Type::Alias(_), _) | (_, Type::Alias(_)) => true,
        (
            Type::Primitive(_)
            | Type::Function(_)
            | Type::Table(_)
            | Type::StringLiteral(_)
            | Type::Array(_)
            | Type::Tuple(_)
            | Type::Dictionary { .. },
            Type::Primitive(_)
            | Type::Function(_)
            | Type::Table(_)
            | Type::StringLiteral(_)
            | Type::Array(_)
            | Type::Tuple(_)
            | Type::Dictionary { .. },
        ) => false,
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

fn plain_field_chain_from_var(var: &Var) -> Option<Vec<String>> {
    match var {
        Var::Name(name) => Some(vec![token_text(name)]),
        Var::Expression(expression) => plain_field_chain_from_var_expression(expression),
        _ => None,
    }
}

fn expression_is_name(expression: &Expression, expected: &str) -> bool {
    matches!(expression, Expression::Var(Var::Name(name)) if token_text(name) == expected)
}

fn function_declaration_table_field(
    function: &ast::FunctionDeclaration,
) -> Option<(String, String, bool)> {
    let name = function.name().to_string();
    let colon = name.rfind(':');
    let dot = name.rfind('.');
    let separator = match (colon, dot) {
        (Some(colon), Some(dot)) => Some((colon.max(dot), colon > dot)),
        (Some(colon), None) => Some((colon, true)),
        (None, Some(dot)) => Some((dot, false)),
        (None, None) => None,
    }?;
    let owner = name[..separator.0].trim();
    let field = name[separator.0 + 1..].trim();

    (!owner.is_empty() && !field.is_empty())
        .then(|| (owner.to_owned(), field.to_owned(), separator.1))
}

fn field_chain_before_first_call(function_call: &FunctionCall) -> Option<Vec<String>> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in function_call.suffixes() {
        match suffix {
            Suffix::Index(Index::Dot { name, .. }) => parts.push(token_text(name)),
            Suffix::Call(Call::MethodCall(method)) => {
                parts.push(token_text(method.name()));
                return Some(parts);
            }
            Suffix::Call(_) => return Some(parts),
            _ => return None,
        }
    }

    None
}

fn assignment_expression_for_local<'a>(
    expressions: &[&'a Expression],
    index: usize,
) -> Option<(&'a Expression, usize)> {
    if expressions.is_empty() {
        return None;
    }

    let fixed_expression_count = expressions.len().saturating_sub(1);

    if index < fixed_expression_count {
        return expressions
            .get(index)
            .copied()
            .map(|expression| (expression, 0));
    }

    expressions
        .last()
        .copied()
        .map(|expression| (expression, index - fixed_expression_count))
}

fn lua_type_field_item(
    environment: &TypeEnvironment,
    ty: &Type,
    field: &str,
) -> Option<lua::LuaItem> {
    match resolve_aliases(ty, environment, 0) {
        Type::Alias(name) => lua::type_field_items(&name, field)
            .into_iter()
            .find(|item| item.label == field),
        Type::Union(types) => types
            .iter()
            .find_map(|ty| lua_type_field_item(environment, ty, field)),
        Type::Any
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::Table(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => None,
    }
}

fn external_function_type_from_lua_item(item: lua::LuaItem) -> Option<ExternalFunctionType> {
    let parameters = parameters_from_function_detail(item.detail)?;
    let mut return_types = return_types_from_function_detail(item.detail);

    if return_types.is_empty() {
        return_types.push(Type::Any);
    }

    Some(ExternalFunctionType {
        parameters,
        return_types,
    })
}

fn parameters_from_function_detail(detail: &str) -> Option<Vec<ExternalParameter>> {
    let start = detail.find('(')? + 1;
    let end = detail[start..].find(')')? + start;
    let parameters = detail[start..end].trim();

    if parameters.is_empty() {
        return Some(Vec::new());
    }

    Some(
        parameters
            .split(',')
            .map(str::trim)
            .filter(|parameter| !parameter.is_empty())
            .map(|parameter| {
                let (name, ty) = parameter
                    .split_once(':')
                    .map(|(name, ty)| (name.trim(), ty.trim()))
                    .unwrap_or(("", parameter));

                ExternalParameter {
                    ty: type_from_lua_label(ty).unwrap_or(Type::Any),
                    optional: name.ends_with('?'),
                }
            })
            .collect(),
    )
}

fn return_types_from_function_detail(detail: &str) -> Vec<Type> {
    let Some(start) = detail.rfind("): ").map(|index| index + "): ".len()) else {
        return Vec::new();
    };

    detail[start..]
        .split(',')
        .map(str::trim)
        .filter_map(type_from_lua_label)
        .collect()
}

fn type_from_lua_label(label: &str) -> Option<Type> {
    let label = label.trim();

    if label.contains('|') {
        let members = label
            .split('|')
            .filter_map(type_from_lua_label)
            .collect::<Vec<_>>();

        return match members.len() {
            0 => None,
            1 => members.into_iter().next(),
            _ => Some(Type::union(members)),
        };
    }

    match label {
        "any" => Some(Type::Any),
        "boolean" => Some(Type::boolean()),
        "nil" => Some(Type::nil()),
        "number" => Some(Type::number()),
        "string" => Some(Type::string()),
        "table" => Some(Type::empty_table()),
        other if is_quoted_string_literal(other) => {
            Some(Type::string_literal(unquote_string(other)))
        }
        other if !other.is_empty() => Some(Type::alias(other)),
        _ => None,
    }
}

fn call_arguments(function_call: &FunctionCall) -> Option<Vec<CallArgument<'_>>> {
    let call = function_call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;

    match call {
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            Some(arguments.iter().map(CallArgument::Expression).collect())
        }
        Call::AnonymousCall(FunctionArgs::String(value)) => Some(vec![CallArgument::Inferred(
            Type::string_literal(unquote_string(&value.to_string())),
        )]),
        Call::MethodCall(method) => match method.args() {
            FunctionArgs::Parentheses { arguments, .. } => {
                Some(arguments.iter().map(CallArgument::Expression).collect())
            }
            FunctionArgs::String(_) => Some(vec![CallArgument::Inferred(Type::string())]),
            FunctionArgs::TableConstructor(_) => {
                Some(vec![CallArgument::Inferred(Type::empty_table())])
            }
            _ => None,
        },
        _ => None,
    }
}

fn setmetatable_arguments(function_call: &FunctionCall) -> Option<(&Expression, &Expression)> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    if token_text(name) != "setmetatable" {
        return None;
    }

    let Some(Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }))) =
        function_call.suffixes().next()
    else {
        return None;
    };

    Some((arguments.iter().next()?, arguments.iter().nth(1)?))
}

fn is_tostring_call(function_call: &FunctionCall) -> bool {
    let Prefix::Name(name) = function_call.prefix() else {
        return false;
    };

    token_text(name) == "tostring"
}

fn merge_static_metatable(base: Type, meta: Type) -> Type {
    MetatableFacts::from_type(&meta).apply_to_type(base)
}

fn index_fields_from_expression(expression: &Expression) -> Vec<(String, TableField)> {
    let Expression::Function(function) = expression else {
        return Vec::new();
    };

    index_fields_from_block(function.body().block())
}

fn index_fields_from_block(block: &ast::Block) -> Vec<(String, TableField)> {
    let mut fields = Vec::new();

    for stmt in block.stmts() {
        if let ast::Stmt::If(if_stmt) = stmt {
            fields.extend(index_fields_from_if(if_stmt));
        }
    }

    fields
}

fn index_fields_from_if(if_stmt: &ast::If) -> Vec<(String, TableField)> {
    let Some(key) = string_key_comparison(if_stmt.condition()) else {
        return Vec::new();
    };
    let Some(LastStmt::Return(return_stmt)) = if_stmt.block().last_stmt() else {
        return Vec::new();
    };
    let Some(expression) = return_stmt.returns().iter().next() else {
        return Vec::new();
    };
    let ty = match expression {
        Expression::Number(_) => Type::number(),
        Expression::String(value) => Type::string_literal(unquote_string(&value.to_string())),
        Expression::Symbol(symbol) => infer_symbol_type(&token_text(symbol)).unwrap_or(Type::Any),
        Expression::TableConstructor(table) => {
            Type::table(table.fields().iter().filter_map(|field| {
                let Field::NameKey { key, .. } = field else {
                    return None;
                };

                Some((token_text(key), TableField::required(Type::Any)))
            }))
        }
        _ => Type::Any,
    };

    vec![(key, TableField::required(ty))]
}

fn string_key_comparison(expression: &Expression) -> Option<String> {
    let Expression::BinaryOperator { lhs, binop, rhs } = expression else {
        return None;
    };

    if !matches!(binop, ast::BinOp::TwoEqual(_)) {
        return None;
    }

    string_literal_expression(rhs).or_else(|| string_literal_expression(lhs))
}

fn string_literal_expression(expression: &Expression) -> Option<String> {
    match expression {
        Expression::String(value) => Some(unquote_string(&value.to_string())),
        _ => None,
    }
}

fn expression_is_number(expression: &Expression) -> bool {
    matches!(expression, Expression::Number(_))
}

fn is_method_call(function_call: &FunctionCall) -> bool {
    function_call
        .suffixes()
        .any(|suffix| matches!(suffix, Suffix::Call(Call::MethodCall(_))))
}

fn function_type_from_body(body: &ast::FunctionBody) -> Type {
    let parameters = function_parameter_types(body);
    let return_type = body
        .return_type()
        .and_then(|specifier| type_from_type_info(specifier.type_info()))
        .unwrap_or(Type::Any);

    Type::function(parameters, return_type)
}

fn function_parameter_types(body: &ast::FunctionBody) -> Vec<Type> {
    let parameters = body
        .parameters()
        .iter()
        .filter_map(|parameter| match parameter {
            ast::Parameter::Name(name) => Some((name.start_position()?, name.end_position()?)),
            ast::Parameter::Ellipsis(_) => None,
            _ => None,
        })
        .collect::<Vec<_>>();
    let specifiers = body
        .type_specifiers()
        .filter_map(|specifier| {
            let specifier = specifier?;
            Some((
                specifier.start_position()?,
                type_from_type_info(specifier.type_info())?,
            ))
        })
        .collect::<Vec<_>>();

    parameters
        .iter()
        .enumerate()
        .map(|(index, (_, parameter_end))| {
            let next_parameter_start = parameters.get(index + 1).map(|(start, _)| *start);

            specifiers
                .iter()
                .find_map(|(specifier_start, ty)| {
                    if specifier_start >= parameter_end
                        && next_parameter_start
                            .map(|next_start| specifier_start < &next_start)
                            .unwrap_or(true)
                    {
                        Some(ty.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or(Type::Any)
        })
        .collect()
}

fn function_type_from_body_with_receiver(body: &ast::FunctionBody, include_receiver: bool) -> Type {
    let Type::Function(mut function) = function_type_from_body(body) else {
        return Type::Any;
    };

    if include_receiver {
        function.parameters.insert(0, Type::Any);
    }

    Type::Function(function)
}

fn function_body_returns_setmetatable_self(body: &ast::FunctionBody) -> bool {
    let Some(LastStmt::Return(return_stmt)) = body.block().last_stmt() else {
        return false;
    };
    let Some(Expression::FunctionCall(function_call)) = return_stmt.returns().iter().next() else {
        return false;
    };
    let Some((_, meta)) = setmetatable_arguments(function_call) else {
        return false;
    };

    expression_is_name(meta, "self")
}

fn asset_loader_call(function_call: &FunctionCall) -> Option<AssetLoaderCall> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };
    let kind = asset::loader_kind(&token_text(name))?;
    let call = function_call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;
    let path = match call {
        Call::AnonymousCall(FunctionArgs::String(path)) => Some(unquote_string(&path.to_string())),
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            let Expression::String(path) = arguments.iter().next()? else {
                return None;
            };

            Some(unquote_string(&path.to_string()))
        }
        _ => None,
    }?;

    Some(AssetLoaderCall { kind, path })
}

fn dot_method_call(function_call: &FunctionCall) -> Option<DotMethodCall> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };
    let mut parts = vec![token_text(name)];
    let mut method_position = None;

    for suffix in function_call.suffixes() {
        match suffix {
            Suffix::Index(Index::Dot { name, .. }) => {
                method_position = name.start_position().map(SourcePosition::from_full_moon);
                parts.push(token_text(name));
            }
            Suffix::Call(Call::AnonymousCall(_)) => {
                let method = parts.last()?.clone();
                let receiver = parts[..parts.len().saturating_sub(1)].join(".");

                return Some(DotMethodCall {
                    parts,
                    receiver,
                    method,
                    position: method_position.unwrap_or(SourcePosition { line: 1, column: 1 }),
                });
            }
            Suffix::Call(Call::MethodCall(_)) => return None,
            _ => return None,
        }
    }

    None
}

fn default_asset_roots(path: &Path) -> Vec<PathBuf> {
    let Some(parent) = path.parent() else {
        return Vec::new();
    };

    if !parent.exists() {
        return Vec::new();
    }

    parent.ancestors().map(Path::to_path_buf).collect()
}

fn normalize_asset_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();

    for root in roots {
        if root.as_os_str().is_empty() {
            continue;
        }

        if !root.exists() {
            continue;
        }

        let root = root.canonicalize().unwrap_or_else(|_| root.clone());

        if seen.insert(root.clone()) {
            output.push(root);
        }
    }

    output
}

fn unquote_string(value: &str) -> String {
    value
        .trim()
        .strip_prefix(['"', '\''])
        .and_then(|value| value.strip_suffix(['"', '\'']))
        .unwrap_or_else(|| value.trim())
        .to_owned()
}

fn is_quoted_string_literal(value: &str) -> bool {
    let value = value.trim();

    (value.len() >= 2 && value.starts_with('"') && value.ends_with('"'))
        || (value.len() >= 2 && value.starts_with('\'') && value.ends_with('\''))
}

fn type_literal_expression(expression: &Expression) -> Option<Type> {
    match expression {
        Expression::Var(Var::Name(name)) => type_from_type_label(&token_text(name)),
        Expression::String(value) => Some(Type::string_literal(unquote_string(&value.to_string()))),
        _ => None,
    }
}

fn type_from_type_label(label: &str) -> Option<Type> {
    let label = label.trim();

    if is_quoted_string_literal(label) {
        return Some(Type::string_literal(unquote_string(label)));
    }

    match label {
        "number" => Some(Type::number()),
        "string" => Some(Type::string()),
        "boolean" => Some(Type::boolean()),
        "nil" => Some(Type::nil()),
        "any" => Some(Type::Any),
        _ => None,
    }
}

fn type_from_type_info(type_info: &TypeInfo) -> Option<Type> {
    match type_info {
        TypeInfo::Basic(token) => type_from_type_label(&token_text(token)),
        TypeInfo::String(token) => Some(Type::string_literal(unquote_string(&token_text(token)))),
        TypeInfo::Optional { base, .. } => {
            Some(Type::union([type_from_type_info(base)?, Type::nil()]))
        }
        TypeInfo::Union(union) => union
            .types()
            .iter()
            .map(type_from_type_info)
            .collect::<Option<Vec<_>>>()
            .map(Type::union),
        _ => type_from_type_label(&type_info.to_string()),
    }
}

fn callable_from_type(ty: &Type, environment: &TypeEnvironment) -> Option<Callable> {
    match resolve_aliases(ty, environment, 0) {
        Type::Any => Some(Callable::Any),
        Type::Function(function) => Some(Callable::Function(function)),
        Type::Table(table) => table
            .metatable_facts()
            .and_then(|facts| facts.call.clone())
            .map(|function| Callable::Function(call_metamethod_type(function))),
        Type::Union(types) => types
            .iter()
            .find_map(|ty| callable_from_type(ty, environment)),
        _ => None,
    }
}

fn call_metamethod_type(mut function: FunctionType) -> FunctionType {
    if !function.parameters.is_empty() {
        function.parameters.remove(0);
    }

    function
}

fn metamethod_return_or(function: &FunctionType, fallback: Type) -> Type {
    let ty = function
        .return_types_or_any()
        .first()
        .cloned()
        .unwrap_or(Type::Any);

    if matches!(ty, Type::Any) {
        fallback
    } else {
        ty
    }
}

fn arithmetic_metamethod_name(binop: &ast::BinOp) -> Option<&'static str> {
    match binop {
        ast::BinOp::Plus(_) => Some("__add"),
        ast::BinOp::Minus(_) => Some("__sub"),
        ast::BinOp::Star(_) => Some("__mul"),
        ast::BinOp::Slash(_) => Some("__div"),
        _ => None,
    }
}

fn comparison_metamethod_name(binop: &ast::BinOp) -> Option<&'static str> {
    match binop {
        ast::BinOp::TwoEqual(_) | ast::BinOp::TildeEqual(_) => Some("__eq"),
        ast::BinOp::LessThan(_) | ast::BinOp::GreaterThan(_) => Some("__lt"),
        ast::BinOp::LessThanEqual(_) | ast::BinOp::GreaterThanEqual(_) => Some("__le"),
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

fn resolve_index_type(
    environment: &TypeEnvironment,
    collection: &Type,
    key: &Type,
) -> Option<Type> {
    match resolve_aliases(collection, environment, 0) {
        Type::Array(item) => is_number_like(key).then_some(*item),
        Type::Tuple(items) => is_number_like(key).then(|| union_types(items)),
        Type::Dictionary {
            key: dict_key,
            value,
        } => key_accepts_index(&dict_key, key).then_some(*value),
        Type::Table(table) => match key {
            Type::StringLiteral(value) => table.semantic_field(value).map(|field| field.ty.clone()),
            Type::Primitive(crate::PrimitiveType::String) => {
                union_types_optional(table.semantic_fields().into_values().map(|field| field.ty))
            }
            _ => None,
        },
        Type::Union(types) => union_types_optional(
            types
                .iter()
                .filter_map(|ty| resolve_index_type(environment, ty, key)),
        ),
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_) => None,
    }
}

fn array_element_type(environment: &TypeEnvironment, ty: &Type) -> Option<Type> {
    match resolve_aliases(ty, environment, 0) {
        Type::Array(item) => Some(*item),
        Type::Tuple(items) => Some(union_types(items)),
        Type::Union(types) => union_types_optional(
            types
                .iter()
                .filter_map(|ty| array_element_type(environment, ty)),
        ),
        Type::Any
        | Type::Alias(_)
        | Type::Dictionary { .. }
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Table(_) => None,
    }
}

fn pair_key_value_types(environment: &TypeEnvironment, ty: &Type) -> Option<(Type, Type)> {
    match resolve_aliases(ty, environment, 0) {
        Type::Array(item) => Some((Type::number(), *item)),
        Type::Tuple(items) => Some((Type::number(), union_types(items))),
        Type::Dictionary { key, value } => Some((*key, *value)),
        Type::Table(table) => {
            let fields = table.semantic_fields();
            let keys = fields
                .keys()
                .cloned()
                .map(Type::string_literal)
                .collect::<Vec<_>>();
            let values = fields
                .into_values()
                .map(|field| field.ty)
                .collect::<Vec<_>>();

            Some((union_types(keys), union_types(values)))
        }
        Type::Union(types) => {
            let pairs = types
                .iter()
                .filter_map(|ty| pair_key_value_types(environment, ty))
                .collect::<Vec<_>>();
            let keys = pairs.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>();
            let values = pairs
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Vec<_>>();

            Some((union_types(keys), union_types(values)))
        }
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_) => None,
    }
}

fn iterator_metamethod_pair_types(
    environment: &TypeEnvironment,
    ty: &Type,
    name: &str,
) -> Option<(Type, Type)> {
    match resolve_aliases(ty, environment, 0) {
        Type::Table(table) => table
            .metatable_facts()
            .and_then(|facts| facts.metamethod(name))
            .and_then(pair_types_from_iterator_function),
        Type::Union(types) => {
            let pairs = types
                .iter()
                .filter_map(|ty| iterator_metamethod_pair_types(environment, ty, name))
                .collect::<Vec<_>>();
            let keys = pairs.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>();
            let values = pairs
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Vec<_>>();

            (!keys.is_empty()).then(|| (union_types(keys), union_types(values)))
        }
        Type::Any
        | Type::Alias(_)
        | Type::Array(_)
        | Type::Dictionary { .. }
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Tuple(_) => None,
    }
}

fn pair_types_from_iterator_function(function: &FunctionType) -> Option<(Type, Type)> {
    let returns = function.return_types_or_any();

    match returns {
        [Type::Tuple(items)] if items.len() >= 2 => {
            Some((items.first()?.clone(), items.get(1)?.clone()))
        }
        [key, value, ..] => Some((key.clone(), value.clone())),
        _ => None,
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
    let mut unique = Vec::new();
    for ty in types {
        if !unique.contains(&ty) {
            unique.push(ty);
        }
    }

    match unique.len() {
        0 => Type::Any,
        1 => unique.into_iter().next().unwrap_or(Type::Any),
        _ => Type::union(unique),
    }
}

fn union_types_optional(types: impl IntoIterator<Item = Type>) -> Option<Type> {
    let types = types.into_iter().collect::<Vec<_>>();

    (!types.is_empty()).then(|| union_types(types))
}

fn is_number_like(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(crate::PrimitiveType::Number))
}

fn key_accepts_index(expected: &Type, actual: &Type) -> bool {
    match (expected, actual) {
        (Type::Any, _) | (_, Type::Any) => true,
        (
            Type::Primitive(crate::PrimitiveType::String),
            Type::Primitive(crate::PrimitiveType::String),
        )
        | (Type::Primitive(crate::PrimitiveType::String), Type::StringLiteral(_))
        | (
            Type::Primitive(crate::PrimitiveType::Number),
            Type::Primitive(crate::PrimitiveType::Number),
        ) => true,
        (Type::StringLiteral(expected), Type::StringLiteral(actual)) => expected == actual,
        (Type::Union(types), actual) => types.iter().any(|ty| key_accepts_index(ty, actual)),
        (expected, Type::Union(types)) => types.iter().any(|ty| key_accepts_index(expected, ty)),
        _ => false,
    }
}

fn iterator_call(function_call: &FunctionCall) -> Option<(String, &Expression)> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };
    let Some(Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }))) =
        function_call.suffixes().next()
    else {
        return None;
    };

    Some((token_text(name), arguments.iter().next()?))
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

fn missing_asset_diagnostic(
    path: &Path,
    node: &impl Node,
    kind: asset::AssetLoaderKind,
    asset_path: &str,
) -> Diagnostic {
    let position = node_position(node);

    Diagnostic::error(
        path,
        position.line,
        position.column,
        format!("{} asset not found: {asset_path}", kind.asset_label()),
    )
    .with_code(DiagnosticCode::ASSET_MISSING)
}

fn unsupported_asset_extension_diagnostic(
    path: &Path,
    node: &impl Node,
    kind: asset::AssetLoaderKind,
    asset_path: &str,
) -> Diagnostic {
    let position = node_position(node);
    let supported = kind.supported_extensions().join(", ");

    Diagnostic::error(
        path,
        position.line,
        position.column,
        format!(
            "unsupported {} asset extension for `{asset_path}`; expected one of: {supported}",
            kind.asset_label()
        ),
    )
    .with_code(DiagnosticCode::ASSET_UNSUPPORTED_EXTENSION)
}

fn dot_method_call_diagnostic(
    path: &Path,
    position: SourcePosition,
    receiver: &str,
    method: &str,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!(
            "LOVE method `{method}` should be called with `:` so self is passed: use `{receiver}:{method}(...)`"
        ),
    )
    .with_code(DiagnosticCode::LINT_DOT_METHOD_CALL)
}

fn mismatch_diagnostic(path: &Path, node: &impl Node, expected: &Type, found: &Type) -> Diagnostic {
    let position = node_position(node);
    let found = mismatch_found_label(expected, found);

    Diagnostic::error(
        path,
        position.line,
        position.column,
        format!("type mismatch: expected {expected}, found {found}"),
    )
    .with_code(DiagnosticCode::TYPE_MISMATCH)
}

fn mismatch_found_label(expected: &Type, found: &Type) -> String {
    if type_contains_string_literal(expected) {
        found.to_string()
    } else {
        type_with_string_literals_erased(found).to_string()
    }
}

fn type_with_string_literals_erased(ty: &Type) -> Type {
    match ty {
        Type::StringLiteral(_) => Type::string(),
        Type::Union(types) => Type::union(types.iter().map(type_with_string_literals_erased)),
        Type::Function(function) => Type::function_returns(
            function
                .parameters
                .iter()
                .map(type_with_string_literals_erased),
            function
                .return_types_or_any()
                .iter()
                .map(type_with_string_literals_erased),
        ),
        Type::Table(table) => {
            let mut erased = crate::TableShape::from_fields(table.fields().map(|(name, field)| {
                let ty = if is_metamethod_field_name(name) {
                    field.ty.clone()
                } else {
                    type_with_string_literals_erased(&field.ty)
                };

                (
                    name.to_owned(),
                    crate::TableField {
                        ty,
                        optional: field.optional,
                    },
                )
            }));

            if let Some(facts) = table.metatable_facts() {
                erased.set_metatable_facts(facts.clone());
            }

            if table.has_self_index() {
                erased.mark_self_index();
            }

            for method in table.constructor_methods() {
                erased.mark_constructor_method(method);
            }

            Type::Table(erased)
        }
        Type::Array(item) => Type::array(type_with_string_literals_erased(item)),
        Type::Tuple(items) => Type::tuple(items.iter().map(type_with_string_literals_erased)),
        Type::Dictionary { key, value } => Type::dictionary(
            type_with_string_literals_erased(key),
            type_with_string_literals_erased(value),
        ),
        Type::Primitive(_) | Type::Any | Type::Alias(_) => ty.clone(),
    }
}

fn widen_inferred_local_type(ty: Type) -> Type {
    type_with_string_literals_erased(&ty)
}

fn is_metamethod_field_name(name: &str) -> bool {
    name.starts_with("__")
}

fn type_contains_string_literal(ty: &Type) -> bool {
    match ty {
        Type::StringLiteral(_) => true,
        Type::Union(types) => types.iter().any(type_contains_string_literal),
        Type::Function(function) => {
            function.parameters.iter().any(type_contains_string_literal)
                || function
                    .return_types_or_any()
                    .iter()
                    .any(type_contains_string_literal)
        }
        Type::Table(table) => table
            .fields()
            .any(|(_, field)| type_contains_string_literal(&field.ty)),
        Type::Array(item) => type_contains_string_literal(item),
        Type::Tuple(items) => items.iter().any(type_contains_string_literal),
        Type::Dictionary { key, value } => {
            type_contains_string_literal(key) || type_contains_string_literal(value)
        }
        Type::Primitive(_) | Type::Any | Type::Alias(_) => false,
    }
}

fn impossible_comparison_diagnostic(
    path: &Path,
    binop: &ast::BinOp,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let position = binop
        .token()
        .start_position()
        .map(SourcePosition::from_full_moon)
        .unwrap_or(SourcePosition { line: 1, column: 1 });
    let result = match binop {
        ast::BinOp::TwoEqual(_) => "false",
        ast::BinOp::TildeEqual(_) => "true",
        _ => unreachable!("only equality operators produce comparison diagnostics"),
    };

    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("comparison is always {result}: {left} and {right} do not overlap"),
    )
    .with_code(DiagnosticCode::TYPE_IMPOSSIBLE_COMPARISON)
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
