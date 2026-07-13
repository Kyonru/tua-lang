use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use full_moon::ast::{
    self, Assignment, Expression, Field, FunctionArgs, FunctionCall, Index, Prefix, Suffix,
    TableConstructor,
};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};
use full_moon::visitors::Visitor;

use crate::ast_utils::{plain_field_chain_from_expression, plain_field_chain_from_var};
use crate::inlay_hints_parsed_source;
use crate::source_text::line_at;
use crate::{Diagnostic, DiagnosticCode, ParsedSource, SourcePosition, SourceSpan, TextEdit};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Assist {
    pub title: String,
    pub kind: AssistKind,
    pub edit: AssistEdit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssistKind {
    QuickFix,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AssistEdit {
    pub changes: BTreeMap<PathBuf, Vec<TextEdit>>,
    pub create_files: Vec<CreateFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateFile {
    pub path: PathBuf,
    pub ignore_if_exists: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssistRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

pub type CodeAction = Assist;
pub type CodeActionKind = AssistKind;
pub type CodeActionEdit = AssistEdit;
pub type CodeActionRange = AssistRange;

impl AssistRange {
    pub fn contains(self, position: SourcePosition) -> bool {
        position_key(self.start) <= position_key(position)
            && position_key(position) <= position_key(self.end)
    }
}

pub fn assists_parsed_source(
    parsed: &ParsedSource,
    diagnostics: &[Diagnostic],
    range: AssistRange,
    source_dir: &Path,
    extension: &str,
) -> Vec<Assist> {
    code_actions_parsed_source(parsed, diagnostics, range, source_dir, extension)
}

pub fn code_actions_parsed_source(
    parsed: &ParsedSource,
    diagnostics: &[Diagnostic],
    range: AssistRange,
    source_dir: &Path,
    extension: &str,
) -> Vec<Assist> {
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
            Some(DiagnosticCode::LINT_FORMAT_TRAILING_WHITESPACE) => {
                if let Some(action) = remove_trailing_whitespace_action(parsed, diagnostic) {
                    actions.push(action);
                }
            }
            Some(DiagnosticCode::LINT_DOT_METHOD_CALL) => {
                if let Some(action) = use_colon_method_call_action(parsed, diagnostic) {
                    actions.push(action);
                }
            }
            _ => {}
        }
    }

    actions.extend(love_constructor_helper_actions(parsed, range));

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
) -> Option<Assist> {
    let module = module_name_from_message(&diagnostic.message)
        .or_else(|| require_module_at(parsed, diagnostic_position(diagnostic)))?;
    let target = source_dir
        .join(module_relative_path(&module)?)
        .with_extension(extension);

    if target.exists() {
        return None;
    }

    let mut edit = AssistEdit::default();

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

    Some(Assist {
        title: format!("Create module `{module}`"),
        kind: AssistKind::QuickFix,
        edit,
    })
}

fn remove_unused_local_action(parsed: &ParsedSource, diagnostic: &Diagnostic) -> Option<Assist> {
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

        return Some(Assist {
            title: "Remove unused local".to_owned(),
            kind: AssistKind::QuickFix,
            edit: edit_for(parsed.path.clone(), edit),
        });
    }

    None
}

fn rename_duplicate_table_key_action(
    parsed: &ParsedSource,
    diagnostic: &Diagnostic,
) -> Option<Assist> {
    let position = diagnostic_position(diagnostic);
    let table = table_containing_name_key_at(parsed, position)?;
    let (key_span, key_name) = name_key_at(table, position)?;
    let new_name = unique_key_name(&key_name, table);

    Some(Assist {
        title: format!("Rename duplicate key to `{new_name}`"),
        kind: AssistKind::QuickFix,
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

fn remove_self_assignment_action(parsed: &ParsedSource, diagnostic: &Diagnostic) -> Option<Assist> {
    let position = diagnostic_position(diagnostic);

    for stmt in parsed.ast.nodes().stmts() {
        let ast::Stmt::Assignment(assignment) = stmt else {
            continue;
        };

        if !is_single_self_assignment_at(assignment, position) {
            continue;
        }

        let edit = remove_single_line_stmt_edit(parsed, stmt)?;

        return Some(Assist {
            title: "Remove self-assignment".to_owned(),
            kind: AssistKind::QuickFix,
            edit: edit_for(parsed.path.clone(), edit),
        });
    }

    None
}

fn remove_trailing_whitespace_action(
    parsed: &ParsedSource,
    diagnostic: &Diagnostic,
) -> Option<Assist> {
    let line = line_at(&parsed.source, diagnostic.line)?;
    let trimmed = line.trim_end_matches([' ', '\t']);

    if trimmed.len() == line.len() {
        return None;
    }

    Some(Assist {
        title: "Remove trailing whitespace".to_owned(),
        kind: AssistKind::QuickFix,
        edit: edit_for(
            parsed.path.clone(),
            TextEdit {
                path: parsed.path.clone(),
                span: SourceSpan {
                    start: SourcePosition {
                        line: diagnostic.line,
                        column: trimmed.len() + 1,
                    },
                    end: SourcePosition {
                        line: diagnostic.line,
                        column: line.len() + 1,
                    },
                },
                new_text: String::new(),
            },
        ),
    })
}

fn use_colon_method_call_action(parsed: &ParsedSource, diagnostic: &Diagnostic) -> Option<Assist> {
    let method_start = diagnostic_position(diagnostic);
    let line = line_at(&parsed.source, method_start.line)?;
    let method_start_byte = crate::source_text::byte_for_column(line, method_start.column);
    let dot_byte = method_start_byte.checked_sub(1)?;

    if line.as_bytes().get(dot_byte) != Some(&b'.') {
        return None;
    }

    Some(Assist {
        title: "Use colon method call".to_owned(),
        kind: AssistKind::QuickFix,
        edit: edit_for(
            parsed.path.clone(),
            TextEdit {
                path: parsed.path.clone(),
                span: SourceSpan {
                    start: SourcePosition {
                        line: method_start.line,
                        column: method_start.column.saturating_sub(1),
                    },
                    end: method_start,
                },
                new_text: ":".to_owned(),
            },
        ),
    })
}

fn love_constructor_helper_actions(parsed: &ParsedSource, range: AssistRange) -> Vec<Assist> {
    let mut visitor = LoveConstructorAssistVisitor {
        parsed,
        range,
        actions: Vec::new(),
    };

    visitor.visit_ast(&parsed.ast);
    visitor.actions
}

struct LoveConstructorAssistVisitor<'a> {
    parsed: &'a ParsedSource,
    range: AssistRange,
    actions: Vec<Assist>,
}

impl Visitor for LoveConstructorAssistVisitor<'_> {
    fn visit_function_call(&mut self, function_call: &FunctionCall) {
        let Some(action) = love_constructor_helper_action(self.parsed, function_call) else {
            return;
        };
        let Some(edit) = action
            .edit
            .changes
            .get(&self.parsed.path)
            .and_then(|edits| edits.first())
        else {
            return;
        };

        if assist_range_intersects_span(self.range, edit.span) {
            self.actions.push(action);
        }
    }
}

fn love_constructor_helper_action(
    parsed: &ParsedSource,
    function_call: &FunctionCall,
) -> Option<Assist> {
    let span = node_span(function_call)?;
    let parts = plain_field_chain_from_function_call(function_call)?;
    let helper = love_constructor_helper_rewrite(parsed, function_call, &parts)?;

    Some(Assist {
        title: format!("Rewrite as `{}` helper", helper.name),
        kind: AssistKind::QuickFix,
        edit: edit_for(
            parsed.path.clone(),
            TextEdit {
                path: parsed.path.clone(),
                span,
                new_text: helper.replacement,
            },
        ),
    })
}

struct LoveConstructorRewrite {
    name: &'static str,
    replacement: String,
}

fn love_constructor_helper_rewrite(
    parsed: &ParsedSource,
    function_call: &FunctionCall,
    parts: &[String],
) -> Option<LoveConstructorRewrite> {
    let call = anonymous_call(function_call)?;

    match parts {
        [love, graphics, new_image]
            if love == "love" && graphics == "graphics" && new_image == "newImage" =>
        {
            let arguments = call_arguments(call);
            let [path] = arguments.as_slice() else {
                return None;
            };
            let path = string_argument_source(parsed, path)?;

            Some(LoveConstructorRewrite {
                name: "image",
                replacement: format!("image {path}"),
            })
        }
        [love, audio, new_source]
            if love == "love" && audio == "audio" && new_source == "newSource" =>
        {
            let arguments = call_arguments(call);
            let [path, mode] = arguments.as_slice() else {
                return None;
            };
            let path_source = string_argument_source(parsed, path)?;
            let mode_value = string_argument_value(mode)?;
            let name = match mode_value.as_str() {
                "static" => "sound",
                "stream" => "music",
                _ => return None,
            };

            Some(LoveConstructorRewrite {
                name,
                replacement: format!("{name} {path_source}"),
            })
        }
        _ => None,
    }
}

fn anonymous_call(function_call: &FunctionCall) -> Option<&ast::Call> {
    function_call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })
}

fn call_arguments(call: &ast::Call) -> Vec<&Expression> {
    match call {
        ast::Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            arguments.iter().collect()
        }
        _ => Vec::new(),
    }
}

fn string_argument_source(parsed: &ParsedSource, expression: &Expression) -> Option<String> {
    let Expression::String(token) = expression else {
        return None;
    };

    source_text_for_span(&parsed.source, node_span(token)?).map(str::to_owned)
}

fn string_argument_value(expression: &Expression) -> Option<String> {
    let Expression::String(token) = expression else {
        return None;
    };

    string_literal_value(token)
}

fn plain_field_chain_from_function_call(function_call: &FunctionCall) -> Option<Vec<String>> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in function_call.suffixes() {
        match suffix {
            Suffix::Index(Index::Dot { name, .. }) => parts.push(token_text(name)),
            Suffix::Call(_) => break,
            _ => return None,
        }
    }

    Some(parts)
}

fn add_missing_local_annotation_actions(parsed: &ParsedSource, range: AssistRange) -> Vec<Assist> {
    inlay_hints_parsed_source(parsed)
        .unwrap_or_default()
        .into_iter()
        .filter(|hint| range.contains(hint.position))
        .filter(|hint| hint.label.starts_with(": "))
        .map(|hint| Assist {
            title: format!("Add type annotation `{}`", hint.label),
            kind: AssistKind::QuickFix,
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

fn edit_for(path: PathBuf, edit: TextEdit) -> AssistEdit {
    let mut output = AssistEdit::default();

    output.changes.insert(path, vec![edit]);
    output
}

fn source_text_for_span(source: &str, span: SourceSpan) -> Option<&str> {
    if span.start.line != span.end.line {
        return None;
    }

    let line = line_at(source, span.start.line)?;
    let start = crate::source_text::byte_for_column(line, span.start.column);
    let end = crate::source_text::byte_for_column(line, span.end.column);

    line.get(start..end)
}

fn assist_range_intersects_span(range: AssistRange, span: SourceSpan) -> bool {
    range.contains(span.start)
        || range.contains(span.end)
        || span_contains(
            span,
            SourcePosition {
                line: range.start.line,
                column: range.start.column,
            },
        )
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
