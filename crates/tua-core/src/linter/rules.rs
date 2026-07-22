//! Syntax-oriented rules in Tua's standard lint set.
//!
//! Symbol rules live in the parent module so all Tua lint configurations share one
//! scope implementation. Rules that require a host runtime catalog are not
//! guessed here.

use full_moon::ast::{
    self, Call, Expression, FunctionArgs, FunctionCall, LastStmt, Prefix, Stmt, Suffix,
};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};
use full_moon::visitors::Visitor;
use std::collections::{BTreeMap, BTreeSet};

use crate::ast_utils::field_chain_before_first_call;
use crate::scope::{ScopeGlobalKind, ScopeMap, ScopeResolution};
use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity, ParsedSource, SourcePosition};

pub(super) fn lint(parsed: &ParsedSource, scope: &ScopeMap) -> Vec<Diagnostic> {
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
    let mut visitor = LintRuleVisitor {
        parsed,
        diagnostics: comment_directive_diagnostics(parsed),
        multiline_statements: Vec::new(),
        multiline_table_depth: 0,
        terminating_builtin_references,
    };
    visitor.visit_ast(&parsed.ast);
    visitor.diagnostics
}

struct LintRuleVisitor<'a> {
    parsed: &'a ParsedSource,
    diagnostics: Vec<Diagnostic>,
    multiline_statements: Vec<MultiLineStatement>,
    multiline_table_depth: usize,
    terminating_builtin_references: BTreeSet<(usize, usize)>,
}

struct MultiLineStatement {
    start: SourcePosition,
    last_line: usize,
    flagged: bool,
    ignore_expressions: bool,
}

impl Visitor for LintRuleVisitor<'_> {
    fn visit_block(&mut self, block: &ast::Block) {
        self.check_statement_layout(block);
        self.check_unreachable_code(block);
        self.check_duplicate_functions(block);
    }

    fn visit_stmt(&mut self, statement: &Stmt) {
        self.begin_multiline_statement(statement, matches!(statement, Stmt::Repeat(_)));
    }

    fn visit_stmt_end(&mut self, _statement: &Stmt) {
        self.multiline_statements.pop();
    }

    fn visit_last_stmt(&mut self, statement: &LastStmt) {
        self.begin_multiline_statement(statement, false);
    }

    fn visit_last_stmt_end(&mut self, _statement: &LastStmt) {
        self.multiline_statements.pop();
    }

    fn visit_table_constructor(&mut self, _table: &ast::TableConstructor) {
        self.multiline_table_depth += 1;
    }

    fn visit_table_constructor_end(&mut self, _table: &ast::TableConstructor) {
        self.multiline_table_depth = self.multiline_table_depth.saturating_sub(1);
    }

    fn visit_local_assignment(&mut self, local: &ast::LocalAssignment) {
        let mut names = BTreeSet::new();

        for name in local.names() {
            let text = token_text(name);
            if !names.insert(text.clone()) {
                self.push(
                    token_position(name),
                    DiagnosticCode::LINT_DUPLICATE_LOCAL,
                    format!("duplicate local `{text}` in the same declaration"),
                );
            }
        }
    }

    fn visit_numeric_for(&mut self, numeric_for: &ast::NumericFor) {
        if numeric_for.step().is_some() {
            return;
        }

        let start = numeric_literal(numeric_for.start());
        let end = numeric_literal(numeric_for.end());
        let reason = match (start, end) {
            (Some(start), Some(end)) if start > end => {
                Some("range counts downward but has no negative step")
            }
            (None, Some(1.0)) if is_length_expression(numeric_for.start()) => {
                Some("range counts from a length down to 1 but has no negative step")
            }
            (_, Some(end)) if end.fract() != 0.0 => {
                Some("range has a fractional limit but no explicit step")
            }
            (Some(0.0), None) if is_length_expression(numeric_for.end()) => {
                Some("range from 0 to a length is usually off by one")
            }
            _ => None,
        };

        if let Some(reason) = reason {
            self.push(
                node_position(numeric_for),
                DiagnosticCode::LINT_FOR_RANGE,
                reason,
            );
        }
    }

    fn visit_function_body(&mut self, body: &ast::FunctionBody) {
        if block_has_valued_return(body.block())
            && block_termination(body.block(), &self.terminating_builtin_references).is_none()
        {
            self.push(
                node_end_position(body),
                DiagnosticCode::LINT_IMPLICIT_RETURN,
                "function can implicitly return no values after returning values on another path",
            );
        }
    }

    fn visit_function_call(&mut self, call: &FunctionCall) {
        let Some(parts) = field_chain_before_first_call(call) else {
            return;
        };
        let name = parts.join(".");
        let Some(arguments) = first_call_arguments(call) else {
            return;
        };

        self.check_format_call(&name, &arguments);
        self.check_table_operation(&name, &arguments);
    }

    fn visit_expression(&mut self, expression: &Expression) {
        self.check_multiline_expression(expression);

        match expression {
            Expression::BinaryOperator { lhs, binop, rhs } => {
                if matches!(binop, ast::BinOp::Or(_))
                    && matches!(
                        unparenthesized_expression(lhs),
                        Expression::BinaryOperator {
                            binop: ast::BinOp::And(_),
                            rhs,
                            ..
                        } if is_false_or_nil(rhs)
                    )
                {
                    self.push(
                        node_position(binop),
                        DiagnosticCode::LINT_MISLEADING_AND_OR,
                        "`a and false/nil or b` does not preserve the false/nil value",
                    );
                }

                if is_comparison_operator(binop)
                    && (is_unparenthesized_comparison(lhs) || is_unparenthesized_comparison(rhs))
                {
                    self.push(
                        node_position(binop),
                        DiagnosticCode::LINT_COMPARISON_PRECEDENCE,
                        "chained comparisons do not compare all operands; add parentheses",
                    );
                }

                if is_comparison_operator(binop) && is_unparenthesized_not(lhs) {
                    self.push(
                        node_position(lhs),
                        DiagnosticCode::LINT_COMPARISON_PRECEDENCE,
                        "`not x == y` compares `(not x)` with `y`; add parentheses to make the intent explicit",
                    );
                }
            }
            Expression::Number(token) if integer_loses_precision(token) => self.push(
                token_position(token),
                DiagnosticCode::LINT_INTEGER_PRECISION,
                "integer literal cannot be represented exactly by a Lua number",
            ),
            _ => {}
        }
    }
}

impl LintRuleVisitor<'_> {
    fn check_statement_layout(&mut self, block: &ast::Block) {
        let statements = block.stmts_with_semicolon().collect::<Vec<_>>();

        for pair in statements.windows(2) {
            let (previous, semicolon) = pair[0];
            let (current, _) = pair[1];
            let Some((_, previous_end)) = previous.range() else {
                continue;
            };
            let Some((current_start, _)) = current.range() else {
                continue;
            };

            if previous_end.line() == current_start.line() && semicolon.is_none() {
                self.push(
                    SourcePosition::from_full_moon(current_start),
                    DiagnosticCode::LINT_SAME_LINE_STATEMENT,
                    "multiple statements on one line should be separated by a semicolon or newline",
                );
            }
        }

        if let (Some((previous, semicolon)), Some(last)) =
            (statements.last().copied(), block.last_stmt())
        {
            if let (Some((_, previous_end)), Some((last_start, _))) =
                (previous.range(), last.range())
            {
                if previous_end.line() == last_start.line() && semicolon.is_none() {
                    self.push(
                        SourcePosition::from_full_moon(last_start),
                        DiagnosticCode::LINT_SAME_LINE_STATEMENT,
                        "multiple statements on one line should be separated by a semicolon or newline",
                    );
                }
            }
        }
    }

    fn begin_multiline_statement(&mut self, statement: &impl Node, ignore_expressions: bool) {
        let start = statement
            .range()
            .map(|(start, _)| SourcePosition::from_full_moon(start))
            .unwrap_or(SourcePosition { line: 1, column: 1 });
        self.multiline_statements.push(MultiLineStatement {
            start,
            last_line: start.line,
            flagged: false,
            ignore_expressions,
        });
    }

    fn check_multiline_expression(&mut self, expression: &Expression) {
        if self.multiline_table_depth > 0 || matches!(expression, Expression::TableConstructor(_)) {
            return;
        }

        let Some((start, _)) = expression.range() else {
            return;
        };
        let position = SourcePosition::from_full_moon(start);
        let should_warn = self
            .multiline_statements
            .last_mut()
            .is_some_and(|statement| {
                if statement.flagged
                    || statement.ignore_expressions
                    || position.line <= statement.last_line
                {
                    return false;
                }

                statement.last_line = position.line;
                statement.flagged = position.column <= statement.start.column;
                statement.flagged
            });

        if should_warn {
            self.push(
                position,
                DiagnosticCode::LINT_MULTI_LINE_STATEMENT,
                "continuation line is not indented relative to the statement",
            );
        }
    }

    fn check_unreachable_code(&mut self, block: &ast::Block) {
        let mut termination: Option<Termination> = None;

        for statement in block.stmts() {
            if let Some(termination) = termination {
                self.push(
                    node_position(statement),
                    DiagnosticCode::LINT_UNREACHABLE_CODE,
                    unreachable_message(termination.reasons),
                );
                return;
            }

            termination = statement_termination(statement, &self.terminating_builtin_references);
        }

        let (Some(termination), Some(last)) = (termination, block.last_stmt()) else {
            return;
        };
        if termination.silences_trailing_return && matches!(last, LastStmt::Return(_)) {
            return;
        }

        self.push(
            node_position(last),
            DiagnosticCode::LINT_UNREACHABLE_CODE,
            unreachable_message(termination.reasons),
        );
    }

    fn check_duplicate_functions(&mut self, block: &ast::Block) {
        let mut names = BTreeMap::<String, SourcePosition>::new();

        for statement in block.stmts() {
            let (name, position) = match statement {
                Stmt::LocalFunction(function) => {
                    (token_text(function.name()), token_position(function.name()))
                }
                Stmt::FunctionDeclaration(function) => (
                    function_name(function.name()),
                    node_position(function.name()),
                ),
                _ => continue,
            };

            if let Some(first) = names.insert(name.clone(), position) {
                self.push(
                    position,
                    DiagnosticCode::LINT_DUPLICATE_FUNCTION,
                    format!(
                        "function `{name}` is defined more than once in this block; first definition is at {}:{}",
                        first.line, first.column
                    ),
                );
            }
        }
    }

    fn check_format_call(&mut self, name: &str, arguments: &[&Expression]) {
        let format_index = if name == "string.format" {
            Some(0)
        } else if matches!(
            name,
            "string.find" | "string.match" | "string.gmatch" | "string.gsub"
        ) {
            Some(1)
        } else if name == "os.date" {
            Some(0)
        } else {
            None
        };
        let Some(format_index) = format_index else {
            return;
        };
        let Some(expression) = arguments.get(format_index).copied() else {
            return;
        };
        let Some(format) = string_literal(expression) else {
            return;
        };

        let error = if name == "string.format" {
            validate_printf_format(&format)
        } else if name == "os.date" {
            validate_date_format(&format)
        } else {
            validate_lua_pattern(&format)
        };

        if let Some(error) = error {
            self.push(
                node_position(expression),
                DiagnosticCode::LINT_FORMAT_STRING,
                error,
            );
        }
    }

    fn check_table_operation(&mut self, name: &str, arguments: &[&Expression]) {
        let (index, message) = match name {
            "table.insert" if arguments.len() >= 3 => match numeric_literal(arguments[1]) {
                Some(0.0) => (Some(arguments[1]), "table.insert uses invalid index 0"),
                _ if is_length_of(arguments[1], arguments[0]) => (
                    Some(arguments[1]),
                    "table.insert at #t inserts before the last element; omit the index to append",
                ),
                _ if is_length_plus_one_of(arguments[1], arguments[0]) => (
                    Some(arguments[1]),
                    "table.insert at #t + 1 is redundant; omit the index to append",
                ),
                _ => (None, ""),
            },
            "table.remove" if arguments.len() >= 2 => match numeric_literal(arguments[1]) {
                Some(0.0) => (Some(arguments[1]), "table.remove uses invalid index 0"),
                _ if is_length_minus_one_of(arguments[1], arguments[0]) => (
                    Some(arguments[1]),
                    "table.remove at #t - 1 removes the penultimate element; omit the index to remove the last",
                ),
                _ => (None, ""),
            },
            _ => (None, ""),
        };

        if let Some(index) = index {
            self.push(
                node_position(index),
                DiagnosticCode::LINT_TABLE_OPERATION,
                message,
            );
        }
    }

    fn push(&mut self, position: SourcePosition, code: DiagnosticCode, message: impl Into<String>) {
        self.diagnostics.push(
            Diagnostic::new(
                &self.parsed.path,
                position.line,
                position.column,
                DiagnosticSeverity::Warning,
                message,
            )
            .with_code(code),
        );
    }
}

fn comment_directive_diagnostics(parsed: &ParsedSource) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut long_bracket_level = None;

    for (index, line) in parsed.source.lines().enumerate() {
        let trimmed = line.trim_start();
        if long_bracket_level.is_none() {
            if let Some(directive) = trimmed.strip_prefix("--!") {
                let directive = directive.split_whitespace().next().unwrap_or("");
                diagnostics.push(
                    Diagnostic::new(
                        &parsed.path,
                        index + 1,
                        line.len() - trimmed.len() + 1,
                        DiagnosticSeverity::Warning,
                        if directive.is_empty() {
                            "empty comment directive".to_owned()
                        } else {
                            format!("comment directive `--!{directive}` is not supported by Tua")
                        },
                    )
                    .with_code(DiagnosticCode::LINT_COMMENT_DIRECTIVE),
                );
            }
        }

        update_long_bracket_state(line, &mut long_bracket_level);
    }

    diagnostics
}

fn update_long_bracket_state(line: &str, active_level: &mut Option<usize>) {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut quote = None;
    let mut escaped = false;

    while index < bytes.len() {
        if let Some(level) = *active_level {
            if long_bracket_delimiter(bytes, index, b']') == Some(level) {
                *active_level = None;
                index += level + 2;
            } else {
                index += 1;
            }
            continue;
        }

        let character = bytes[index];
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == b'\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }

        if matches!(character, b'\'' | b'"') {
            quote = Some(character);
            index += 1;
            continue;
        }

        if bytes.get(index..index + 2) == Some(b"--") {
            if let Some(level) = long_bracket_delimiter(bytes, index + 2, b'[') {
                *active_level = Some(level);
                index += level + 4;
                continue;
            }
            return;
        }

        if let Some(level) = long_bracket_delimiter(bytes, index, b'[') {
            *active_level = Some(level);
            index += level + 2;
            continue;
        }

        index += 1;
    }
}

fn long_bracket_delimiter(bytes: &[u8], index: usize, bracket: u8) -> Option<usize> {
    if bytes.get(index).copied()? != bracket {
        return None;
    }

    let mut cursor = index + 1;
    while bytes.get(cursor) == Some(&b'=') {
        cursor += 1;
    }

    (bytes.get(cursor) == Some(&bracket)).then_some(cursor - index - 1)
}

fn first_call_arguments(call: &FunctionCall) -> Option<Vec<&Expression>> {
    let suffix = call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;
    let args = match suffix {
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            arguments.iter().collect()
        }
        Call::MethodCall(method) => match method.args() {
            FunctionArgs::Parentheses { arguments, .. } => arguments.iter().collect(),
            _ => return None,
        },
        _ => return None,
    };
    Some(args)
}

#[derive(Clone, Copy)]
struct Termination {
    reasons: TerminationReasons,
    silences_trailing_return: bool,
}

#[derive(Clone, Copy)]
struct TerminationReasons {
    returns: bool,
    breaks: bool,
    errors: bool,
}

impl TerminationReasons {
    const RETURNS: Self = Self {
        returns: true,
        breaks: false,
        errors: false,
    };
    const BREAKS: Self = Self {
        returns: false,
        breaks: true,
        errors: false,
    };
    const ERRORS: Self = Self {
        returns: false,
        breaks: false,
        errors: true,
    };

    fn merge(self, other: Self) -> Self {
        Self {
            returns: self.returns || other.returns,
            breaks: self.breaks || other.breaks,
            errors: self.errors || other.errors,
        }
    }

    fn description(self) -> &'static str {
        match (self.returns, self.breaks, self.errors) {
            (true, false, false) => "returns",
            (false, true, false) => "breaks",
            (false, false, true) => "errors",
            (true, true, false) => "returns or breaks",
            (true, false, true) => "returns or errors",
            (false, true, true) => "breaks or errors",
            (true, true, true) => "returns, breaks, or errors",
            (false, false, false) => "terminates",
        }
    }
}

fn statement_termination(
    statement: &Stmt,
    terminating_builtin_references: &BTreeSet<(usize, usize)>,
) -> Option<Termination> {
    match statement {
        Stmt::Do(block) => {
            block_termination(block.block(), terminating_builtin_references).map(|termination| {
                Termination {
                    reasons: termination.reasons,
                    silences_trailing_return: false,
                }
            })
        }
        Stmt::If(if_statement) => {
            let mut reasons =
                block_termination(if_statement.block(), terminating_builtin_references)?.reasons;

            if let Some(branches) = if_statement.else_if() {
                for branch in branches {
                    reasons = reasons.merge(
                        block_termination(branch.block(), terminating_builtin_references)?.reasons,
                    );
                }
            }

            reasons = reasons.merge(
                block_termination(if_statement.else_block()?, terminating_builtin_references)?
                    .reasons,
            );

            Some(Termination {
                reasons,
                silences_trailing_return: false,
            })
        }
        Stmt::FunctionCall(call) => terminating_builtin_call(call, terminating_builtin_references),
        _ => None,
    }
}

fn block_termination(
    block: &ast::Block,
    terminating_builtin_references: &BTreeSet<(usize, usize)>,
) -> Option<Termination> {
    for statement in block.stmts() {
        if let Some(termination) = statement_termination(statement, terminating_builtin_references)
        {
            return Some(termination);
        }
    }

    match block.last_stmt() {
        Some(LastStmt::Return(_)) => Some(Termination {
            reasons: TerminationReasons::RETURNS,
            silences_trailing_return: false,
        }),
        Some(LastStmt::Break(_)) => Some(Termination {
            reasons: TerminationReasons::BREAKS,
            silences_trailing_return: false,
        }),
        _ => None,
    }
}

fn terminating_builtin_call(
    call: &FunctionCall,
    terminating_builtin_references: &BTreeSet<(usize, usize)>,
) -> Option<Termination> {
    let Prefix::Name(name) = call.prefix() else {
        return None;
    };
    let position = token_position(name);
    if !terminating_builtin_references.contains(&(position.line, position.column)) {
        return None;
    }

    let Some(Suffix::Call(Call::AnonymousCall(arguments))) = call.suffixes().next() else {
        return None;
    };
    let terminates = match token_text(name).as_str() {
        "error" => true,
        "assert" => match arguments {
            FunctionArgs::Parentheses { arguments, .. } => {
                arguments.iter().next().is_some_and(is_false_literal)
            }
            _ => false,
        },
        _ => false,
    };

    terminates.then_some(Termination {
        reasons: TerminationReasons::ERRORS,
        silences_trailing_return: true,
    })
}

fn is_false_literal(expression: &Expression) -> bool {
    match expression {
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => is_false_literal(expression),
        Expression::Symbol(token) => token_text(token) == "false",
        _ => false,
    }
}

fn unreachable_message(reasons: TerminationReasons) -> String {
    format!(
        "unreachable code (previous statement always {})",
        reasons.description()
    )
}

fn block_has_valued_return(block: &ast::Block) -> bool {
    if let Some(LastStmt::Return(return_statement)) = block.last_stmt() {
        if return_statement.returns().iter().next().is_some() {
            return true;
        }
    }

    block.stmts().any(|statement| match statement {
        Stmt::Do(block) => block_has_valued_return(block.block()),
        Stmt::If(if_statement) => {
            block_has_valued_return(if_statement.block())
                || if_statement.else_if().is_some_and(|branches| {
                    branches
                        .iter()
                        .any(|branch| block_has_valued_return(branch.block()))
                })
                || if_statement
                    .else_block()
                    .is_some_and(block_has_valued_return)
        }
        Stmt::GenericFor(statement) => block_has_valued_return(statement.block()),
        Stmt::NumericFor(statement) => block_has_valued_return(statement.block()),
        Stmt::Repeat(statement) => block_has_valued_return(statement.block()),
        Stmt::While(statement) => block_has_valued_return(statement.block()),
        _ => false,
    })
}

fn function_name(name: &ast::FunctionName) -> String {
    let mut parts = name.names().iter().map(token_text).collect::<Vec<_>>();
    if let Some(method) = name.method_name() {
        parts.push(token_text(method));
    }
    parts.join(".")
}

fn unparenthesized_expression(expression: &Expression) -> &Expression {
    match expression {
        Expression::TypeAssertion { expression, .. } => unparenthesized_expression(expression),
        _ => expression,
    }
}

fn is_false_or_nil(expression: &Expression) -> bool {
    matches!(
        unparenthesized_expression(expression),
        Expression::Symbol(token) if matches!(token_text(token).as_str(), "false" | "nil")
    )
}

fn is_comparison_operator(operator: &ast::BinOp) -> bool {
    matches!(
        operator,
        ast::BinOp::GreaterThan(_)
            | ast::BinOp::GreaterThanEqual(_)
            | ast::BinOp::LessThan(_)
            | ast::BinOp::LessThanEqual(_)
            | ast::BinOp::TwoEqual(_)
            | ast::BinOp::TildeEqual(_)
    )
}

fn is_unparenthesized_comparison(expression: &Expression) -> bool {
    matches!(
        unparenthesized_expression(expression),
        Expression::BinaryOperator { binop, .. } if is_comparison_operator(binop)
    )
}

fn is_unparenthesized_not(expression: &Expression) -> bool {
    matches!(
        unparenthesized_expression(expression),
        Expression::UnaryOperator {
            unop: ast::UnOp::Not(_),
            ..
        }
    )
}

fn is_length_expression(expression: &Expression) -> bool {
    matches!(
        unparenthesized_expression(expression),
        Expression::UnaryOperator {
            unop: ast::UnOp::Hash(_),
            ..
        }
    )
}

fn numeric_literal(expression: &Expression) -> Option<f64> {
    match expression {
        Expression::Number(token) => number_text(token)?.replace('_', "").parse().ok(),
        Expression::UnaryOperator {
            unop: ast::UnOp::Minus(_),
            expression,
        } => numeric_literal(expression).map(|value| -value),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => numeric_literal(expression),
        _ => None,
    }
}

fn is_length_of(expression: &Expression, table: &Expression) -> bool {
    matches!(
        unparenthesized_expression(expression),
        Expression::UnaryOperator { unop: ast::UnOp::Hash(_), expression }
            if expression_key(expression) == expression_key(table)
    )
}

fn is_length_plus_one_of(expression: &Expression, table: &Expression) -> bool {
    matches!(
        unparenthesized_expression(expression),
        Expression::BinaryOperator { lhs, binop: ast::BinOp::Plus(_), rhs }
            if is_length_of(lhs, table) && numeric_literal(rhs) == Some(1.0)
    )
}

fn is_length_minus_one_of(expression: &Expression, table: &Expression) -> bool {
    matches!(
        unparenthesized_expression(expression),
        Expression::BinaryOperator { lhs, binop: ast::BinOp::Minus(_), rhs }
            if is_length_of(lhs, table) && numeric_literal(rhs) == Some(1.0)
    )
}

fn expression_key(expression: &Expression) -> String {
    expression
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn string_literal(expression: &Expression) -> Option<String> {
    match expression {
        Expression::String(token) => match token.token().token_type() {
            TokenType::StringLiteral { literal, .. } => Some(literal.to_string()),
            _ => None,
        },
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => string_literal(expression),
        _ => None,
    }
}

fn validate_printf_format(format: &str) -> Option<String> {
    let mut characters = format.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '%' {
            continue;
        }
        if characters.next_if_eq(&'%').is_some() {
            continue;
        }

        while characters
            .peek()
            .is_some_and(|character| "-+ #0".contains(*character))
        {
            characters.next();
        }
        while characters.peek().is_some_and(char::is_ascii_digit) {
            characters.next();
        }
        if characters.next_if_eq(&'.').is_some() {
            while characters.peek().is_some_and(char::is_ascii_digit) {
                characters.next();
            }
        }

        let Some(specifier) = characters.next() else {
            return Some("unterminated `%` in format string".to_owned());
        };
        if !"cdiouxXeEfgGqs".contains(specifier) {
            return Some(format!("invalid format specifier `%{specifier}`"));
        }
    }
    None
}

fn validate_date_format(format: &str) -> Option<String> {
    let mut characters = format.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '%' {
            continue;
        }
        let Some(specifier) = characters.next() else {
            return Some("unterminated `%` in date format".to_owned());
        };
        if !"%aAbBcCdHIjmMpSUwWxXyYzZ".contains(specifier) {
            return Some(format!("invalid date format specifier `%{specifier}`"));
        }
    }
    None
}

fn validate_lua_pattern(pattern: &str) -> Option<String> {
    let mut escaped = false;
    let mut bracket_depth = 0usize;
    for character in pattern.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '%' => escaped = true,
            '[' => bracket_depth += 1,
            ']' if bracket_depth == 0 => return Some("unmatched `]` in Lua pattern".to_owned()),
            ']' => bracket_depth -= 1,
            _ => {}
        }
    }
    if escaped {
        Some("unterminated `%` escape in Lua pattern".to_owned())
    } else if bracket_depth != 0 {
        Some("unterminated character class in Lua pattern".to_owned())
    } else {
        None
    }
}

fn integer_loses_precision(token: &TokenReference) -> bool {
    let Some(text) = number_text(token) else {
        return false;
    };
    let text = text.replace('_', "");
    if text.contains(['.', 'e', 'E', 'p', 'P']) {
        return false;
    }
    let parsed = if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        u128::from_str_radix(hex, 16).ok()
    } else {
        text.parse::<u128>().ok()
    };
    parsed.is_some_and(|value| value > 9_007_199_254_740_992)
}

fn number_text(token: &TokenReference) -> Option<String> {
    match token.token().token_type() {
        TokenType::Number { text } => Some(text.to_string()),
        _ => None,
    }
}

fn node_position(node: &impl Node) -> SourcePosition {
    node.range()
        .map(|(start, _)| SourcePosition::from_full_moon(start))
        .unwrap_or(SourcePosition { line: 1, column: 1 })
}

fn node_end_position(node: &impl Node) -> SourcePosition {
    node.range()
        .map(|(_, end)| SourcePosition::from_full_moon(end))
        .unwrap_or(SourcePosition { line: 1, column: 1 })
}

fn token_position(token: &TokenReference) -> SourcePosition {
    SourcePosition::from_full_moon(token.token().start_position())
}

fn token_text(token: &TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}
