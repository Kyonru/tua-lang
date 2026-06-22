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
        && !symbol_lints_enabled(config)
    {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();

    if config.duplicate_keys || config.missing_requires || config.self_assignments {
        let mut visitor = LintVisitor {
            parsed,
            config,
            diagnostics: Vec::new(),
        };

        visitor.visit_ast(&parsed.ast);
        diagnostics.extend(visitor.diagnostics);
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

struct LintVisitor<'a> {
    parsed: &'a ParsedSource,
    config: &'a DiagnosticsConfig,
    diagnostics: Vec<Diagnostic>,
}

impl Visitor for LintVisitor<'_> {
    fn visit_assignment(&mut self, assignment: &full_moon::ast::Assignment) {
        if !self.config.self_assignments {
            return;
        }

        for (variable, expression) in assignment.variables().iter().zip(assignment.expressions()) {
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

    fn visit_function_call(&mut self, function_call: &FunctionCall) {
        if !self.config.missing_requires {
            return;
        }

        let Some((module, position)) = require_module_literal(function_call) else {
            return;
        };

        if resolve_required_module(&self.parsed.path, &module).is_none() {
            self.diagnostics.push(missing_require_diagnostic(
                &self.parsed.path,
                position,
                &module,
            ));
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
