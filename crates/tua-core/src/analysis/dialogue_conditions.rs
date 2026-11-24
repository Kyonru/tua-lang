//! Conditions share imported context facts with placeholder bindings.
use super::*;
use crate::dialogue_asset::{Condition, ConditionKind as K, DialogueLiteral as L};
use crate::types::PrimitiveType as P;

fn category(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::Primitive(P::String) | Type::StringLiteral(_) => Some("string"),
        Type::Primitive(P::Number) => Some("number"),
        Type::Primitive(P::Boolean) => Some("boolean"),
        Type::Primitive(P::Nil) => Some("nil"),
        Type::Readonly(inner) => category(inner),
        Type::Union(types) => {
            let kind = category(types.first()?)?;
            types
                .iter()
                .all(|ty| category(ty) == Some(kind))
                .then_some(kind)
        }
        _ => None,
    }
}

impl AnalysisSnapshot {
    fn condition_type(&self, path: &Path, value: &Condition, errors: &mut Vec<Diagnostic>) -> Type {
        let mut report = |message: String| {
            errors.push(
                Diagnostic::error(
                    path,
                    value.span.start.line,
                    value.span.start.column,
                    message,
                )
                .with_code(crate::DiagnosticCode::DOMAIN_DIALOGUE)
                .with_range(
                    value.span.start.line,
                    value.span.start.column,
                    value.span.end.line,
                    value.span.end.column,
                ),
            )
        };
        match &value.kind {
            K::Nil => Type::nil(),
            K::Value(L::String(_)) => Type::string(),
            K::Value(L::Number(_)) => Type::number(),
            K::Value(L::Boolean(_)) => Type::boolean(),
            K::Path(name) => match self.context_path(path, name) {
                Ok((ty, _)) => ty,
                Err(message) => {
                    report(message);
                    Type::Any
                }
            },
            K::Not(inner) => {
                let ty = self.condition_type(path, inner, errors);
                if category(&ty) != Some("boolean") && category(&ty) != Some("nil") {
                    errors.push(condition_error(
                        path,
                        value,
                        "not requires a boolean or nil",
                    ));
                }
                Type::boolean()
            }
            K::Binary(op, left, right) => {
                let left = self.condition_type(path, left, errors);
                let right = self.condition_type(path, right, errors);
                let (a, b) = (category(&left), category(&right));
                let valid = match op.as_str() {
                    "and" | "or" => {
                        matches!(a, Some("boolean" | "nil")) && matches!(b, Some("boolean" | "nil"))
                    }
                    "==" | "!=" | "~=" => {
                        a == Some("nil") || b == Some("nil") || (a.is_some() && a == b)
                    }
                    _ => a == Some("number") && b == Some("number"),
                };
                if !valid {
                    errors.push(condition_error(
                        path,
                        value,
                        &format!("invalid condition operands for `{op}`: {left} and {right}"),
                    ));
                }
                Type::boolean()
            }
        }
    }
    pub(super) fn dialogue_condition_diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        let Some(model) = self
            .file(path)
            .and_then(|f| f.dialogue_model.get())
            .and_then(Option::as_ref)
        else {
            return Vec::new();
        };
        let mut errors = Vec::new();
        for section in &model.sections {
            let conditions = section.branch.iter().map(|b| &b.condition).chain(
                section
                    .lines
                    .iter()
                    .flat_map(|line| &line.choices)
                    .flat_map(|choice| choice.visible.iter().chain(choice.enabled.iter())),
            );
            for condition in conditions {
                let ty = self.condition_type(path, condition, &mut errors);
                if !matches!(category(&ty), Some("boolean" | "nil")) {
                    errors.push(condition_error(
                        path,
                        condition,
                        "condition must produce a boolean or nil",
                    ));
                }
            }
        }
        errors
    }
}
fn condition_error(path: &Path, value: &Condition, message: &str) -> Diagnostic {
    Diagnostic::error(
        path,
        value.span.start.line,
        value.span.start.column,
        message,
    )
    .with_code(crate::DiagnosticCode::DOMAIN_DIALOGUE)
    .with_range(
        value.span.start.line,
        value.span.start.column,
        value.span.end.line,
        value.span.end.column,
    )
}
