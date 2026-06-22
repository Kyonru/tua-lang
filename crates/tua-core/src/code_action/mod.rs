use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use full_moon::ast::{
    self, Assignment, Expression, Field, FunctionArgs, FunctionCall, Index, Prefix, Suffix,
    TableConstructor, Var,
};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};

use crate::inlay_hints_parsed_source;
use crate::source_text::line_at;
use crate::{Diagnostic, DiagnosticCode, ParsedSource, SourcePosition, SourceSpan, TextEdit};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeAction {
    pub title: String,
    pub kind: CodeActionKind,
    pub edit: CodeActionEdit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeActionKind {
    QuickFix,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CodeActionEdit {
    pub changes: BTreeMap<PathBuf, Vec<TextEdit>>,
    pub create_files: Vec<CreateFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateFile {
    pub path: PathBuf,
    pub ignore_if_exists: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodeActionRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

impl CodeActionRange {
    pub fn contains(self, position: SourcePosition) -> bool {
        position_key(self.start) <= position_key(position)
            && position_key(position) <= position_key(self.end)
    }
}

pub fn code_actions_parsed_source(
    parsed: &ParsedSource,
    diagnostics: &[Diagnostic],
    range: CodeActionRange,
    source_dir: &Path,
    extension: &str,
) -> Vec<CodeAction> {
    let mut actions = Vec::new();

    for diagnostic in diagnostics {
        let position = SourcePosition {
            line: diagnostic.line,
            column: diagnostic.column,
        };

        if !range.contains(position) {
            continue;
        }

        match diagnostic.code {
            Some(DiagnosticCode::LINT_MISSING_REQUIRE) => {
                if let Some(action) =
                    create_missing_require_action(parsed, diagnostic, source_dir, extension)
                {
                    actions.push(action);
                }
            }
            Some(DiagnosticCode::LINT_UNUSED_LOCAL) => {
                if let Some(action) = remove_unused_local_action(parsed, diagnostic) {
                    actions.push(action);
                }
            }
            Some(DiagnosticCode::LINT_DUPLICATE_TABLE_KEY) => {
                if let Some(action) = rename_duplicate_table_key_action(parsed, diagnostic) {
                    actions.push(action);
                }
            }
            Some(DiagnosticCode::LINT_SELF_ASSIGNMENT) => {
                if let Some(action) = remove_self_assignment_action(parsed, diagnostic) {
                    actions.push(action);
                }
            }
            _ => {}
        }
    }

    if diagnostics.is_empty() {
        actions.extend(add_missing_local_annotation_actions(parsed, range));
    }

    actions.sort_by(|left, right| left.title.cmp(&right.title));
    actions
}

fn create_missing_require_action(
    parsed: &ParsedSource,
    diagnostic: &Diagnostic,
    source_dir: &Path,
    extension: &str,
) -> Option<CodeAction> {
    let module = module_name_from_message(&diagnostic.message)
        .or_else(|| require_module_at(parsed, diagnostic_position(diagnostic)))?;
    let target = source_dir
        .join(module_relative_path(&module)?)
        .with_extension(extension);

    if target.exists() {
        return None;
    }

    let mut edit = CodeActionEdit::default();

    edit.create_files.push(CreateFile {
        path: target.clone(),
        ignore_if_exists: true,
    });
    edit.changes.insert(
        target,
        vec![TextEdit {
            path: PathBuf::new(),
            span: empty_start_span(),
            new_text: "return {}\n".to_owned(),
        }],
    );

    Some(CodeAction {
        title: format!("Create module `{module}`"),
        kind: CodeActionKind::QuickFix,
        edit,
    })
}

fn remove_unused_local_action(
    parsed: &ParsedSource,
    diagnostic: &Diagnostic,
) -> Option<CodeAction> {
    let position = diagnostic_position(diagnostic);

    for stmt in parsed.ast.nodes().stmts() {
        let ast::Stmt::LocalAssignment(local) = stmt else {
            continue;
        };

        if local.names().len() != 1 || !span_contains(node_span(local.names().first()?)?, position)
        {
            continue;
        }

        let edit = remove_single_line_stmt_edit(parsed, stmt)?;

        return Some(CodeAction {
            title: "Remove unused local".to_owned(),
            kind: CodeActionKind::QuickFix,
            edit: edit_for(parsed.path.clone(), edit),
        });
    }

    None
}

fn rename_duplicate_table_key_action(
    parsed: &ParsedSource,
    diagnostic: &Diagnostic,
) -> Option<CodeAction> {
    let position = diagnostic_position(diagnostic);
    let table = table_containing_name_key_at(parsed, position)?;
    let (key_span, key_name) = name_key_at(table, position)?;
    let new_name = unique_key_name(&key_name, table);

    Some(CodeAction {
        title: format!("Rename duplicate key to `{new_name}`"),
        kind: CodeActionKind::QuickFix,
        edit: edit_for(
            parsed.path.clone(),
            TextEdit {
                path: parsed.path.clone(),
                span: key_span,
                new_text: new_name,
            },
        ),
    })
}

fn remove_self_assignment_action(
    parsed: &ParsedSource,
    diagnostic: &Diagnostic,
) -> Option<CodeAction> {
    let position = diagnostic_position(diagnostic);

    for stmt in parsed.ast.nodes().stmts() {
        let ast::Stmt::Assignment(assignment) = stmt else {
            continue;
        };

        if !is_single_self_assignment_at(assignment, position) {
            continue;
        }

        let edit = remove_single_line_stmt_edit(parsed, stmt)?;

        return Some(CodeAction {
            title: "Remove self-assignment".to_owned(),
            kind: CodeActionKind::QuickFix,
            edit: edit_for(parsed.path.clone(), edit),
        });
    }

    None
}

fn add_missing_local_annotation_actions(
    parsed: &ParsedSource,
    range: CodeActionRange,
) -> Vec<CodeAction> {
    inlay_hints_parsed_source(parsed)
        .unwrap_or_default()
        .into_iter()
        .filter(|hint| range.contains(hint.position))
        .filter(|hint| hint.label.starts_with(": "))
        .map(|hint| CodeAction {
            title: format!("Add type annotation `{}`", hint.label),
            kind: CodeActionKind::QuickFix,
            edit: edit_for(
                parsed.path.clone(),
                TextEdit {
                    path: parsed.path.clone(),
                    span: SourceSpan {
                        start: hint.position,
                        end: hint.position,
                    },
                    new_text: hint.label,
                },
            ),
        })
        .collect()
}

fn require_module_at(parsed: &ParsedSource, position: SourcePosition) -> Option<String> {
    for stmt in parsed.ast.nodes().stmts() {
        let Some(module) = require_module_in_stmt(stmt, position) else {
            continue;
        };

        return Some(module);
    }

    None
}

fn require_module_in_stmt(stmt: &ast::Stmt, position: SourcePosition) -> Option<String> {
    match stmt {
        ast::Stmt::FunctionCall(call) => require_module_from_call(call, position),
        ast::Stmt::LocalAssignment(local) => local
            .expressions()
            .iter()
            .find_map(|expression| require_module_from_expression(expression, position)),
        ast::Stmt::Assignment(assignment) => assignment
            .expressions()
            .iter()
            .find_map(|expression| require_module_from_expression(expression, position)),
        _ => None,
    }
}

fn require_module_from_expression(
    expression: &Expression,
    position: SourcePosition,
) -> Option<String> {
    match expression {
        Expression::FunctionCall(call) => require_module_from_call(call, position),
        Expression::Parentheses { expression, .. } => {
            require_module_from_expression(expression, position)
        }
        _ => None,
    }
}

fn require_module_from_call(call: &FunctionCall, position: SourcePosition) -> Option<String> {
    let Prefix::Name(name) = call.prefix() else {
        return None;
    };

    if token_text(name) != "require" {
        return None;
    }

    let Some(Suffix::Call(ast::Call::AnonymousCall(args))) = call.suffixes().next() else {
        return None;
    };

    match args {
        FunctionArgs::Parentheses { arguments, .. } => {
            let Some(Expression::String(token)) = arguments.iter().next() else {
                return None;
            };

            span_contains(node_span(token)?, position)
                .then(|| string_literal_value(token))
                .flatten()
        }
        FunctionArgs::String(token) => span_contains(node_span(token)?, position)
            .then(|| string_literal_value(token))
            .flatten(),
        FunctionArgs::TableConstructor(_) => None,
        _ => None,
    }
}

fn table_containing_name_key_at(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Option<&TableConstructor> {
    parsed
        .ast
        .nodes()
        .stmts()
        .find_map(|stmt| table_containing_name_key_in_stmt(stmt, position))
}

fn table_containing_name_key_in_stmt(
    stmt: &ast::Stmt,
    position: SourcePosition,
) -> Option<&TableConstructor> {
    match stmt {
        ast::Stmt::Assignment(assignment) => assignment
            .expressions()
            .iter()
            .find_map(|expression| table_containing_name_key_in_expression(expression, position)),
        ast::Stmt::LocalAssignment(local) => local
            .expressions()
            .iter()
            .find_map(|expression| table_containing_name_key_in_expression(expression, position)),
        _ => None,
    }
}

fn table_containing_name_key_in_expression(
    expression: &Expression,
    position: SourcePosition,
) -> Option<&TableConstructor> {
    match expression {
        Expression::TableConstructor(table) => {
            if name_key_at(table, position).is_some() {
                return Some(table);
            }

            table.fields().iter().find_map(|field| match field {
                Field::NameKey { value, .. } => {
                    table_containing_name_key_in_expression(value, position)
                }
                Field::ExpressionKey { key, value, .. } => {
                    table_containing_name_key_in_expression(key, position)
                        .or_else(|| table_containing_name_key_in_expression(value, position))
                }
                Field::NoKey(value) => table_containing_name_key_in_expression(value, position),
                _ => None,
            })
        }
        Expression::Parentheses { expression, .. } => {
            table_containing_name_key_in_expression(expression, position)
        }
        _ => None,
    }
}

fn name_key_at(table: &TableConstructor, position: SourcePosition) -> Option<(SourceSpan, String)> {
    table.fields().iter().find_map(|field| {
        let Field::NameKey { key, .. } = field else {
            return None;
        };
        let span = node_span(key)?;

        span_contains(span, position).then(|| (span, token_text(key)))
    })
}

fn unique_key_name(base: &str, table: &TableConstructor) -> String {
    let used = table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NameKey { key, .. } = field else {
                return None;
            };

            Some(token_text(key))
        })
        .collect::<BTreeSet<_>>();

    for index in 2.. {
        let candidate = format!("{base}{index}");

        if !used.contains(&candidate) {
            return candidate;
        }
    }

    unreachable!("finite table keys always leave a numeric suffix available")
}

fn is_single_self_assignment_at(assignment: &Assignment, position: SourcePosition) -> bool {
    if assignment.variables().len() != 1 || assignment.expressions().len() != 1 {
        return false;
    }

    let Some(variable) = assignment.variables().iter().next() else {
        return false;
    };
    let Some(expression) = assignment.expressions().iter().next() else {
        return false;
    };
    let Some(variable_span) = node_span(variable) else {
        return false;
    };

    if !span_contains(variable_span, position) {
        return false;
    }

    plain_field_chain_from_var(variable)
        .zip(plain_field_chain_from_expression(expression))
        .is_some_and(|(left, right)| left == right)
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

fn plain_field_chain_from_var_expression(expression: &ast::VarExpression) -> Option<Vec<String>> {
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

fn remove_single_line_stmt_edit(parsed: &ParsedSource, stmt: &ast::Stmt) -> Option<TextEdit> {
    let span = node_span(stmt)?;

    if span.start.line != span.end.line {
        return None;
    }

    Some(TextEdit {
        path: parsed.path.clone(),
        span: line_removal_span(&parsed.source, span.start.line),
        new_text: String::new(),
    })
}

fn line_removal_span(source: &str, line: usize) -> SourceSpan {
    if line < source.lines().count() || source.ends_with('\n') {
        return SourceSpan {
            start: SourcePosition { line, column: 1 },
            end: SourcePosition {
                line: line + 1,
                column: 1,
            },
        };
    }

    let end_column = line_at(source, line)
        .map(|line| line.len() + 1)
        .unwrap_or(1);

    SourceSpan {
        start: SourcePosition { line, column: 1 },
        end: SourcePosition {
            line,
            column: end_column,
        },
    }
}

fn edit_for(path: PathBuf, edit: TextEdit) -> CodeActionEdit {
    let mut output = CodeActionEdit::default();

    output.changes.insert(path, vec![edit]);
    output
}

fn diagnostic_position(diagnostic: &Diagnostic) -> SourcePosition {
    SourcePosition {
        line: diagnostic.line,
        column: diagnostic.column,
    }
}

fn module_name_from_message(message: &str) -> Option<String> {
    let start = message.find('`')? + 1;
    let end = message[start..].find('`')? + start;

    Some(message[start..end].to_owned())
}

fn module_relative_path(module: &str) -> Option<PathBuf> {
    if module.is_empty()
        || module.starts_with('/')
        || module.starts_with('\\')
        || module
            .split(['.', '/', '\\'])
            .any(|component| component.is_empty())
    {
        return None;
    }

    let normalized = module.replace('.', "/");
    let path = Path::new(&normalized);
    let mut output = PathBuf::new();

    for component in path.components() {
        let Component::Normal(part) = component else {
            return None;
        };

        output.push(part);
    }

    (!output.as_os_str().is_empty()).then_some(output)
}

fn empty_start_span() -> SourceSpan {
    SourceSpan {
        start: SourcePosition { line: 1, column: 1 },
        end: SourcePosition { line: 1, column: 1 },
    }
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn span_contains(span: SourceSpan, position: SourcePosition) -> bool {
    let start = (span.start.line, span.start.column);
    let end = (span.end.line, span.end.column);
    let position = (position.line, position.column);

    start <= position && position < end
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn string_literal_value(token: &TokenReference) -> Option<String> {
    match token.token().token_type() {
        TokenType::StringLiteral { literal, .. } => Some(literal.to_string()),
        _ => None,
    }
}

fn token_text(token: &TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

#[cfg(test)]
mod tests;
