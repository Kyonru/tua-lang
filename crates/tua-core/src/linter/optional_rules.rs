//! Opinionated syntax lints that projects enable explicitly.

use full_moon::ast::{self, Expression, Field, Stmt, Var};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};
use full_moon::visitors::Visitor;

use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity, DiagnosticsConfig, ParsedSource};

pub(super) fn lint(parsed: &ParsedSource, config: &DiagnosticsConfig) -> Vec<Diagnostic> {
    let mut visitor = OptionalRuleVisitor {
        parsed,
        config,
        diagnostics: Vec::new(),
        function_complexities: Vec::new(),
    };
    visitor.visit_ast(&parsed.ast);
    visitor.diagnostics
}

struct FunctionComplexity {
    line: usize,
    column: usize,
    value: usize,
}

struct OptionalRuleVisitor<'a> {
    parsed: &'a ParsedSource,
    config: &'a DiagnosticsConfig,
    diagnostics: Vec<Diagnostic>,
    function_complexities: Vec<FunctionComplexity>,
}

impl Visitor for OptionalRuleVisitor<'_> {
    fn visit_block(&mut self, block: &ast::Block) {
        if !self.config.almost_swapped {
            return;
        }

        let statements = block.stmts().collect::<Vec<_>>();
        for pair in statements.windows(2) {
            let (Stmt::Assignment(first), Stmt::Assignment(second)) = (pair[0], pair[1]) else {
                continue;
            };
            let Some((first_target, first_value)) = simple_name_assignment(first) else {
                continue;
            };
            let Some((second_target, second_value)) = simple_name_assignment(second) else {
                continue;
            };

            if first_target == second_value
                && first_value == second_target
                && first_target != first_value
            {
                self.push_node(
                    second,
                    DiagnosticCode::LINT_ALMOST_SWAPPED,
                    format!(
                        "`{first_target} = {first_value}` followed by `{second_target} = {second_value}` reuses the overwritten value; use `{first_target}, {second_target} = {first_value}, {second_value}` to swap"
                    ),
                );
            }
        }
    }

    fn visit_function_body(&mut self, body: &ast::FunctionBody) {
        if self.config.max_cyclomatic_complexity == 0 {
            return;
        }

        let (line, column) = node_position(body);
        self.function_complexities.push(FunctionComplexity {
            line,
            column,
            value: 1,
        });
    }

    fn visit_function_body_end(&mut self, _body: &ast::FunctionBody) {
        if self.config.max_cyclomatic_complexity == 0 {
            return;
        }

        let complexity = self
            .function_complexities
            .pop()
            .expect("function complexity stack should match visitor nesting");
        if complexity.value > self.config.max_cyclomatic_complexity {
            self.diagnostics.push(
                Diagnostic::new(
                    &self.parsed.path,
                    complexity.line,
                    complexity.column,
                    DiagnosticSeverity::Warning,
                    format!(
                        "function cyclomatic complexity is {}, exceeding the configured maximum of {}",
                        complexity.value, self.config.max_cyclomatic_complexity
                    ),
                )
                .with_code(DiagnosticCode::LINT_HIGH_CYCLOMATIC_COMPLEXITY),
            );
        }
    }

    fn visit_if(&mut self, statement: &ast::If) {
        self.add_complexity(1 + statement.else_if().map_or(0, |branches| branches.len()));
    }

    fn visit_while(&mut self, _statement: &ast::While) {
        self.add_complexity(1);
    }

    fn visit_repeat(&mut self, _statement: &ast::Repeat) {
        self.add_complexity(1);
    }

    fn visit_generic_for(&mut self, _statement: &ast::GenericFor) {
        self.add_complexity(1);
    }

    fn visit_numeric_for(&mut self, _statement: &ast::NumericFor) {
        self.add_complexity(1);
    }

    fn visit_expression(&mut self, expression: &Expression) {
        if let Expression::BinaryOperator { lhs, binop, rhs } = expression {
            if matches!(binop, ast::BinOp::And(_) | ast::BinOp::Or(_)) {
                self.add_complexity(1);
            }

            if self.config.divide_by_zero
                && matches!(binop, ast::BinOp::Slash(_) | ast::BinOp::DoubleSlash(_))
                && numeric_literal(rhs) == Some(0.0)
                && numeric_literal(lhs) != Some(0.0)
            {
                self.push_node(
                    expression,
                    DiagnosticCode::LINT_DIVIDE_BY_ZERO,
                    "division by a literal zero is non-idiomatic and target-dependent; use an explicit infinity or handle zero before dividing",
                );
            }
        }

        if let Expression::IfExpression(if_expression) = expression {
            self.add_complexity(
                1 + if_expression
                    .else_if_expressions()
                    .map_or(0, |branches| branches.len()),
            );
        }
    }

    fn visit_table_constructor(&mut self, table: &ast::TableConstructor) {
        if !self.config.mixed_tables {
            return;
        }

        let has_array_fields = table
            .fields()
            .iter()
            .any(|field| matches!(field, Field::NoKey(_)));
        let has_map_fields = table
            .fields()
            .iter()
            .any(|field| !matches!(field, Field::NoKey(_)));

        if has_array_fields && has_map_fields {
            self.push_node(
                table,
                DiagnosticCode::LINT_MIXED_TABLE,
                "table literal mixes sequential array entries with keyed fields; split the data shapes or suppress this project-specific policy",
            );
        }
    }
}

impl OptionalRuleVisitor<'_> {
    fn add_complexity(&mut self, amount: usize) {
        if let Some(complexity) = self.function_complexities.last_mut() {
            complexity.value = complexity.value.saturating_add(amount);
        }
    }

    fn push_node(&mut self, node: &impl Node, code: DiagnosticCode, message: impl Into<String>) {
        let (line, column) = node_position(node);
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

fn simple_name_assignment(assignment: &ast::Assignment) -> Option<(String, String)> {
    if assignment.variables().len() != 1 || assignment.expressions().len() != 1 {
        return None;
    }

    let Var::Name(target) = assignment.variables().iter().next()? else {
        return None;
    };
    let Expression::Var(Var::Name(value)) =
        unparenthesized(assignment.expressions().iter().next()?)
    else {
        return None;
    };

    Some((token_text(target), token_text(value)))
}

fn unparenthesized(expression: &Expression) -> &Expression {
    match expression {
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => unparenthesized(expression),
        _ => expression,
    }
}

fn numeric_literal(expression: &Expression) -> Option<f64> {
    match unparenthesized(expression) {
        Expression::Number(token) => super::parse_lua_number(&number_text(token)?),
        Expression::UnaryOperator {
            unop: ast::UnOp::Minus(_),
            expression,
        } => numeric_literal(expression).map(|value| -value),
        _ => None,
    }
}

fn number_text(token: &TokenReference) -> Option<String> {
    match token.token().token_type() {
        TokenType::Number { text } => Some(text.to_string()),
        _ => None,
    }
}

fn token_text(token: &TokenReference) -> String {
    token.token().to_string()
}

fn node_position(node: &impl Node) -> (usize, usize) {
    node.start_position()
        .map(|position| (position.line(), position.character()))
        .unwrap_or((1, 1))
}
