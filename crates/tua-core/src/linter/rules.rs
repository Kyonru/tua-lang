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

use super::{duplicate_local_diagnostic, DuplicateLocalKind, DuplicateLocalOrigin};
use crate::ast_utils::field_chain_before_first_call;
use crate::control_flow::{block_fallthrough_position, call_is_terminating_builtin};
use crate::scope::{ScopeGlobalKind, ScopeMap, ScopeResolution};
use crate::semantic::SemanticModel;
use crate::types::{PrimitiveType, Type, TypeEnvironment};
use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity, ParsedSource, SourcePosition};

pub(super) fn lint<'a>(
    parsed: &'a ParsedSource,
    scope: &ScopeMap,
    environment: &'a TypeEnvironment,
) -> Vec<Diagnostic> {
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
    let lua_builtin_references = scope
        .references()
        .iter()
        .filter_map(|reference| {
            matches!(
                &reference.resolution,
                ScopeResolution::Global(global) if global.kind == ScopeGlobalKind::LuaBuiltin
            )
            .then_some((reference.span.start.line, reference.span.start.column))
        })
        .collect();
    let mut visitor = LintRuleVisitor {
        parsed,
        environment,
        semantic: SemanticModel::new(parsed, environment),
        diagnostics: comment_directive_diagnostics(parsed),
        multiline_statements: Vec::new(),
        multiline_table_depth: 0,
        function_names: Vec::new(),
        terminating_builtin_references,
        lua_builtin_references,
    };
    visitor.visit_ast(&parsed.ast);
    visitor.diagnostics
}

struct LintRuleVisitor<'a> {
    parsed: &'a ParsedSource,
    environment: &'a TypeEnvironment,
    semantic: SemanticModel<'a>,
    diagnostics: Vec<Diagnostic>,
    multiline_statements: Vec<MultiLineStatement>,
    multiline_table_depth: usize,
    function_names: Vec<Option<String>>,
    terminating_builtin_references: BTreeSet<(usize, usize)>,
    lua_builtin_references: BTreeSet<(usize, usize)>,
}

struct MultiLineStatement {
    start: SourcePosition,
    last_line: usize,
    flagged: bool,
    ignore_expressions: bool,
}

#[derive(Clone, Copy)]
enum TableIndexerUse {
    Length,
    Ipairs,
}

impl TableIndexerUse {
    fn label(self) -> &'static str {
        match self {
            Self::Length => "#",
            Self::Ipairs => "ipairs",
        }
    }
}

#[derive(Clone, Copy)]
enum TableIndexerIssue {
    NoArrayPart,
    StringKeys,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DictionaryKeyKind {
    Number,
    String,
    Other,
    Unknown,
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
        let mut names = BTreeMap::new();

        for name in local.names() {
            let text = token_text(name);
            if text == "_" {
                continue;
            }
            let position = token_position(name);

            if let Some(original) = names.insert(text.clone(), position) {
                self.diagnostics.push(duplicate_local_diagnostic(
                    &self.parsed.path,
                    position,
                    &text,
                    DuplicateLocalKind::Variable,
                    DuplicateLocalOrigin::Explicit(original),
                ));
            }
        }
    }

    fn visit_function_declaration(&mut self, function: &ast::FunctionDeclaration) {
        self.function_names
            .push(Some(function_name(function.name())));
    }

    fn visit_function_declaration_end(&mut self, _function: &ast::FunctionDeclaration) {
        self.function_names.pop();
    }

    fn visit_local_function(&mut self, function: &ast::LocalFunction) {
        self.function_names.push(Some(token_text(function.name())));
    }

    fn visit_local_function_end(&mut self, _function: &ast::LocalFunction) {
        self.function_names.pop();
    }

    fn visit_anonymous_function(&mut self, _function: &ast::AnonymousFunction) {
        self.function_names.push(None);
    }

    fn visit_anonymous_function_end(&mut self, _function: &ast::AnonymousFunction) {
        self.function_names.pop();
    }

    fn visit_numeric_for(&mut self, numeric_for: &ast::NumericFor) {
        if numeric_for.step().is_some() {
            return;
        }

        let start = numeric_literal(numeric_for.start());
        let end = numeric_literal(numeric_for.end());
        let reason = match (start, end) {
            (Some(start), Some(end)) if start > end => {
                Some("range counts downward but has no negative step".to_owned())
            }
            (None, Some(1.0)) if is_length_expression(numeric_for.start()) => {
                Some("range counts from a length down to 1 but has no negative step".to_owned())
            }
            (None, Some(0.0)) if is_length_expression(numeric_for.start()) => Some(
                "range counts from a length down to 0; use a negative step and consider changing 0 to 1 because arrays start at 1"
                    .to_owned(),
            ),
            (Some(start), Some(end)) => {
                let loop_end = start + (end - start).floor();

                (loop_end != end).then(|| {
                    format!(
                        "range ends at {loop_end} instead of {end}; specify an explicit step"
                    )
                })
            }
            (Some(0.0), None) if is_length_expression(numeric_for.end()) => {
                Some("range from 0 to a length is usually off by one".to_owned())
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
        let Some(value_return) = first_valued_return(body.block()) else {
            return;
        };
        let Some(fallthrough) =
            block_fallthrough_position(body.block(), node_end_position(body), &|statement| {
                statement_termination(statement, &self.terminating_builtin_references).is_some()
            })
        else {
            return;
        };
        let name = self.function_names.last().and_then(Option::as_deref);

        self.push(
            fallthrough,
            DiagnosticCode::LINT_IMPLICIT_RETURN,
            implicit_return_message(name, node_position(value_return).line),
        );
    }

    fn visit_function_call(&mut self, call: &FunctionCall) {
        self.check_literal_format_method(call);

        let Some(parts) = field_chain_before_first_call(call) else {
            return;
        };
        let name = parts.join(".");
        let Some(arguments) = first_call_arguments(call) else {
            return;
        };

        if matches!(call.prefix(), Prefix::Name(root) if self.is_lua_builtin_reference(root)) {
            self.check_format_call(&name, &arguments);
            if name == "ipairs" && arguments.len() == 1 {
                self.check_table_indexer(call, arguments[0], TableIndexerUse::Ipairs);
            }
            self.check_table_operation(&name, &arguments);
        }
    }

    fn visit_expression(&mut self, expression: &Expression) {
        self.check_multiline_expression(expression);

        if let Expression::UnaryOperator {
            unop: ast::UnOp::Hash(_),
            expression: operand,
        } = expression
        {
            self.check_table_indexer(expression, operand, TableIndexerUse::Length);
            self.check_vector_length(expression, operand);
        }

        match expression {
            Expression::BinaryOperator { lhs, binop, rhs } => {
                let misleading_alternative = if matches!(binop, ast::BinOp::Or(_)) {
                    match unparenthesized_expression(lhs) {
                        Expression::BinaryOperator {
                            binop: ast::BinOp::And(_),
                            rhs,
                            ..
                        } => false_or_nil_literal(rhs),
                        _ => None,
                    }
                } else {
                    None
                };

                if let Some(alternative) = misleading_alternative {
                    self.push_node(
                        expression,
                        DiagnosticCode::LINT_MISLEADING_AND_OR,
                        format!(
                            "`and`-`or` expression always selects the fallback because its first alternative is `{alternative}`; use an `if` statement assignment to preserve `{alternative}`"
                        ),
                    );
                }

                if is_comparison_operator(binop) && is_unparenthesized_comparison(lhs) {
                    let Expression::BinaryOperator {
                        binop: left_operator,
                        ..
                    } = unparenthesized_expression(lhs)
                    else {
                        unreachable!("comparison helper must identify a binary expression");
                    };

                    self.push_node(
                        expression,
                        DiagnosticCode::LINT_COMPARISON_PRECEDENCE,
                        chained_comparison_message(left_operator, binop),
                    );
                }

                if is_comparison_operator(binop)
                    && is_unparenthesized_not(lhs)
                    && !is_unparenthesized_not(rhs)
                {
                    self.push_node(
                        expression,
                        DiagnosticCode::LINT_COMPARISON_PRECEDENCE,
                        negated_comparison_message(binop),
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

            if let Some(first) = names.get(&name).copied() {
                self.push(
                    position,
                    DiagnosticCode::LINT_DUPLICATE_FUNCTION,
                    format!(
                        "function `{name}` is defined more than once in this block; first definition is at {}:{}",
                        first.line, first.column
                    ),
                );
            } else {
                names.insert(name, position);
            }
        }
    }

    fn check_literal_format_method(&mut self, call: &FunctionCall) {
        let Prefix::Expression(receiver) = call.prefix() else {
            return;
        };
        let Some(Suffix::Call(Call::MethodCall(method))) = call.suffixes().next() else {
            return;
        };
        if token_text(method.name()) == "format" {
            self.check_printf_expression(receiver);
        }
    }

    fn is_lua_builtin_reference(&self, token: &TokenReference) -> bool {
        let position = token_position(token);
        self.lua_builtin_references
            .contains(&(position.line, position.column))
    }

    fn check_format_call(&mut self, name: &str, arguments: &[&Expression]) {
        match name {
            "string.format" => {
                if let Some(format) = arguments.first() {
                    self.check_printf_expression(format);
                }
            }
            "string.find" if should_validate_find_pattern(arguments) => {
                if let Some(pattern) = arguments.get(1) {
                    self.check_pattern_expression(pattern);
                }
            }
            "string.match" | "string.gmatch" => {
                if let Some(pattern) = arguments.get(1) {
                    self.check_pattern_expression(pattern);
                }
            }
            "string.gsub" => {
                let captures = arguments
                    .get(1)
                    .and_then(|pattern| self.check_pattern_expression(pattern));

                if let Some(replacement) = arguments.get(2) {
                    self.check_replacement_expression(replacement, captures);
                }
            }
            _ => {}
        }
    }

    fn check_printf_expression(&mut self, expression: &Expression) {
        let Some(format) = string_literal(expression) else {
            return;
        };
        if let Some(error) = validate_printf_format(&format) {
            self.push(
                node_position(expression),
                DiagnosticCode::LINT_FORMAT_STRING,
                format!("invalid format string: {error}"),
            );
        }
    }

    fn check_pattern_expression(&mut self, expression: &Expression) -> Option<usize> {
        let pattern = string_literal(expression)?;

        match validate_lua_pattern(&pattern) {
            Ok(captures) => Some(captures),
            Err(error) => {
                self.push(
                    node_position(expression),
                    DiagnosticCode::LINT_FORMAT_STRING,
                    format!("invalid match pattern: {error}"),
                );
                None
            }
        }
    }

    fn check_replacement_expression(&mut self, expression: &Expression, captures: Option<usize>) {
        let Some(replacement) = string_literal(expression) else {
            return;
        };
        if let Some(error) = validate_lua_replacement(&replacement, captures) {
            self.push(
                node_position(expression),
                DiagnosticCode::LINT_FORMAT_STRING,
                format!("invalid match replacement: {error}"),
            );
        }
    }

    fn check_table_operation(&mut self, name: &str, arguments: &[&Expression]) {
        if name == "table.insert" && arguments.len() == 2 {
            if let Expression::FunctionCall(tail) = unparenthesized_expression(arguments[1]) {
                if self
                    .semantic
                    .return_types_of_call(tail, node_position(tail))
                    .len()
                    > 1
                {
                    self.push(
                        node_position(tail),
                        DiagnosticCode::LINT_TABLE_OPERATION,
                        "table.insert may change behavior when the final call returns more than one value; wrap the second argument in parentheses to keep only its first result",
                    );
                }
            }
        }

        let (index, message) = match name {
            "table.insert" if arguments.len() >= 3 => match numeric_literal(arguments[1]) {
                Some(0.0) => (
                    Some(arguments[1]),
                    "table.insert uses index 0 but arrays are 1-based; did you mean 1 instead?",
                ),
                _ if is_length_of(arguments[1], arguments[0]) => (
                    Some(arguments[1]),
                    "table.insert will insert before the last element, which is likely a bug; omit the index to append or wrap it in parentheses to silence",
                ),
                _ if is_length_plus_one_of(arguments[1], arguments[0]) => (
                    Some(arguments[1]),
                    "table.insert will append the value; omit the index for efficiency",
                ),
                _ => (None, ""),
            },
            "table.remove" if arguments.len() >= 2 => match numeric_literal(arguments[1]) {
                Some(0.0) => (
                    Some(arguments[1]),
                    "table.remove uses index 0 but arrays are 1-based; did you mean 1 instead?",
                ),
                _ if is_length_minus_one_of(arguments[1], arguments[0]) => (
                    Some(arguments[1]),
                    "table.remove will remove the value before the last element, which is likely a bug; omit the index to remove the last or wrap it in parentheses to silence",
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

    fn check_table_indexer(
        &mut self,
        node: &impl Node,
        expression: &Expression,
        usage: TableIndexerUse,
    ) {
        let Some(ty) =
            self.semantic
                .type_of_expression_return(expression, node_position(expression), 0)
        else {
            return;
        };
        let Some(issue) = table_indexer_issue(self.environment, &ty, usage) else {
            return;
        };
        let operation = usage.label();
        let message = match issue {
            TableIndexerIssue::NoArrayPart => {
                format!("using '{operation}' on a table without an array part is likely a bug")
            }
            TableIndexerIssue::StringKeys => {
                format!("using '{operation}' on a table with string keys is likely a bug")
            }
        };

        self.push(
            node_position(node),
            DiagnosticCode::LINT_TABLE_OPERATION,
            message,
        );
    }

    /// BrineVector backs `Vec2` with FFI structs only when LuaJIT's JIT is on
    /// at its first `require`. Otherwise vectors are plain tables, and LuaJIT
    /// ignores `__len` on tables, so `#v` is 0 instead of the vector's length.
    fn check_vector_length(&mut self, expression: &Expression, operand: &Expression) {
        let Some(ty) = self
            .semantic
            .type_of_expression_return(operand, node_position(operand), 0)
        else {
            return;
        };
        if self.environment.resolve_aliases(&ty) != crate::vector::vec2_type() {
            return;
        }

        let operand_text = operand.to_string();
        let operand_text = operand_text.trim();
        let message = match crate::vector::length_replacement(operand_text) {
            Some(replacement) => format!(
                "`#{operand_text}` is 0 when LuaJIT's JIT is off, because vectors are then plain tables; use `{replacement}` instead"
            ),
            None => "`#` on a Vec2 is 0 when LuaJIT's JIT is off, because vectors are then plain tables; use `.length` instead".to_owned(),
        };
        self.push_node(expression, DiagnosticCode::LINT_VECTOR_LENGTH, message);
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

    fn push_node(&mut self, node: &impl Node, code: DiagnosticCode, message: impl Into<String>) {
        let start = node_position(node);
        let end = node_end_position(node);
        self.diagnostics.push(
            Diagnostic::new(
                &self.parsed.path,
                start.line,
                start.column,
                DiagnosticSeverity::Warning,
                message,
            )
            .with_code(code)
            .with_range(start.line, start.column, end.line, end.column),
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
                if directive.trim() == "tua generated" {
                    update_long_bracket_state(line, &mut long_bracket_level);
                    continue;
                }
                let directive = directive.split_whitespace().next().unwrap_or("");
                let message = match directive {
                    "" => "empty comment directive".to_owned(),
                    "nolint" => "Luau comment directive `--!nolint` is not supported by Tua; use `-- tua-ignore-file TLxxxx` as the first nonblank line".to_owned(),
                    _ => format!("comment directive `--!{directive}` is not supported by Tua"),
                };
                diagnostics.push(
                    Diagnostic::new(
                        &parsed.path,
                        index + 1,
                        line.len() - trimmed.len() + 1,
                        DiagnosticSeverity::Warning,
                        message,
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

fn should_validate_find_pattern(arguments: &[&Expression]) -> bool {
    match arguments
        .get(3)
        .and_then(|expression| boolean_literal(expression))
    {
        Some(plain) => !plain,
        None => arguments.len() < 4,
    }
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
    call_is_terminating_builtin(call, terminating_builtin_references).then_some(Termination {
        reasons: TerminationReasons::ERRORS,
        silences_trailing_return: true,
    })
}

fn unreachable_message(reasons: TerminationReasons) -> String {
    format!(
        "unreachable code (previous statement always {})",
        reasons.description()
    )
}

fn first_valued_return(block: &ast::Block) -> Option<&ast::Return> {
    for statement in block.stmts() {
        let result = match statement {
            Stmt::Do(statement) => first_valued_return(statement.block()),
            Stmt::If(statement) => first_valued_return(statement.block())
                .or_else(|| {
                    statement.else_if().and_then(|branches| {
                        branches
                            .iter()
                            .find_map(|branch| first_valued_return(branch.block()))
                    })
                })
                .or_else(|| statement.else_block().and_then(first_valued_return)),
            Stmt::GenericFor(statement) => first_valued_return(statement.block()),
            Stmt::NumericFor(statement) => first_valued_return(statement.block()),
            Stmt::Repeat(statement) => first_valued_return(statement.block()),
            Stmt::While(statement) => first_valued_return(statement.block()),
            _ => None,
        };

        if result.is_some() {
            return result;
        }
    }

    match block.last_stmt() {
        Some(LastStmt::Return(statement)) if statement.returns().iter().next().is_some() => {
            Some(statement)
        }
        _ => None,
    }
}

fn implicit_return_message(function_name: Option<&str>, return_line: usize) -> String {
    let subject = function_name
        .map(|name| format!("function `{name}`"))
        .unwrap_or_else(|| "function".to_owned());

    format!(
        "{subject} can implicitly return no values even though there is an explicit return at line {return_line}; add an explicit return to silence"
    )
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

fn false_or_nil_literal(expression: &Expression) -> Option<&'static str> {
    let Expression::Symbol(token) = unparenthesized_expression(expression) else {
        return None;
    };

    match token_text(token).as_str() {
        "false" => Some("false"),
        "nil" => Some("nil"),
        _ => None,
    }
}

fn boolean_literal(expression: &Expression) -> Option<bool> {
    match unparenthesized_expression(expression) {
        Expression::Symbol(token) => match token_text(token).as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        Expression::Parentheses { expression, .. } => boolean_literal(expression),
        _ => None,
    }
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

fn comparison_operator_text(operator: &ast::BinOp) -> &'static str {
    match operator {
        ast::BinOp::GreaterThan(_) => ">",
        ast::BinOp::GreaterThanEqual(_) => ">=",
        ast::BinOp::LessThan(_) => "<",
        ast::BinOp::LessThanEqual(_) => "<=",
        ast::BinOp::TwoEqual(_) => "==",
        ast::BinOp::TildeEqual(_) => "~=",
        _ => unreachable!("comparison text requires a comparison operator"),
    }
}

fn is_equality_operator(operator: &ast::BinOp) -> bool {
    matches!(
        operator,
        ast::BinOp::TwoEqual(_) | ast::BinOp::TildeEqual(_)
    )
}

fn negated_comparison_message(operator: &ast::BinOp) -> String {
    let operator = comparison_operator_text(operator);
    let prefix = format!("`not x {operator} y` is equivalent to `(not x) {operator} y`");

    match operator {
        "==" => format!("{prefix}; consider using `x ~= y`, or add parentheses to silence"),
        "~=" => format!("{prefix}; consider using `x == y`, or add parentheses to silence"),
        _ => format!("{prefix}; add parentheses to silence"),
    }
}

fn chained_comparison_message(left: &ast::BinOp, right: &ast::BinOp) -> String {
    let has_equality = is_equality_operator(left) || is_equality_operator(right);
    let left = comparison_operator_text(left);
    let right = comparison_operator_text(right);
    let prefix = format!("`x {left} y {right} z` is equivalent to `(x {left} y) {right} z`");

    if has_equality {
        format!("{prefix}; add parentheses to silence")
    } else {
        format!("{prefix}; did you mean `x {left} y and y {right} z`?")
    }
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

fn table_indexer_issue(
    environment: &TypeEnvironment,
    ty: &Type,
    usage: TableIndexerUse,
) -> Option<TableIndexerIssue> {
    match environment.resolve_aliases(ty) {
        Type::Any | Type::Union(_) => None,
        Type::Array(_) | Type::Tuple(_) => None,
        Type::Dictionary { key, .. } => match dictionary_key_kind(environment, &key) {
            DictionaryKeyKind::Number | DictionaryKeyKind::Unknown => None,
            DictionaryKeyKind::String => Some(TableIndexerIssue::StringKeys),
            DictionaryKeyKind::Other => Some(TableIndexerIssue::NoArrayPart),
        },
        Type::Table(table) => {
            let has_custom_operation = table.metatable_facts().is_some_and(|facts| match usage {
                TableIndexerUse::Length => facts.len.is_some(),
                TableIndexerUse::Ipairs => facts.ipairs.is_some(),
            });

            if has_custom_operation || table.fields().next().is_none() {
                None
            } else {
                Some(TableIndexerIssue::NoArrayPart)
            }
        }
        Type::Primitive(_) | Type::StringLiteral(_) | Type::Function(_) => None,
        Type::Alias(_) => None,
    }
}

fn dictionary_key_kind(environment: &TypeEnvironment, ty: &Type) -> DictionaryKeyKind {
    match environment.resolve_aliases(ty) {
        Type::Primitive(PrimitiveType::Number) => DictionaryKeyKind::Number,
        Type::Primitive(PrimitiveType::String) | Type::StringLiteral(_) => {
            DictionaryKeyKind::String
        }
        Type::Any | Type::Alias(_) => DictionaryKeyKind::Unknown,
        Type::Union(types) => {
            let mut kinds = types.iter().map(|ty| dictionary_key_kind(environment, ty));
            let Some(first) = kinds.next() else {
                return DictionaryKeyKind::Unknown;
            };

            if first == DictionaryKeyKind::Unknown || kinds.any(|kind| kind != first) {
                DictionaryKeyKind::Unknown
            } else {
                first
            }
        }
        Type::Primitive(_)
        | Type::Function(_)
        | Type::Table(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => DictionaryKeyKind::Other,
    }
}

fn numeric_literal(expression: &Expression) -> Option<f64> {
    match expression {
        Expression::Number(token) => super::parse_lua_number(&number_text(token)?),
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
            if expression_key(ungrouped_expression(expression))
                == expression_key(ungrouped_expression(table))
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

fn ungrouped_expression(expression: &Expression) -> &Expression {
    match expression {
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => ungrouped_expression(expression),
        _ => expression,
    }
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

        let mut width_digits = 0usize;
        while characters.peek().is_some_and(char::is_ascii_digit) && width_digits < 2 {
            characters.next();
            width_digits += 1;
        }

        if characters.next_if_eq(&'.').is_some() {
            let mut precision_digits = 0usize;
            while characters.peek().is_some_and(char::is_ascii_digit) && precision_digits < 2 {
                characters.next();
                precision_digits += 1;
            }
        }
        if characters.peek().is_some_and(char::is_ascii_digit) {
            return Some("format width or precision exceeds two digits".to_owned());
        }

        let Some(specifier) = characters.next() else {
            return Some("unfinished format specifier".to_owned());
        };
        if !"aAcdiouxXeEfgGqsp".contains(specifier) {
            return Some(format!("invalid format specifier `%{specifier}`"));
        }
    }
    None
}

fn validate_lua_pattern(pattern: &str) -> Result<usize, String> {
    const MAGIC: &[u8] = b"^$()%.[]*+-?)";
    const CLASSES: &[u8] = b"acdglpsuwxz";

    let bytes = pattern.as_bytes();
    let mut open_captures = Vec::new();
    let mut total_captures = 0usize;
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                index += 1;
                let Some(&escaped) = bytes.get(index) else {
                    return Err("unfinished character class".to_owned());
                };

                if escaped.is_ascii_digit() {
                    if escaped == b'0' {
                        return Err("capture references must be between `%1` and `%9`".to_owned());
                    }

                    let capture = usize::from(escaped - b'0');
                    if capture > total_captures {
                        return Err(format!("capture reference `%{capture}` does not exist"));
                    }
                    if open_captures.contains(&capture) {
                        return Err(format!(
                            "capture reference `%{capture}` must refer to a closed capture"
                        ));
                    }
                } else if escaped.is_ascii_alphabetic() {
                    match escaped {
                        b'b' => {
                            if index + 2 >= bytes.len() {
                                return Err(
                                    "balanced match `%b` requires two delimiter characters"
                                        .to_owned(),
                                );
                            }
                            index += 2;
                        }
                        b'f' => {
                            if bytes.get(index + 1) != Some(&b'[') {
                                return Err(
                                    "frontier pattern `%f` must be followed by a character class"
                                        .to_owned(),
                                );
                            }
                        }
                        class if !CLASSES.contains(&class.to_ascii_lowercase()) => {
                            return Err(format!(
                                "unknown character class `%{}`",
                                char::from(escaped)
                            ));
                        }
                        _ => {}
                    }
                } else if !MAGIC.contains(&escaped) {
                    return Err(format!(
                        "`%{}` does not escape a Lua pattern magic character",
                        char::from(escaped)
                    ));
                }
            }
            b'[' => {
                let mut end = index + 1;

                if bytes.get(end) == Some(&b'^') {
                    end += 1;
                }
                if bytes.get(end) == Some(&b']') {
                    end += 1;
                }

                while end < bytes.len() && bytes[end] != b']' {
                    if bytes[end] == b'%' && end + 1 < bytes.len() {
                        end += 1;
                    }
                    end += 1;
                }

                if end == bytes.len() {
                    return Err("character class is missing a closing `]`".to_owned());
                }

                validate_lua_pattern_set(&bytes[index + 1..end], MAGIC, CLASSES)?;
                index = end;
            }
            b'(' => {
                total_captures += 1;
                open_captures.push(total_captures);
            }
            b')' => {
                open_captures
                    .pop()
                    .ok_or_else(|| "unexpected `)` without a matching `(`".to_owned())?;
            }
            _ => {}
        }

        index += 1;
    }

    if !open_captures.is_empty() {
        Err("capture is missing a closing `)`".to_owned())
    } else {
        Ok(total_captures)
    }
}

fn validate_lua_pattern_set(set: &[u8], magic: &[u8], classes: &[u8]) -> Result<(), String> {
    let mut index = 0usize;

    while index < set.len() {
        if set[index] == b'%' {
            index += 1;
            let Some(&escaped) = set.get(index) else {
                return Err("unfinished character class".to_owned());
            };

            if escaped.is_ascii_digit() {
                return Err("character classes cannot contain capture references".to_owned());
            }
            if escaped.is_ascii_alphabetic() {
                if !classes.contains(&escaped.to_ascii_lowercase()) {
                    return Err(format!(
                        "unknown character class `%{}`",
                        char::from(escaped)
                    ));
                }
            } else if !magic.contains(&escaped) {
                return Err(format!(
                    "`%{}` does not escape a Lua pattern magic character",
                    char::from(escaped)
                ));
            }

            if set.get(index + 1) == Some(&b'-') {
                return Err("character ranges cannot include character classes".to_owned());
            }
        } else if set[index] == b'-' && set.get(index + 1) == Some(&b'%') {
            return Err("character ranges cannot include character classes".to_owned());
        }

        index += 1;
    }

    Ok(())
}

fn validate_lua_replacement(replacement: &str, captures: Option<usize>) -> Option<String> {
    let bytes = replacement.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }

        index += 1;
        let Some(&escaped) = bytes.get(index) else {
            return Some("unfinished `%` replacement".to_owned());
        };
        if escaped.is_ascii_digit() && escaped != b'0' {
            let capture = usize::from(escaped - b'0');
            if captures.is_some_and(|count| capture > count) {
                return Some(format!("capture reference `%{capture}` does not exist"));
            }
        }

        index += 1;
    }

    None
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
