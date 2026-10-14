use std::path::Path;

use full_moon::ast::{Expression, Field};
use full_moon::node::Node;

use crate::types::{AssignabilityFailure, AssignabilityFailureKind, AssignabilityPathSegment};
use crate::{Diagnostic, DiagnosticCode, SourcePosition};

use super::{
    expression_is_number, mismatch_found_label, node_position, node_span,
    string_literal_expression, token_text,
};

pub(super) fn mismatch_diagnostic(
    path: &Path,
    node: &impl Node,
    failure: &AssignabilityFailure,
    root: Option<&str>,
) -> Diagnostic {
    let position = node_position(node);
    let message = mismatch_message(failure, root);

    let mut diagnostic = Diagnostic::error(path, position.line, position.column, message)
        .with_code(DiagnosticCode::TYPE_MISMATCH);

    if let Some(span) = node_span(node) {
        diagnostic = diagnostic.with_range(
            span.start.line,
            span.start.column,
            span.end.line,
            span.end.column,
        );
    }

    diagnostic
}

pub(super) fn mismatch_diagnostic_for_expression(
    path: &Path,
    expression: &Expression,
    failure: &AssignabilityFailure,
    root: Option<&str>,
) -> Diagnostic {
    let expression = expression_at_assignability_path(expression, &failure.path);
    mismatch_diagnostic(path, expression, failure, root)
}

pub(super) fn mismatch_diagnostic_at_position(
    path: &Path,
    position: SourcePosition,
    failure: &AssignabilityFailure,
) -> Diagnostic {
    Diagnostic::error(
        path,
        position.line,
        position.column,
        mismatch_message(failure, None),
    )
    .with_range(
        position.line,
        position.column,
        position.line,
        position.column.saturating_add(1),
    )
    .with_code(DiagnosticCode::TYPE_MISMATCH)
}

fn mismatch_message(failure: &AssignabilityFailure, root: Option<&str>) -> String {
    let at = assignability_path_label(root, &failure.path)
        .map(|path| format!(" at {path}"))
        .unwrap_or_default();

    match failure.kind {
        AssignabilityFailureKind::TypeMismatch => {
            let found = mismatch_found_label(&failure.expected, &failure.found);
            format!(
                "type mismatch{at}: expected {}, found {found}",
                failure.expected
            )
        }
        AssignabilityFailureKind::MissingField => format!(
            "type mismatch{at}: required field is missing; add a value of type {}",
            failure.expected
        ),
        AssignabilityFailureKind::OptionalField => {
            format!("type mismatch{at}: field is optional but must be required")
        }
        AssignabilityFailureKind::FunctionParameterCount { expected, found } => format!(
            "type mismatch{at}: callback requires {found} parameters, but it may be called with only {expected}"
        ),
        AssignabilityFailureKind::FunctionVararg => {
            format!("type mismatch{at}: callback does not accept the required variadic arguments")
        }
        AssignabilityFailureKind::FunctionReturnCount { expected, found } => format!(
            "type mismatch{at}: callback must return {expected} values, but it returns {found}"
        ),
        AssignabilityFailureKind::TupleLength { expected, found } => {
            format!("type mismatch{at}: expected {expected} tuple values, found {found}")
        }
    }
}

fn assignability_path_label(
    root: Option<&str>,
    path: &[AssignabilityPathSegment],
) -> Option<String> {
    let mut label = root.unwrap_or_default().to_owned();

    for segment in path {
        match segment {
            AssignabilityPathSegment::Field(name) => append_field_path(&mut label, name),
            AssignabilityPathSegment::ArrayElement => {
                if label.is_empty() {
                    label.push_str("array element");
                } else {
                    label.push_str("[]");
                }
            }
            AssignabilityPathSegment::TupleElement(index) => {
                if label.is_empty() {
                    label.push_str("tuple");
                }
                label.push_str(&format!("[{}]", index + 1));
            }
            AssignabilityPathSegment::DictionaryKey(name) => {
                if let Some(name) = name {
                    append_field_path(&mut label, name);
                    label.push_str(" key");
                } else if label.is_empty() {
                    label.push_str("dictionary key");
                } else {
                    label.push_str(" dictionary key");
                }
            }
            AssignabilityPathSegment::DictionaryValue => {
                if label.is_empty() {
                    label.push_str("dictionary value");
                } else {
                    label.push_str(" dictionary value");
                }
            }
            AssignabilityPathSegment::FunctionParameter { index, name } => {
                if !label.is_empty() {
                    label.push(' ');
                }
                if let Some(name) = name {
                    label.push_str(&format!("parameter `{name}`"));
                } else {
                    label.push_str(&format!("parameter {}", index + 1));
                }
            }
            AssignabilityPathSegment::FunctionReturn(index) => {
                if !label.is_empty() {
                    label.push(' ');
                }
                if *index == 0 {
                    label.push_str("return value");
                } else {
                    label.push_str(&format!("return value {}", index + 1));
                }
            }
        }
    }

    (!label.is_empty()).then_some(label)
}

fn append_field_path(label: &mut String, name: &str) {
    let identifier = name.chars().enumerate().all(|(index, character)| {
        character == '_'
            || character.is_ascii_alphanumeric() && (index > 0 || !character.is_ascii_digit())
    });

    if label.is_empty() {
        if identifier {
            label.push_str(name);
        } else {
            label.push_str(&format!("[{name:?}]"));
        }
    } else if identifier {
        label.push('.');
        label.push_str(name);
    } else {
        label.push_str(&format!("[{name:?}]"));
    }
}

fn expression_at_assignability_path<'a>(
    expression: &'a Expression,
    path: &[AssignabilityPathSegment],
) -> &'a Expression {
    let mut current = unwrap_expression(expression);

    for segment in path {
        let next = match segment {
            AssignabilityPathSegment::Field(name) => {
                table_field_value(current, name).map(unwrap_expression)
            }
            AssignabilityPathSegment::TupleElement(index) => {
                table_array_value(current, *index).map(unwrap_expression)
            }
            AssignabilityPathSegment::ArrayElement
            | AssignabilityPathSegment::DictionaryKey(_)
            | AssignabilityPathSegment::DictionaryValue
            | AssignabilityPathSegment::FunctionParameter { .. }
            | AssignabilityPathSegment::FunctionReturn(_) => None,
        };

        let Some(next) = next else {
            break;
        };
        current = next;
    }

    current
}

fn unwrap_expression(expression: &Expression) -> &Expression {
    match expression {
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => unwrap_expression(expression),
        _ => expression,
    }
}

fn table_field_value<'a>(expression: &'a Expression, name: &str) -> Option<&'a Expression> {
    let Expression::TableConstructor(table) = expression else {
        return None;
    };

    table.fields().iter().find_map(|field| match field {
        Field::NameKey { key, value, .. } if token_text(key) == name => Some(value),
        Field::ExpressionKey { key, value, .. }
            if string_literal_expression(key).as_deref() == Some(name) =>
        {
            Some(value)
        }
        _ => None,
    })
}

fn table_array_value(expression: &Expression, index: usize) -> Option<&Expression> {
    let Expression::TableConstructor(table) = expression else {
        return None;
    };

    table
        .fields()
        .iter()
        .filter_map(|field| match field {
            Field::NoKey(value) => Some(value),
            Field::ExpressionKey { key, value, .. } if expression_is_number(key) => Some(value),
            _ => None,
        })
        .nth(index)
}
