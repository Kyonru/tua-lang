use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, Index, LastStmt, Parameter, Prefix,
    Suffix, TableConstructor, Var,
};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};
use full_moon::visitors::Visitor;

use crate::config::DiagnosticsConfig;
use crate::module_index::resolve_required_module;
use crate::{lua, Diagnostic, DiagnosticCode, DiagnosticSeverity, ParsedSource, SourcePosition};

pub(crate) fn lint_parsed_source(
    parsed: &ParsedSource,
    config: &DiagnosticsConfig,
) -> Vec<Diagnostic> {
    if !config.duplicate_keys
        && !config.missing_requires
        && !config.self_assignments
        && !config.formatting_lints
        && !symbol_lints_enabled(config)
        && !correctness_lints_enabled(config)
    {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();

    if config.duplicate_keys
        || config.missing_requires
        || config.self_assignments
        || correctness_lints_enabled(config)
    {
        let mut visitor = LintVisitor {
            parsed,
            config,
            diagnostics: Vec::new(),
        };

        visitor.visit_ast(&parsed.ast);
        diagnostics.extend(visitor.diagnostics);
    }

    if config.formatting_lints {
        diagnostics.extend(formatting_lints(parsed));
    }

    if symbol_lints_enabled(config) {
        diagnostics.extend(SymbolAnalyzer::new(parsed, config).analyze());
    }

    diagnostics
}

fn symbol_lints_enabled(config: &DiagnosticsConfig) -> bool {
    config.undefined_variables
        || config.unused_locals
        || config.unused_parameters
        || config.use_before_definition
}

fn correctness_lints_enabled(config: &DiagnosticsConfig) -> bool {
    config.duplicate_parameters
        || config.bad_requires
        || config.impossible_boolean_expressions
        || config.duplicate_if_conditions
        || config.duplicate_binary_operands
        || config.float_equality
        || config.const_assignments
}

struct LintVisitor<'a> {
    parsed: &'a ParsedSource,
    config: &'a DiagnosticsConfig,
    diagnostics: Vec<Diagnostic>,
}

impl Visitor for LintVisitor<'_> {
    fn visit_assignment(&mut self, assignment: &full_moon::ast::Assignment) {
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

    fn visit_function_call(&mut self, function_call: &FunctionCall) {
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
        if !self.config.duplicate_keys {
            return;
        }

        let mut seen = BTreeMap::<String, String>::new();

        for field in table.fields() {
            let Some((key, label, position)) = table_field_key(field) else {
                continue;
            };

            if seen.insert(key, label.clone()).is_some() {
                self.diagnostics.push(duplicate_key_diagnostic(
                    &self.parsed.path,
                    position,
                    &label,
                ));
            }
        }
    }

    fn visit_function_body(&mut self, body: &ast::FunctionBody) {
        if !self.config.duplicate_parameters {
            return;
        }

        let mut seen = BTreeSet::new();

        for parameter in body.parameters() {
            let Parameter::Name(name) = parameter else {
                continue;
            };
            let name_text = token_text(name);

            if !seen.insert(name_text.clone()) {
                self.diagnostics.push(duplicate_parameter_diagnostic(
                    &self.parsed.path,
                    token_position(name),
                    &name_text,
                ));
            }
        }
    }

    fn visit_if(&mut self, if_stmt: &ast::If) {
        if !self.config.duplicate_if_conditions {
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
            self.check_binary_expression(lhs, binop, rhs);
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
    fn check_binary_expression(&mut self, lhs: &Expression, binop: &ast::BinOp, rhs: &Expression) {
        if self.config.impossible_boolean_expressions {
            self.check_impossible_boolean_expression(lhs, binop, rhs);
        }

        if self.config.duplicate_binary_operands
            && is_suspicious_duplicate_operand_operator(binop)
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
    }

    fn check_impossible_boolean_expression(
        &mut self,
        lhs: &Expression,
        binop: &ast::BinOp,
        rhs: &Expression,
    ) {
        match binop {
            ast::BinOp::And(_) => {
                if boolean_literal(lhs) == Some(false) || boolean_literal(rhs) == Some(false) {
                    self.diagnostics.push(impossible_boolean_diagnostic(
                        &self.parsed.path,
                        node_position(binop),
                        false,
                    ));
                }
            }
            ast::BinOp::Or(_) => {
                if boolean_literal(lhs) == Some(true) || boolean_literal(rhs) == Some(true) {
                    self.diagnostics.push(impossible_boolean_diagnostic(
                        &self.parsed.path,
                        node_position(binop),
                        true,
                    ));
                }
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
    known_globals: BTreeSet<String>,
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
    used: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeclarationKind {
    Local,
    Parameter,
}

impl<'a> SymbolAnalyzer<'a> {
    fn new(parsed: &'a ParsedSource, config: &'a DiagnosticsConfig) -> Self {
        Self {
            parsed,
            config,
            diagnostics: Vec::new(),
            scopes: Vec::new(),
            declarations: Vec::new(),
            known_globals: config.globals.iter().cloned().collect(),
        }
    }

    fn analyze(mut self) -> Vec<Diagnostic> {
        self.analyze_block(self.parsed.ast.nodes());

        self.diagnostics
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

    fn analyze_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Assignment(assignment) => self.analyze_assignment(assignment),
            ast::Stmt::FunctionCall(function_call) => self.analyze_function_call(function_call),
            ast::Stmt::LocalAssignment(local) => self.analyze_local_assignment(local),
            ast::Stmt::LocalFunction(function) => self.analyze_local_function(function),
            ast::Stmt::FunctionDeclaration(function) => self.analyze_function_declaration(function),
            ast::Stmt::Do(block) => self.analyze_child_block(block.block()),
            ast::Stmt::If(if_stmt) => {
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
            }
            ast::Stmt::GenericFor(for_stmt) => {
                for expression in for_stmt.expressions() {
                    self.analyze_expression(expression);
                }

                self.enter_scope(BTreeMap::new());

                for name in for_stmt.names() {
                    self.declare(
                        token_text(name),
                        token_position(name),
                        DeclarationKind::Local,
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
            }
            ast::Stmt::NumericFor(for_stmt) => {
                self.analyze_expression(for_stmt.start());
                self.analyze_expression(for_stmt.end());

                if let Some(step) = for_stmt.step() {
                    self.analyze_expression(step);
                }

                self.enter_scope(BTreeMap::new());
                self.declare(
                    token_text(for_stmt.index_variable()),
                    token_position(for_stmt.index_variable()),
                    DeclarationKind::Local,
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
            }
            ast::Stmt::Repeat(repeat) => {
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
            }
            ast::Stmt::While(while_stmt) => {
                self.analyze_expression(while_stmt.condition());
                self.analyze_child_block(while_stmt.block());
            }
            _ => {}
        }
    }

    fn analyze_assignment(&mut self, assignment: &ast::Assignment) {
        for expression in assignment.expressions() {
            self.analyze_expression(expression);
        }

        for variable in assignment.variables() {
            self.analyze_assignment_target(variable);
        }
    }

    fn analyze_assignment_target(&mut self, variable: &Var) {
        match variable {
            Var::Name(name) => {
                let name_text = token_text(name);

                if self.resolve_local(&name_text).is_none() {
                    self.known_globals.insert(name_text);
                }
            }
            Var::Expression(expression) => self.analyze_var_expression_base(expression),
            _ => {}
        }
    }

    fn analyze_local_assignment(&mut self, local: &ast::LocalAssignment) {
        for expression in local.expressions() {
            self.analyze_expression(expression);
        }

        for name in local.names() {
            self.declare(
                token_text(name),
                token_position(name),
                DeclarationKind::Local,
            );
        }
    }

    fn analyze_local_function(&mut self, function: &ast::LocalFunction) {
        self.declare(
            token_text(function.name()),
            token_position(function.name()),
            DeclarationKind::Local,
        );
        self.analyze_function_body(function.body());
    }

    fn analyze_function_declaration(&mut self, function: &ast::FunctionDeclaration) {
        let names = function.name().names().iter().collect::<Vec<_>>();

        if let Some(root) = names.first() {
            let root_name = token_text(root);

            if names.len() == 1 && function.name().method_name().is_none() {
                self.known_globals.insert(root_name);
            } else {
                self.read_name(&root_name, token_position(root));
            }
        }

        self.analyze_function_body(function.body());
    }

    fn analyze_function_body(&mut self, body: &ast::FunctionBody) {
        self.enter_scope(future_locals_for_block(body.block()));

        for parameter in body.parameters() {
            if let Parameter::Name(name) = parameter {
                self.declare(
                    token_text(name),
                    token_position(name),
                    DeclarationKind::Parameter,
                );
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

        for declaration_id in scope.declarations {
            let declaration = &self.declarations[declaration_id];

            if declaration.used || should_ignore_unused(&declaration.name) {
                continue;
            }

            match declaration.kind {
                DeclarationKind::Local if self.config.unused_locals => {
                    self.diagnostics.push(unused_local_diagnostic(
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
    }

    fn declare(&mut self, name: String, position: SourcePosition, kind: DeclarationKind) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.future_locals.remove(&name);

            let id = self.declarations.len();
            self.declarations.push(Declaration {
                name: name.clone(),
                position,
                kind,
                used: false,
            });
            scope.bindings.insert(name, id);
            scope.declarations.push(id);
        }
    }

    fn read_name(&mut self, name: &str, position: SourcePosition) {
        if let Some(id) = self.resolve_local(name) {
            self.declarations[id].used = true;
            return;
        }

        if self.is_known_global(name) {
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

        if self.config.undefined_variables {
            self.diagnostics.push(undefined_variable_diagnostic(
                &self.parsed.path,
                position,
                name,
            ));
        }
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
        self.known_globals.contains(name) || lua::hover_item(&[name.to_owned()]).is_some()
    }
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
    name == "_" || name.starts_with('_')
}

fn table_field_key(field: &Field) -> Option<(String, String, SourcePosition)> {
    match field {
        Field::NameKey { key, .. } => {
            let label = token_text(key);

            Some((format!("string:{label}"), label, token_position(key)))
        }
        Field::ExpressionKey { key, .. } => literal_table_key(key),
        _ => None,
    }
}

fn literal_table_key(expression: &Expression) -> Option<(String, String, SourcePosition)> {
    match expression {
        Expression::String(token) => {
            let label = string_literal_value(token)?;

            Some((format!("string:{label}"), label, token_position(token)))
        }
        Expression::Number(token) => {
            let label = number_literal_text(token)?;

            Some((format!("number:{label}"), label, token_position(token)))
        }
        Expression::Symbol(token) => {
            let label = token_text(token);

            matches!(label.as_str(), "true" | "false")
                .then(|| (format!("boolean:{label}"), label, token_position(token)))
        }
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => literal_table_key(expression),
        _ => None,
    }
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

fn plain_field_chain_from_expression(expression: &Expression) -> Option<Vec<String>> {
    match expression {
        Expression::Var(var) => plain_field_chain_from_var(var),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => {
            plain_field_chain_from_expression(expression)
        }
        _ => None,
    }
}

fn plain_field_chain_from_var(var: &Var) -> Option<Vec<String>> {
    match var {
        Var::Name(name) => Some(vec![token_text(name)]),
        Var::Expression(expression) => plain_field_chain_from_var_expression(expression),
        _ => None,
    }
}

fn plain_field_chain_from_var_expression(
    expression: &full_moon::ast::VarExpression,
) -> Option<Vec<String>> {
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

fn string_literal_value(token: &TokenReference) -> Option<String> {
    match token.token().token_type() {
        TokenType::StringLiteral { literal, .. } => Some(literal.to_string()),
        _ => None,
    }
}

fn number_literal_text(token: &TokenReference) -> Option<String> {
    match token.token().token_type() {
        TokenType::Number { text } => Some(text.to_string()),
        _ => None,
    }
}

fn duplicate_key_diagnostic(path: &Path, position: SourcePosition, key: &str) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("duplicate table key `{key}`"),
    )
    .with_code(DiagnosticCode::LINT_DUPLICATE_TABLE_KEY)
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
        format!("unused local `{name}`"),
    )
    .with_code(DiagnosticCode::LINT_UNUSED_LOCAL)
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

fn duplicate_parameter_diagnostic(path: &Path, position: SourcePosition, name: &str) -> Diagnostic {
    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("duplicate function parameter `{name}`"),
    )
    .with_code(DiagnosticCode::LINT_DUPLICATE_PARAMETER)
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

fn formatting_lints(parsed: &ParsedSource) -> Vec<Diagnostic> {
    let mut analyzer = FormattingAnalyzer::new(parsed);

    analyzer.analyze();
    analyzer.diagnostics
}

struct FormattingAnalyzer<'a> {
    parsed: &'a ParsedSource,
    diagnostics: Vec<Diagnostic>,
    state: SourceMaskState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceMaskState {
    Code,
    ShortString { quote: u8, escaped: bool },
    LongString { close: Vec<u8> },
    BlockComment { close: Vec<u8> },
}

impl<'a> FormattingAnalyzer<'a> {
    fn new(parsed: &'a ParsedSource) -> Self {
        Self {
            parsed,
            diagnostics: Vec::new(),
            state: SourceMaskState::Code,
        }
    }

    fn analyze(&mut self) {
        let lines = self.parsed.source.split_inclusive('\n').collect::<Vec<_>>();

        for (index, raw_line) in lines.iter().enumerate() {
            self.analyze_line(index + 1, raw_line);
        }

        if !self.parsed.source.ends_with('\n') {
            if let Some(last) = self.parsed.source.lines().last() {
                if self.parsed.source.is_empty() || lines.is_empty() {
                    self.analyze_line(1, last);
                }
            }
        }
    }

    fn analyze_line(&mut self, line_number: usize, raw_line: &str) {
        let line = raw_line.trim_end_matches(['\n', '\r']);
        let mask = self.code_mask_for_line(line);

        self.check_trailing_whitespace(line_number, line);
        self.check_indentation(line_number, line, &mask);
        self.check_keyword_spacing(line_number, line.as_bytes(), &mask);
        self.check_separator_spacing(line_number, line.as_bytes(), &mask);
        self.check_bracket_spacing(line_number, line.as_bytes(), &mask);
        self.check_assignment_spacing(line_number, line.as_bytes(), &mask);
    }

    fn code_mask_for_line(&mut self, line: &str) -> Vec<bool> {
        let bytes = line.as_bytes();
        let mut mask = vec![false; bytes.len()];
        let mut index = 0;

        while index < bytes.len() {
            match &mut self.state {
                SourceMaskState::Code => {
                    if bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'-') {
                        if let Some((close, consumed)) = long_bracket_open(&bytes[index + 2..]) {
                            self.state = SourceMaskState::BlockComment { close };
                            index += 2 + consumed;
                        } else {
                            break;
                        }
                    } else if bytes[index] == b'\'' || bytes[index] == b'"' {
                        mask[index] = true;
                        self.state = SourceMaskState::ShortString {
                            quote: bytes[index],
                            escaped: false,
                        };
                        index += 1;
                    } else if let Some((close, consumed)) = long_bracket_open(&bytes[index..]) {
                        mask[index] = true;
                        self.state = SourceMaskState::LongString { close };
                        index += consumed;
                    } else {
                        mask[index] = true;
                        index += 1;
                    }
                }
                SourceMaskState::ShortString { quote, escaped } => {
                    if *escaped {
                        *escaped = false;
                    } else if bytes[index] == b'\\' {
                        *escaped = true;
                    } else if bytes[index] == *quote {
                        mask[index] = true;
                        self.state = SourceMaskState::Code;
                    }

                    index += 1;
                }
                SourceMaskState::LongString { close } | SourceMaskState::BlockComment { close } => {
                    if bytes[index..].starts_with(close) {
                        index += close.len();
                        self.state = SourceMaskState::Code;
                    } else {
                        index += 1;
                    }
                }
            }
        }

        if matches!(self.state, SourceMaskState::ShortString { .. }) {
            self.state = SourceMaskState::Code;
        }

        mask
    }

    fn check_trailing_whitespace(&mut self, line_number: usize, line: &str) {
        let trimmed = line.trim_end_matches([' ', '\t']);

        if trimmed.len() == line.len() {
            return;
        }

        self.push(
            DiagnosticCode::LINT_FORMAT_TRAILING_WHITESPACE,
            line_number,
            trimmed.len() + 1,
            "remove trailing whitespace",
        );
    }

    fn check_indentation(&mut self, line_number: usize, line: &str, mask: &[bool]) {
        let bytes = line.as_bytes();
        let Some(first_code) = mask
            .iter()
            .enumerate()
            .position(|(index, is_code)| *is_code && !bytes[index].is_ascii_whitespace())
        else {
            return;
        };

        if line[..first_code].trim().is_empty() && bytes[..first_code].contains(&b'\t') {
            let column = bytes[..first_code]
                .iter()
                .position(|byte| *byte == b'\t')
                .unwrap_or(0)
                + 1;

            self.push(
                DiagnosticCode::LINT_FORMAT_INDENTATION,
                line_number,
                column,
                "indentation should use spaces, not tabs",
            );
            return;
        }

        let spaces = bytes[..first_code]
            .iter()
            .take_while(|byte| **byte == b' ')
            .count();

        if spaces > 0 && spaces % 2 != 0 {
            self.push(
                DiagnosticCode::LINT_FORMAT_INDENTATION,
                line_number,
                1,
                "indentation should use multiples of two spaces",
            );
        }
    }

    fn check_keyword_spacing(&mut self, line_number: usize, bytes: &[u8], mask: &[bool]) {
        for (keyword, label) in [(b"then".as_slice(), "`then`"), (b"do".as_slice(), "`do`")] {
            let mut index = 0;

            while index + keyword.len() <= bytes.len() {
                if !is_code_range(mask, index, keyword.len())
                    || !bytes[index..].starts_with(keyword)
                    || !is_word_boundary(bytes, index, keyword.len())
                {
                    index += 1;
                    continue;
                }

                if let Some(previous) = previous_code_index(bytes, mask, index) {
                    let spaces = count_spaces_before(bytes, previous, index);

                    if spaces != 1 {
                        self.push(
                            DiagnosticCode::LINT_FORMAT_KEYWORD_SPACING,
                            line_number,
                            index + 1,
                            format!("use one space before {label}"),
                        );
                    }
                }

                if let Some(next) = next_code_index(bytes, mask, index + keyword.len()) {
                    let spaces = count_spaces_after(bytes, index + keyword.len(), next);

                    if spaces > 1 {
                        self.push(
                            DiagnosticCode::LINT_FORMAT_KEYWORD_SPACING,
                            line_number,
                            index + keyword.len() + 1,
                            format!("use at most one space after {label}"),
                        );
                    }
                }

                index += keyword.len();
            }
        }
    }

    fn check_separator_spacing(&mut self, line_number: usize, bytes: &[u8], mask: &[bool]) {
        for index in code_byte_positions(bytes, mask, b',') {
            if previous_code_index(bytes, mask, index).is_some_and(|previous| {
                bytes[previous + 1..index]
                    .iter()
                    .any(|byte| matches!(byte, b' ' | b'\t'))
            }) {
                self.push(
                    DiagnosticCode::LINT_FORMAT_TOKEN_SPACING,
                    line_number,
                    index + 1,
                    "remove whitespace before `,`",
                );
            }

            let Some(next) = next_code_index(bytes, mask, index + 1) else {
                continue;
            };

            if matches!(bytes[next], b')' | b']' | b'}') {
                continue;
            }

            let spaces = count_spaces_after(bytes, index + 1, next);

            if spaces != 1 {
                self.push(
                    DiagnosticCode::LINT_FORMAT_TOKEN_SPACING,
                    line_number,
                    index + 1,
                    "use one space after `,`",
                );
            }
        }
    }

    fn check_bracket_spacing(&mut self, line_number: usize, bytes: &[u8], mask: &[bool]) {
        for index in 0..bytes.len() {
            if !mask.get(index).copied().unwrap_or(false) {
                continue;
            }

            match bytes[index] {
                b'(' | b'[' => {
                    if next_code_index(bytes, mask, index + 1)
                        .is_some_and(|next| bytes[next].is_ascii_graphic() && next > index + 1)
                    {
                        self.push(
                            DiagnosticCode::LINT_FORMAT_TOKEN_SPACING,
                            line_number,
                            index + 2,
                            format!("remove whitespace after `{}`", bytes[index] as char),
                        );
                    }
                }
                b')' | b']' => {
                    if previous_code_index(bytes, mask, index)
                        .is_some_and(|previous| previous + 1 < index)
                    {
                        self.push(
                            DiagnosticCode::LINT_FORMAT_TOKEN_SPACING,
                            line_number,
                            index + 1,
                            format!("remove whitespace before `{}`", bytes[index] as char),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    fn check_assignment_spacing(&mut self, line_number: usize, bytes: &[u8], mask: &[bool]) {
        let mut index = 0;

        while index < bytes.len() {
            let Some(operator_len) = assignment_operator_len(bytes, mask, index) else {
                index += 1;
                continue;
            };

            let Some(previous) = previous_code_index(bytes, mask, index) else {
                index += operator_len;
                continue;
            };
            let Some(next) = next_code_index(bytes, mask, index + operator_len) else {
                index += operator_len;
                continue;
            };
            let before = count_spaces_before(bytes, previous, index);
            let after = count_spaces_after(bytes, index + operator_len, next);

            if before != 1 || after != 1 {
                let operator =
                    std::str::from_utf8(&bytes[index..index + operator_len]).unwrap_or("=");

                self.push(
                    DiagnosticCode::LINT_FORMAT_TOKEN_SPACING,
                    line_number,
                    index + 1,
                    format!("use one space around `{operator}`"),
                );
            }

            index += operator_len;
        }
    }

    fn push(
        &mut self,
        code: DiagnosticCode,
        line: usize,
        column: usize,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(
            Diagnostic::new(
                &self.parsed.path,
                line,
                column,
                DiagnosticSeverity::Warning,
                message,
            )
            .with_code(code),
        );
    }
}

fn long_bracket_open(bytes: &[u8]) -> Option<(Vec<u8>, usize)> {
    if bytes.first() != Some(&b'[') {
        return None;
    }

    let mut index = 1;

    while bytes.get(index) == Some(&b'=') {
        index += 1;
    }

    if bytes.get(index) != Some(&b'[') {
        return None;
    }

    let equals = index - 1;
    let mut close = Vec::with_capacity(equals + 2);

    close.push(b']');
    close.extend(std::iter::repeat_n(b'=', equals));
    close.push(b']');

    Some((close, index + 1))
}

fn is_code_range(mask: &[bool], start: usize, len: usize) -> bool {
    mask.get(start..start + len)
        .is_some_and(|range| range.iter().all(|is_code| *is_code))
}

fn is_word_boundary(bytes: &[u8], start: usize, len: usize) -> bool {
    let before = start
        .checked_sub(1)
        .and_then(|index| bytes.get(index))
        .copied();
    let after = bytes.get(start + len).copied();

    !before.is_some_and(is_identifier_byte) && !after.is_some_and(is_identifier_byte)
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn previous_code_index(bytes: &[u8], mask: &[bool], before: usize) -> Option<usize> {
    mask.get(..before)?
        .iter()
        .enumerate()
        .rposition(|(index, is_code)| *is_code && !bytes[index].is_ascii_whitespace())
}

fn next_code_index(bytes: &[u8], mask: &[bool], after: usize) -> Option<usize> {
    mask.get(after..)?
        .iter()
        .enumerate()
        .position(|(offset, is_code)| *is_code && !bytes[after + offset].is_ascii_whitespace())
        .map(|offset| after + offset)
}

fn count_spaces_before(bytes: &[u8], previous: usize, current: usize) -> usize {
    bytes[previous + 1..current]
        .iter()
        .filter(|byte| matches!(byte, b' ' | b'\t'))
        .count()
}

fn count_spaces_after(bytes: &[u8], start: usize, next: usize) -> usize {
    bytes[start..next]
        .iter()
        .filter(|byte| matches!(byte, b' ' | b'\t'))
        .count()
}

fn code_byte_positions<'a>(
    bytes: &'a [u8],
    mask: &'a [bool],
    byte: u8,
) -> impl Iterator<Item = usize> + 'a {
    bytes
        .iter()
        .enumerate()
        .filter(move |(index, current)| **current == byte && mask.get(*index) == Some(&true))
        .map(|(index, _)| index)
}

fn assignment_operator_len(bytes: &[u8], mask: &[bool], index: usize) -> Option<usize> {
    if mask.get(index) != Some(&true) {
        return None;
    }

    if matches!(bytes.get(index), Some(b'+' | b'-' | b'*' | b'/' | b'%'))
        && bytes.get(index + 1) == Some(&b'=')
        && mask.get(index + 1) == Some(&true)
    {
        return Some(2);
    }

    if bytes.get(index) != Some(&b'=') {
        return None;
    }

    if matches!(
        bytes.get(index.checked_sub(1)?),
        Some(b'=' | b'<' | b'>' | b'~')
    ) || bytes.get(index + 1) == Some(&b'=')
    {
        return None;
    }

    Some(1)
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

fn token_position(token: &TokenReference) -> SourcePosition {
    SourcePosition::from_full_moon(token.token().start_position())
}

fn token_text(token: &TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

#[cfg(test)]
mod tests;
