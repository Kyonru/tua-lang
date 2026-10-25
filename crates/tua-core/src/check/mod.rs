use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, Index, LastStmt, Prefix, Return,
    Suffix, TableConstructor, Var,
};
use full_moon::node::Node;

use crate::ast_utils::{
    assignment_expression_for_local, field_chain_before_first_call, plain_field_chain_from_var,
    plain_field_chain_from_var_expression,
};
use crate::classes;
use crate::config::{DiagnosticsConfig, EcsConfig, JitConfig};
use crate::control_flow::{block_fallthrough_position, statement_is_terminating_builtin};
use crate::inline_ignore::apply_inline_ignores;
use crate::linter::lint_parsed_source;
use crate::love_conf;
use crate::module_index::{
    require_bindings_for_parsed_source, required_module_export_type_for_fields,
    required_module_exports_for_receiver,
};
use crate::narrowing::{
    directly_assigned_name, extend_with_return_guards, forget_return_guards,
    narrowed_locals_after_false_condition, narrowed_locals_for_condition, return_guards_for_names,
    ReturnGuard,
};
use crate::parser::{parse_tua_source, ParsedLanguage, ParsedSource, SourcePosition, SourceSpan};
use crate::profile::{self, DiagnosticsTraceCategory};
use crate::runtime_type;
use crate::scope::{
    scope_map_parsed_source_with_globals, ScopeDeclarationKind, ScopeGlobalKind, ScopeResolution,
};
use crate::semantic::{resolve_index_type, resolve_table_field_type, SemanticModel};
use crate::semantic_domains::{BuiltinConstructor, BuiltinDomain, BuiltinDomainRegistry};
use crate::types::{
    build_type_environment_with_domain_registry, deduplicate_inferred_types,
    has_leading_lua_doc_type, widen_inferred_type, AssignabilityFailure, AssignabilityFailureKind,
    FunctionType, IndexFallback, MetatableFacts, TableField, Type, TypeContext, TypeEnvironment,
    TypedFunction,
};
use crate::{asset, love_callback, lua, signal, sprite, state_machine};
use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity};

mod query;
mod type_mismatch;

use query::{CheckQuery, CheckQueryOptions};
use type_mismatch::{
    mismatch_diagnostic, mismatch_diagnostic_at_position, mismatch_diagnostic_for_expression,
};

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
    check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config(
        parsed,
        asset_roots,
        diagnostics_config,
        &JitConfig::default(),
        &EcsConfig::default(),
    )
}

pub(crate) fn check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config(
    parsed: &ParsedSource,
    asset_roots: &[PathBuf],
    diagnostics_config: &DiagnosticsConfig,
    jit_config: &JitConfig,
    ecs_config: &EcsConfig,
) -> Result<(), Vec<Diagnostic>> {
    CheckQuery::new(
        parsed,
        CheckQueryOptions::new(
            asset_roots,
            diagnostics_config,
            jit_config,
            ecs_config,
            None,
        ),
    )
    .run()
}

pub(crate) fn check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config_and_asset_index(
    parsed: &ParsedSource,
    asset_roots: &[PathBuf],
    diagnostics_config: &DiagnosticsConfig,
    jit_config: &JitConfig,
    ecs_config: &EcsConfig,
    asset_index: Option<&asset::AssetIndex>,
) -> Result<(), Vec<Diagnostic>> {
    CheckQuery::new(
        parsed,
        CheckQueryOptions::new(
            asset_roots,
            diagnostics_config,
            jit_config,
            ecs_config,
            asset_index,
        ),
    )
    .run()
}

pub(crate) fn check_parsed_source_with_environment_asset_roots_diagnostics_jit_config_and_asset_index(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    asset_roots: &[PathBuf],
    diagnostics_config: &DiagnosticsConfig,
    jit_config: &JitConfig,
    asset_index: Option<&asset::AssetIndex>,
) -> Result<(), Vec<Diagnostic>> {
    CheckQuery::new(
        parsed,
        CheckQueryOptions::new(
            asset_roots,
            diagnostics_config,
            jit_config,
            &EcsConfig::default(),
            asset_index,
        ),
    )
    .run_with_environment(environment)
}

struct Checker<'a> {
    parsed: &'a ParsedSource,
    environment: &'a TypeEnvironment,
    context: TypeContext,
    scoped_locals: Vec<BTreeMap<String, Type>>,
    scoped_local_contracts: Vec<BTreeMap<String, Type>>,
    /// Locals declared with later values of a call, per block scope.
    scoped_return_guards: Vec<Vec<ReturnGuard>>,
    stable_inferred_locals: bool,
    unresolved_project_aliases: BTreeSet<String>,
    asset_roots: Vec<PathBuf>,
    asset_index: Option<&'a asset::AssetIndex>,
    reported_asset_diagnostics: BTreeSet<(usize, usize, u16, String)>,
    reported_method_style_diagnostics: BTreeSet<(usize, usize, String)>,
    checked_function_bodies: BTreeSet<(usize, usize)>,
    terminating_builtin_references: BTreeSet<(usize, usize)>,
    exact_module_references: BTreeMap<(usize, usize), String>,
    runtime_imports: BTreeMap<String, String>,
    unknown_record_fields: bool,
    /// Locals holding a shader asset, with the uniforms its source declares.
    shader_locals: BTreeMap<String, ShaderUniforms>,
    shader_sources: BTreeMap<PathBuf, Option<asset::ShaderSource>>,
    /// Numeric `for` variables whose literal bounds give every value.
    constant_loop_values: Vec<(String, Vec<i64>)>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
struct ShaderUniforms {
    asset_path: String,
    model: std::sync::Arc<asset::ShaderUniformSet>,
}

impl<'a> Checker<'a> {
    fn new(
        parsed: &'a ParsedSource,
        environment: &'a TypeEnvironment,
        asset_roots: &[PathBuf],
        asset_index: Option<&'a asset::AssetIndex>,
        diagnostics_config: &DiagnosticsConfig,
    ) -> Self {
        let scope = scope_map_parsed_source_with_globals(
            parsed,
            diagnostics_config.globals.iter().cloned(),
        );
        let terminating_builtin_references = scope
            .references()
            .iter()
            .filter_map(|reference| {
                (matches!(reference.name.as_str(), "error" | "assert")
                    && matches!(
                        &reference.resolution,
                        ScopeResolution::Global(global)
                            if global.kind == ScopeGlobalKind::LuaBuiltin
                    ))
                .then_some((reference.span.start.line, reference.span.start.column))
            })
            .collect();
        let exact_module_symbols = scope
            .declarations()
            .iter()
            .filter_map(|declaration| {
                let ScopeDeclarationKind::ModuleAlias { module } = &declaration.kind else {
                    return None;
                };
                environment
                    .exact_module_binding(&declaration.name)
                    .is_some()
                    .then_some((declaration.id, module.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        let exact_module_references = scope
            .references()
            .iter()
            .filter_map(|reference| {
                let module = exact_module_symbols.get(&reference.resolution.symbol_id()?)?;
                Some((
                    (reference.span.start.line, reference.span.start.column),
                    module.clone(),
                ))
            })
            .collect();
        let runtime_imports = require_bindings_for_parsed_source(parsed)
            .into_iter()
            .filter(|binding| crate::is_bundled_runtime_module(&binding.module))
            .map(|binding| (binding.alias, binding.module))
            .collect();

        Self {
            parsed,
            environment,
            context: environment.to_type_context(),
            scoped_locals: Vec::new(),
            scoped_local_contracts: Vec::new(),
            scoped_return_guards: Vec::new(),
            stable_inferred_locals: parsed.raw.language == ParsedLanguage::Tua,
            unresolved_project_aliases: unresolved_project_aliases(parsed, environment),
            asset_roots: if asset_index.is_some() {
                normalize_asset_roots_without_fs(asset_roots)
            } else {
                normalize_asset_roots(asset_roots)
            },
            asset_index,
            reported_asset_diagnostics: BTreeSet::new(),
            reported_method_style_diagnostics: BTreeSet::new(),
            checked_function_bodies: BTreeSet::new(),
            terminating_builtin_references,
            exact_module_references,
            runtime_imports,
            unknown_record_fields: diagnostics_config.unknown_record_fields,
            shader_locals: BTreeMap::new(),
            shader_sources: BTreeMap::new(),
            constant_loop_values: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn check_block(&mut self, block: &ast::Block, current_return: Option<&[Type]>) {
        self.check_block_with_locals(block, current_return, BTreeMap::new());
    }

    fn remap_parser_diagnostics(&mut self) {
        for diagnostic in &mut self.diagnostics {
            let parser_start = SourcePosition {
                line: diagnostic.line,
                column: diagnostic.column,
            };
            let parser_end = SourcePosition {
                line: diagnostic.end_line,
                column: diagnostic.end_column,
            };
            let Some(start) = self.parsed.raw.parser_position_origin(parser_start) else {
                continue;
            };
            let end = self
                .parsed
                .raw
                .parser_position_origin(parser_end)
                .unwrap_or(start);
            let source_start = start.source_position;
            let source_end = if start.class_generated || end.class_generated {
                SourcePosition {
                    line: source_start.line,
                    column: source_start.column.saturating_add(1),
                }
            } else {
                end.source_position
            };

            diagnostic.line = source_start.line;
            diagnostic.column = source_start.column;
            diagnostic.end_line = source_end.line.max(source_start.line);
            diagnostic.end_column = if diagnostic.end_line == source_start.line {
                source_end.column.max(source_start.column.saturating_add(1))
            } else {
                source_end.column.max(1)
            };
        }
    }

    fn check_block_with_locals(
        &mut self,
        block: &ast::Block,
        current_return: Option<&[Type]>,
        locals: BTreeMap<String, Type>,
    ) {
        self.check_block_with_bindings(block, current_return, locals, BTreeMap::new());
    }

    fn check_block_with_bindings(
        &mut self,
        block: &ast::Block,
        current_return: Option<&[Type]>,
        locals: BTreeMap<String, Type>,
        contracts: BTreeMap<String, Type>,
    ) {
        self.scoped_locals.push(locals);
        self.scoped_local_contracts.push(contracts);
        self.scoped_return_guards.push(Vec::new());

        for stmt in block.stmts() {
            self.check_stmt(stmt, current_return);
            self.record_guard_clause_narrowing(stmt);
        }

        if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
            self.check_return(return_stmt, current_return);
        }

        self.scoped_locals.pop();
        self.scoped_local_contracts.pop();
        self.scoped_return_guards.pop();
    }

    fn record_guard_clause_narrowing(&mut self, statement: &ast::Stmt) {
        let ast::Stmt::If(statement) = statement else {
            return;
        };
        if statement.else_if().is_some() || statement.else_block().is_some() {
            return;
        }
        let empty_position = node_position(statement);
        if block_fallthrough_position(statement.block(), empty_position, &|statement| {
            statement_is_terminating_builtin(statement, &self.terminating_builtin_references)
        })
        .is_some()
        {
            return;
        }

        let aliases = self
            .environment
            .aliases()
            .map(|(name, ty)| (name.to_owned(), ty.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut narrowed =
            narrowed_locals_after_false_condition(statement.condition(), &aliases, |name| {
                self.infer_named_value(name)
            });
        self.extend_with_return_guards(&mut narrowed);
        if let Some(scope) = self.scoped_locals.last_mut() {
            scope.extend(narrowed);
        }
    }

    fn check_stmt(&mut self, stmt: &ast::Stmt, current_return: Option<&[Type]>) {
        match stmt {
            ast::Stmt::Assignment(assignment) => self.check_assignment(assignment),
            ast::Stmt::CompoundAssignment(assignment) => self.check_compound_assignment(assignment),
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
                self.check_function_body(
                    function.body(),
                    return_types.as_deref(),
                    scoped_locals,
                    function.body().return_type().is_some(),
                );
            }
            ast::Stmt::FunctionDeclaration(function) => {
                self.check_love_callback_declaration(function);
                self.record_function_declaration_type(function);

                let name = function.name().to_string();
                let return_types = self.function_return_types(&name);
                let scoped_locals = self.function_declaration_parameter_locals(function);
                self.check_function_body(
                    function.body(),
                    return_types.as_deref(),
                    scoped_locals,
                    function.body().return_type().is_some(),
                );
            }
            ast::Stmt::NumericFor(for_stmt) => self.check_numeric_for(for_stmt, current_return),
            ast::Stmt::Repeat(repeat) => {
                self.check_condition_expression(repeat.until());
                self.check_block(repeat.block(), current_return);
            }
            ast::Stmt::While(while_stmt) => {
                self.check_condition_expression(while_stmt.condition());
                let narrowed_locals = self.narrowed_locals_for_condition(while_stmt.condition());
                self.check_block_with_locals(while_stmt.block(), current_return, narrowed_locals);
            }
            _ => {}
        }
    }

    fn check_condition_expression(&mut self, expression: &Expression) {
        match expression {
            Expression::BinaryOperator { lhs, binop, rhs } => {
                if is_equality_operator(binop) {
                    self.check_equality_comparison(lhs, binop, rhs);
                } else {
                    self.infer_expression(expression);
                }

                self.check_condition_expression(lhs);
                self.with_short_circuit_narrowing(lhs, binop, |checker| {
                    checker.check_condition_expression(rhs);
                });
            }
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. } => {
                self.check_condition_expression(expression);
            }
            Expression::UnaryOperator { .. } => {
                self.infer_expression(expression);
            }
            Expression::FunctionCall(function_call)
                if runtime_type::is_check_call(function_call) =>
            {
                self.check_runtime_type_check(function_call);
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
        self.check_block_with_bindings(for_stmt.block(), current_return, locals.clone(), locals);
    }

    fn check_numeric_for(&mut self, for_stmt: &ast::NumericFor, current_return: Option<&[Type]>) {
        self.infer_expression(for_stmt.start());
        self.infer_expression(for_stmt.end());

        if let Some(step) = for_stmt.step() {
            self.infer_expression(step);
        }

        let name = token_text(for_stmt.index_variable());
        let values = constant_loop_values(for_stmt).filter(|_| {
            node_span(for_stmt).is_none_or(|span| {
                !crate::narrowing::name_assigned_between(self.parsed, &name, span.start, span.end)
            })
        });
        let locals = BTreeMap::from([(name.clone(), Type::number())]);
        if let Some(values) = &values {
            self.constant_loop_values.push((name, values.clone()));
        }
        self.check_block_with_bindings(for_stmt.block(), current_return, locals.clone(), locals);
        if values.is_some() {
            self.constant_loop_values.pop();
        }
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

        if (expression_is_nil(lhs) && self.index_read_may_be_missing(rhs))
            || (expression_is_nil(rhs) && self.index_read_may_be_missing(lhs))
        {
            return;
        }

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
        let expressions = local.expressions().iter().collect::<Vec<_>>();
        for (index, name) in local.names().iter().enumerate() {
            let expression = assignment_expression_for_local(&expressions, index)
                .filter(|(_, return_index)| *return_index == 0)
                .map(|(expression, _)| expression);
            self.record_shader_local(token_text(name), expression);
        }
        let lua_doc_contract = local
            .start_position()
            .is_some_and(|position| has_leading_lua_doc_type(self.parsed, position.line()));

        for (index, (name, specifier)) in local
            .names()
            .iter()
            .zip(local.type_specifiers())
            .enumerate()
        {
            if specifier.is_none() && !(index == 0 && lua_doc_contract) {
                continue;
            }

            let name = token_text(name);
            let Some(target) = self.environment.local(&name).cloned() else {
                continue;
            };

            if let Some((expression, return_index)) =
                assignment_expression_for_local(&expressions, index)
            {
                if return_index == 0
                    && matches!(target, Type::Any)
                    && matches!(expression, Expression::TableConstructor(_))
                {
                    // `any` is an explicit trust boundary. Avoid materializing
                    // the full structural type of generated data that cannot
                    // fail assignability against it.
                    continue;
                }
                if return_index == 0 && self.check_large_table_against_contract(expression, &target)
                {
                    continue;
                }
                let source = self
                    .infer_expression_return(expression, return_index)
                    .unwrap_or(Type::Any);
                self.check_inferred_expression_assignable_at(expression, &source, &target, None);
            } else {
                self.check_type_assignable(local, &Type::nil(), &target);
            }
        }

        self.record_local_assignment_types(local);
        self.record_local_return_guards(local);
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
            let indexed_contract_type = assignment_expression_for_local(&expressions, index)
                .filter(|(expression, return_index)| {
                    *return_index == 0 && expression_has_bracket_index(expression)
                })
                .and_then(|(expression, return_index)| {
                    self.infer_expression_return(expression, return_index)
                })
                .filter(type_contains_string_literal);
            let environment_type =
                indexed_contract_type.or_else(|| self.environment.local(&name).cloned());
            let has_environment_type = environment_type.is_some();
            if environment_type.is_some() && !has_annotation {
                // The environment already typed this local, so the value is
                // not inferred below. A call still has to have its arguments
                // checked; do it once per expression, at its first return.
                if let Some((expression @ Expression::FunctionCall(_), 0)) =
                    assignment_expression_for_local(&expressions, index)
                {
                    self.infer_expression(expression);
                }
            }
            let ty = environment_type.or_else(|| {
                match assignment_expression_for_local(&expressions, index) {
                    Some((expression, return_index)) => {
                        self.infer_expression_return(expression, return_index)
                    }
                    None => Some(Type::nil()),
                }
            });

            if let Some(ty) = ty {
                let preserve_literal_unions =
                    state_machine::type_is_state_machine(self.environment, &ty)
                        || sprite::type_is_sprite_sheet(self.environment, &ty)
                        || sprite::type_is_sprite_player(self.environment, &ty);
                let ty = if has_annotation || has_environment_type || preserve_literal_unions {
                    ty
                } else {
                    widen_inferred_type(&ty)
                };
                self.insert_scoped_local(name.clone(), ty.clone());
                if has_annotation || self.stable_inferred_locals {
                    self.insert_scoped_local_contract(name, ty);
                }
            }
        }
    }

    fn index_read_may_be_missing(&mut self, expression: &Expression) -> bool {
        let Expression::Var(Var::Expression(expression)) = expression else {
            return false;
        };
        let suffixes = expression.suffixes().collect::<Vec<_>>();
        if !matches!(suffixes.last(), Some(Suffix::Index(Index::Brackets { .. }))) {
            return false;
        }

        let mut current = match expression.prefix() {
            Prefix::Name(name) => match self.infer_named_value(&token_text(name)) {
                Some(ty) => ty,
                None => return false,
            },
            Prefix::Expression(expression) => match self.infer_expression(expression) {
                Some(ty) => ty,
                None => return false,
            },
            _ => return false,
        };

        for suffix in suffixes.iter().take(suffixes.len().saturating_sub(1)) {
            current = match suffix {
                Suffix::Index(Index::Dot { name, .. }) => {
                    match resolve_table_field_type(self.environment, &current, &token_text(name)) {
                        Some(ty) => ty,
                        None => return false,
                    }
                }
                Suffix::Index(Index::Brackets { expression, .. }) => {
                    let Some(key) = self.index_key_type(expression) else {
                        return false;
                    };
                    match resolve_index_type(self.environment, &current, &key) {
                        Some(ty) => ty,
                        None => return false,
                    }
                }
                _ => return false,
            };
        }

        self.environment
            .optional_index_value_type(&current)
            .is_some()
    }

    /// Validate large array literals one item at a time against an explicit
    /// native or LuaDoc contract. This keeps peak type size proportional to one
    /// record instead of constructing and deduplicating a union for the entire
    /// generated module.
    fn check_large_table_against_contract(
        &mut self,
        expression: &Expression,
        target: &Type,
    ) -> bool {
        let Expression::TableConstructor(table) = expression else {
            return false;
        };
        let resolved = self.environment.resolve_aliases(target);
        let Type::Array(item_type) = resolved else {
            return false;
        };
        let item_type = self.environment.resolve_aliases(&item_type);

        for field in table.fields() {
            let value = match field {
                Field::NoKey(value) => value,
                Field::ExpressionKey { key, value, .. } if expression_is_number(key) => value,
                _ => return false,
            };
            let source = self.infer_expression(value).unwrap_or(Type::Any);
            self.check_inferred_expression_assignable_at(value, &source, &item_type, None);
        }

        true
    }

    fn insert_scoped_local(&mut self, name: String, ty: Type) {
        if let Some(scope) = self.scoped_locals.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn insert_scoped_local_contract(&mut self, name: String, ty: Type) {
        if let Some(scope) = self.scoped_local_contracts.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn check_assignment(&mut self, assignment: &ast::Assignment) {
        for (variable, expression) in assignment.variables().iter().zip(assignment.expressions()) {
            if let Some(name) = directly_assigned_name(variable) {
                self.record_shader_local(name, Some(expression));
            }
            let source = self.infer_expression(expression);
            let target = self.infer_var_assignment_target(variable);

            if target.is_none() {
                self.check_unknown_class_assignment_member(variable);
            }

            if let (Some(source), Some(target)) = (&source, &target) {
                self.check_inferred_expression_assignable_at(expression, source, target, None);
            }

            self.record_assignment_type(variable, source, target, expression);
        }
    }

    fn check_compound_assignment(&mut self, assignment: &ast::CompoundAssignment) {
        let source = self.infer_expression(assignment.rhs());
        let target = self.infer_var_assignment_target(assignment.lhs());

        if target.is_none() {
            self.check_unknown_class_assignment_member(assignment.lhs());
        }

        // `target op= rhs` means `target = target op rhs`. When the target's
        // metatable defines the operator, check the right-hand side against
        // the metamethod's operand and record its result, so `v *= 2` on a
        // Vec2 is accepted and keeps `v` a Vec2.
        let metamethod = target.as_ref().and_then(|target| {
            let name = compound_metamethod_name(assignment.compound_operator())?;
            self.resolve_metatable_facts(target)?
                .metamethod(name)
                .cloned()
        });
        let operand = metamethod
            .as_ref()
            .and_then(|function| function.parameters.get(1))
            .map(|parameter| parameter.ty.clone());

        if let (Some(source), Some(target)) = (&source, &target) {
            let expected = operand.as_ref().unwrap_or(target);
            self.check_inferred_expression_assignable_at(assignment.rhs(), source, expected, None);
        }

        let result = match (&metamethod, &target) {
            (Some(function), Some(target)) => Some(metamethod_return_or(function, target.clone())),
            _ => source,
        };
        self.record_assignment_type(assignment.lhs(), result, target, assignment.rhs());
    }

    fn record_assignment_type(
        &mut self,
        variable: &Var,
        ty: Option<Type>,
        target: Option<Type>,
        expression: &Expression,
    ) {
        if self.record_table_field_assignment_type(variable, ty.clone(), expression) {
            return;
        }

        let Some(name) = directly_assigned_name(variable) else {
            return;
        };
        self.forget_return_guards(&name);

        let flow_type = match (&ty, &target) {
            (Some(source), Some(target))
                if !matches!(source, Type::Any) && self.context.is_assignable(source, target) =>
            {
                Some(source.clone())
            }
            (_, Some(target)) => Some(target.clone()),
            (Some(source), None) => Some(source.clone()),
            (None, None) => None,
        };

        match flow_type {
            Some(ty) => self.insert_scoped_local(name, widen_inferred_type(&ty)),
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

    fn check_love_callback_declaration(&mut self, function: &ast::FunctionDeclaration) {
        let Some((label, token)) = love_callback::declaration_name(function) else {
            return;
        };
        let Some(item) = love_callback::item(&label) else {
            let Some(suggestion) = love_callback::closest_name(&label) else {
                return;
            };
            let Some(span) = node_span(token).or_else(|| node_span(function.name())) else {
                return;
            };

            self.diagnostics.push(
                Diagnostic::new(
                    &self.parsed.path,
                    span.start.line,
                    span.start.column,
                    DiagnosticSeverity::Warning,
                    format!(
                        "unknown LOVE callback `love.{label}`; did you mean `love.{suggestion}`?"
                    ),
                )
                .with_range(
                    span.start.line,
                    span.start.column,
                    span.end.line,
                    span.end.column,
                )
                .with_code(DiagnosticCode::DOMAIN_LOVE_CALLBACK),
            );
            return;
        };

        let name = format!("love.{label}");
        let Some(declared) = self.environment.function(&name) else {
            return;
        };
        let Some(expected) = external_function_type_from_lua_item(item) else {
            return;
        };
        let parameter_names = function
            .body()
            .parameters()
            .iter()
            .filter_map(|parameter| match parameter {
                ast::Parameter::Name(name) => Some(token_text(name)),
                ast::Parameter::Ellipsis(_) => None,
                _ => None,
            })
            .collect::<Vec<_>>();

        if expected
            .variants
            .iter()
            .any(|variant| self.love_callback_variant_matches(declared, &parameter_names, variant))
        {
            return;
        }

        let Some(span) = node_span(token).or_else(|| node_span(function.name())) else {
            return;
        };
        let max_parameters = expected
            .variants
            .iter()
            .map(|variant| variant.parameters.len())
            .max()
            .unwrap_or_default();
        let message = if declared.required_parameter_count > max_parameters {
            let parameter_word = if max_parameters == 1 {
                "parameter"
            } else {
                "parameters"
            };
            format!(
                "LOVE callback `{name}` receives at most {max_parameters} {parameter_word}, but this declaration requires {}",
                declared.required_parameter_count
            )
        } else {
            format!(
                "LOVE callback `{name}` has an incompatible parameter type; expected `{}`",
                item.detail
            )
        };

        self.diagnostics.push(
            Diagnostic::error(
                &self.parsed.path,
                span.start.line,
                span.start.column,
                message,
            )
            .with_range(
                span.start.line,
                span.start.column,
                span.end.line,
                span.end.column,
            )
            .with_code(DiagnosticCode::DOMAIN_LOVE_CALLBACK),
        );
    }

    fn love_callback_variant_matches(
        &self,
        declared: &TypedFunction,
        parameter_names: &[String],
        expected: &ExternalFunctionVariant,
    ) -> bool {
        if declared.required_parameter_count > expected.parameters.len() {
            return false;
        }

        parameter_names.iter().enumerate().all(|(index, name)| {
            let Some(declared_parameter) = declared
                .parameters
                .iter()
                .find(|parameter| parameter.name == *name)
            else {
                return true;
            };
            let Some(expected_parameter) = expected.parameter_type(index) else {
                return true;
            };

            self.love_callback_parameter_assignable(expected_parameter, &declared_parameter.ty)
        })
    }

    fn love_callback_parameter_assignable(&self, source: &Type, target: &Type) -> bool {
        if self.context.is_assignable(source, target) {
            return true;
        }

        matches!(source, Type::Alias(name) if !lua::enum_value_items(name, "").is_empty())
            && matches!(target, Type::Primitive(crate::PrimitiveType::String))
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
            widen_inferred_type(&ty),
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

    fn check_expression_assignable_at(
        &mut self,
        expression: &Expression,
        target: &Type,
        root: Option<&str>,
    ) {
        let Some(source) = self.infer_expression(expression) else {
            return;
        };

        self.check_inferred_expression_assignable_at(expression, &source, target, root);
    }

    fn check_inferred_expression_assignable_at(
        &mut self,
        expression: &Expression,
        source: &Type,
        target: &Type,
        root: Option<&str>,
    ) {
        if type_contains_any_alias(target, &self.unresolved_project_aliases) {
            return;
        }

        let Some(failure) = self.diagnostic_assignability_failure(source, target) else {
            return;
        };

        self.diagnostics.push(mismatch_diagnostic_for_expression(
            &self.parsed.path,
            expression,
            &failure,
            root,
        ));
    }

    fn check_type_assignable(&mut self, node: &impl Node, source: &Type, target: &Type) {
        if type_contains_any_alias(target, &self.unresolved_project_aliases) {
            return;
        }

        let Some(failure) = self.diagnostic_assignability_failure(source, target) else {
            return;
        };

        self.diagnostics
            .push(mismatch_diagnostic(&self.parsed.path, node, &failure, None));
    }

    fn check_type_assignable_at_position(
        &mut self,
        position: SourcePosition,
        source: &Type,
        target: &Type,
    ) {
        if type_contains_any_alias(target, &self.unresolved_project_aliases) {
            return;
        }

        let Some(failure) = self.diagnostic_assignability_failure(source, target) else {
            return;
        };

        self.diagnostics.push(mismatch_diagnostic_at_position(
            &self.parsed.path,
            position,
            &failure,
        ));
    }

    fn check_function_body(
        &mut self,
        body: &ast::FunctionBody,
        return_types: Option<&[Type]>,
        scoped_locals: BTreeMap<String, Type>,
        has_explicit_return_contract: bool,
    ) {
        let Some(span) = node_span(body) else {
            return;
        };
        if !self
            .checked_function_bodies
            .insert((span.start.line, span.start.column))
        {
            return;
        }

        let contracts = scoped_locals.clone();
        self.scoped_locals.push(scoped_locals);
        self.scoped_local_contracts.push(contracts);
        self.check_block(body.block(), return_types);
        self.scoped_locals.pop();
        self.scoped_local_contracts.pop();

        if !has_explicit_return_contract {
            return;
        }
        let Some(targets) = return_types else {
            return;
        };
        let Some(position) = block_fallthrough_position(body.block(), span.end, &|statement| {
            statement_is_terminating_builtin(statement, &self.terminating_builtin_references)
        }) else {
            return;
        };

        for target in targets {
            self.check_type_assignable_at_position(position, &Type::nil(), target);
        }
    }

    fn diagnostic_assignability_failure(
        &self,
        source: &Type,
        target: &Type,
    ) -> Option<AssignabilityFailure> {
        let mut failure = self.context.assignability_failure(source, target)?;

        if failure.path.is_empty() {
            failure.expected = self.display_type_for_mismatch(target);
        }

        Some(failure)
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
            for target in target {
                self.check_type_assignable(return_stmt, &Type::nil(), target);
            }
            return;
        }

        for (index, target) in target.iter().enumerate() {
            let Some((expression, return_index)) =
                assignment_expression_for_local(&expressions, index)
            else {
                self.check_type_assignable(return_stmt, &Type::nil(), target);
                continue;
            };
            let source = match self.infer_expression_return(expression, return_index) {
                Some(source) => source,
                // A call that yields fewer values leaves the rest nil. An
                // expression whose type is unknown, such as `tonumber(s) or 0`,
                // is dynamic here as it is in a local assignment.
                None if return_index > 0 => Type::nil(),
                None => Type::Any,
            };

            self.check_inferred_expression_assignable_at(expression, &source, target, None);
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
            Expression::FunctionCall(function_call)
                if runtime_type::is_check_call(function_call) =>
            {
                (return_index == 0).then(Type::boolean)
            }
            Expression::FunctionCall(function_call) => self
                .check_function_call_returns(function_call)
                .get(return_index)
                .cloned(),
            _ if return_index > 0 => None,
            Expression::BinaryOperator { lhs, binop, rhs } => {
                let left = self.infer_expression(lhs)?;
                let right = self.with_short_circuit_narrowing(lhs, binop, |checker| {
                    checker.infer_expression(rhs)
                })?;

                self.check_binary_metamethod_operand(lhs, binop, rhs, &left, &right);
                self.check_primitive_binary_operands(expression, binop, &left, &right);

                self.infer_binary_operator_type(binop, &left, &right)
            }
            Expression::UnaryOperator { unop, expression } => {
                let operand = self.infer_expression(expression)?;

                if self.known_primitive_type(&operand)
                    && ((matches!(unop, ast::UnOp::Minus(_)) && !self.is_number_type(&operand))
                        || (matches!(unop, ast::UnOp::Hash(_)) && !self.is_string_type(&operand)))
                {
                    self.invalid_operator(expression, &unop.to_string(), &operand.to_string());
                }

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
            Expression::Function(function) => {
                let ty = function_type_from_body(function.body());
                let Type::Function(signature) = &ty else {
                    unreachable!("function bodies always produce function types")
                };
                let scoped_locals = signature
                    .parameters
                    .iter()
                    .filter_map(|parameter| {
                        parameter
                            .name
                            .as_ref()
                            .map(|name| (name.clone(), parameter.ty.clone()))
                    })
                    .collect();
                self.check_function_body(
                    function.body(),
                    Some(signature.return_types_or_any()),
                    scoped_locals,
                    function.body().return_type().is_some(),
                );
                Some(ty)
            }
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
                    let field = token_text(name);
                    let Some(next) = resolve_table_field_type(self.environment, &current, &field)
                    else {
                        self.report_unsafe_union_operation(
                            name,
                            &current,
                            &format!("access field `{field}` on"),
                        );
                        self.report_unknown_class_member(name, &current, &field);
                        if let Prefix::Name(root) = expression.prefix() {
                            self.report_unknown_module_export(name, root, &current, &field);
                        }
                        return None;
                    };
                    current = next;
                }
                Suffix::Index(Index::Brackets { expression, .. }) => {
                    let key = self.index_key_type(expression)?;
                    let Some(next) = resolve_index_type(self.environment, &current, &key) else {
                        self.report_unsafe_union_operation(expression, &current, "index");
                        return None;
                    };
                    current = next;
                }
                _ => return None,
            }
        }

        Some(current)
    }

    fn check_unknown_class_assignment_member(&mut self, variable: &Var) {
        let Some(parts) = plain_field_chain_from_var(variable) else {
            return;
        };
        let Some((field, receiver_parts)) = parts.split_last() else {
            return;
        };
        if receiver_parts.is_empty() {
            return;
        }
        let Some(receiver_type) = self.resolve_field_chain_type(receiver_parts) else {
            return;
        };

        self.report_unknown_class_member(variable, &receiver_type, field);
        let mut exact_module = false;
        if let Var::Expression(expression) = variable {
            if let Prefix::Name(root) = expression.prefix() {
                let root_position = node_position(root);
                exact_module = self
                    .exact_module_references
                    .contains_key(&(root_position.line, root_position.column));
                self.report_unknown_module_export(variable, root, &receiver_type, field);
            }
        }
        if !exact_module {
            self.report_unknown_record_field(variable, &receiver_type, field);
        }
    }

    fn report_unknown_record_field(&mut self, node: &impl Node, ty: &Type, field: &str) {
        if !self.unknown_record_fields {
            return;
        }
        if matches!(ty, Type::Alias(name) if self.environment.is_class(name)) {
            return;
        }
        let Type::Table(table) = resolve_aliases(ty, self.environment, 0) else {
            return;
        };
        if table.is_plain_empty()
            || table.has_index_signature()
            || table.semantic_field(field).is_some()
            || table.metatable_facts().is_some_and(|facts| {
                matches!(
                    facts.index,
                    IndexFallback::Function(_) | IndexFallback::Unknown
                )
            })
        {
            return;
        }

        let position = node_span(node)
            .map(|span| span.start)
            .unwrap_or_else(|| node_position(node));
        self.diagnostics.push(
            Diagnostic::new(
                &self.parsed.path,
                position.line,
                position.column,
                DiagnosticSeverity::Warning,
                format!("record has no known field `{field}`; check the spelling or declare the field in its type"),
            )
            .with_code(DiagnosticCode::LINT_UNKNOWN_RECORD_FIELD),
        );
    }

    fn report_unknown_class_member(&mut self, node: &impl Node, ty: &Type, field: &str) {
        let Type::Alias(class_name) = ty else {
            return;
        };
        if !self.environment.is_class(class_name) {
            return;
        }

        let Some(position) = node_span(node).map(|span| span.start) else {
            return;
        };
        let diagnostic = Diagnostic::error(
            &self.parsed.path,
            position.line,
            position.column,
            format!("unknown member `{field}` on class `{class_name}`"),
        )
        .with_code(DiagnosticCode::TYPE_CLASS);

        if !self.diagnostics.iter().any(|existing| {
            existing.line == diagnostic.line
                && existing.column == diagnostic.column
                && existing.message == diagnostic.message
        }) {
            self.diagnostics.push(diagnostic);
        }
    }

    fn report_unknown_module_export(
        &mut self,
        node: &impl Node,
        root: &impl Node,
        receiver_type: &Type,
        field: &str,
    ) {
        let Type::Table(table) = resolve_aliases(receiver_type, self.environment, 0) else {
            return;
        };
        if table.metatable_facts().is_some_and(|facts| {
            matches!(
                facts.index,
                IndexFallback::Function(_) | IndexFallback::Unknown
            )
        }) {
            return;
        }

        let root_position = node_position(root);
        let Some(module) = self
            .exact_module_references
            .get(&(root_position.line, root_position.column))
        else {
            return;
        };

        let position = node_span(node)
            .map(|span| span.start)
            .unwrap_or_else(|| node_position(node));
        let diagnostic = Diagnostic::error(
            &self.parsed.path,
            position.line,
            position.column,
            format!("module `{module}` has no exported member `{field}`"),
        )
        .with_code(DiagnosticCode::TYPE_UNKNOWN_MODULE_EXPORT);

        if !self.diagnostics.iter().any(|existing| {
            existing.line == diagnostic.line
                && existing.column == diagnostic.column
                && existing.message == diagnostic.message
        }) {
            self.diagnostics.push(diagnostic);
        }
    }

    fn report_unsafe_union_operation(&mut self, node: &impl Node, ty: &Type, operation: &str) {
        let (message, code) = match resolve_aliases(ty, self.environment, 0) {
            Type::Union(_) => (
                format!(
                    "cannot {operation} union type `{ty}`; narrow it to a supported member first"
                ),
                DiagnosticCode::TYPE_UNSAFE_UNION_OPERATION,
            ),
            resolved if resolved == Type::nil() => (
                format!("cannot {operation} nil"),
                DiagnosticCode::TYPE_MISMATCH,
            ),
            _ => return,
        };

        let position = node_span(node)
            .map(|span| span.start)
            .unwrap_or_else(|| node_position(node));
        let diagnostic =
            Diagnostic::error(&self.parsed.path, position.line, position.column, message)
                .with_code(code);

        if !self.diagnostics.iter().any(|existing| {
            existing.line == diagnostic.line
                && existing.column == diagnostic.column
                && existing.message == diagnostic.message
        }) {
            self.diagnostics.push(diagnostic);
        }
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
        let returns = self.check_first_call_returns(function_call);

        self.check_chained_calls(function_call, returns)
    }

    /// Keep checking after a chain's first call, as in
    /// `tween.to(box, 1, values):ease("quadout"):delay(0.5)`. Each later call
    /// is checked against the type the chain has reached, and the chain takes
    /// the last call's result. Once a link is unknown, the rest of the chain
    /// stays dynamic: its arguments are still inferred, but not checked
    /// against a contract.
    fn check_chained_calls(&mut self, function_call: &FunctionCall, first: Vec<Type>) -> Vec<Type> {
        let mut suffixes = function_call
            .suffixes()
            .skip_while(|suffix| !matches!(suffix, Suffix::Call(_)));
        suffixes.next();
        let rest = suffixes.collect::<Vec<_>>();
        if rest.is_empty() {
            return first;
        }

        let mut current = first.into_iter().next();
        for suffix in rest {
            current = match suffix {
                Suffix::Index(Index::Dot { name, .. }) => current.and_then(|ty| {
                    resolve_table_field_type(self.environment, &ty, &token_text(name))
                }),
                Suffix::Call(call) => self.check_chained_call(suffix, call, current),
                _ => None,
            };
        }

        current.into_iter().collect()
    }

    fn check_chained_call(
        &mut self,
        suffix: &Suffix,
        call: &Call,
        current: Option<Type>,
    ) -> Option<Type> {
        let (receiver, method, args) = match call {
            Call::AnonymousCall(args) => (None, None, args),
            Call::MethodCall(method) => (
                current.clone(),
                Some(token_text(method.name())),
                method.args(),
            ),
            _ => return None,
        };
        let mut arguments = function_args_call_arguments(args);

        let callable = match (&receiver, &method) {
            (Some(receiver), Some(method)) => {
                resolve_table_field_type(self.environment, receiver, method)
                    .and_then(|ty| callable_from_type(&ty, self.environment))
                    .or_else(|| {
                        lua_type_field_item(self.environment, receiver, method)
                            .and_then(external_function_type_from_lua_item)
                            .map(Callable::ExternalFunction)
                    })
            }
            (None, None) => current
                .as_ref()
                .and_then(|ty| callable_from_type(ty, self.environment)),
            _ => None,
        };

        match callable {
            Some(Callable::Function(function)) => {
                if let Some(receiver) = receiver {
                    arguments.insert(0, CallArgument::Inferred(receiver));
                }
                self.check_argument_list(suffix, arguments, &function, None);
                function.return_types_or_any().first().cloned()
            }
            // LÖVE object methods are catalogued without their receiver.
            Some(Callable::ExternalFunction(function)) => self
                .check_external_argument_list(suffix, arguments, &function)
                .into_iter()
                .next(),
            Some(Callable::TypedFunction(_)) | Some(Callable::Any) | None => {
                for argument in arguments {
                    if let CallArgument::Expression(expression) = argument {
                        self.infer_expression(expression);
                    }
                }
                None
            }
        }
    }

    fn check_first_call_returns(&mut self, function_call: &FunctionCall) -> Vec<Type> {
        self.check_asset_loader_call(function_call);
        self.check_dot_love_method_call(function_call);
        self.check_shader_send_call(function_call);

        if let Some(ty) = self.setmetatable_return_type(function_call) {
            return vec![ty];
        }

        if is_tostring_call(function_call) {
            return vec![Type::string()];
        }

        if let Some(ty) = self.checked_rawget_return_type(function_call) {
            return vec![ty];
        }

        let Some(callee) = self.infer_call_target(function_call) else {
            self.report_unsafe_union_call_target(function_call);
            self.check_dynamic_call_argument_expressions(function_call);
            return Vec::new();
        };

        match callee {
            Callable::Any => {
                // A dynamic outer call still evaluates its argument
                // expressions. Check nested calls there even though the
                // outer signature provides no argument contract.
                self.check_dynamic_call_argument_expressions(function_call);
                vec![Type::Any]
            }
            Callable::Function(function) => {
                self.check_call_arguments(function_call, &function);

                if let Some(ty) = self.builtin_constructor_call_return_type(function_call) {
                    return vec![ty];
                }

                if let Some(ty) = self.constructor_call_return_type(function_call) {
                    return vec![ty];
                }

                function.return_types_or_any().to_vec()
            }
            Callable::TypedFunction(function) => {
                let return_types =
                    self.check_typed_function_call_arguments(function_call, &function);
                self.check_signal_emit_payload_call(function_call);

                if let Some(ty) = self.builtin_constructor_call_return_type(function_call) {
                    return vec![ty];
                }

                if let Some(ty) = self.constructor_call_return_type(function_call) {
                    return vec![ty];
                }

                return_types
            }
            Callable::ExternalFunction(function) => {
                self.check_external_call_arguments(function_call, &function)
            }
        }
    }

    fn report_unsafe_union_call_target(&mut self, function_call: &FunctionCall) {
        let Some(mut current) = (match function_call.prefix() {
            Prefix::Name(name) => self.infer_named_value(&token_text(name)),
            Prefix::Expression(expression) => self.infer_expression(expression),
            _ => None,
        }) else {
            return;
        };

        for suffix in function_call.suffixes() {
            match suffix {
                Suffix::Index(Index::Dot { name, .. }) => {
                    let field = token_text(name);
                    let Some(next) = resolve_table_field_type(self.environment, &current, &field)
                    else {
                        self.report_unsafe_union_operation(
                            name,
                            &current,
                            &format!("access field `{field}` on"),
                        );
                        if let Prefix::Name(root) = function_call.prefix() {
                            self.report_unknown_module_export(name, root, &current, &field);
                        }
                        return;
                    };
                    current = next;
                }
                Suffix::Index(Index::Brackets { expression, .. }) => {
                    let Some(key) = self.index_key_type(expression) else {
                        return;
                    };
                    let Some(next) = resolve_index_type(self.environment, &current, &key) else {
                        self.report_unsafe_union_operation(expression, &current, "index");
                        return;
                    };
                    current = next;
                }
                Suffix::Call(Call::MethodCall(method)) => {
                    let field = token_text(method.name());
                    let Some(next) = resolve_table_field_type(self.environment, &current, &field)
                    else {
                        self.report_unsafe_union_operation(
                            method.name(),
                            &current,
                            &format!("access method `{field}` on"),
                        );
                        if let Prefix::Name(root) = function_call.prefix() {
                            self.report_unknown_module_export(
                                method.name(),
                                root,
                                &current,
                                &field,
                            );
                        }
                        return;
                    };
                    current = next;
                    break;
                }
                Suffix::Call(Call::AnonymousCall(_)) => break,
                _ => return,
            }
        }

        if matches!(
            resolve_aliases(&current, self.environment, 0),
            Type::Union(_)
        ) {
            self.report_unsafe_union_operation(function_call, &current, "call");
        }
    }

    fn checked_rawget_return_type(&mut self, function_call: &FunctionCall) -> Option<Type> {
        let parts = field_chain_before_first_call(function_call)?;
        if parts.as_slice() != ["rawget"] || self.infer_named_value("rawget").is_some() {
            return None;
        }

        let arguments = call_arguments(function_call)?;
        arguments.get(1)?;
        let CallArgument::Expression(collection) = arguments.first()? else {
            return None;
        };
        let collection = self.infer_expression(collection)?;

        self.environment.optional_index_value_type(&collection)
    }

    fn check_dynamic_call_argument_expressions(&mut self, function_call: &FunctionCall) {
        for argument in call_arguments(function_call).unwrap_or_default() {
            if let CallArgument::Expression(expression) = argument {
                self.infer_expression(expression);
            }
        }
    }

    fn setmetatable_return_type(&mut self, function_call: &FunctionCall) -> Option<Type> {
        let (target, meta) = setmetatable_arguments(function_call)?;
        if let Some(class_type) = self.class_setmetatable_instance_type(meta) {
            return Some(class_type);
        }
        let base = self.infer_expression(target)?;
        let meta = self.infer_expression(meta)?;

        Some(merge_static_metatable(base, meta))
    }

    fn class_setmetatable_instance_type(&self, expression: &Expression) -> Option<Type> {
        let Expression::Var(Var::Name(name)) = expression else {
            return None;
        };
        let name = token_text(name);

        self.parsed
            .class_declarations
            .iter()
            .any(|class| class.name == name)
            .then_some(Type::alias(name))
    }

    fn builtin_constructor_call_return_type(
        &mut self,
        function_call: &FunctionCall,
    ) -> Option<Type> {
        let parts = field_chain_before_first_call(function_call)?;
        let domains = BuiltinDomainRegistry::standard();
        let imported =
            domains.imported_constructor_domain_from_bindings(&self.runtime_imports, &parts);
        let domain = imported.or_else(|| {
            let [name] = parts.as_slice() else {
                return None;
            };
            if self.environment.local(name).is_some() || self.environment.function(name).is_some() {
                return None;
            }
            domains.constructor_domain(name)
        })?;
        let argument = if matches!(domain, BuiltinDomain::ObjectPool | BuiltinDomain::Store) {
            call_arguments(function_call)
                .and_then(|arguments| arguments.into_iter().next())
                .and_then(|argument| match argument {
                    CallArgument::Expression(expression) => self.infer_expression(expression),
                    CallArgument::Inferred(ty) => Some(ty),
                })
        } else {
            None
        };

        domain.inferred_constructor_return_type(function_call, argument.as_ref())
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
        let Some(asset_call) = asset::asset_loader_call(function_call) else {
            return;
        };
        let span = node_span(function_call).unwrap_or(SourceSpan {
            start: node_position(function_call),
            end: node_position(function_call),
        });
        let metadata = asset::validate_asset_with_index(
            asset_call.kind,
            &asset_call.path,
            span,
            &self.asset_roots,
            self.asset_index,
        );

        match metadata.status {
            asset::AssetValidationStatus::WrongKind => {
                self.push_asset_diagnostic(wrong_asset_kind_diagnostic(
                    &self.parsed.path,
                    asset_call.path_span,
                    asset_call.kind,
                    &asset_call.path,
                ));
            }
            asset::AssetValidationStatus::UnsupportedExtension => {
                self.push_asset_diagnostic(unsupported_asset_extension_diagnostic(
                    &self.parsed.path,
                    asset_call.path_span,
                    asset_call.kind,
                    &asset_call.path,
                ));
            }
            asset::AssetValidationStatus::Missing => {
                self.push_asset_diagnostic(missing_asset_diagnostic(
                    &self.parsed.path,
                    asset_call.path_span,
                    asset_call.kind,
                    &asset_call.path,
                ));
            }
            asset::AssetValidationStatus::PathCaseMismatch => {
                self.push_asset_diagnostic(asset_path_case_diagnostic(
                    &self.parsed.path,
                    asset_call.path_span,
                    &asset_call.path,
                    metadata
                        .canonical_path
                        .as_deref()
                        .unwrap_or(&asset_call.path),
                ));
            }
            asset::AssetValidationStatus::Valid | asset::AssetValidationStatus::Unchecked => {}
        }
    }

    /// Remembers the uniforms of a shader asset assigned to `name`, so that
    /// `name:send(...)` can be checked; any other value forgets them.
    fn record_shader_local(&mut self, name: String, expression: Option<&Expression>) {
        let uniforms = expression.and_then(|expression| {
            let Expression::FunctionCall(function_call) = expression else {
                return None;
            };
            let call = asset::asset_loader_call(function_call)?;
            if call.kind != asset::AssetLoaderKind::Shader {
                return None;
            }
            let metadata = asset::validate_asset_with_index(
                call.kind,
                &call.path,
                call.path_span,
                &self.asset_roots,
                self.asset_index,
            );
            let path = asset::resolved_asset_path(&metadata, &self.asset_roots, self.asset_index)?;
            let source = if let Some(index) = self.asset_index {
                index.shader(&path)?
            } else {
                self.shader_sources
                    .entry(path.clone())
                    .or_insert_with(|| asset::ShaderSource::read(&path, None))
                    .as_ref()?
            };
            Some(ShaderUniforms {
                asset_path: call.path,
                model: source.uniforms.clone()?,
            })
        });

        match uniforms {
            Some(uniforms) => {
                self.shader_locals.insert(name, uniforms);
            }
            None => {
                self.shader_locals.remove(&name);
            }
        }
    }

    fn check_shader_send_call(&mut self, function_call: &FunctionCall) {
        let Some(call) = asset::shader_send_call(function_call) else {
            return;
        };
        let Some(uniforms) = self.shader_locals.get(&call.receiver).cloned() else {
            return;
        };

        match &call.uniform {
            asset::ShaderUniformReference::Exact(name) => {
                self.check_sent_uniform_names(&call, &uniforms, std::slice::from_ref(name), &[]);
            }
            asset::ShaderUniformReference::Pattern {
                prefix,
                suffix,
                variable,
            } => {
                let values = variable.as_deref().and_then(|variable| {
                    self.constant_loop_values
                        .iter()
                        .rev()
                        .find(|(name, _)| name == variable)
                        .map(|(_, values)| values.clone())
                });
                let matching = uniforms
                    .model
                    .names
                    .iter()
                    .filter(|name| {
                        name.len() > prefix.len() + suffix.len()
                            && name.starts_with(prefix.as_str())
                            && name.ends_with(suffix.as_str())
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if let Some(values) = values {
                    let names = values
                        .iter()
                        .map(|value| format!("{prefix}{value}{suffix}"))
                        .collect::<Vec<_>>();
                    self.check_sent_uniform_names(&call, &uniforms, &names, &matching);
                } else if matching.is_empty() {
                    let requested = format!("{prefix}{suffix}");
                    self.report_missing_shader_uniform(
                        &call,
                        &uniforms,
                        &format!("uniform matching {}", call.uniform.describe()),
                        &requested,
                        &[],
                    );
                }
            }
        }
    }

    /// Reports the names a `send` call can produce that the shader lacks or
    /// never reads. `matching` lists declared names that fit the call's
    /// pattern, which explain a loop that runs past them.
    fn check_sent_uniform_names(
        &mut self,
        call: &asset::ShaderSendCall,
        uniforms: &ShaderUniforms,
        names: &[String],
        matching: &[String],
    ) {
        let missing = names
            .iter()
            .filter(|name| !uniforms.model.names.contains(name.as_str()))
            .collect::<Vec<_>>();
        if let Some(first) = missing.first() {
            let described = match missing.as_slice() {
                [name] => format!("uniform `{name}`"),
                names => format!(
                    "uniforms {}",
                    names
                        .iter()
                        .map(|name| format!("`{name}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            };
            self.report_missing_shader_uniform(call, uniforms, &described, first, matching);
            return;
        }

        for name in names {
            if uniforms.model.unread.contains(name) {
                let diagnostic = unread_shader_uniform_diagnostic(
                    &self.parsed.path,
                    call.span,
                    &uniforms.asset_path,
                    name,
                );
                self.push_asset_diagnostic(diagnostic);
            }
        }
    }

    fn report_missing_shader_uniform(
        &mut self,
        call: &asset::ShaderSendCall,
        uniforms: &ShaderUniforms,
        described: &str,
        requested: &str,
        matching: &[String],
    ) {
        let listed = |names: Vec<&String>, total: usize| {
            let mut names = names
                .into_iter()
                .take(6)
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>();
            if total > names.len() {
                names.push("...".to_owned());
            }
            names.join(", ")
        };
        let available = match closest_uniform(&uniforms.model.names, requested) {
            _ if !matching.is_empty() => {
                format!(
                    "it declares {}",
                    listed(matching.iter().collect(), matching.len())
                )
            }
            Some(suggestion) => format!("did you mean `{suggestion}`?"),
            None if uniforms.model.names.is_empty() => "it declares no uniforms".to_owned(),
            None => format!(
                "it declares {}",
                listed(
                    uniforms.model.names.iter().collect(),
                    uniforms.model.names.len()
                )
            ),
        };
        self.push_asset_diagnostic(
            Diagnostic::error(
                &self.parsed.path,
                call.span.start.line,
                call.span.start.column,
                format!(
                    "shader `{}` has no {described}, so LOVE raises an error when it is sent; {available}",
                    uniforms.asset_path
                ),
            )
            .with_range(
                call.span.start.line,
                call.span.start.column,
                call.span.end.line,
                call.span.end.column,
            )
            .with_code(DiagnosticCode::ASSET_SHADER_UNIFORM),
        );
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

            if let Some(constructor) = BuiltinDomainRegistry::standard().constructor(root) {
                return Some(match constructor {
                    BuiltinConstructor::Function(function) => Callable::Function(function),
                    BuiltinConstructor::Typed(function) => Callable::from_typed_function(&function),
                });
            }
        }

        if !fields.is_empty() {
            if let Some(function) = self.builtin_domain_method_call_target(&parts) {
                return Some(Callable::from_typed_function(&function));
            }

            if let Some(function) = self.love_object_method_call_target(&parts) {
                return Some(Callable::ExternalFunction(function));
            }

            if parts.first().map(String::as_str) == Some("love") {
                if let Some(function) = lua::hover_item(&parts)
                    .filter(|item| item.kind == lua::LuaItemKind::Function)
                    .and_then(external_function_type_from_lua_item)
                {
                    return Some(Callable::ExternalFunction(function));
                }
            }

            if let Some(callable) = self
                .resolve_field_chain_type(&parts)
                .as_ref()
                .and_then(|ty| callable_from_type(ty, self.environment))
            {
                return Some(callable);
            }

            // Standard-library calls participate in local flow inference just
            // like LOVE calls. Resolve user locals first so a shadowing table
            // keeps its own callable fields instead of inheriting the global
            // catalog signature.
            if self.infer_named_value(root).is_none() {
                if let Some(function) = lua::hover_item(&parts)
                    .filter(|item| item.kind == lua::LuaItemKind::Function)
                    .and_then(external_function_type_from_lua_item)
                    .filter(external_function_has_known_return)
                {
                    return Some(Callable::ExternalFunction(function));
                }
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

    fn builtin_domain_method_call_target(&self, parts: &[String]) -> Option<TypedFunction> {
        let method = parts.last()?;
        let receiver_parts = parts.get(..parts.len().checked_sub(1)?)?;
        let receiver_type = self.resolve_field_chain_type(receiver_parts)?;

        BuiltinDomainRegistry::standard().typed_member(self.environment, &receiver_type, method)
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

    /// A hint for a method called with `.` where its receiver isn't passed:
    /// `ship.steer(1)` across a module boundary leaves `self` unfilled. It is
    /// offered only when the method's first parameter is `self` and the
    /// receiver's known type can be passed as that `self`, so following the
    /// hint never trades one error for another.
    fn missing_self_hint(
        &self,
        function_call: &FunctionCall,
        function: &FunctionType,
        found: usize,
    ) -> Option<String> {
        let dot_call = dot_method_call(function_call)?;
        let self_parameter = function
            .parameters
            .first()
            .filter(|parameter| parameter.name.as_deref() == Some("self"))?;
        if dot_call.receiver.is_empty() || found >= function.required_parameter_count {
            return None;
        }
        let receiver_parts = &dot_call.parts[..dot_call.parts.len() - 1];
        let receiver = self.resolve_field_chain_type(receiver_parts)?;
        if matches!(receiver, Type::Any)
            || !self.context.is_assignable(&receiver, &self_parameter.ty)
        {
            return None;
        }

        Some(format!(
            "; `{}` takes `self`, so call it as `{}:{}(...)`",
            dot_call.method, dot_call.receiver, dot_call.method
        ))
    }

    fn check_call_arguments(&mut self, function_call: &FunctionCall, function: &FunctionType) {
        let arguments = self.call_arguments_for_function(function_call);
        let hint = (arguments.len() < function.required_parameter_count)
            .then(|| self.missing_self_hint(function_call, function, arguments.len()))
            .flatten();

        self.check_argument_list(function_call, arguments, function, hint);
    }

    /// Check `arguments` against `function`, reporting count and type
    /// mismatches at `node`. `missing_hint` is appended to a too-few-arguments
    /// message.
    fn check_argument_list(
        &mut self,
        node: &impl Node,
        arguments: Vec<CallArgument<'_>>,
        function: &FunctionType,
        missing_hint: Option<String>,
    ) {
        if arguments.len() < function.required_parameter_count {
            let mut diagnostic = argument_count_diagnostic(
                &self.parsed.path,
                node,
                function.required_parameter_count,
                arguments.len(),
            );
            if let Some(hint) = missing_hint {
                diagnostic.message.push_str(&hint);
            }
            self.diagnostics.push(diagnostic);
            return;
        }

        if arguments.len() > function.parameters.len() && function.vararg_type.is_none() {
            self.diagnostics.push(argument_count_diagnostic(
                &self.parsed.path,
                node,
                function.parameters.len(),
                arguments.len(),
            ));
            return;
        }

        for (index, argument) in arguments.into_iter().enumerate() {
            let parameter = function
                .parameters
                .get(index)
                .map(|parameter| (&parameter.ty, parameter.name.as_deref()))
                .or_else(|| function.vararg_type.as_deref().map(|ty| (ty, Some("..."))))
                .expect("argument count was validated");

            match argument {
                CallArgument::Expression(argument) => {
                    self.check_expression_assignable_at(argument, parameter.0, parameter.1);
                }
                CallArgument::Inferred(argument_type) => {
                    if let Some(failure) =
                        self.diagnostic_assignability_failure(&argument_type, parameter.0)
                    {
                        self.diagnostics.push(mismatch_diagnostic(
                            &self.parsed.path,
                            node,
                            &failure,
                            parameter.1,
                        ));
                    }
                }
            }
        }
    }

    fn check_typed_function_call_arguments(
        &mut self,
        function_call: &FunctionCall,
        function: &TypedFunction,
    ) -> Vec<Type> {
        let arguments = self.call_arguments_for_function(function_call);
        let argument_types = self.infer_call_argument_types(&arguments);
        let candidates = typed_call_candidates(function);

        for candidate in &candidates {
            if typed_call_candidate_matches(self, &argument_types, candidate) {
                return candidate.return_types.clone();
            }
        }

        let best_candidate = candidates
            .iter()
            .find(|candidate| {
                expected_typed_argument_count(candidate, argument_types.len()).is_none()
            })
            .or_else(|| candidates.first());

        if let Some(candidate) = best_candidate {
            self.report_typed_call_mismatch(function_call, &arguments, &argument_types, candidate);

            return candidate.return_types.clone();
        }

        vec![Type::Any]
    }

    fn check_signal_emit_payload_call(&mut self, function_call: &FunctionCall) {
        if !is_method_call(function_call) {
            return;
        }

        let Some(parts) = field_chain_before_first_call(function_call) else {
            return;
        };
        let Some(method) = parts.last() else {
            return;
        };

        if method != "emit" {
            return;
        }

        let Some(receiver_parts) = parts.get(..parts.len().saturating_sub(1)) else {
            return;
        };
        let Some(receiver_type) = self.resolve_field_chain_type(receiver_parts) else {
            return;
        };

        if !signal::type_is_signal_bus(self.environment, &receiver_type) {
            return;
        }

        let arguments = call_arguments(function_call).unwrap_or_default();
        let Some(name) = arguments.first().and_then(signal_event_name_argument) else {
            return;
        };
        let Some(schema) = signal::event_schema_for_receiver(
            self.parsed,
            self.environment,
            receiver_parts,
            &name,
            node_position(function_call),
        ) else {
            return;
        };
        let payload_arguments = arguments.iter().skip(1).collect::<Vec<_>>();

        if payload_arguments.len() != schema.payload.len() {
            self.diagnostics.push(argument_count_diagnostic(
                &self.parsed.path,
                function_call,
                schema.payload.len() + 1,
                arguments.len(),
            ));
            return;
        }

        for (argument, parameter) in payload_arguments.into_iter().zip(&schema.payload) {
            let argument_type = match argument {
                CallArgument::Expression(argument) => self.infer_expression(argument),
                CallArgument::Inferred(argument_type) => Some(argument_type.clone()),
            };
            let Some(argument_type) = argument_type else {
                continue;
            };

            let Some(failure) =
                self.diagnostic_assignability_failure(&argument_type, &parameter.ty)
            else {
                continue;
            };

            match argument {
                CallArgument::Expression(argument) => {
                    self.diagnostics.push(mismatch_diagnostic_for_expression(
                        &self.parsed.path,
                        argument,
                        &failure,
                        Some(&parameter.name),
                    ));
                }
                CallArgument::Inferred(_) => {
                    self.diagnostics.push(mismatch_diagnostic(
                        &self.parsed.path,
                        function_call,
                        &failure,
                        Some(&parameter.name),
                    ));
                }
            }
            return;
        }
    }

    fn infer_call_argument_types(&mut self, arguments: &[CallArgument<'_>]) -> Vec<Option<Type>> {
        arguments
            .iter()
            .map(|argument| match argument {
                CallArgument::Expression(argument) => self.infer_expression(argument),
                CallArgument::Inferred(argument_type) => Some(argument_type.clone()),
            })
            .collect()
    }

    fn report_typed_call_mismatch(
        &mut self,
        function_call: &FunctionCall,
        arguments: &[CallArgument<'_>],
        argument_types: &[Option<Type>],
        candidate: &TypedCallCandidate,
    ) {
        if let Some(expected) = expected_typed_argument_count(candidate, argument_types.len()) {
            self.diagnostics.push(argument_count_diagnostic(
                &self.parsed.path,
                function_call,
                expected,
                argument_types.len(),
            ));
            return;
        }

        for (index, argument_type) in argument_types.iter().enumerate() {
            let Some(argument_type) = argument_type else {
                continue;
            };
            let Some(parameter_type) = typed_call_parameter_type(candidate, index) else {
                continue;
            };

            let Some(failure) =
                self.diagnostic_assignability_failure(argument_type, parameter_type)
            else {
                continue;
            };
            let argument_label = typed_call_parameter_name(candidate, index)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("argument {}", index + 1));

            match arguments.get(index) {
                Some(CallArgument::Expression(argument)) => {
                    self.diagnostics.push(mismatch_diagnostic_for_expression(
                        &self.parsed.path,
                        argument,
                        &failure,
                        Some(&argument_label),
                    ));
                }
                Some(CallArgument::Inferred(_)) | None => {
                    self.diagnostics.push(mismatch_diagnostic(
                        &self.parsed.path,
                        function_call,
                        &failure,
                        Some(&argument_label),
                    ));
                }
            }
            return;
        }
    }

    fn call_arguments_for_function<'b>(
        &mut self,
        function_call: &'b FunctionCall,
    ) -> Vec<CallArgument<'b>> {
        let mut arguments = call_arguments(function_call).unwrap_or_default();

        if !is_method_call(function_call) {
            if let Some(receiver_type) = self.class_dot_call_receiver_type(function_call) {
                arguments.insert(0, CallArgument::Inferred(receiver_type));
            }
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

    fn class_dot_call_receiver_type(&self, function_call: &FunctionCall) -> Option<Type> {
        let dot_call = dot_method_call(function_call)?;
        let class_name = self.class_name_for_receiver(&dot_call.receiver, dot_call.position)?;

        if !classes::class_method_exists(
            &self.parsed.class_declarations,
            &class_name,
            &dot_call.method,
        ) {
            return None;
        }

        let parts = dot_call
            .receiver
            .split('.')
            .map(str::to_owned)
            .collect::<Vec<_>>();

        self.resolve_field_chain_type(&parts)
    }

    fn class_name_for_receiver(&self, receiver: &str, position: SourcePosition) -> Option<String> {
        if receiver.contains('.') {
            return None;
        }

        let prefix = format!("local {receiver} = ");
        let max_line = position.line.saturating_sub(1);

        let previous_lines = self
            .parsed
            .raw
            .parser_source
            .lines()
            .take(max_line)
            .collect::<Vec<_>>();

        for line in previous_lines.into_iter().rev() {
            let trimmed = line.trim();
            let Some(expression) = trimmed.strip_prefix(&prefix) else {
                continue;
            };
            let expression = expression.trim();

            for class in &self.parsed.class_declarations {
                let constructor = format!("{}.new", class.name);

                if expression.starts_with(&constructor)
                    && expression[constructor.len()..]
                        .trim_start()
                        .starts_with('(')
                {
                    return Some(class.name.clone());
                }
            }
        }

        None
    }

    fn check_external_call_arguments(
        &mut self,
        function_call: &FunctionCall,
        function: &ExternalFunctionType,
    ) -> Vec<Type> {
        let arguments = call_arguments(function_call).unwrap_or_default();

        self.check_external_argument_list(function_call, arguments, function)
    }

    fn check_external_argument_list(
        &mut self,
        node: &impl Node,
        arguments: Vec<CallArgument<'_>>,
        function: &ExternalFunctionType,
    ) -> Vec<Type> {
        let argument_types = self.infer_call_argument_types(&arguments);

        for variant in &function.variants {
            if self.external_call_variant_matches(&argument_types, variant) {
                return variant.return_types.clone();
            }
        }

        let best_variant = function
            .variants
            .iter()
            .find(|variant| variant.expected_argument_count(arguments.len()).is_none())
            .or_else(|| function.variants.first());

        let Some(variant) = best_variant else {
            return vec![Type::Any];
        };

        if let Some(expected) = variant.expected_argument_count(arguments.len()) {
            self.diagnostics.push(argument_count_diagnostic(
                &self.parsed.path,
                node,
                expected,
                arguments.len(),
            ));

            return variant.return_types.clone();
        }

        for (index, argument_type) in argument_types.iter().enumerate() {
            let Some(argument_type) = argument_type else {
                continue;
            };
            let Some(parameter_type) = variant.parameter_type(index) else {
                continue;
            };

            if self.external_argument_assignable(argument_type, parameter_type) {
                continue;
            }

            let argument_label = variant
                .parameter_name(index)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("argument {}", index + 1));
            match arguments.get(index) {
                Some(CallArgument::Expression(argument)) => {
                    self.check_external_argument_type(
                        *argument,
                        argument_type,
                        parameter_type,
                        Some(&argument_label),
                    );
                }
                Some(CallArgument::Inferred(_)) | None => {
                    self.check_external_argument_type(
                        node,
                        argument_type,
                        parameter_type,
                        Some(&argument_label),
                    );
                }
            }

            break;
        }

        variant.return_types.clone()
    }

    fn external_call_variant_matches(
        &self,
        argument_types: &[Option<Type>],
        variant: &ExternalFunctionVariant,
    ) -> bool {
        variant
            .expected_argument_count(argument_types.len())
            .is_none()
            && argument_types
                .iter()
                .enumerate()
                .all(|(index, argument_type)| {
                    let Some(argument_type) = argument_type else {
                        return true;
                    };
                    let Some(parameter_type) = variant.parameter_type(index) else {
                        return true;
                    };

                    self.external_argument_assignable(argument_type, parameter_type)
                })
    }

    fn check_external_argument_type(
        &mut self,
        node: &impl Node,
        source: &Type,
        target: &Type,
        root: Option<&str>,
    ) {
        if self.external_argument_assignable(source, target) {
            return;
        }

        let failure = self
            .diagnostic_assignability_failure(source, target)
            .unwrap_or_else(|| AssignabilityFailure {
                path: Vec::new(),
                expected: target.clone(),
                found: source.clone(),
                kind: AssignabilityFailureKind::TypeMismatch,
            });

        self.diagnostics
            .push(mismatch_diagnostic(&self.parsed.path, node, &failure, root));
    }

    fn external_argument_assignable(&self, source: &Type, target: &Type) -> bool {
        if self.context.is_assignable(source, target) {
            return true;
        }

        let Type::Alias(name) = target else {
            return false;
        };

        if self.context.alias(name).is_some() {
            return false;
        }

        match source {
            Type::Primitive(crate::PrimitiveType::String) => true,
            Type::StringLiteral(value) => {
                let enum_values = lua::enum_value_items(name, "");
                enum_values.is_empty() || enum_values.iter().any(|item| item.label == value)
            }
            _ => false,
        }
    }

    fn function_return_types(&self, name: &str) -> Option<Vec<Type>> {
        self.environment
            .function(name.trim())
            .map(|function| function.return_types_or_any())
    }

    fn infer_var_assignment_target(&mut self, var: &Var) -> Option<Type> {
        match var {
            Var::Name(name) => self.infer_named_assignment_target(&token_text(name)),
            Var::Expression(expression) => {
                if let Some(parts) = plain_field_chain_from_var_expression(expression) {
                    let (field, receiver_parts) = parts.split_last()?;
                    let receiver_type = self.resolve_field_chain_type(receiver_parts)?;

                    return self
                        .resolve_direct_table_field_type(&receiver_type, field)
                        .or_else(|| self.resolve_newindex_assignment_type(&parts));
                }

                self.infer_var_expression(expression)
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
        self.scoped_local_contracts
            .iter()
            .rev()
            .find_map(|locals| locals.get(name).cloned())
            .or_else(|| self.environment.local(name).cloned())
    }

    fn resolve_field_chain_type(&self, parts: &[String]) -> Option<Type> {
        let (root, fields) = parts.split_first()?;
        let local_type = self.infer_named_value(root).and_then(|mut current| {
            for field in fields {
                current = resolve_table_field_type(self.environment, &current, field)?;
            }

            Some(current)
        });

        if local_type.is_some() {
            return local_type;
        }

        if !fields.is_empty() {
            if let Some(export) = required_module_export_type_for_fields(self.parsed, root, fields)
            {
                return Some(export);
            }
        }

        None
    }

    fn resolve_direct_table_field_type(&self, ty: &Type, field: &str) -> Option<Type> {
        match resolve_aliases(ty, self.environment, 0) {
            Type::Table(table) => table.field(field).map(|field| field.ty.clone()),
            Type::Union(types) => {
                let resolved = types
                    .iter()
                    .filter_map(|ty| self.resolve_direct_table_field_type(ty, field))
                    .collect::<Vec<_>>();

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

    fn function_declaration_parameter_locals(
        &self,
        function: &ast::FunctionDeclaration,
    ) -> BTreeMap<String, Type> {
        let name = function.name().to_string();
        let mut locals = self.function_parameter_locals(&name);

        if let Some((parameter, ty)) = love_conf::parameter_type_for_function(function) {
            locals.insert(parameter, ty);
        }

        locals
    }

    fn types_can_overlap(&self, left: &Type, right: &Type) -> bool {
        types_can_overlap(
            &resolve_aliases(left, self.environment, 0),
            &resolve_aliases(right, self.environment, 0),
        )
    }

    fn with_short_circuit_narrowing<T>(
        &mut self,
        lhs: &Expression,
        operator: &ast::BinOp,
        check_rhs: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let narrowed = match operator {
            ast::BinOp::And(_) => self.narrowed_locals_for_condition(lhs),
            ast::BinOp::Or(_) => {
                let aliases = self
                    .environment
                    .aliases()
                    .map(|(name, ty)| (name.to_owned(), ty.clone()))
                    .collect::<BTreeMap<_, _>>();
                let mut narrowed = narrowed_locals_after_false_condition(lhs, &aliases, |name| {
                    self.infer_named_value(name)
                });
                self.extend_with_return_guards(&mut narrowed);
                narrowed
            }
            _ => return check_rhs(self),
        };
        self.scoped_locals.push(narrowed);
        self.scoped_local_contracts.push(BTreeMap::new());
        let result = check_rhs(self);
        self.scoped_local_contracts.pop();
        self.scoped_locals.pop();
        result
    }

    fn narrowed_locals_for_condition(&self, condition: &Expression) -> BTreeMap<String, Type> {
        let aliases = self
            .environment
            .aliases()
            .map(|(name, ty)| (name.to_owned(), ty.clone()))
            .collect::<BTreeMap<_, _>>();

        let mut narrowed =
            narrowed_locals_for_condition(condition, &aliases, |name| self.infer_named_value(name));
        self.extend_with_return_guards(&mut narrowed);
        narrowed
    }

    fn extend_with_return_guards(&self, narrowed: &mut BTreeMap<String, Type>) {
        extend_with_return_guards(narrowed, self.scoped_return_guards.iter().flatten());
    }

    fn forget_return_guards(&mut self, name: &str) {
        for guards in &mut self.scoped_return_guards {
            forget_return_guards(guards, name);
        }
    }

    /// Records `local first, ... = call()` when the call's first value guards
    /// the others, so checking `first` also narrows them.
    fn record_local_return_guards(&mut self, local: &ast::LocalAssignment) {
        let names = local.names().iter().map(token_text).collect::<Vec<_>>();
        for name in &names {
            self.forget_return_guards(name);
        }

        let mut expressions = local.expressions().iter();
        let (Some(Expression::FunctionCall(function_call)), None) =
            (expressions.next(), expressions.next())
        else {
            return;
        };
        if names.len() < 2 {
            return;
        }
        let Some(guarded) = self.call_guarded_return_types(function_call) else {
            return;
        };
        if let Some(scope) = self.scoped_return_guards.last_mut() {
            scope.extend(return_guards_for_names(&names, &guarded));
        }
    }

    fn call_guarded_return_types(&self, function_call: &FunctionCall) -> Option<Vec<Type>> {
        let calls = function_call
            .suffixes()
            .filter(|suffix| matches!(suffix, Suffix::Call(_)))
            .count();
        if calls != 1 || !matches!(function_call.suffixes().last(), Some(Suffix::Call(_))) {
            return None;
        }

        match self.infer_call_target(function_call)? {
            Callable::Function(function) => function.guarded_return_types,
            Callable::TypedFunction(function) => function.guarded_return_types,
            Callable::Any | Callable::ExternalFunction(_) => None,
        }
    }

    fn check_runtime_type_check(&mut self, function_call: &FunctionCall) {
        let Some(call) = runtime_type::is_check_call_parts(function_call) else {
            self.diagnostics.push(unsupported_runtime_check_diagnostic(
                &self.parsed.path,
                function_call,
                "`is` checks must use a local value and one runtime-checkable type target",
            ));
            return;
        };

        if let Err(message) =
            self.validate_runtime_type_target(&call.target, node_position(function_call))
        {
            self.diagnostics.push(unsupported_runtime_check_diagnostic(
                &self.parsed.path,
                function_call,
                message,
            ));
        }
    }

    fn validate_runtime_type_target(
        &self,
        target: &str,
        position: SourcePosition,
    ) -> Result<(), String> {
        let target = target.trim();

        if runtime_type::target_lua_type_name(target).is_some() {
            return Ok(());
        }

        if !runtime_type::is_identifier(target) {
            return Ok(());
        }

        if let Some(runtime_target) = self.infer_named_value(target).or_else(|| {
            SemanticModel::new(self.parsed, self.environment).type_of_name(target, position)
        }) {
            return match resolve_aliases(&runtime_target, self.environment, 0) {
                Type::Table(_) => Ok(()),
                other => Err(format!(
                    "unsupported `is` target `{target}`: expected a metatable table, found {other}"
                )),
            };
        }

        if let Some(alias) = self.environment.alias(target) {
            return match resolve_aliases(alias, self.environment, 0) {
                Type::Primitive(_) => Ok(()),
                other => Err(format!(
                    "unsupported `is` target `{target}`: {other} is erased at runtime"
                )),
            };
        }

        Ok(())
    }

    fn known_primitive_type(&self, ty: &Type) -> bool {
        match resolve_aliases(ty, self.environment, 0) {
            Type::Primitive(_) | Type::StringLiteral(_) => true,
            Type::Union(types) => types.iter().all(|ty| self.known_primitive_type(ty)),
            _ => false,
        }
    }

    fn invalid_operator(&mut self, expression: &Expression, operator: &str, operands: &str) {
        let position = node_position(expression);
        self.diagnostics.push(
            Diagnostic::error(
                &self.parsed.path,
                position.line,
                position.column,
                format!(
                    "operator `{}` cannot be applied to {operands}",
                    operator.trim()
                ),
            )
            .with_code(DiagnosticCode::TYPE_MISMATCH),
        );
    }

    fn check_primitive_binary_operands(
        &mut self,
        expression: &Expression,
        binop: &ast::BinOp,
        left: &Type,
        right: &Type,
    ) {
        // Unknown values and object/metatable operators retain gradual behavior.
        if !self.known_primitive_type(left) || !self.known_primitive_type(right) {
            return;
        }
        let valid = if is_numeric_operator(binop) {
            self.is_number_type(left) && self.is_number_type(right)
        } else if is_ordering_operator(binop) {
            (self.is_number_type(left) && self.is_number_type(right))
                || (self.is_string_type(left) && self.is_string_type(right))
        } else if matches!(binop, ast::BinOp::TwoDots(_)) {
            self.is_concat_operand_type(left) && self.is_concat_operand_type(right)
        } else {
            true
        };
        if !valid {
            self.invalid_operator(
                expression,
                &binop.to_string(),
                &format!("{left} and {right}"),
            );
        }
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

        if is_ordering_operator(binop)
            && ((self.is_number_type(left) && self.is_number_type(right))
                || (self.is_string_type(left) && self.is_string_type(right)))
        {
            return Some(Type::boolean());
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

    fn check_binary_metamethod_operand(
        &mut self,
        lhs: &Expression,
        binop: &ast::BinOp,
        rhs: &Expression,
        left: &Type,
        right: &Type,
    ) {
        let Some(name) = arithmetic_metamethod_name(binop) else {
            return;
        };

        // Lua uses the left operand's metamethod when it has one and the
        // right operand's otherwise, but always calls it as `mm(lhs, rhs)`.
        // The operand that supplied the metamethod satisfies its own
        // contract, so check the other one against its positional parameter:
        // `2 * v` passes BrineVector's `__mul(number | Vec2, ...)`, while
        // `2 / v` fails `__div(Vec2, ...)` as it does at runtime.
        let (function, operand, operand_type, parameter) = if let Some(function) = self
            .resolve_metatable_facts(left)
            .and_then(|facts| facts.metamethod(name).cloned())
        {
            (function, rhs, right, 1)
        } else if let Some(function) = self
            .resolve_metatable_facts(right)
            .and_then(|facts| facts.metamethod(name).cloned())
        {
            (function, lhs, left, 0)
        } else {
            return;
        };

        let Some(expected) = function
            .parameters
            .get(parameter)
            .map(|parameter| &parameter.ty)
        else {
            return;
        };
        self.check_inferred_expression_assignable_at(operand, operand_type, expected, None);
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
                } else if let Some(function) = self
                    .resolve_metatable_facts(operand)
                    .and_then(|facts| facts.unm)
                {
                    Some(metamethod_return_or(&function, operand.clone()))
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

    fn is_string_type(&self, ty: &Type) -> bool {
        match resolve_aliases(ty, self.environment, 0) {
            Type::Primitive(crate::PrimitiveType::String) | Type::StringLiteral(_) => true,
            Type::Union(types) => types.iter().all(|ty| self.is_string_type(ty)),
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
    TypedFunction(Box<TypedFunction>),
    ExternalFunction(ExternalFunctionType),
}

enum CallArgument<'a> {
    Expression(&'a Expression),
    Inferred(Type),
}

struct DotMethodCall {
    parts: Vec<String>,
    receiver: String,
    method: String,
    position: SourcePosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExternalFunctionType {
    variants: Vec<ExternalFunctionVariant>,
}

fn external_function_has_known_return(function: &ExternalFunctionType) -> bool {
    function.variants.iter().any(|variant| {
        variant
            .return_types
            .iter()
            .any(|ty| !matches!(ty, Type::Any))
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExternalFunctionVariant {
    parameters: Vec<ExternalParameter>,
    vararg_type: Option<Type>,
    return_types: Vec<Type>,
}

impl ExternalFunctionVariant {
    fn required_parameter_count(&self) -> usize {
        self.parameters
            .iter()
            .filter(|parameter| !parameter.optional)
            .count()
    }

    fn expected_argument_count(&self, actual: usize) -> Option<usize> {
        let required = self.required_parameter_count();

        if actual < required {
            return Some(required);
        }

        if self.vararg_type.is_none() && actual > self.parameters.len() {
            return Some(self.parameters.len());
        }

        None
    }

    fn parameter_type(&self, index: usize) -> Option<&Type> {
        self.parameters
            .get(index)
            .map(|parameter| &parameter.ty)
            .or(self.vararg_type.as_ref())
    }

    fn parameter_name(&self, index: usize) -> Option<&str> {
        self.parameters
            .get(index)
            .and_then(|parameter| parameter.name.as_deref())
            .or((index >= self.parameters.len()).then_some("..."))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExternalParameter {
    name: Option<String>,
    ty: Type,
    optional: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TypedCallCandidate {
    parameters: Vec<Type>,
    parameter_names: Vec<Option<String>>,
    required_parameter_count: usize,
    has_varargs: bool,
    vararg_type: Option<Type>,
    return_types: Vec<Type>,
}

impl Callable {
    fn from_typed_function(function: &TypedFunction) -> Self {
        if !function.overloads.is_empty()
            || function.has_fully_annotated_parameters()
            || function.vararg_type.is_some()
        {
            Self::TypedFunction(Box::new(function.clone()))
        } else {
            Self::Any
        }
    }
}

fn typed_call_candidates(function: &TypedFunction) -> Vec<TypedCallCandidate> {
    let mut candidates = function
        .overloads
        .iter()
        .map(|overload| TypedCallCandidate {
            parameters: overload
                .parameters
                .iter()
                .map(|parameter| parameter.ty.clone())
                .collect(),
            parameter_names: overload
                .parameters
                .iter()
                .map(|parameter| Some(parameter.name.clone()))
                .collect(),
            required_parameter_count: overload.parameters.len(),
            has_varargs: overload.has_varargs,
            vararg_type: overload.vararg_type.clone(),
            return_types: overload.return_types_or_any(),
        })
        .collect::<Vec<_>>();

    let primary_has_typed_parameters = function
        .parameters
        .iter()
        .any(|parameter| parameter.ty != Type::Any)
        || function.vararg_type.is_some();

    if function.overloads.is_empty()
        || function.has_varargs
        || (function.has_fully_annotated_parameters() && primary_has_typed_parameters)
    {
        candidates.push(TypedCallCandidate {
            parameters: function
                .parameters
                .iter()
                .map(|parameter| parameter.ty.clone())
                .collect(),
            parameter_names: function
                .parameters
                .iter()
                .map(|parameter| Some(parameter.name.clone()))
                .collect(),
            required_parameter_count: function.required_parameter_count,
            has_varargs: function.has_varargs,
            vararg_type: function.vararg_type.clone(),
            return_types: function.return_types_or_any(),
        });
    }

    candidates
}

fn typed_call_candidate_matches(
    checker: &Checker<'_>,
    argument_types: &[Option<Type>],
    candidate: &TypedCallCandidate,
) -> bool {
    if expected_typed_argument_count(candidate, argument_types.len()).is_some() {
        return false;
    }

    argument_types
        .iter()
        .enumerate()
        .all(|(index, argument_type)| {
            let Some(argument_type) = argument_type else {
                return true;
            };
            let Some(parameter_type) = typed_call_parameter_type(candidate, index) else {
                return true;
            };

            checker.context.is_assignable(argument_type, parameter_type)
        })
}

fn expected_typed_argument_count(candidate: &TypedCallCandidate, actual: usize) -> Option<usize> {
    if actual < candidate.required_parameter_count {
        return Some(candidate.required_parameter_count);
    }

    if !candidate.has_varargs && actual > candidate.parameters.len() {
        return Some(candidate.parameters.len());
    }

    None
}

fn typed_call_parameter_type(candidate: &TypedCallCandidate, index: usize) -> Option<&Type> {
    candidate
        .parameters
        .get(index)
        .or(candidate.vararg_type.as_ref())
}

fn typed_call_parameter_name(candidate: &TypedCallCandidate, index: usize) -> Option<&str> {
    candidate
        .parameter_names
        .get(index)
        .and_then(Option::as_deref)
        .or((index >= candidate.parameters.len()).then_some("..."))
}

fn infer_symbol_type(symbol: &str) -> Option<Type> {
    match symbol {
        "true" | "false" => Some(Type::boolean()),
        "nil" => Some(Type::nil()),
        _ => None,
    }
}

fn expression_is_nil(expression: &Expression) -> bool {
    matches!(expression, Expression::Symbol(symbol) if token_text(symbol) == "nil")
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

fn is_ordering_operator(binop: &ast::BinOp) -> bool {
    matches!(
        binop,
        ast::BinOp::GreaterThan(_)
            | ast::BinOp::GreaterThanEqual(_)
            | ast::BinOp::LessThan(_)
            | ast::BinOp::LessThanEqual(_)
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

fn lua_type_field_item(
    environment: &TypeEnvironment,
    ty: &Type,
    field: &str,
) -> Option<lua::LuaItem> {
    if let Type::Alias(name) = ty {
        if let Some(item) = lua::type_field_items(name, field)
            .into_iter()
            .find(|item| item.label == field)
        {
            return Some(item);
        }
    }

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
    let variants = lua::signature_variants(item)
        .map(|signatures| {
            signatures
                .iter()
                .filter_map(|signature| external_function_variant_from_detail(signature.label))
                .collect::<Vec<_>>()
        })
        .filter(|variants| !variants.is_empty())
        .or_else(|| {
            external_function_variant_from_detail(item.detail).map(|variant| vec![variant])
        })?;

    Some(ExternalFunctionType { variants })
}

fn external_function_variant_from_detail(detail: &str) -> Option<ExternalFunctionVariant> {
    let (parameters, vararg_type) = parameters_from_function_detail(detail)?;
    let mut return_types = return_types_from_function_detail(detail);

    if return_types.is_empty() {
        return_types.push(Type::Any);
    }

    Some(ExternalFunctionVariant {
        parameters,
        vararg_type,
        return_types,
    })
}

fn parameters_from_function_detail(detail: &str) -> Option<(Vec<ExternalParameter>, Option<Type>)> {
    let start = detail.find('(')? + 1;
    let end = detail[start..].find(')')? + start;
    let parameters = detail[start..end].trim();

    if parameters.is_empty() {
        return Some((Vec::new(), None));
    }

    let mut fixed = Vec::new();
    let mut vararg_type = None;

    for parameter in parameters
        .split(',')
        .map(str::trim)
        .filter(|parameter| !parameter.is_empty())
    {
        if parameter == "..." {
            vararg_type = Some(Type::Any);
            continue;
        }

        let (name, ty) = parameter
            .split_once(':')
            .map(|(name, ty)| (name.trim(), ty.trim()))
            .unwrap_or((parameter, "any"));
        let ty = type_from_lua_label(ty).unwrap_or(Type::Any);

        if name == "..." {
            vararg_type = Some(ty);
            continue;
        }

        fixed.push(ExternalParameter {
            name: (!name.is_empty()).then(|| name.trim_end_matches('?').to_owned()),
            ty,
            optional: name.ends_with('?'),
        });
    }

    Some((fixed, vararg_type))
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
        "any" | "function" | "light userdata" | "Variant" => Some(Type::Any),
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

/// Arguments of one call suffix, for calls after the first in a chain.
fn function_args_call_arguments(args: &FunctionArgs) -> Vec<CallArgument<'_>> {
    match args {
        FunctionArgs::Parentheses { arguments, .. } => {
            arguments.iter().map(CallArgument::Expression).collect()
        }
        FunctionArgs::String(value) => vec![CallArgument::Inferred(Type::string_literal(
            unquote_string(&value.to_string()),
        ))],
        FunctionArgs::TableConstructor(_) => vec![CallArgument::Inferred(Type::empty_table())],
        _ => Vec::new(),
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

fn signal_event_name_argument(argument: &CallArgument<'_>) -> Option<String> {
    match argument {
        CallArgument::Expression(Expression::String(value)) => {
            Some(unquote_string(&value.to_string()))
        }
        CallArgument::Inferred(Type::StringLiteral(value)) => Some(value.clone()),
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

/// Whether the call this node's arguments come from is a `:` call. Only the
/// first call suffix counts: in `m.make(1):go()`, `make` is a plain call whose
/// result happens to receive a later method call.
fn is_method_call(function_call: &FunctionCall) -> bool {
    function_call
        .suffixes()
        .find_map(|suffix| match suffix {
            Suffix::Call(call) => Some(matches!(call, Call::MethodCall(_))),
            _ => None,
        })
        .unwrap_or(false)
}

fn function_type_from_body(body: &ast::FunctionBody) -> Type {
    crate::types::function_type_from_body(body)
}

fn function_type_from_body_with_receiver(body: &ast::FunctionBody, include_receiver: bool) -> Type {
    let Type::Function(mut function) = function_type_from_body(body) else {
        return Type::Any;
    };

    if include_receiver {
        function
            .parameters
            .insert(0, crate::types::FunctionParameter::anonymous(Type::Any));
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

fn normalize_asset_roots_without_fs(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .filter(|root| !root.as_os_str().is_empty())
        .map(|root| root.components().collect::<PathBuf>())
        .collect()
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

fn callable_from_type(ty: &Type, environment: &TypeEnvironment) -> Option<Callable> {
    match resolve_aliases(ty, environment, 0) {
        Type::Any => Some(Callable::Any),
        Type::Function(function) => Some(Callable::Function(function)),
        Type::Table(table) => table
            .metatable_facts()
            .and_then(|facts| facts.call.clone())
            .map(|function| Callable::Function(call_metamethod_type(function))),
        Type::Union(types) => {
            let mut members = types.iter().map(|ty| resolve_aliases(ty, environment, 0));
            let first = members.next()?;

            members
                .all(|member| member == first)
                .then(|| callable_from_type(&first, environment))
                .flatten()
        }
        _ => None,
    }
}

fn call_metamethod_type(mut function: FunctionType) -> FunctionType {
    if !function.parameters.is_empty() {
        function.parameters.remove(0);
        function.required_parameter_count = function.required_parameter_count.saturating_sub(1);
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

fn compound_metamethod_name(operator: &ast::CompoundOp) -> Option<&'static str> {
    match operator {
        ast::CompoundOp::PlusEqual(_) => Some("__add"),
        ast::CompoundOp::MinusEqual(_) => Some("__sub"),
        ast::CompoundOp::StarEqual(_) => Some("__mul"),
        ast::CompoundOp::SlashEqual(_) => Some("__div"),
        _ => None,
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
    let _ = depth;
    environment.resolve_aliases(ty)
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
    let array = (!array_items.is_empty())
        .then(|| Type::array(union_types(crate::types::merge_record_rows(array_items))));

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

fn union_types_optional(types: impl IntoIterator<Item = Type>) -> Option<Type> {
    let types = types.into_iter().collect::<Vec<_>>();

    (!types.is_empty()).then(|| union_types(types))
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
    span: SourceSpan,
    kind: asset::AssetLoaderKind,
    asset_path: &str,
) -> Diagnostic {
    let position = span.start;
    let message = if asset_path.trim().is_empty() {
        format!("{} asset path is empty", kind.asset_label())
    } else {
        format!("{} asset not found: {asset_path}", kind.asset_label())
    };

    Diagnostic::error(path, position.line, position.column, message)
        .with_code(DiagnosticCode::ASSET_MISSING)
}

fn unread_shader_uniform_diagnostic(
    path: &Path,
    span: SourceSpan,
    asset_path: &str,
    name: &str,
) -> Diagnostic {
    Diagnostic::new(
        path,
        span.start.line,
        span.start.column,
        DiagnosticSeverity::Warning,
        format!(
            "shader `{asset_path}` declares uniform `{name}` but never reads it; GLSL compilers remove unread uniforms, so LOVE raises an error when it is sent. Read it in the shader or guard the call with `Shader:hasUniform`"
        ),
    )
    .with_range(
        span.start.line,
        span.start.column,
        span.end.line,
        span.end.column,
    )
    .with_code(DiagnosticCode::ASSET_SHADER_UNREAD_UNIFORM)
}

/// The declared uniform nearest to a misspelled name, if any is close.
fn closest_uniform<'a>(names: &'a BTreeSet<String>, requested: &str) -> Option<&'a str> {
    names
        .iter()
        .map(|name| (edit_distance(name, requested), name))
        .filter(|(distance, name)| *distance <= (name.len().max(requested.len()) / 3).max(1))
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, name)| name.as_str())
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();

    for (row, left_char) in left.chars().enumerate() {
        let mut current = vec![row + 1];
        for (column, right_char) in right.iter().enumerate() {
            let substitution = previous[column] + usize::from(left_char != *right_char);
            current.push(
                substitution
                    .min(previous[column + 1] + 1)
                    .min(current[column] + 1),
            );
        }
        previous = current;
    }

    previous[right.len()]
}

/// Every value of `for index = first, last[, step]` when the bounds are integer
/// literals and the loop is short enough to expand.
fn constant_loop_values(for_stmt: &ast::NumericFor) -> Option<Vec<i64>> {
    const MAX_EXPANDED_VALUES: i64 = 256;
    let start = integer_literal(for_stmt.start())?;
    let end = integer_literal(for_stmt.end())?;
    let step = match for_stmt.step() {
        Some(step) => integer_literal(step)?,
        None => 1,
    };
    if step == 0 || (end - start) / step >= MAX_EXPANDED_VALUES {
        return None;
    }

    let mut values = Vec::new();
    let mut value = start;
    while (step > 0 && value <= end) || (step < 0 && value >= end) {
        values.push(value);
        value += step;
    }
    Some(values)
}

fn integer_literal(expression: &Expression) -> Option<i64> {
    match expression {
        Expression::Number(token) => token.token().to_string().trim().parse().ok(),
        Expression::UnaryOperator {
            unop: ast::UnOp::Minus(_),
            expression,
        } => integer_literal(expression).map(|value| -value),
        Expression::Parentheses { expression, .. } => integer_literal(expression),
        _ => None,
    }
}

fn wrong_asset_kind_diagnostic(
    path: &Path,
    span: SourceSpan,
    kind: asset::AssetLoaderKind,
    asset_path: &str,
) -> Diagnostic {
    let position = span.start;
    let actual = asset::asset_source_type_label(asset_path).unwrap_or("different");
    let supported = kind.supported_extensions().join(", ");

    Diagnostic::error(
        path,
        position.line,
        position.column,
        format!(
            "wrong {} asset type for `{asset_path}`; found {actual} asset, expected one of: {supported}",
            kind.asset_label()
        ),
    )
    .with_code(DiagnosticCode::ASSET_WRONG_KIND)
}

fn unsupported_asset_extension_diagnostic(
    path: &Path,
    span: SourceSpan,
    kind: asset::AssetLoaderKind,
    asset_path: &str,
) -> Diagnostic {
    let position = span.start;
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

fn asset_path_case_diagnostic(
    path: &Path,
    span: SourceSpan,
    asset_path: &str,
    canonical_path: &str,
) -> Diagnostic {
    Diagnostic::new(
        path,
        span.start.line,
        span.start.column,
        DiagnosticSeverity::Warning,
        format!(
            "asset path casing `{asset_path}` does not match the filesystem path `{canonical_path}`"
        ),
    )
    .with_code(DiagnosticCode::ASSET_PATH_CASE)
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

fn unsupported_runtime_check_diagnostic(
    path: &Path,
    node: &impl Node,
    message: impl Into<String>,
) -> Diagnostic {
    let position = node_position(node);

    Diagnostic::error(path, position.line, position.column, message)
        .with_code(DiagnosticCode::TYPE_UNSUPPORTED_RUNTIME_CHECK)
}

fn mismatch_found_label(expected: &Type, found: &Type) -> String {
    if type_contains_string_literal(expected) || type_contains_lua_enum_alias(expected) {
        found.to_string()
    } else {
        widen_inferred_type(found).to_string()
    }
}

fn unresolved_project_aliases(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
) -> BTreeSet<String> {
    let mut aliases = BTreeSet::new();

    for import in &parsed.type_imports {
        if import.reexport {
            continue;
        }
        for binding in &import.bindings {
            if environment.alias(&binding.local).is_none() {
                aliases.insert(binding.local.clone());
            }
        }
    }

    for binding in require_bindings_for_parsed_source(parsed) {
        let Some(exports) = required_module_exports_for_receiver(parsed, &binding.alias) else {
            continue;
        };

        for field in exports.fields.values() {
            collect_unresolved_aliases(&field.ty, environment, &mut aliases);
        }
    }

    aliases
}

fn collect_unresolved_aliases(
    ty: &Type,
    environment: &TypeEnvironment,
    aliases: &mut BTreeSet<String>,
) {
    match ty {
        Type::Alias(name) => {
            if environment.alias(name).is_none() {
                aliases.insert(name.clone());
            }
        }
        Type::Union(types) | Type::Tuple(types) => {
            for ty in types {
                collect_unresolved_aliases(ty, environment, aliases);
            }
        }
        Type::Function(function) => {
            for parameter in &function.parameters {
                collect_unresolved_aliases(&parameter.ty, environment, aliases);
            }
            if let Some(vararg) = function.vararg_type.as_deref() {
                collect_unresolved_aliases(vararg, environment, aliases);
            }
            for return_type in function.return_types_or_any() {
                collect_unresolved_aliases(return_type, environment, aliases);
            }
        }
        Type::Table(table) => {
            for (_, field) in table.fields() {
                collect_unresolved_aliases(&field.ty, environment, aliases);
            }
        }
        Type::Array(item) => collect_unresolved_aliases(item, environment, aliases),
        Type::Dictionary { key, value } => {
            collect_unresolved_aliases(key, environment, aliases);
            collect_unresolved_aliases(value, environment, aliases);
        }
        Type::Any | Type::Primitive(_) | Type::StringLiteral(_) => {}
    }
}

fn type_contains_any_alias(ty: &Type, aliases: &BTreeSet<String>) -> bool {
    match ty {
        Type::Alias(name) => aliases.contains(name),
        Type::Union(types) | Type::Tuple(types) => {
            types.iter().any(|ty| type_contains_any_alias(ty, aliases))
        }
        Type::Function(function) => {
            function
                .parameters
                .iter()
                .any(|parameter| type_contains_any_alias(&parameter.ty, aliases))
                || function
                    .vararg_type
                    .as_deref()
                    .is_some_and(|ty| type_contains_any_alias(ty, aliases))
                || function
                    .return_types_or_any()
                    .iter()
                    .any(|ty| type_contains_any_alias(ty, aliases))
        }
        Type::Table(table) => table
            .fields()
            .any(|(_, field)| type_contains_any_alias(&field.ty, aliases)),
        Type::Array(item) => type_contains_any_alias(item, aliases),
        Type::Dictionary { key, value } => {
            type_contains_any_alias(key, aliases) || type_contains_any_alias(value, aliases)
        }
        Type::Any | Type::Primitive(_) | Type::StringLiteral(_) => false,
    }
}

fn type_contains_string_literal(ty: &Type) -> bool {
    match ty {
        Type::StringLiteral(_) => true,
        Type::Union(types) => types.iter().any(type_contains_string_literal),
        Type::Function(function) => {
            function
                .parameters
                .iter()
                .any(|parameter| type_contains_string_literal(&parameter.ty))
                || function
                    .vararg_type
                    .as_deref()
                    .is_some_and(type_contains_string_literal)
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

fn expression_has_bracket_index(expression: &Expression) -> bool {
    let Expression::Var(Var::Expression(expression)) = expression else {
        return false;
    };

    expression
        .suffixes()
        .any(|suffix| matches!(suffix, Suffix::Index(Index::Brackets { .. })))
}

fn type_contains_lua_enum_alias(ty: &Type) -> bool {
    match ty {
        Type::Alias(name) => !lua::enum_value_items(name, "").is_empty(),
        Type::Union(types) => types.iter().any(type_contains_lua_enum_alias),
        Type::Function(function) => {
            function
                .parameters
                .iter()
                .any(|parameter| type_contains_lua_enum_alias(&parameter.ty))
                || function
                    .vararg_type
                    .as_deref()
                    .is_some_and(type_contains_lua_enum_alias)
                || function
                    .return_types_or_any()
                    .iter()
                    .any(type_contains_lua_enum_alias)
        }
        Type::Table(table) => table
            .fields()
            .any(|(_, field)| type_contains_lua_enum_alias(&field.ty)),
        Type::Array(item) => type_contains_lua_enum_alias(item),
        Type::Tuple(items) => items.iter().any(type_contains_lua_enum_alias),
        Type::Dictionary { key, value } => {
            type_contains_lua_enum_alias(key) || type_contains_lua_enum_alias(value)
        }
        Type::Primitive(_) | Type::Any | Type::StringLiteral(_) => false,
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

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

#[cfg(test)]
mod tests;
