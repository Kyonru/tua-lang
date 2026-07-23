use std::collections::BTreeMap;

use full_moon::ast::luau::{
    GenericDeclaration, IndexedTypeInfo, LuauAttribute, TypeField, TypeInfo, TypeInstantiation,
};
use full_moon::ast::{BinOp, Expression, LastStmt, Stmt};
use full_moon::node::Node;
use full_moon::visitors::Visitor;

use super::normalize::native_array_item_end;
use super::{RawParsedSource, SourcePosition, SourceSpan};
use crate::{Diagnostic, DiagnosticCode};

pub(super) fn validate_tua_dialect(raw: &RawParsedSource) -> Vec<Diagnostic> {
    let mut validator = TuaDialectValidator {
        raw,
        diagnostics: Vec::new(),
        native_array_types: raw
            .native_array_types
            .iter()
            .copied()
            .map(|syntax| (position_key(syntax.item_end), syntax.suffix_span))
            .collect(),
    };
    validator.visit_ast(&raw.ast);
    for parser_span in validator.native_array_types.values().copied() {
        validator.diagnostics.push(syntax_diagnostic_for_span(
            raw,
            parser_span,
            "array suffix `[]` is only valid after a type",
            DiagnosticCode::SYNTAX_PARSE,
        ));
    }
    validator.diagnostics
}

struct TuaDialectValidator<'a> {
    raw: &'a RawParsedSource,
    diagnostics: Vec<Diagnostic>,
    native_array_types: BTreeMap<(usize, usize), SourceSpan>,
}

impl TuaDialectValidator<'_> {
    fn report(&mut self, node: &impl Node, message: impl Into<String>) {
        self.diagnostics
            .push(unsupported_syntax_diagnostic(self.raw, node, message));
    }
}

impl Visitor for TuaDialectValidator<'_> {
    fn visit_stmt(&mut self, statement: &Stmt) {
        match statement {
            Stmt::ConstAssignment(assignment) => self.report(
                assignment.const_token(),
                "Luau `const` declarations are not part of Tua; use `local` instead",
            ),
            Stmt::ConstFunction(function) => self.report(
                function.const_token(),
                "Luau `const function` declarations are not part of Tua; use `local function` instead",
            ),
            Stmt::ExportedTypeDeclaration(declaration) => self.report(
                declaration.export_token(),
                "Luau `export type` declarations are not part of Tua; declare a normal Tua type alias instead",
            ),
            Stmt::TypeFunction(function) => self.report(
                function.type_token(),
                "Luau type functions are not part of Tua",
            ),
            Stmt::ExportedTypeFunction(function) => self.report(
                function.export_token(),
                "Luau exported type functions are not part of Tua",
            ),
            _ => {}
        }
    }

    fn visit_last_stmt(&mut self, statement: &LastStmt) {
        let LastStmt::Continue(token) = statement else {
            return;
        };

        self.report(
            token,
            "Luau `continue` is not part of Tua; rewrite the loop with Lua 5.1-compatible control flow",
        );
    }

    fn visit_expression(&mut self, expression: &Expression) {
        match expression {
            Expression::IfExpression(expression) => self.report(
                expression,
                "Luau conditional expressions are not part of Tua; use an `if` statement assignment instead",
            ),
            Expression::InterpolatedString(string) => self.report(
                string,
                "Luau interpolated strings are not part of Tua; use concatenation or `string.format` instead",
            ),
            Expression::TypeAssertion { type_assertion, .. } => self.report(
                type_assertion.assertion_op(),
                "Luau type assertions (`::`) are not part of Tua; use a declaration, parameter, or return type annotation instead",
            ),
            Expression::BinaryOperator {
                binop: BinOp::DoubleSlash(token),
                ..
            } => self.report(
                token,
                "Luau floor division (`//`) is not part of Tua; use `math.floor(left / right)` instead",
            ),
            _ => {}
        }
    }

    fn visit_luau_attribute(&mut self, attribute: &LuauAttribute) {
        self.report(attribute, "Luau function attributes are not part of Tua");
    }

    fn visit_generic_declaration(&mut self, declaration: &GenericDeclaration) {
        self.report(declaration, "generic declarations are not part of Tua");
    }

    fn visit_type_info(&mut self, type_info: &TypeInfo) {
        if let Some(item_end) = native_array_item_end(type_info) {
            self.native_array_types.remove(&position_key(item_end));
        }

        match type_info {
            TypeInfo::Array { access: None, .. } => self.report(
                type_info,
                "Luau `{ T }` array types are not part of Tua; use native `T[]` syntax instead",
            ),
            TypeInfo::Array {
                access: Some(access),
                ..
            } => self.report(
                access,
                "Luau `read`/`write` array access modifiers are not part of Tua",
            ),
            TypeInfo::Generic { .. } => {
                self.report(type_info, "generic type applications are not part of Tua")
            }
            TypeInfo::GenericPack { .. } | TypeInfo::VariadicPack { .. } => {
                self.report(type_info, "Luau generic type packs are not part of Tua")
            }
            TypeInfo::Intersection(_) => {
                self.report(type_info, "Luau intersection types are not part of Tua")
            }
            TypeInfo::Typeof { .. } => {
                self.report(type_info, "Luau `typeof(...)` types are not part of Tua")
            }
            _ => {}
        }
    }

    fn visit_indexed_type_info(&mut self, type_info: &IndexedTypeInfo) {
        if matches!(type_info, IndexedTypeInfo::Generic { .. }) {
            self.report(type_info, "generic type applications are not part of Tua");
        }
    }

    fn visit_type_field(&mut self, field: &TypeField) {
        if let Some(access) = field.access() {
            self.report(
                access,
                "Luau `read`/`write` field access modifiers are not part of Tua",
            );
        }
    }

    fn visit_type_instantiation(&mut self, instantiation: &TypeInstantiation) {
        self.report(
            instantiation,
            "Luau runtime type instantiation is not part of Tua",
        );
    }
}

fn unsupported_syntax_diagnostic(
    raw: &RawParsedSource,
    node: &impl Node,
    message: impl Into<String>,
) -> Diagnostic {
    let parser_span = node
        .range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
        .or(raw.span)
        .unwrap_or(SourceSpan {
            start: SourcePosition { line: 1, column: 1 },
            end: SourcePosition { line: 1, column: 2 },
        });
    syntax_diagnostic_for_span(
        raw,
        parser_span,
        message,
        DiagnosticCode::SYNTAX_UNSUPPORTED_DIALECT,
    )
}

fn syntax_diagnostic_for_span(
    raw: &RawParsedSource,
    parser_span: SourceSpan,
    message: impl Into<String>,
    code: DiagnosticCode,
) -> Diagnostic {
    let start = raw.parser_position_origin(parser_span.start);
    let end = raw.parser_position_origin(parser_span.end);
    let source_start = start
        .map(|origin| origin.source_position)
        .unwrap_or(parser_span.start);
    let source_end = if start.is_some_and(|origin| origin.class_generated)
        || end.is_some_and(|origin| origin.class_generated)
    {
        SourcePosition {
            line: source_start.line,
            column: source_start.column.saturating_add(1),
        }
    } else {
        end.map(|origin| origin.source_position)
            .unwrap_or(parser_span.end)
    };

    Diagnostic::error(
        raw.path.clone(),
        source_start.line,
        source_start.column,
        message,
    )
    .with_range(
        source_start.line,
        source_start.column,
        source_end.line,
        source_end.column,
    )
    .with_code(code)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}
