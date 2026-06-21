use std::collections::BTreeMap;
use std::path::Path;

use full_moon::ast::{
    Call, Expression, Field, FunctionArgs, FunctionCall, Index, Prefix, Suffix, TableConstructor,
    Var,
};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};
use full_moon::visitors::Visitor;

use crate::config::DiagnosticsConfig;
use crate::module_index::resolve_required_module;
use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity, ParsedSource, SourcePosition};

pub(crate) fn lint_parsed_source(
    parsed: &ParsedSource,
    config: &DiagnosticsConfig,
) -> Vec<Diagnostic> {
    if !config.duplicate_keys && !config.missing_requires && !config.self_assignments {
        return Vec::new();
    }

    let mut visitor = LintVisitor {
        parsed,
        config,
        diagnostics: Vec::new(),
    };

    visitor.visit_ast(&parsed.ast);

    visitor.diagnostics
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

fn node_position(node: &impl Node) -> SourcePosition {
    node.range()
        .map(|(start, _)| SourcePosition::from_full_moon(start))
        .unwrap_or(SourcePosition { line: 1, column: 1 })
}

fn token_position(token: &TokenReference) -> SourcePosition {
    SourcePosition::from_full_moon(token.token().start_position())
}

fn token_text(token: &TokenReference) -> String {
    token.to_string().trim().to_owned()
}

#[cfg(test)]
mod tests;
