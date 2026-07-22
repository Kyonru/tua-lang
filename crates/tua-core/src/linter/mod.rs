use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use full_moon::ast::luau::{TypeFieldKey, TypeInfo};
use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, Index, LastStmt, Parameter, Prefix,
    Suffix, TableConstructor, Var,
};
use full_moon::node::Node;
use full_moon::tokenizer::{StringLiteralQuoteType, TokenReference, TokenType};
use full_moon::visitors::Visitor;

use crate::ast_utils::{
    field_chain_before_first_call, plain_field_chain_from_expression, plain_field_chain_from_var,
};
use crate::config::{DiagnosticsConfig, JitConfig};
use crate::module_index::resolve_required_module;
use crate::profile::{self, DiagnosticsTraceCategory};
use crate::runtime_type;
use crate::scope::{scope_map_parsed_source_with_globals, ScopeGlobalKind, ScopeResolution};
use crate::semantic::SemanticModel;
use crate::types::{Type, TypeContext, TypeEnvironment};
use crate::{
    ecs, lua, Diagnostic, DiagnosticCode, DiagnosticSeverity, ParsedSource, SourcePosition,
    SourceSpan, TuaType,
};

mod rules;

pub(crate) fn lint_parsed_source(
    parsed: &ParsedSource,
    config: &DiagnosticsConfig,
    jit_config: &JitConfig,
    environment: &TypeEnvironment,
) -> Vec<Diagnostic> {
    let _trace = profile::diagnostics_span(DiagnosticsTraceCategory::Lints);

    if !config.rules
        && !config.duplicate_keys
        && !config.missing_requires
        && !config.self_assignments
        && !symbol_lints_enabled(config)
        && !correctness_lints_enabled(config)
        && !jit_config.enabled
    {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    let scope = config
        .rules
        .then(|| scope_map_parsed_source_with_globals(parsed, config.globals.iter().cloned()));
    let resolved_luajit_type_references = scope
        .as_ref()
        .map(|scope| {
            scope
                .references()
                .iter()
                .filter_map(|reference| {
                    (reference.name == "type"
                        && matches!(
                            &reference.resolution,
                            ScopeResolution::Global(global)
                                if global.kind == ScopeGlobalKind::LuaBuiltin
                        ))
                    .then_some((reference.span.start.line, reference.span.start.column))
                })
                .collect()
        })
        .unwrap_or_default();

    if config.rules
        || config.duplicate_keys
        || config.missing_requires
        || config.self_assignments
        || correctness_lints_enabled(config)
    {
        let mut visitor = LintVisitor {
            parsed,
            config,
            semantic: SemanticModel::new(parsed, environment),
            type_context: environment.to_type_context(),
            resolved_luajit_type_references,
            diagnostics: Vec::new(),
        };

        visitor.visit_ast(&parsed.ast);
        diagnostics.extend(visitor.diagnostics);
    }

    if symbol_lints_enabled(config) {
        diagnostics.extend(SymbolAnalyzer::new(parsed, config).analyze().diagnostics);
    }

    if config.rules {
        diagnostics.extend(rules::lint(
            parsed,
            scope
                .as_ref()
                .expect("standard rules should build a scope map"),
        ));
    }

    if jit_config.enabled {
        diagnostics.extend(JitAnalyzer::new(parsed, jit_config, environment).analyze());
    }

    diagnostics
}

pub(crate) fn import_usages(parsed: &ParsedSource) -> Vec<ImportUsage> {
    let config = DiagnosticsConfig {
        rules: false,
        ..DiagnosticsConfig::default()
    };

    SymbolAnalyzer::new(parsed, &config).analyze().imports
}

fn symbol_lints_enabled(config: &DiagnosticsConfig) -> bool {
    config.rules
        || config.undefined_variables
        || config.unused_locals
        || config.unused_parameters
        || config.use_before_definition
        || config.unused_assignments
        || config.uncalled_local_functions
}

fn correctness_lints_enabled(config: &DiagnosticsConfig) -> bool {
    config.rules
        || config.duplicate_parameters
        || config.bad_requires
        || config.impossible_boolean_expressions
        || config.duplicate_if_conditions
        || config.duplicate_binary_operands
        || config.float_equality
        || config.const_assignments
        || config.assignment_arity
        || config.local_declaration_arity
        || config.binary_operator_type_mismatches
}

struct LintVisitor<'a> {
    parsed: &'a ParsedSource,
    config: &'a DiagnosticsConfig,
    semantic: SemanticModel<'a>,
    type_context: TypeContext,
    resolved_luajit_type_references: BTreeSet<(usize, usize)>,
    diagnostics: Vec<Diagnostic>,
}

impl Visitor for LintVisitor<'_> {
    fn visit_function_declaration(&mut self, function: &ast::FunctionDeclaration) {
        self.check_duplicate_parameters(function.body(), function.name().method_name().is_some());
    }

    fn visit_local_function(&mut self, function: &ast::LocalFunction) {
        self.check_duplicate_parameters(function.body(), false);
    }

    fn visit_anonymous_function(&mut self, function: &ast::AnonymousFunction) {
        self.check_duplicate_parameters(function.body(), false);
    }

    fn visit_assignment(&mut self, assignment: &full_moon::ast::Assignment) {
        if self.config.rules || self.config.assignment_arity {
            self.check_assignment_arity(assignment);
        }

        if self.config.self_assignments {
            for (variable, expression) in
                assignment.variables().iter().zip(assignment.expressions())
            {
                let Some(left) = plain_field_chain_from_var(variable) else {
                    continue;
                };
                let Some(right) = plain_field_chain_from_expression(expression) else {
                    continue;
                };

                if left == right {
                    self.diagnostics.push(self_assignment_diagnostic(
                        &self.parsed.path,
                        variable,
                        &left.join("."),
                    ));
                }
            }
        }

        if self.config.const_assignments {
            for variable in assignment.variables() {
                let Some(name) = plain_field_chain_from_var(variable).and_then(|parts| {
                    (parts.len() == 1 && looks_like_const_name(&parts[0])).then(|| parts[0].clone())
                }) else {
                    continue;
                };

                self.diagnostics.push(const_assignment_diagnostic(
                    &self.parsed.path,
                    node_position(variable),
                    &name,
                ));
            }
        }
    }

    fn visit_local_assignment(&mut self, local: &ast::LocalAssignment) {
        if !self.config.rules && !self.config.local_declaration_arity {
            return;
        }

        let local_count = local.names().into_iter().count();

        if local_count == 0 || local.expressions().is_empty() {
            return;
        }

        let Some(value_count) = unbalanced_assignment_value_count(local_count, local.expressions())
        else {
            return;
        };

        if local_count != value_count {
            self.diagnostics.push(local_declaration_arity_diagnostic(
                &self.parsed.path,
                node_position(local),
                local_count,
                value_count,
            ));
        }
    }

    fn visit_function_call(&mut self, function_call: &FunctionCall) {
        if self.config.rules {
            self.check_love_type_name_argument(function_call);
        }

        if !self.config.missing_requires && !self.config.bad_requires {
            return;
        }

        let Some((module, position)) = require_module_literal(function_call) else {
            return;
        };

        if resolve_required_module(&self.parsed.path, &module).is_none() {
            if self.config.bad_requires {
                self.diagnostics
                    .push(bad_require_diagnostic(&self.parsed.path, position, &module));
            } else {
                self.diagnostics.push(missing_require_diagnostic(
                    &self.parsed.path,
                    position,
                    &module,
                ));
            }
        }
    }

    fn visit_table_constructor(&mut self, table: &TableConstructor) {
        if !self.config.rules && !self.config.duplicate_keys {
            return;
        }

        let mut seen = BTreeMap::<LiteralTableKey, DuplicateTableKeyOrigin>::new();
        let list_fields = table
            .fields()
            .iter()
            .filter(|field| matches!(field, Field::NoKey(_)))
            .count();

        for index in 1..=list_fields {
            seen.insert(
                LiteralTableKey::number(index as f64),
                DuplicateTableKeyOrigin::ListEntry(index),
            );
        }

        for field in table.fields() {
            let Some(field_key) = table_field_key(field) else {
                continue;
            };

            if let Some(origin) = seen.insert(
                field_key.identity,
                DuplicateTableKeyOrigin::Explicit(field_key.position),
            ) {
                self.diagnostics.push(duplicate_key_diagnostic(
                    &self.parsed.path,
                    field_key.position,
                    &field_key.label,
                    origin,
                ));
            }
        }
    }

    fn visit_type_info(&mut self, type_info: &TypeInfo) {
        if !self.config.rules && !self.config.duplicate_keys {
            return;
        }

        let TypeInfo::Table { fields, .. } = type_info else {
            return;
        };
        let mut seen = BTreeMap::<String, SourcePosition>::new();

        for field in fields {
            let TypeFieldKey::Name(key) = field.key() else {
                continue;
            };
            let name = token_text(key).trim_end_matches('?').to_owned();
            let position = token_position(key);

            if let Some(origin) = seen.insert(name.clone(), position) {
                self.diagnostics.push(duplicate_type_field_diagnostic(
                    &self.parsed.path,
                    position,
                    &name,
                    origin,
                ));
            }
        }
    }

    fn visit_if(&mut self, if_stmt: &ast::If) {
        if !self.config.rules && !self.config.duplicate_if_conditions {
            return;
        }

        let mut seen = BTreeMap::new();
        let first = expression_key(if_stmt.condition());
        seen.insert(first, node_position(if_stmt.condition()));

        if let Some(else_ifs) = if_stmt.else_if() {
            for else_if in else_ifs {
                let key = expression_key(else_if.condition());

                if let Some(first_position) = seen.get(&key).copied() {
                    self.diagnostics.push(duplicate_if_condition_diagnostic(
                        &self.parsed.path,
                        node_position(else_if.condition()),
                        first_position,
                    ));
                } else {
                    seen.insert(key, node_position(else_if.condition()));
                }
            }
        }
    }

    fn visit_expression(&mut self, expression: &Expression) {
        if let Expression::BinaryOperator { lhs, binop, rhs } = expression {
            self.check_binary_expression(expression, lhs, binop, rhs);
        }

        if self.config.impossible_boolean_expressions {
            if let Expression::UnaryOperator { unop, expression } = expression {
                if matches!(unop, ast::UnOp::Not(_)) {
                    if let Some(value) = boolean_literal(expression) {
                        self.diagnostics.push(impossible_boolean_diagnostic(
                            &self.parsed.path,
                            node_position(unop),
                            !value,
                        ));
                    }
                }
            }
        }
    }
}

impl LintVisitor<'_> {
    fn check_duplicate_parameters(&mut self, body: &ast::FunctionBody, implicit_self: bool) {
        if !self.config.rules && !self.config.duplicate_parameters {
            return;
        }

        let mut seen = BTreeMap::new();
        if implicit_self {
            seen.insert("self".to_owned(), DuplicateLocalOrigin::ImplicitSelf);
        }

        for parameter in body.parameters() {
            let Parameter::Name(name) = parameter else {
                continue;
            };
            let name_text = token_text(name);
            if name_text == "_" {
                continue;
            }
            let position = token_position(name);

            if let Some(original) =
                seen.insert(name_text.clone(), DuplicateLocalOrigin::Explicit(position))
            {
                self.diagnostics.push(duplicate_local_diagnostic(
                    &self.parsed.path,
                    position,
                    &name_text,
                    DuplicateLocalKind::FunctionParameter,
                    original,
                ));
            }
        }
    }

    fn check_assignment_arity(&mut self, assignment: &ast::Assignment) {
        let target_count = assignment.variables().into_iter().count();

        if target_count == 0 {
            return;
        }

        let Some(value_count) =
            unbalanced_assignment_value_count(target_count, assignment.expressions())
        else {
            return;
        };

        if target_count != value_count {
            self.diagnostics.push(assignment_arity_diagnostic(
                &self.parsed.path,
                node_position(assignment),
                target_count,
                value_count,
            ));
        }
    }

    fn check_binary_expression(
        &mut self,
        expression: &Expression,
        lhs: &Expression,
        binop: &ast::BinOp,
        rhs: &Expression,
    ) {
        if self.config.rules && is_equality_operator(binop) {
            self.check_runtime_type_name_comparison(lhs, rhs);
            self.check_runtime_type_name_comparison(rhs, lhs);
        }

        if self.config.impossible_boolean_expressions {
            self.check_impossible_boolean_expression(lhs, binop, rhs);
        }

        if ((self.config.duplicate_binary_operands
            && is_suspicious_duplicate_operand_operator(binop))
            || (self.config.rules && is_boolean_operator(binop)))
            && expression_key(lhs) == expression_key(rhs)
        {
            let operator = binop_label(binop);

            self.diagnostics.push(duplicate_binary_operand_diagnostic(
                &self.parsed.path,
                node_position(binop),
                &operator,
            ));
        }

        if self.config.float_equality
            && is_equality_operator(binop)
            && (is_float_literal(lhs) || is_float_literal(rhs))
        {
            self.diagnostics.push(float_equality_diagnostic(
                &self.parsed.path,
                node_position(binop),
            ));
        }

        if self.config.binary_operator_type_mismatches {
            self.check_binary_operator_type(expression, lhs, binop, rhs);
        }
    }

    fn check_runtime_type_name_comparison(
        &mut self,
        call_expression: &Expression,
        type_expression: &Expression,
    ) {
        let Some(call) = function_call_expression(call_expression) else {
            return;
        };
        let Some((value, position)) = string_expression_value(type_expression) else {
            return;
        };

        if self.is_resolved_luajit_type_call(call) {
            if !runtime_type::is_luajit_type_name(&value) {
                self.diagnostics.push(unknown_luajit_type_diagnostic(
                    &self.parsed.path,
                    position,
                    &value,
                ));
            }
            return;
        }

        if self.is_love_object_method_call(call, "type")
            && !lua::love_type_names().contains(&value.as_str())
        {
            self.diagnostics.push(unknown_love_type_diagnostic(
                &self.parsed.path,
                position,
                &value,
            ));
        }
    }

    fn check_love_type_name_argument(&mut self, call: &FunctionCall) {
        if !self.is_love_object_method_call(call, "typeOf") {
            return;
        }
        let Some((value, position)) = first_call_string_argument(call) else {
            return;
        };
        if lua::love_type_names().contains(&value.as_str()) {
            return;
        }

        self.diagnostics.push(unknown_love_type_diagnostic(
            &self.parsed.path,
            position,
            &value,
        ));
    }

    fn is_resolved_luajit_type_call(&self, call: &FunctionCall) -> bool {
        let Some(parts) = single_call_field_chain(call) else {
            return false;
        };
        let [name] = parts.as_slice() else {
            return false;
        };
        if name != "type" {
            return false;
        }
        let Prefix::Name(name) = call.prefix() else {
            return false;
        };
        let position = token_position(name);

        self.resolved_luajit_type_references
            .contains(&(position.line, position.column))
    }

    fn is_love_object_method_call(&self, call: &FunctionCall, expected_method: &str) -> bool {
        let Some(parts) = single_call_field_chain(call) else {
            return false;
        };
        let Some((method, receiver_parts)) = parts.split_last() else {
            return false;
        };
        if method != expected_method || receiver_parts.is_empty() {
            return false;
        }
        let Some(receiver_type) = self
            .semantic
            .type_of_field_chain(receiver_parts, node_position(call))
        else {
            return false;
        };

        type_is_love_object(&receiver_type, &self.type_context, 0)
    }

    fn check_binary_operator_type(
        &mut self,
        expression: &Expression,
        lhs: &Expression,
        binop: &ast::BinOp,
        rhs: &Expression,
    ) {
        if !is_type_checked_binary_operator(binop) {
            return;
        }

        let Some(left) = self
            .semantic
            .type_of_expression_return(lhs, node_position(lhs), 0)
        else {
            return;
        };
        let Some(right) = self
            .semantic
            .type_of_expression_return(rhs, node_position(rhs), 0)
        else {
            return;
        };

        if type_is_dynamic(&left, &self.type_context) || type_is_dynamic(&right, &self.type_context)
        {
            return;
        }

        if self
            .semantic
            .type_of_expression_return(expression, node_position(expression), 0)
            .is_some()
        {
            return;
        }

        self.diagnostics.push(binary_operator_type_diagnostic(
            &self.parsed.path,
            node_position(binop),
            &binop_label(binop),
            &left,
            &right,
        ));
    }

    fn check_impossible_boolean_expression(
        &mut self,
        lhs: &Expression,
        binop: &ast::BinOp,
        rhs: &Expression,
    ) {
        match binop {
            ast::BinOp::And(_)
                if boolean_literal(lhs) == Some(false) || boolean_literal(rhs) == Some(false) =>
            {
                self.diagnostics.push(impossible_boolean_diagnostic(
                    &self.parsed.path,
                    node_position(binop),
                    false,
                ));
            }
            ast::BinOp::Or(_)
                if boolean_literal(lhs) == Some(true) || boolean_literal(rhs) == Some(true) =>
            {
                self.diagnostics.push(impossible_boolean_diagnostic(
                    &self.parsed.path,
                    node_position(binop),
                    true,
                ));
            }
            _ => {}
        }
    }
}

struct SymbolAnalyzer<'a> {
    parsed: &'a ParsedSource,
    config: &'a DiagnosticsConfig,
    diagnostics: Vec<Diagnostic>,
    scopes: Vec<Scope>,
    declarations: Vec<Declaration>,
    configured_globals: BTreeSet<String>,
    known_globals: BTreeSet<String>,
    defined_globals: BTreeSet<String>,
    shadowable_globals: BTreeMap<String, SourcePosition>,
    unresolved_global_reads: Vec<UnresolvedGlobalRead>,
    user_globals: BTreeMap<String, GlobalUsage>,
    function_stack: Vec<FunctionContext>,
    next_function_id: usize,
    next_declaration_group: usize,
}

#[derive(Clone, Debug)]
struct UnresolvedGlobalRead {
    name: String,
    function_stack: Vec<usize>,
    read_before_write: bool,
    diagnostic_index: usize,
}

#[derive(Clone, Debug)]
struct GlobalUsage {
    definition: SourcePosition,
    common_function_stack: Vec<usize>,
    read_before_write: bool,
    defined_in_module_scope: bool,
    function_definition: Option<SourcePosition>,
    used: bool,
}

impl GlobalUsage {
    fn record_reference(&mut self, function_stack: &[usize], read_before_write: bool) {
        let common_length = self
            .common_function_stack
            .iter()
            .zip(function_stack)
            .take_while(|(left, right)| left == right)
            .count();
        self.common_function_stack.truncate(common_length);
        self.read_before_write |= read_before_write;
    }
}

#[derive(Clone, Debug)]
struct FunctionContext {
    id: usize,
    dominated_globals: BTreeSet<String>,
    conditional_depth: usize,
}

#[derive(Clone, Debug)]
struct Scope {
    bindings: BTreeMap<String, usize>,
    future_locals: BTreeMap<String, SourcePosition>,
    declarations: Vec<usize>,
}

#[derive(Clone, Debug)]
struct Declaration {
    name: String,
    position: SourcePosition,
    kind: DeclarationKind,
    function_id: Option<usize>,
    declaration_group: usize,
    shadowed_local: Option<usize>,
    used: bool,
    called: bool,
    initialized: bool,
    assigned: bool,
    first_read: Option<SourcePosition>,
    assignment_positions: Vec<SourcePosition>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportUsage {
    pub name: String,
    pub position: SourcePosition,
    pub used: bool,
}

struct SymbolAnalysis {
    diagnostics: Vec<Diagnostic>,
    imports: Vec<ImportUsage>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ImportCallKind {
    Require,
    StaticImport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeclarationKind {
    Local,
    Loop,
    LocalFunction,
    Import,
    Parameter,
    ImplicitSelf,
}

impl<'a> SymbolAnalyzer<'a> {
    fn new(parsed: &'a ParsedSource, config: &'a DiagnosticsConfig) -> Self {
        let configured_globals = config.globals.iter().cloned().collect::<BTreeSet<_>>();

        Self {
            parsed,
            config,
            diagnostics: Vec::new(),
            scopes: Vec::new(),
            declarations: Vec::new(),
            configured_globals: configured_globals.clone(),
            known_globals: configured_globals.clone(),
            defined_globals: configured_globals,
            shadowable_globals: BTreeMap::new(),
            unresolved_global_reads: Vec::new(),
            user_globals: BTreeMap::new(),
            function_stack: Vec::new(),
            next_function_id: 0,
            next_declaration_group: 0,
        }
    }

    fn analyze(mut self) -> SymbolAnalysis {
        self.analyze_block(self.parsed.ast.nodes());
        self.resolve_unresolved_global_reads();
        self.report_local_shadows();

        for (name, usage) in &self.user_globals {
            if (self.config.rules || self.config.unused_locals)
                && usage.function_definition.is_some()
                && !usage.used
                && !should_ignore_unused(name)
            {
                self.diagnostics.push(unused_function_diagnostic(
                    &self.parsed.path,
                    usage.function_definition.unwrap_or(usage.definition),
                    name,
                ));
            }

            if self.config.rules && !is_placeholder(name) {
                if !usage.common_function_stack.is_empty() {
                    self.diagnostics.push(global_used_as_local_diagnostic(
                        &self.parsed.path,
                        usage.definition,
                        name,
                    ));
                } else if !usage.read_before_write && !usage.defined_in_module_scope {
                    self.diagnostics
                        .push(global_never_read_before_write_diagnostic(
                            &self.parsed.path,
                            usage.definition,
                            name,
                        ));
                }
            }
        }

        let imports = self
            .declarations
            .iter()
            .filter(|declaration| declaration.kind == DeclarationKind::Import)
            .map(|declaration| ImportUsage {
                name: declaration.name.clone(),
                position: declaration.position,
                used: declaration.used,
            })
            .collect();

        SymbolAnalysis {
            diagnostics: self.diagnostics,
            imports,
        }
    }

    fn report_local_shadows(&mut self) {
        if !self.config.rules {
            return;
        }

        let mut diagnostics = Vec::new();

        for declaration in &self.declarations {
            if !declaration.used {
                continue;
            }

            if let Some(shadowed_id) = declaration.shadowed_local {
                let Some(shadowed) = self.declarations.get(shadowed_id) else {
                    continue;
                };

                if declaration.declaration_group == shadowed.declaration_group
                    || (declaration.kind == DeclarationKind::LocalFunction
                        && shadowed.kind == DeclarationKind::LocalFunction)
                    || declaration.function_id != shadowed.function_id
                {
                    continue;
                }

                diagnostics.push(local_shadow_diagnostic(
                    &self.parsed.path,
                    declaration.position,
                    &declaration.name,
                    shadowed.position,
                ));
                continue;
            }

            if let Some(global_position) = self.shadowable_globals.get(&declaration.name) {
                diagnostics.push(local_shadows_global_diagnostic(
                    &self.parsed.path,
                    declaration.position,
                    &declaration.name,
                    *global_position,
                ));
            }
        }

        self.diagnostics.extend(diagnostics);
    }

    fn analyze_block(&mut self, block: &ast::Block) {
        self.enter_scope(future_locals_for_block(block));

        for stmt in block.stmts() {
            self.analyze_stmt(stmt);
        }

        if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
            for expression in return_stmt.returns() {
                self.analyze_expression(expression);
            }
        }

        self.exit_scope();
    }

    fn analyze_child_block(&mut self, block: &ast::Block) {
        self.analyze_block(block);
    }

    fn enter_conditional_execution(&mut self) {
        if let Some(function) = self.function_stack.last_mut() {
            function.conditional_depth += 1;
        }
    }

    fn exit_conditional_execution(&mut self) {
        if let Some(function) = self.function_stack.last_mut() {
            function.conditional_depth = function.conditional_depth.saturating_sub(1);
        }
    }

    fn analyze_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Assignment(assignment) => self.analyze_assignment(assignment),
            ast::Stmt::CompoundAssignment(assignment) => {
                self.analyze_expression(assignment.rhs());
                self.analyze_compound_assignment_target(assignment.lhs());
            }
            ast::Stmt::FunctionCall(function_call) => self.analyze_function_call(function_call),
            ast::Stmt::LocalAssignment(local) => self.analyze_local_assignment(local),
            ast::Stmt::LocalFunction(function) => self.analyze_local_function(function),
            ast::Stmt::FunctionDeclaration(function) => self.analyze_function_declaration(function),
            ast::Stmt::TypeDeclaration(declaration) => {
                let ty = crate::parser::tua_type_from_type_info(declaration.type_definition());
                self.mark_imports_used_in_type(&ty);
            }
            ast::Stmt::ExportedTypeDeclaration(declaration) => {
                let ty = crate::parser::tua_type_from_type_info(
                    declaration.type_declaration().type_definition(),
                );
                self.mark_imports_used_in_type(&ty);
            }
            ast::Stmt::Do(block) => self.analyze_child_block(block.block()),
            ast::Stmt::If(if_stmt) => {
                self.enter_conditional_execution();
                self.analyze_expression(if_stmt.condition());
                self.analyze_child_block(if_stmt.block());

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        self.analyze_expression(else_if.condition());
                        self.analyze_child_block(else_if.block());
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    self.analyze_child_block(else_block);
                }
                self.exit_conditional_execution();
            }
            ast::Stmt::GenericFor(for_stmt) => {
                self.enter_conditional_execution();
                for expression in for_stmt.expressions() {
                    self.analyze_expression(expression);
                }

                self.enter_scope(BTreeMap::new());
                let declaration_group = self.next_declaration_group();

                for name in for_stmt.names() {
                    self.declare(
                        token_text(name),
                        token_position(name),
                        DeclarationKind::Loop,
                        declaration_group,
                    );
                }

                for stmt in for_stmt.block().stmts() {
                    self.analyze_stmt(stmt);
                }

                if let Some(LastStmt::Return(return_stmt)) = for_stmt.block().last_stmt() {
                    for expression in return_stmt.returns() {
                        self.analyze_expression(expression);
                    }
                }

                self.exit_scope();
                self.exit_conditional_execution();
            }
            ast::Stmt::NumericFor(for_stmt) => {
                self.enter_conditional_execution();
                self.analyze_expression(for_stmt.start());
                self.analyze_expression(for_stmt.end());

                if let Some(step) = for_stmt.step() {
                    self.analyze_expression(step);
                }

                self.enter_scope(BTreeMap::new());
                let declaration_group = self.next_declaration_group();
                self.declare(
                    token_text(for_stmt.index_variable()),
                    token_position(for_stmt.index_variable()),
                    DeclarationKind::Loop,
                    declaration_group,
                );

                for stmt in for_stmt.block().stmts() {
                    self.analyze_stmt(stmt);
                }

                if let Some(LastStmt::Return(return_stmt)) = for_stmt.block().last_stmt() {
                    for expression in return_stmt.returns() {
                        self.analyze_expression(expression);
                    }
                }

                self.exit_scope();
                self.exit_conditional_execution();
            }
            ast::Stmt::Repeat(repeat) => {
                self.enter_conditional_execution();
                self.enter_scope(future_locals_for_block(repeat.block()));

                for stmt in repeat.block().stmts() {
                    self.analyze_stmt(stmt);
                }

                if let Some(LastStmt::Return(return_stmt)) = repeat.block().last_stmt() {
                    for expression in return_stmt.returns() {
                        self.analyze_expression(expression);
                    }
                }

                self.analyze_expression(repeat.until());
                self.exit_scope();
                self.exit_conditional_execution();
            }
            ast::Stmt::While(while_stmt) => {
                self.enter_conditional_execution();
                self.analyze_expression(while_stmt.condition());
                self.analyze_child_block(while_stmt.block());
                self.exit_conditional_execution();
            }
            _ => {}
        }
    }

    fn analyze_assignment(&mut self, assignment: &ast::Assignment) {
        for expression in assignment.expressions() {
            self.analyze_expression(expression);
        }

        for variable in assignment.variables() {
            self.analyze_assignment_target(variable, true, true);
        }
    }

    fn analyze_assignment_target(
        &mut self,
        variable: &Var,
        defines_global: bool,
        initializes_local: bool,
    ) {
        match variable {
            Var::Name(name) => {
                let name_text = token_text(name);

                if self.resolve_local(&name_text).is_none() {
                    if self.config.rules && Self::is_builtin_global(&name_text) {
                        self.diagnostics.push(builtin_global_write_diagnostic(
                            &self.parsed.path,
                            token_position(name),
                            &name_text,
                        ));
                    }
                    self.record_global_assignment(&name_text, token_position(name));
                    if defines_global {
                        self.defined_globals.insert(name_text.clone());
                    }
                    self.known_globals.insert(name_text);
                } else if let Some(id) = self.resolve_local(&name_text) {
                    self.declarations[id]
                        .assignment_positions
                        .push(token_position(name));
                    self.declarations[id].assigned |= initializes_local;
                }
            }
            Var::Expression(expression) => self.analyze_var_expression_base(expression),
            _ => {}
        }
    }

    fn analyze_compound_assignment_target(&mut self, variable: &Var) {
        match variable {
            Var::Name(name) => {
                self.read_name(&token_text(name), token_position(name));
                self.analyze_assignment_target(variable, false, false);
            }
            Var::Expression(expression) => {
                self.analyze_var_expression(expression);
                self.analyze_assignment_target(variable, false, false);
            }
            _ => self.analyze_assignment_target(variable, false, false),
        }
    }

    fn analyze_local_assignment(&mut self, local: &ast::LocalAssignment) {
        let names = local.names().into_iter().collect::<Vec<_>>();
        let expressions = local.expressions().into_iter().collect::<Vec<_>>();
        let import_kind = (names.len() == 1 && expressions.len() == 1)
            .then(|| self.import_call_kind(expressions[0]))
            .flatten();

        for expression in &expressions {
            match import_kind {
                Some(ImportCallKind::StaticImport) => {
                    let Expression::FunctionCall(call) = expression else {
                        continue;
                    };

                    self.analyze_suffixes(call.suffixes());
                }
                Some(ImportCallKind::Require) | None => self.analyze_expression(expression),
            }
        }

        for specifier in local.type_specifiers().flatten() {
            let ty = crate::parser::tua_type_from_type_info(specifier.type_info());
            self.mark_imports_used_in_type(&ty);
        }

        let declaration_group = self.next_declaration_group();
        for (index, name) in names.into_iter().enumerate() {
            self.declare_with_initialization(
                token_text(name),
                token_position(name),
                if import_kind.is_some() {
                    DeclarationKind::Import
                } else {
                    DeclarationKind::Local
                },
                expression_initializes_local(&expressions, index),
                declaration_group,
            );
        }
    }

    fn import_call_kind(&self, expression: &Expression) -> Option<ImportCallKind> {
        let Expression::FunctionCall(call) = expression else {
            return None;
        };
        let Prefix::Name(name) = call.prefix() else {
            return None;
        };
        let name = token_text(name);
        let mut suffixes = call.suffixes();
        let suffix = suffixes.next()?;

        if suffixes.next().is_some() || !matches!(suffix, Suffix::Call(Call::AnonymousCall(_))) {
            return None;
        }

        match name.as_str() {
            "require" if self.resolve_local(&name).is_none() => Some(ImportCallKind::Require),
            "import" if has_static_import_argument(suffix) => Some(ImportCallKind::StaticImport),
            _ => None,
        }
    }

    fn analyze_local_function(&mut self, function: &ast::LocalFunction) {
        let declaration_group = self.next_declaration_group();
        self.declare(
            token_text(function.name()),
            token_position(function.name()),
            DeclarationKind::LocalFunction,
            declaration_group,
        );
        self.analyze_function_body(function.body(), None);
    }

    fn analyze_function_declaration(&mut self, function: &ast::FunctionDeclaration) {
        let names = function.name().names().iter().collect::<Vec<_>>();

        if let Some(root) = names.first() {
            let root_name = token_text(root);

            if names.len() == 1 && function.name().method_name().is_none() {
                if let Some(id) = self.resolve_local(&root_name) {
                    self.declarations[id]
                        .assignment_positions
                        .push(token_position(root));
                    self.declarations[id].assigned = true;
                } else {
                    if self.config.rules && Self::is_builtin_global(&root_name) {
                        self.diagnostics.push(builtin_global_write_diagnostic(
                            &self.parsed.path,
                            token_position(root),
                            &root_name,
                        ));
                    }
                    self.record_global_assignment(&root_name, token_position(root));
                    self.record_global_function_definition(&root_name, token_position(root));
                    self.defined_globals.insert(root_name.clone());
                    self.known_globals.insert(root_name);
                }
            } else {
                self.read_name(&root_name, token_position(root));
            }
        }

        let implicit_self = function.name().method_name().map(token_position);

        self.analyze_function_body(function.body(), implicit_self);
    }

    fn analyze_function_body(
        &mut self,
        body: &ast::FunctionBody,
        implicit_self: Option<SourcePosition>,
    ) {
        for specifier in body.type_specifiers().flatten() {
            let ty = crate::parser::tua_type_from_type_info(specifier.type_info());
            self.mark_imports_used_in_type(&ty);
        }

        if let Some(specifier) = body.return_type() {
            let ty = crate::parser::tua_type_from_type_info(specifier.type_info());
            self.mark_imports_used_in_type(&ty);
        }

        let function_id = self.next_function_id;
        self.next_function_id += 1;
        self.function_stack.push(FunctionContext {
            id: function_id,
            dominated_globals: BTreeSet::new(),
            conditional_depth: 0,
        });
        self.enter_scope(future_locals_for_block(body.block()));
        let declaration_group = self.next_declaration_group();

        if let Some(position) = implicit_self {
            self.declare_implicit_self(position, declaration_group);
        }

        for parameter in body.parameters() {
            if let Parameter::Name(name) = parameter {
                self.declare(
                    token_text(name),
                    token_position(name),
                    DeclarationKind::Parameter,
                    declaration_group,
                );
            }
        }

        if let Some(body_span) = node_span(body) {
            let default_references = self
                .parsed
                .parameter_default_references
                .iter()
                .filter(|reference| reference.function_span == body_span)
                .map(|reference| (reference.name.clone(), reference.position))
                .collect::<Vec<_>>();

            for (name, position) in default_references {
                self.read_name(&name, position);
            }
        }

        for stmt in body.block().stmts() {
            self.analyze_stmt(stmt);
        }

        if let Some(LastStmt::Return(return_stmt)) = body.block().last_stmt() {
            for expression in return_stmt.returns() {
                self.analyze_expression(expression);
            }
        }

        self.exit_scope();
        self.function_stack.pop();
    }

    fn analyze_expression(&mut self, expression: &Expression) {
        match expression {
            Expression::BinaryOperator { lhs, rhs, .. } => {
                self.analyze_expression(lhs);
                self.analyze_expression(rhs);
            }
            Expression::Parentheses { expression, .. } => self.analyze_expression(expression),
            Expression::TypeAssertion {
                expression,
                type_assertion,
            } => {
                self.analyze_expression(expression);
                let ty = crate::parser::tua_type_from_type_info(type_assertion.cast_to());
                self.mark_imports_used_in_type(&ty);
            }
            Expression::UnaryOperator { expression, .. } => self.analyze_expression(expression),
            Expression::Function(function) => self.analyze_function_body(function.body(), None),
            Expression::FunctionCall(function_call) => self.analyze_function_call(function_call),
            Expression::TableConstructor(table) => self.analyze_table_constructor(table),
            Expression::Var(var) => self.analyze_var(var),
            _ => {}
        }
    }

    fn analyze_table_constructor(&mut self, table: &TableConstructor) {
        for field in table.fields() {
            match field {
                Field::ExpressionKey { key, value, .. } => {
                    self.analyze_expression(key);
                    self.analyze_expression(value);
                }
                Field::NameKey { value, .. } => self.analyze_expression(value),
                Field::NoKey(expression) => self.analyze_expression(expression),
                _ => {}
            }
        }
    }

    fn analyze_var(&mut self, var: &Var) {
        match var {
            Var::Name(name) => self.read_name(&token_text(name), token_position(name)),
            Var::Expression(expression) => self.analyze_var_expression(expression),
            _ => {}
        }
    }

    fn analyze_var_expression(&mut self, expression: &ast::VarExpression) {
        self.analyze_prefix(expression.prefix());
        self.analyze_suffixes(expression.suffixes());
    }

    fn analyze_var_expression_base(&mut self, expression: &ast::VarExpression) {
        self.analyze_prefix(expression.prefix());

        for suffix in expression.suffixes() {
            if let Suffix::Index(Index::Brackets { expression, .. }) = suffix {
                self.analyze_expression(expression);
            }
        }
    }

    fn analyze_function_call(&mut self, function_call: &FunctionCall) {
        self.mark_direct_local_function_call(function_call);
        if runtime_type::is_check_call(function_call) {
            self.analyze_suffixes(function_call.suffixes());
            return;
        }

        self.analyze_prefix(function_call.prefix());
        self.analyze_suffixes(function_call.suffixes());
    }

    fn analyze_prefix(&mut self, prefix: &Prefix) {
        match prefix {
            Prefix::Name(name) => self.read_name(&token_text(name), token_position(name)),
            Prefix::Expression(expression) => self.analyze_expression(expression),
            _ => {}
        }
    }

    fn analyze_suffixes<'b>(&mut self, suffixes: impl Iterator<Item = &'b Suffix>) {
        for suffix in suffixes {
            match suffix {
                Suffix::Call(call) => self.analyze_call(call),
                Suffix::Index(Index::Brackets { expression, .. }) => {
                    self.analyze_expression(expression);
                }
                Suffix::Index(Index::Dot { .. }) => {}
                _ => {}
            }
        }
    }

    fn analyze_call(&mut self, call: &Call) {
        match call {
            Call::AnonymousCall(args) => self.analyze_function_args(args),
            Call::MethodCall(method) => self.analyze_function_args(method.args()),
            _ => {}
        }
    }

    fn analyze_function_args(&mut self, args: &FunctionArgs) {
        match args {
            FunctionArgs::Parentheses { arguments, .. } => {
                for argument in arguments {
                    self.analyze_expression(argument);
                }
            }
            FunctionArgs::TableConstructor(table) => self.analyze_table_constructor(table),
            FunctionArgs::String(_) => {}
            _ => {}
        }
    }

    fn mark_imports_used_in_type(&mut self, ty: &TuaType) {
        match ty {
            TuaType::Named(name) => {
                let Some((prefix, _)) = name.split_once('.') else {
                    return;
                };
                let Some(declaration_id) = self.resolve_local(prefix) else {
                    return;
                };

                if self.declarations[declaration_id].kind == DeclarationKind::Import {
                    self.declarations[declaration_id].used = true;
                }
            }
            TuaType::Union(types) | TuaType::Tuple(types) => {
                for ty in types {
                    self.mark_imports_used_in_type(ty);
                }
            }
            TuaType::Table(fields) => {
                for field in fields {
                    self.mark_imports_used_in_type(&field.ty);
                }
            }
            TuaType::Array(element) => self.mark_imports_used_in_type(element),
            TuaType::Dictionary { key, value } => {
                self.mark_imports_used_in_type(key);
                self.mark_imports_used_in_type(value);
            }
            TuaType::Function {
                parameters,
                returns,
            } => {
                for parameter in parameters {
                    self.mark_imports_used_in_type(&parameter.ty);
                }
                for ty in returns {
                    self.mark_imports_used_in_type(ty);
                }
            }
            TuaType::StringLiteral(_) => {}
        }
    }

    fn enter_scope(&mut self, future_locals: BTreeMap<String, SourcePosition>) {
        self.scopes.push(Scope {
            bindings: BTreeMap::new(),
            future_locals,
            declarations: Vec::new(),
        });
    }

    fn exit_scope(&mut self) {
        let Some(scope) = self.scopes.pop() else {
            return;
        };

        for declaration_id in &scope.declarations {
            let declaration = &self.declarations[*declaration_id];

            if self.config.rules
                && declaration.kind == DeclarationKind::Local
                && !declaration.initialized
                && !declaration.assigned
                && declaration.first_read.is_some()
            {
                self.diagnostics.push(uninitialized_local_diagnostic(
                    &self.parsed.path,
                    declaration.first_read.unwrap_or(declaration.position),
                    declaration.position,
                    &declaration.name,
                ));
            }

            if declaration.used || should_ignore_unused(&declaration.name) {
                continue;
            }

            match declaration.kind {
                DeclarationKind::Local if self.config.unused_locals || self.config.rules => {
                    self.diagnostics.push(unused_local_diagnostic(
                        &self.parsed.path,
                        declaration.position,
                        &declaration.name,
                    ));
                }
                DeclarationKind::LocalFunction
                    if self.config.unused_locals || self.config.rules =>
                {
                    self.diagnostics.push(unused_function_diagnostic(
                        &self.parsed.path,
                        declaration.position,
                        &declaration.name,
                    ));
                }
                DeclarationKind::Import if self.config.rules => {
                    self.diagnostics.push(unused_import_diagnostic(
                        &self.parsed.path,
                        declaration.position,
                        &declaration.name,
                    ));
                }
                DeclarationKind::Parameter if self.config.unused_parameters => {
                    self.diagnostics.push(unused_parameter_diagnostic(
                        &self.parsed.path,
                        declaration.position,
                        &declaration.name,
                    ));
                }
                _ => {}
            }
        }

        for declaration_id in &scope.declarations {
            let declaration = &self.declarations[*declaration_id];

            if declaration.used || should_ignore_unused(&declaration.name) {
                continue;
            }

            if self.config.unused_assignments {
                for position in &declaration.assignment_positions {
                    self.diagnostics.push(unused_assignment_diagnostic(
                        &self.parsed.path,
                        *position,
                        &declaration.name,
                    ));
                }
            }
        }

        for declaration_id in &scope.declarations {
            let declaration = &self.declarations[*declaration_id];

            if declaration.kind != DeclarationKind::LocalFunction
                || !declaration.used
                || declaration.called
                || should_ignore_unused(&declaration.name)
            {
                continue;
            }

            if self.config.uncalled_local_functions {
                self.diagnostics.push(uncalled_local_function_diagnostic(
                    &self.parsed.path,
                    declaration.position,
                    &declaration.name,
                ));
            }
        }
    }

    fn declare(
        &mut self,
        name: String,
        position: SourcePosition,
        kind: DeclarationKind,
        declaration_group: usize,
    ) {
        self.declare_with_initialization(name, position, kind, true, declaration_group);
    }

    fn declare_with_initialization(
        &mut self,
        name: String,
        position: SourcePosition,
        kind: DeclarationKind,
        initialized: bool,
        declaration_group: usize,
    ) {
        let shadowed_local = self.resolve_local(&name);
        let function_id = self.current_function_id();

        if let Some(scope) = self.scopes.last_mut() {
            scope.future_locals.remove(&name);

            let id = self.declarations.len();
            self.declarations.push(Declaration {
                name: name.clone(),
                position,
                kind,
                function_id,
                declaration_group,
                shadowed_local,
                used: false,
                called: false,
                initialized,
                assigned: false,
                first_read: None,
                assignment_positions: Vec::new(),
            });
            scope.bindings.insert(name, id);
            scope.declarations.push(id);
        }
    }

    fn declare_implicit_self(&mut self, position: SourcePosition, declaration_group: usize) {
        let shadowed_local = self.resolve_local("self");
        let function_id = self.current_function_id();

        if let Some(scope) = self.scopes.last_mut() {
            scope.future_locals.remove("self");

            let id = self.declarations.len();
            self.declarations.push(Declaration {
                name: "self".to_owned(),
                position,
                kind: DeclarationKind::ImplicitSelf,
                function_id,
                declaration_group,
                shadowed_local,
                used: false,
                called: false,
                initialized: true,
                assigned: false,
                first_read: None,
                assignment_positions: Vec::new(),
            });
            scope.bindings.insert("self".to_owned(), id);
            scope.declarations.push(id);
        }
    }

    fn next_declaration_group(&mut self) -> usize {
        let declaration_group = self.next_declaration_group;
        self.next_declaration_group += 1;
        declaration_group
    }

    fn read_name(&mut self, name: &str, position: SourcePosition) {
        if self.config.rules && is_placeholder(name) {
            self.diagnostics
                .push(placeholder_read_diagnostic(&self.parsed.path, position));
            return;
        }

        if let Some(id) = self.resolve_local(name) {
            self.declarations[id].used = true;
            self.declarations[id].first_read.get_or_insert(position);
            return;
        }

        self.record_shadowable_global(name, position);

        if self.is_known_global(name) {
            self.record_global_read(name);
            return;
        }

        if let Some(future_position) = self.current_future_local(name) {
            if self.config.use_before_definition {
                self.diagnostics.push(use_before_definition_diagnostic(
                    &self.parsed.path,
                    position,
                    name,
                    future_position,
                ));
                return;
            }
        }

        if self.config.undefined_variables || self.config.rules {
            let diagnostic_index = self.diagnostics.len();
            self.diagnostics.push(undefined_variable_diagnostic(
                &self.parsed.path,
                position,
                name,
            ));
            self.unresolved_global_reads.push(UnresolvedGlobalRead {
                name: name.to_owned(),
                function_stack: self.current_function_stack(),
                read_before_write: self.is_global_read_before_write(name),
                diagnostic_index,
            });
        }
    }

    fn resolve_unresolved_global_reads(&mut self) {
        let mut resolved_diagnostics = BTreeSet::new();

        for read in std::mem::take(&mut self.unresolved_global_reads) {
            if self.is_defined_global(&read.name) {
                resolved_diagnostics.insert(read.diagnostic_index);

                if let Some(usage) = self.user_globals.get_mut(&read.name) {
                    usage.record_reference(&read.function_stack, read.read_before_write);
                    usage.used = true;
                }
            }
        }

        self.diagnostics = std::mem::take(&mut self.diagnostics)
            .into_iter()
            .enumerate()
            .filter_map(|(index, diagnostic)| {
                (!resolved_diagnostics.contains(&index)).then_some(diagnostic)
            })
            .collect();
    }

    fn mark_direct_local_function_call(&mut self, function_call: &FunctionCall) {
        if !self.config.uncalled_local_functions {
            return;
        }

        let Prefix::Name(name) = function_call.prefix() else {
            return;
        };
        let Some(Suffix::Call(_)) = function_call.suffixes().next() else {
            return;
        };
        let Some(id) = self.resolve_local(&token_text(name)) else {
            return;
        };

        self.declarations[id].called = true;
    }

    fn record_global_read(&mut self, name: &str) {
        let function_stack = self.current_function_stack();
        let read_before_write = self.is_global_read_before_write(name);

        if let Some(usage) = self.user_globals.get_mut(name) {
            usage.record_reference(&function_stack, read_before_write);
            usage.used = true;
        }
    }

    fn record_global_assignment(&mut self, name: &str, position: SourcePosition) {
        self.record_shadowable_global(name, position);

        if !self.user_globals.contains_key(name)
            && (self.known_globals.contains(name) || Self::is_builtin_global(name))
        {
            return;
        }

        let function_stack = self.current_function_stack();
        let defined_in_module_scope = function_stack.is_empty();

        if let Some(usage) = self.user_globals.get_mut(name) {
            usage.record_reference(&function_stack, false);
            usage.defined_in_module_scope |= defined_in_module_scope;
        } else {
            self.user_globals.insert(
                name.to_owned(),
                GlobalUsage {
                    definition: position,
                    common_function_stack: function_stack,
                    read_before_write: false,
                    defined_in_module_scope,
                    function_definition: None,
                    used: false,
                },
            );
        }

        if let Some(function) = self.function_stack.last_mut() {
            if function.conditional_depth == 0 {
                function.dominated_globals.insert(name.to_owned());
            }
        }
    }

    fn record_global_function_definition(&mut self, name: &str, position: SourcePosition) {
        if let Some(usage) = self.user_globals.get_mut(name) {
            usage.function_definition = Some(position);
        }
    }

    fn current_function_stack(&self) -> Vec<usize> {
        self.function_stack
            .iter()
            .map(|function| function.id)
            .collect()
    }

    fn current_function_id(&self) -> Option<usize> {
        self.function_stack.last().map(|function| function.id)
    }

    fn record_shadowable_global(&mut self, name: &str, position: SourcePosition) {
        if !self.config.rules
            || self.configured_globals.contains(name)
            || Self::is_builtin_global(name)
            || name == "love"
        {
            return;
        }

        self.shadowable_globals
            .entry(name.to_owned())
            .or_insert(position);
    }

    fn is_global_read_before_write(&self, name: &str) -> bool {
        self.function_stack
            .last()
            .is_none_or(|function| !function.dominated_globals.contains(name))
    }

    fn resolve_local(&self, name: &str) -> Option<usize> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.bindings.get(name).copied())
    }

    fn current_future_local(&self, name: &str) -> Option<SourcePosition> {
        self.scopes
            .last()
            .and_then(|scope| scope.future_locals.get(name).copied())
    }

    fn is_known_global(&self, name: &str) -> bool {
        self.known_globals.contains(name) || Self::is_builtin_global(name)
    }

    fn is_defined_global(&self, name: &str) -> bool {
        self.defined_globals.contains(name) || Self::is_builtin_global(name)
    }

    fn is_builtin_global(name: &str) -> bool {
        name == ecs::MODULE_NAME || lua::is_protected_global(name)
    }
}

struct JitAnalyzer<'a> {
    parsed: &'a ParsedSource,
    config: &'a JitConfig,
    semantic: SemanticModel<'a>,
    type_context: TypeContext,
    diagnostics: Vec<Diagnostic>,
    loop_depth: usize,
}

impl<'a> JitAnalyzer<'a> {
    fn new(
        parsed: &'a ParsedSource,
        config: &'a JitConfig,
        environment: &'a TypeEnvironment,
    ) -> Self {
        Self {
            parsed,
            config,
            semantic: SemanticModel::new(parsed, environment),
            type_context: environment.to_type_context(),
            diagnostics: Vec::new(),
            loop_depth: 0,
        }
    }

    fn analyze(mut self) -> Vec<Diagnostic> {
        self.analyze_block(self.parsed.ast.nodes());
        self.diagnostics
    }

    fn analyze_block(&mut self, block: &ast::Block) {
        for stmt in block.stmts() {
            self.analyze_stmt(stmt);
        }

        if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
            for expression in return_stmt.returns() {
                self.analyze_expression(expression);
            }
        }
    }

    fn analyze_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Assignment(assignment) => self.analyze_assignment(assignment),
            ast::Stmt::FunctionCall(function_call) => self.analyze_function_call(function_call),
            ast::Stmt::LocalAssignment(local) => {
                for expression in local.expressions() {
                    self.analyze_expression(expression);
                }
            }
            ast::Stmt::LocalFunction(function) => self.analyze_function_body(function.body()),
            ast::Stmt::FunctionDeclaration(function) => {
                self.analyze_function_body(function.body());
            }
            ast::Stmt::Do(block) => self.analyze_block(block.block()),
            ast::Stmt::If(if_stmt) => {
                self.analyze_expression(if_stmt.condition());
                self.analyze_block(if_stmt.block());

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        self.analyze_expression(else_if.condition());
                        self.analyze_block(else_if.block());
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    self.analyze_block(else_block);
                }
            }
            ast::Stmt::GenericFor(for_stmt) => {
                for expression in for_stmt.expressions() {
                    self.analyze_expression(expression);
                }

                self.with_loop(|this| this.analyze_block(for_stmt.block()));
            }
            ast::Stmt::NumericFor(for_stmt) => {
                self.analyze_expression(for_stmt.start());
                self.analyze_expression(for_stmt.end());

                if let Some(step) = for_stmt.step() {
                    self.analyze_expression(step);
                }

                self.with_loop(|this| this.analyze_block(for_stmt.block()));
            }
            ast::Stmt::Repeat(repeat) => {
                self.with_loop(|this| this.analyze_block(repeat.block()));
                self.analyze_expression(repeat.until());
            }
            ast::Stmt::While(while_stmt) => {
                self.analyze_expression(while_stmt.condition());
                self.with_loop(|this| this.analyze_block(while_stmt.block()));
            }
            _ => {}
        }
    }

    fn with_loop(&mut self, f: impl FnOnce(&mut Self)) {
        self.loop_depth += 1;
        f(self);
        self.loop_depth = self.loop_depth.saturating_sub(1);
    }

    fn analyze_function_body(&mut self, body: &ast::FunctionBody) {
        self.analyze_block(body.block());
    }

    fn analyze_assignment(&mut self, assignment: &ast::Assignment) {
        for expression in assignment.expressions() {
            self.analyze_expression(expression);
        }

        let expressions = assignment.expressions().iter().collect::<Vec<_>>();

        for (index, variable) in assignment.variables().iter().enumerate() {
            if self.config.hot_loop_hints && self.loop_depth > 0 {
                self.check_hot_loop_dynamic_key(variable);
            }

            let Some(expression) = expressions.get(index).copied() else {
                continue;
            };

            if self.config.variable_type_drift {
                self.check_variable_type_drift(variable, expression);
            }

            if self.config.table_shape_drift {
                self.check_table_shape_drift(variable, expression);
            }
        }
    }

    fn analyze_expression(&mut self, expression: &Expression) {
        match expression {
            Expression::BinaryOperator { lhs, rhs, .. } => {
                self.analyze_expression(lhs);
                self.analyze_expression(rhs);
            }
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. } => self.analyze_expression(expression),
            Expression::UnaryOperator { expression, .. } => self.analyze_expression(expression),
            Expression::Function(function) => self.analyze_function_body(function.body()),
            Expression::FunctionCall(function_call) => self.analyze_function_call(function_call),
            Expression::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        Field::ExpressionKey { key, value, .. } => {
                            self.analyze_expression(key);
                            self.analyze_expression(value);
                        }
                        Field::NameKey { value, .. } => self.analyze_expression(value),
                        Field::NoKey(expression) => self.analyze_expression(expression),
                        _ => {}
                    }
                }
            }
            Expression::Var(var) => self.analyze_var(var),
            _ => {}
        }
    }

    fn analyze_var(&mut self, var: &Var) {
        if let Var::Expression(expression) = var {
            self.analyze_prefix(expression.prefix());
            self.analyze_suffixes(expression.suffixes());
        }
    }

    fn analyze_function_call(&mut self, function_call: &FunctionCall) {
        self.analyze_prefix(function_call.prefix());
        self.analyze_suffixes(function_call.suffixes());
    }

    fn analyze_prefix(&mut self, prefix: &Prefix) {
        if let Prefix::Expression(expression) = prefix {
            self.analyze_expression(expression);
        }
    }

    fn analyze_suffixes<'b>(&mut self, suffixes: impl Iterator<Item = &'b Suffix>) {
        for suffix in suffixes {
            match suffix {
                Suffix::Call(call) => self.analyze_call(call),
                Suffix::Index(Index::Brackets { expression, .. }) => {
                    self.analyze_expression(expression);
                }
                _ => {}
            }
        }
    }

    fn analyze_call(&mut self, call: &Call) {
        match call {
            Call::AnonymousCall(args) => self.analyze_function_args(args),
            Call::MethodCall(method) => self.analyze_function_args(method.args()),
            _ => {}
        }
    }

    fn analyze_function_args(&mut self, args: &FunctionArgs) {
        match args {
            FunctionArgs::Parentheses { arguments, .. } => {
                for argument in arguments {
                    self.analyze_expression(argument);
                }
            }
            FunctionArgs::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        Field::ExpressionKey { key, value, .. } => {
                            self.analyze_expression(key);
                            self.analyze_expression(value);
                        }
                        Field::NameKey { value, .. } => self.analyze_expression(value),
                        Field::NoKey(expression) => self.analyze_expression(expression),
                        _ => {}
                    }
                }
            }
            FunctionArgs::String(_) => {}
            _ => {}
        }
    }

    fn check_variable_type_drift(&mut self, variable: &Var, expression: &Expression) {
        let Some(name) = assigned_local_name(variable) else {
            return;
        };
        let position = node_position(variable);
        let Some(previous_type) = self.previous_name_type(&name, position) else {
            return;
        };
        let Some(next_type) =
            self.semantic
                .type_of_expression_return(expression, node_position(expression), 0)
        else {
            return;
        };

        if self.type_context.is_assignable(&next_type, &previous_type)
            && !jit_types_are_stable(&previous_type, &next_type, &self.type_context)
        {
            self.diagnostics.push(jit_type_drift_diagnostic(
                &self.parsed.path,
                position,
                &name,
                &previous_type,
                &next_type,
            ));
        }
    }

    fn check_table_shape_drift(&mut self, variable: &Var, expression: &Expression) {
        let Some(parts) = plain_field_chain_from_var(variable) else {
            return;
        };

        if parts.len() < 2 {
            return;
        }

        let position = node_position(variable);
        let Some(field) = parts.last() else {
            return;
        };
        let receiver_parts = &parts[..parts.len() - 1];
        let Some(receiver_type) = self.previous_field_chain_type(receiver_parts, position) else {
            return;
        };
        let receiver_type = resolve_alias_for_jit(&receiver_type, &self.type_context);
        let Type::Table(table) = receiver_type else {
            return;
        };
        let Some(next_type) =
            self.semantic
                .type_of_expression_return(expression, node_position(expression), 0)
        else {
            return;
        };

        match table.semantic_field(field) {
            Some(existing_field) => {
                if type_is_dynamic(&existing_field.ty, &self.type_context)
                    || type_is_dynamic(&next_type, &self.type_context)
                {
                    return;
                }

                if !jit_types_are_stable(&existing_field.ty, &next_type, &self.type_context) {
                    self.diagnostics.push(jit_table_field_type_drift_diagnostic(
                        &self.parsed.path,
                        position,
                        &parts.join("."),
                        &existing_field.ty,
                        &next_type,
                    ));
                }
            }
            None => {
                if type_is_dynamic(&next_type, &self.type_context) {
                    return;
                }

                self.diagnostics.push(jit_table_shape_drift_diagnostic(
                    &self.parsed.path,
                    position,
                    &parts.join("."),
                ));
            }
        }
    }

    fn check_hot_loop_dynamic_key(&mut self, variable: &Var) {
        if !var_has_dynamic_bracket_index(variable) {
            return;
        }

        self.diagnostics.push(jit_hot_loop_dynamic_key_diagnostic(
            &self.parsed.path,
            node_position(variable),
        ));
    }

    fn previous_name_type(&self, name: &str, position: SourcePosition) -> Option<Type> {
        self.semantic.type_of_name(name, position_before(position))
    }

    fn previous_field_chain_type(
        &self,
        parts: &[String],
        position: SourcePosition,
    ) -> Option<Type> {
        self.semantic
            .type_of_field_chain(parts, position_before(position))
    }
}

fn has_static_import_argument(suffix: &Suffix) -> bool {
    match suffix {
        Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })) => {
            matches!(arguments.iter().next(), Some(Expression::String(_)))
        }
        Suffix::Call(Call::AnonymousCall(FunctionArgs::String(_))) => true,
        _ => false,
    }
}

fn expression_initializes_local(expressions: &[&Expression], index: usize) -> bool {
    index < expressions.len()
        || expressions.last().is_some_and(|expression| {
            matches!(expression, Expression::FunctionCall(_))
                || matches!(expression, Expression::Symbol(symbol) if token_text(symbol) == "...")
        })
}

fn future_locals_for_block(block: &ast::Block) -> BTreeMap<String, SourcePosition> {
    let mut locals = BTreeMap::new();

    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::LocalAssignment(local) => {
                for name in local.names() {
                    locals
                        .entry(token_text(name))
                        .or_insert_with(|| token_position(name));
                }
            }
            ast::Stmt::LocalFunction(function) => {
                locals
                    .entry(token_text(function.name()))
                    .or_insert_with(|| token_position(function.name()));
            }
            _ => {}
        }
    }

    locals
}

fn should_ignore_unused(name: &str) -> bool {
    name.starts_with('_')
}

fn is_placeholder(name: &str) -> bool {
    name == "_"
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum LiteralTableKey {
    String(Vec<u8>),
    Number(u64),
    Boolean(bool),
}

impl LiteralTableKey {
    fn number(value: f64) -> Self {
        let normalized = if value == 0.0 { 0.0 } else { value };

        Self::Number(normalized.to_bits())
    }
}

#[derive(Clone, Debug)]
struct ResolvedTableFieldKey {
    identity: LiteralTableKey,
    label: String,
    position: SourcePosition,
}

#[derive(Clone, Copy, Debug)]
enum DuplicateTableKeyOrigin {
    ListEntry(usize),
    Explicit(SourcePosition),
}

fn table_field_key(field: &Field) -> Option<ResolvedTableFieldKey> {
    match field {
        Field::NameKey { key, .. } => {
            let label = token_text(key);

            Some(ResolvedTableFieldKey {
                identity: LiteralTableKey::String(label.as_bytes().to_vec()),
                label,
                position: token_position(key),
            })
        }
        Field::ExpressionKey { key, .. } => literal_table_key(key),
        _ => None,
    }
}

fn literal_table_key(expression: &Expression) -> Option<ResolvedTableFieldKey> {
    match expression {
        Expression::String(token) => {
            let label = string_literal_value(token)?;

            Some(ResolvedTableFieldKey {
                identity: LiteralTableKey::String(string_literal_runtime_bytes(token)?),
                label,
                position: token_position(token),
            })
        }
        Expression::Number(token) => {
            let label = number_literal_text(token)?;
            let value = parse_lua_number(&label)?;

            Some(ResolvedTableFieldKey {
                identity: LiteralTableKey::number(value),
                label,
                position: token_position(token),
            })
        }
        Expression::Symbol(token) => {
            let label = token_text(token);

            match label.as_str() {
                "true" => Some(ResolvedTableFieldKey {
                    identity: LiteralTableKey::Boolean(true),
                    label,
                    position: token_position(token),
                }),
                "false" => Some(ResolvedTableFieldKey {
                    identity: LiteralTableKey::Boolean(false),
                    label,
                    position: token_position(token),
                }),
                _ => None,
            }
        }
        Expression::UnaryOperator {
            unop: ast::UnOp::Minus(_),
            expression: inner,
        } => {
            let value = numeric_table_key_value(inner)?;

            Some(ResolvedTableFieldKey {
                identity: LiteralTableKey::number(-value),
                label: expression.to_string().trim().to_owned(),
                position: node_position(expression),
            })
        }
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => literal_table_key(expression),
        _ => None,
    }
}

fn numeric_table_key_value(expression: &Expression) -> Option<f64> {
    match expression {
        Expression::Number(token) => parse_lua_number(&number_literal_text(token)?),
        Expression::UnaryOperator {
            unop: ast::UnOp::Minus(_),
            expression,
        } => numeric_table_key_value(expression).map(|value| -value),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => numeric_table_key_value(expression),
        _ => None,
    }
}

pub(crate) fn table_has_literal_string_key(table: &TableConstructor, candidate: &str) -> bool {
    let candidate = LiteralTableKey::String(candidate.as_bytes().to_vec());

    table.fields().iter().any(|field| {
        table_field_key(field)
            .map(|field| field.identity == candidate)
            .unwrap_or(false)
    })
}

fn require_module_literal(function_call: &FunctionCall) -> Option<(String, SourcePosition)> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    if token_text(name) != "require" {
        return None;
    }

    let Some(Suffix::Call(Call::AnonymousCall(args))) = function_call.suffixes().next() else {
        return None;
    };

    match args {
        FunctionArgs::Parentheses { arguments, .. } => {
            let Some(Expression::String(token)) = arguments.iter().next() else {
                return None;
            };

            Some((string_literal_value(token)?, token_position(token)))
        }
        FunctionArgs::String(token) => Some((string_literal_value(token)?, token_position(token))),
        FunctionArgs::TableConstructor(_) => None,
        _ => None,
    }
}

fn assigned_local_name(variable: &Var) -> Option<String> {
    match variable {
        Var::Name(name) => Some(token_text(name)),
        _ => None,
    }
}

fn position_before(position: SourcePosition) -> SourcePosition {
    if position.column > 1 {
        SourcePosition {
            line: position.line,
            column: position.column - 1,
        }
    } else if position.line > 1 {
        SourcePosition {
            line: position.line - 1,
            column: usize::MAX,
        }
    } else {
        position
    }
}

fn var_has_dynamic_bracket_index(variable: &Var) -> bool {
    let Var::Expression(expression) = variable else {
        return false;
    };

    expression.suffixes().any(|suffix| {
        matches!(
            suffix,
            Suffix::Index(Index::Brackets { expression, .. })
                if literal_table_key(expression).is_none()
        )
    })
}

fn jit_types_are_stable(previous: &Type, next: &Type, context: &TypeContext) -> bool {
    if type_is_dynamic(previous, context) || type_is_dynamic(next, context) {
        return true;
    }

    let previous = resolve_alias_for_jit(previous, context);
    let next = resolve_alias_for_jit(next, context);

    match (&previous, &next) {
        (Type::Union(types), _) => types
            .iter()
            .any(|ty| jit_types_are_stable(ty, &next, context)),
        (_, Type::Union(types)) => types
            .iter()
            .all(|ty| jit_types_are_stable(&previous, ty, context)),
        (Type::StringLiteral(_), Type::StringLiteral(_))
        | (Type::StringLiteral(_), Type::Primitive(crate::PrimitiveType::String))
        | (Type::Primitive(crate::PrimitiveType::String), Type::StringLiteral(_)) => true,
        (Type::Primitive(left), Type::Primitive(right)) => left == right,
        (Type::Table(_), Type::Table(_)) => true,
        (Type::Function(_), Type::Function(_)) => true,
        (Type::Array(left), Type::Array(right)) => jit_types_are_stable(left, right, context),
        (Type::Tuple(left), Type::Tuple(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| jit_types_are_stable(left, right, context))
        }
        (
            Type::Dictionary {
                key: left_key,
                value: left_value,
            },
            Type::Dictionary {
                key: right_key,
                value: right_value,
            },
        ) => {
            jit_types_are_stable(left_key, right_key, context)
                && jit_types_are_stable(left_value, right_value, context)
        }
        _ => false,
    }
}

fn resolve_alias_for_jit(ty: &Type, context: &TypeContext) -> Type {
    match ty {
        Type::Alias(name) => context
            .alias(name)
            .map(|ty| resolve_alias_for_jit(ty, context))
            .unwrap_or_else(|| ty.clone()),
        _ => ty.clone(),
    }
}

fn unbalanced_assignment_value_count<'a>(
    target_count: usize,
    expressions: impl IntoIterator<Item = &'a Expression>,
) -> Option<usize> {
    let expressions = expressions.into_iter().collect::<Vec<_>>();
    let value_count = expressions.len();

    if value_count == 0 || target_count == value_count {
        return None;
    }

    if target_count > value_count {
        let last = expressions.last()?;
        let permits_missing_targets = match last {
            Expression::FunctionCall(_) => true,
            Expression::Symbol(symbol) => matches!(token_text(symbol).as_str(), "..." | "nil"),
            _ => false,
        };

        if permits_missing_targets {
            return None;
        }
    }

    Some(value_count)
}

fn string_literal_value(token: &TokenReference) -> Option<String> {
    match token.token().token_type() {
        TokenType::StringLiteral { literal, .. } => Some(literal.to_string()),
        _ => None,
    }
}

fn string_literal_runtime_bytes(token: &TokenReference) -> Option<Vec<u8>> {
    let TokenType::StringLiteral {
        literal,
        quote_type,
        ..
    } = token.token().token_type()
    else {
        return None;
    };

    match quote_type {
        StringLiteralQuoteType::Brackets => Some(long_string_runtime_bytes(literal.as_bytes())),
        StringLiteralQuoteType::Double | StringLiteralQuoteType::Single => {
            decode_short_string_runtime_bytes(literal.as_bytes())
        }
        _ => None,
    }
}

fn long_string_runtime_bytes(literal: &[u8]) -> Vec<u8> {
    let literal = if literal.starts_with(b"\r\n") {
        &literal[2..]
    } else if literal.starts_with(b"\n") || literal.starts_with(b"\r") {
        &literal[1..]
    } else {
        literal
    };
    let mut output = Vec::with_capacity(literal.len());
    let mut index = 0;

    while index < literal.len() {
        match literal[index] {
            b'\r' => {
                if literal.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                output.push(b'\n');
            }
            byte => output.push(byte),
        }

        index += 1;
    }

    output
}

fn decode_short_string_runtime_bytes(literal: &[u8]) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(literal.len());
    let mut index = 0;

    while index < literal.len() {
        if literal[index] != b'\\' {
            output.push(literal[index]);
            index += 1;
            continue;
        }

        index += 1;
        let escaped = *literal.get(index)?;

        if escaped.is_ascii_digit() {
            let start = index;
            let end = (index + 3).min(literal.len());

            while index < end && literal[index].is_ascii_digit() {
                index += 1;
            }

            let value = std::str::from_utf8(&literal[start..index])
                .ok()?
                .parse::<u16>()
                .ok()?;
            output.push(u8::try_from(value).ok()?);
            continue;
        }

        match escaped {
            b'a' => output.push(0x07),
            b'b' => output.push(0x08),
            b'f' => output.push(0x0c),
            b'n' => output.push(b'\n'),
            b'r' => output.push(b'\r'),
            b't' => output.push(b'\t'),
            b'v' => output.push(0x0b),
            b'\\' => output.push(b'\\'),
            b'"' => output.push(b'"'),
            b'\'' => output.push(b'\''),
            b'\n' => output.push(b'\n'),
            b'\r' => {
                if literal.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                output.push(b'\n');
            }
            b'x' => {
                let high = *literal.get(index + 1)?;
                let low = *literal.get(index + 2)?;
                output.push(hex_digit(high)? * 16 + hex_digit(low)?);
                index += 2;
            }
            b'z' => {
                while literal.get(index + 1).is_some_and(u8::is_ascii_whitespace) {
                    index += 1;
                }
            }
            _ => return None,
        }

        index += 1;
    }

    Some(output)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn function_call_expression(expression: &Expression) -> Option<&FunctionCall> {
    match expression {
        Expression::FunctionCall(call) => Some(call),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => function_call_expression(expression),
        _ => None,
    }
}

fn string_expression_value(expression: &Expression) -> Option<(String, SourcePosition)> {
    match expression {
        Expression::String(token) => Some((string_literal_value(token)?, token_position(token))),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => string_expression_value(expression),
        _ => None,
    }
}

fn single_call_field_chain(function_call: &FunctionCall) -> Option<Vec<String>> {
    let mut call_count = 0;
    let mut ends_with_call = false;

    for suffix in function_call.suffixes() {
        ends_with_call = matches!(suffix, Suffix::Call(_));
        call_count += usize::from(ends_with_call);
    }

    if call_count != 1 || !ends_with_call {
        return None;
    }

    field_chain_before_first_call(function_call)
}

fn first_call_string_argument(function_call: &FunctionCall) -> Option<(String, SourcePosition)> {
    let call = function_call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;
    let arguments = match call {
        Call::AnonymousCall(arguments) => arguments,
        Call::MethodCall(method) => method.args(),
        _ => return None,
    };

    match arguments {
        FunctionArgs::Parentheses { arguments, .. } => {
            string_expression_value(arguments.iter().next()?)
        }
        FunctionArgs::String(token) => Some((string_literal_value(token)?, token_position(token))),
        FunctionArgs::TableConstructor(_) => None,
        _ => None,
    }
}

fn type_is_love_object(ty: &Type, context: &TypeContext, depth: usize) -> bool {
    if depth >= 64 {
        return false;
    }

    match ty {
        Type::Alias(name) => context.alias(name).map_or_else(
            || lua::love_type_names().contains(&name.as_str()),
            |ty| type_is_love_object(ty, context, depth + 1),
        ),
        Type::Union(types) => {
            !types.is_empty()
                && types
                    .iter()
                    .all(|ty| type_is_love_object(ty, context, depth + 1))
        }
        _ => false,
    }
}

fn number_literal_text(token: &TokenReference) -> Option<String> {
    match token.token().token_type() {
        TokenType::Number { text } => Some(text.to_string()),
        _ => None,
    }
}

fn parse_lua_number(text: &str) -> Option<f64> {
    let normalized = text.replace('_', "");

    if let Some(hex) = normalized
        .strip_prefix("0x")
        .or_else(|| normalized.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).ok().map(|value| value as f64)
    } else {
        normalized.parse().ok()
    }
}

fn duplicate_key_diagnostic(
    path: &Path,
    position: SourcePosition,
    key: &str,
    origin: DuplicateTableKeyOrigin,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        duplicate_table_member_message("table key", key, position, origin),
    )
    .with_code(DiagnosticCode::LINT_DUPLICATE_TABLE_KEY)
}

fn duplicate_type_field_diagnostic(
    path: &Path,
    position: SourcePosition,
    field: &str,
    origin: SourcePosition,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        duplicate_table_member_message(
            "table type field",
            field,
            position,
            DuplicateTableKeyOrigin::Explicit(origin),
        ),
    )
    .with_code(DiagnosticCode::LINT_DUPLICATE_TABLE_KEY)
}

fn duplicate_table_member_message(
    subject: &str,
    name: &str,
    position: SourcePosition,
    origin: DuplicateTableKeyOrigin,
) -> String {
    match origin {
        DuplicateTableKeyOrigin::ListEntry(index) => {
            format!("{subject} `{name}` duplicates list entry {index}")
        }
        DuplicateTableKeyOrigin::Explicit(origin) if origin.line == position.line => format!(
            "{subject} `{name}` is a duplicate; previously defined at column {}",
            origin.column
        ),
        DuplicateTableKeyOrigin::Explicit(origin) => format!(
            "{subject} `{name}` is a duplicate; previously defined at line {}",
            origin.line
        ),
    }
}

fn missing_require_diagnostic(path: &Path, position: SourcePosition, module: &str) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("require file not found: `{module}`"),
    )
    .with_code(DiagnosticCode::LINT_MISSING_REQUIRE)
}

fn self_assignment_diagnostic(path: &Path, variable: &Var, name: &str) -> Diagnostic {
    let position = node_position(variable);

    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("self-assignment of `{name}`"),
    )
    .with_code(DiagnosticCode::LINT_SELF_ASSIGNMENT)
}

fn undefined_variable_diagnostic(path: &Path, position: SourcePosition, name: &str) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("undefined variable `{name}`"),
    )
    .with_code(DiagnosticCode::LINT_UNDEFINED_VARIABLE)
}

fn unused_local_diagnostic(path: &Path, position: SourcePosition, name: &str) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("unused local `{name}`; prefix with `_` to silence"),
    )
    .with_code(DiagnosticCode::LINT_UNUSED_LOCAL)
}

fn unused_function_diagnostic(path: &Path, position: SourcePosition, name: &str) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("function `{name}` is never used; prefix with `_` to silence"),
    )
    .with_code(DiagnosticCode::LINT_UNUSED_FUNCTION)
}

fn unused_import_diagnostic(path: &Path, position: SourcePosition, name: &str) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("import `{name}` is never used; prefix with `_` to silence"),
    )
    .with_code(DiagnosticCode::LINT_UNUSED_IMPORT)
}

fn local_shadow_diagnostic(
    path: &Path,
    position: SourcePosition,
    name: &str,
    shadowed_position: SourcePosition,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!(
            "local `{name}` shadows a declaration at {}:{}",
            shadowed_position.line, shadowed_position.column
        ),
    )
    .with_code(DiagnosticCode::LINT_LOCAL_SHADOW)
}

fn local_shadows_global_diagnostic(
    path: &Path,
    position: SourcePosition,
    name: &str,
    global_position: SourcePosition,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!(
            "local `{name}` shadows a global used at {}:{}",
            global_position.line, global_position.column
        ),
    )
    .with_code(DiagnosticCode::LINT_LOCAL_SHADOW)
}

fn builtin_global_write_diagnostic(
    path: &Path,
    position: SourcePosition,
    name: &str,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!(
            "built-in global `{name}` is overwritten here; consider using a local or changing the name"
        ),
    )
    .with_code(DiagnosticCode::LINT_BUILTIN_GLOBAL_WRITE)
}

fn unknown_luajit_type_diagnostic(path: &Path, position: SourcePosition, name: &str) -> Diagnostic {
    let message = runtime_type::LUAJIT_TYPE_NAMES
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(name))
        .map_or_else(
            || {
                format!(
                    "unknown LuaJIT runtime type name `{name}`; expected a value returned by the built-in `type(...)`"
                )
            },
            |candidate| {
                format!(
                    "unknown LuaJIT runtime type name `{name}`; did you mean `{candidate}`?"
                )
            },
        );

    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        message,
    )
    .with_code(DiagnosticCode::LINT_UNKNOWN_RUNTIME_TYPE)
}

fn unknown_love_type_diagnostic(path: &Path, position: SourcePosition, name: &str) -> Diagnostic {
    let message = lua::love_type_names()
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(name))
        .map_or_else(
            || {
                format!(
                    "unknown LOVE object type name `{name}`; expected a LOVE 11.x object type name"
                )
            },
            |candidate| {
                format!("unknown LOVE object type name `{name}`; did you mean `{candidate}`?")
            },
        );

    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        message,
    )
    .with_code(DiagnosticCode::LINT_UNKNOWN_RUNTIME_TYPE)
}

fn placeholder_read_diagnostic(path: &Path, position: SourcePosition) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        "placeholder value `_` is read here; consider using a named variable",
    )
    .with_code(DiagnosticCode::LINT_PLACEHOLDER_READ)
}

fn uninitialized_local_diagnostic(
    path: &Path,
    position: SourcePosition,
    declaration: SourcePosition,
    name: &str,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!(
            "local `{name}` defined at line {} is never initialized or assigned; initialize with `nil` to silence",
            declaration.line
        ),
    )
    .with_code(DiagnosticCode::LINT_UNINITIALIZED_LOCAL)
}

fn global_used_as_local_diagnostic(
    path: &Path,
    position: SourcePosition,
    name: &str,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("global `{name}` is only used from one enclosing function; make it local"),
    )
    .with_code(DiagnosticCode::LINT_GLOBAL_USED_AS_LOCAL)
}

fn global_never_read_before_write_diagnostic(
    path: &Path,
    position: SourcePosition,
    name: &str,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("global `{name}` is never read before being written; make it local"),
    )
    .with_code(DiagnosticCode::LINT_GLOBAL_USED_AS_LOCAL)
}

fn unused_parameter_diagnostic(path: &Path, position: SourcePosition, name: &str) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("unused parameter `{name}`"),
    )
    .with_code(DiagnosticCode::LINT_UNUSED_PARAMETER)
}

fn use_before_definition_diagnostic(
    path: &Path,
    position: SourcePosition,
    name: &str,
    declaration: SourcePosition,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!(
            "use of local `{name}` before its definition at {}:{}",
            declaration.line, declaration.column
        ),
    )
    .with_code(DiagnosticCode::LINT_USE_BEFORE_DEFINITION)
}

#[derive(Clone, Copy)]
enum DuplicateLocalKind {
    Variable,
    FunctionParameter,
}

#[derive(Clone, Copy)]
enum DuplicateLocalOrigin {
    Explicit(SourcePosition),
    ImplicitSelf,
}

fn duplicate_local_diagnostic(
    path: &Path,
    position: SourcePosition,
    name: &str,
    kind: DuplicateLocalKind,
    origin: DuplicateLocalOrigin,
) -> Diagnostic {
    let subject = match kind {
        DuplicateLocalKind::Variable => "variable",
        DuplicateLocalKind::FunctionParameter => "function parameter",
    };
    let message = match origin {
        DuplicateLocalOrigin::ImplicitSelf => {
            format!("{subject} `{name}` already defined implicitly")
        }
        DuplicateLocalOrigin::Explicit(original) if original.line == position.line => {
            format!(
                "{subject} `{name}` already defined on column {}",
                original.column
            )
        }
        DuplicateLocalOrigin::Explicit(original) => {
            format!(
                "{subject} `{name}` already defined on line {}",
                original.line
            )
        }
    };

    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        message,
    )
    .with_code(DiagnosticCode::LINT_DUPLICATE_LOCAL)
}

fn bad_require_diagnostic(path: &Path, position: SourcePosition, module: &str) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("bad require file: `{module}`"),
    )
    .with_code(DiagnosticCode::LINT_BAD_REQUIRE)
}

fn impossible_boolean_diagnostic(
    path: &Path,
    position: SourcePosition,
    result: bool,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("boolean expression is always {result}"),
    )
    .with_code(DiagnosticCode::LINT_IMPOSSIBLE_BOOLEAN)
}

fn duplicate_if_condition_diagnostic(
    path: &Path,
    position: SourcePosition,
    first_position: SourcePosition,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!(
            "duplicate if condition; first seen at {}:{}",
            first_position.line, first_position.column
        ),
    )
    .with_code(DiagnosticCode::LINT_DUPLICATE_IF_CONDITION)
}

fn duplicate_binary_operand_diagnostic(
    path: &Path,
    position: SourcePosition,
    operator: &str,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("duplicate binary operands around `{operator}`"),
    )
    .with_code(DiagnosticCode::LINT_DUPLICATE_BINARY_OPERAND)
}

fn float_equality_diagnostic(path: &Path, position: SourcePosition) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        "float equality comparison may be unstable",
    )
    .with_code(DiagnosticCode::LINT_FLOAT_EQUALITY)
}

fn const_assignment_diagnostic(path: &Path, position: SourcePosition, name: &str) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("assignment to const-like name `{name}`"),
    )
    .with_code(DiagnosticCode::LINT_CONST_ASSIGNMENT)
}

fn assignment_arity_diagnostic(
    path: &Path,
    position: SourcePosition,
    targets: usize,
    values: usize,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        unbalanced_assignment_message(targets, values),
    )
    .with_code(DiagnosticCode::LINT_ASSIGNMENT_ARITY)
}

fn local_declaration_arity_diagnostic(
    path: &Path,
    position: SourcePosition,
    locals: usize,
    values: usize,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        unbalanced_assignment_message(locals, values),
    )
    .with_code(DiagnosticCode::LINT_LOCAL_DECLARATION_ARITY)
}

fn unbalanced_assignment_message(variables: usize, values: usize) -> String {
    let variable_word = if variables == 1 {
        "variable"
    } else {
        "variables"
    };
    let value_word = if values == 1 { "value" } else { "values" };

    if values < variables {
        format!(
            "assigning {values} {value_word} to {variables} {variable_word} initializes extra variables with nil; add `nil` to the value list to silence"
        )
    } else {
        format!(
            "assigning {values} {value_word} to {variables} {variable_word} leaves some values unused"
        )
    }
}

fn unused_assignment_diagnostic(path: &Path, position: SourcePosition, name: &str) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("assignment to local `{name}` is never read"),
    )
    .with_code(DiagnosticCode::LINT_UNUSED_ASSIGNMENT)
}

fn binary_operator_type_diagnostic(
    path: &Path,
    position: SourcePosition,
    operator: &str,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("operator `{operator}` cannot be applied to {left} and {right}"),
    )
    .with_code(DiagnosticCode::LINT_BINARY_OPERATOR_TYPE)
}

fn uncalled_local_function_diagnostic(
    path: &Path,
    position: SourcePosition,
    name: &str,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("local function `{name}` is referenced but never directly called"),
    )
    .with_code(DiagnosticCode::LINT_UNCALLED_LOCAL_FUNCTION)
}

fn jit_type_drift_diagnostic(
    path: &Path,
    position: SourcePosition,
    name: &str,
    previous: &Type,
    next: &Type,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!(
            "LuaJIT shape drift for `{name}`: previously inferred {previous}, now assigned {next}"
        ),
    )
    .with_code(DiagnosticCode::LINT_JIT_TYPE_DRIFT)
}

fn jit_table_shape_drift_diagnostic(
    path: &Path,
    position: SourcePosition,
    field: &str,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("LuaJIT table shape drift: late field insertion `{field}`"),
    )
    .with_code(DiagnosticCode::LINT_JIT_TABLE_SHAPE_DRIFT)
}

fn jit_table_field_type_drift_diagnostic(
    path: &Path,
    position: SourcePosition,
    field: &str,
    previous: &Type,
    next: &Type,
) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!(
            "LuaJIT table field type drift for `{field}`: expected {previous}, assigned {next}"
        ),
    )
    .with_code(DiagnosticCode::LINT_JIT_TABLE_SHAPE_DRIFT)
}

fn jit_hot_loop_dynamic_key_diagnostic(path: &Path, position: SourcePosition) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Hint,
        "LuaJIT hot loop uses a dynamic table write; consider a stable literal field or pre-shaped table",
    )
    .with_code(DiagnosticCode::LINT_JIT_HOT_LOOP_DYNAMIC_KEY)
}

fn expression_key(expression: &Expression) -> String {
    expression
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn boolean_literal(expression: &Expression) -> Option<bool> {
    match expression {
        Expression::Symbol(token) => match token_text(token).as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => boolean_literal(expression),
        _ => None,
    }
}

fn is_float_literal(expression: &Expression) -> bool {
    match expression {
        Expression::Number(token) => number_literal_text(token)
            .map(|number| number.contains('.') || number.contains('e') || number.contains('E'))
            .unwrap_or(false),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => is_float_literal(expression),
        _ => false,
    }
}

fn is_equality_operator(binop: &ast::BinOp) -> bool {
    matches!(binop, ast::BinOp::TwoEqual(_) | ast::BinOp::TildeEqual(_))
}

fn is_type_checked_binary_operator(binop: &ast::BinOp) -> bool {
    matches!(
        binop,
        ast::BinOp::Caret(_)
            | ast::BinOp::GreaterThan(_)
            | ast::BinOp::GreaterThanEqual(_)
            | ast::BinOp::LessThan(_)
            | ast::BinOp::LessThanEqual(_)
            | ast::BinOp::Minus(_)
            | ast::BinOp::Percent(_)
            | ast::BinOp::Plus(_)
            | ast::BinOp::Slash(_)
            | ast::BinOp::Star(_)
            | ast::BinOp::TwoDots(_)
    )
}

fn type_is_dynamic(ty: &Type, context: &TypeContext) -> bool {
    match ty {
        Type::Any => true,
        Type::Alias(name) => context
            .alias(name)
            .map(|ty| type_is_dynamic(ty, context))
            .unwrap_or(true),
        Type::Union(types) => types.iter().any(|ty| type_is_dynamic(ty, context)),
        Type::Function(function) => {
            function
                .parameters
                .iter()
                .any(|parameter| type_is_dynamic(&parameter.ty, context))
                || function
                    .vararg_type
                    .as_deref()
                    .is_some_and(|ty| type_is_dynamic(ty, context))
                || function
                    .return_types_or_any()
                    .iter()
                    .any(|ty| type_is_dynamic(ty, context))
        }
        Type::Table(table) => table
            .fields()
            .any(|(_, field)| type_is_dynamic(&field.ty, context)),
        Type::Array(item) => type_is_dynamic(item, context),
        Type::Tuple(items) => items.iter().any(|ty| type_is_dynamic(ty, context)),
        Type::Dictionary { key, value } => {
            type_is_dynamic(key, context) || type_is_dynamic(value, context)
        }
        Type::Primitive(_) | Type::StringLiteral(_) => false,
    }
}

fn is_suspicious_duplicate_operand_operator(binop: &ast::BinOp) -> bool {
    matches!(
        binop,
        ast::BinOp::And(_)
            | ast::BinOp::Or(_)
            | ast::BinOp::GreaterThan(_)
            | ast::BinOp::GreaterThanEqual(_)
            | ast::BinOp::LessThan(_)
            | ast::BinOp::LessThanEqual(_)
            | ast::BinOp::TildeEqual(_)
            | ast::BinOp::TwoEqual(_)
            | ast::BinOp::Minus(_)
            | ast::BinOp::Slash(_)
            | ast::BinOp::Percent(_)
    )
}

fn is_boolean_operator(binop: &ast::BinOp) -> bool {
    matches!(binop, ast::BinOp::And(_) | ast::BinOp::Or(_))
}

fn binop_label(binop: &ast::BinOp) -> String {
    token_text(binop.token())
}

fn looks_like_const_name(name: &str) -> bool {
    let mut has_uppercase = false;

    for character in name.chars() {
        if character.is_ascii_uppercase() {
            has_uppercase = true;
            continue;
        }

        if character.is_ascii_digit() || character == '_' {
            continue;
        }

        return false;
    }

    has_uppercase
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

fn token_position(token: &TokenReference) -> SourcePosition {
    SourcePosition::from_full_moon(token.token().start_position())
}

fn token_text(token: &TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

#[cfg(test)]
mod tests;
