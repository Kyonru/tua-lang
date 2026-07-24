use crate::classes::{class_lint_facts, ClassLintFact};
use crate::{
    Diagnostic, DiagnosticCode, DiagnosticSeverity, ParsedSource, SourceSpan, TypeEnvironment,
};

pub(super) fn lint(parsed: &ParsedSource, environment: &TypeEnvironment) -> Vec<Diagnostic> {
    class_lint_facts(parsed, environment)
        .into_iter()
        .map(|fact| match fact {
            ClassLintFact::ParentConstructorPath {
                class_name,
                parent_name,
                span,
            } => class_lint_diagnostic(
                parsed,
                span,
                DiagnosticCode::LINT_CLASS_PARENT_CONSTRUCTOR,
                format!(
                    "constructor for class `{class_name}` can return without calling parent constructor `{parent_name}`; call `super(...)` on every returning path"
                ),
            ),
            ClassLintFact::VirtualCallInInit {
                class_name,
                method_name,
                span,
            } => class_lint_diagnostic(
                parsed,
                span,
                DiagnosticCode::LINT_CLASS_VIRTUAL_CALL_IN_INIT,
                format!(
                    "overridable method `{method_name}` is called during `{class_name}.init`; a subclass override can observe incomplete state, so mark the method `final` or move the call out of `init`"
                ),
            ),
            ClassLintFact::RedundantInheritedField {
                class_name,
                parent_name,
                field_name,
                span,
            } => class_lint_diagnostic(
                parsed,
                span,
                DiagnosticCode::LINT_CLASS_REDUNDANT_INHERITED_FIELD,
                format!(
                    "field `{field_name}` in class `{class_name}` redundantly redeclares the inherited field from class `{parent_name}`; remove this declaration"
                ),
            ),
        })
        .collect()
}

fn class_lint_diagnostic(
    parsed: &ParsedSource,
    span: SourceSpan,
    code: DiagnosticCode,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::new(
        parsed.path.clone(),
        span.start.line,
        span.start.column,
        DiagnosticSeverity::Warning,
        message,
    )
    .with_range(
        span.start.line,
        span.start.column,
        span.end.line,
        span.end.column,
    )
    .with_code(code)
}
