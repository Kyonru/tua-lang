use std::collections::BTreeMap;
use std::path::Path;

use full_moon::ast::{
    self, Expression, Field, FunctionBody, Index, LastStmt, Parameter, Suffix, Var,
};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};

use crate::definition_parsed_source;
use crate::parser::{ParsedSource, SourcePosition, SourceSpan, TuaType};
use crate::source_text::{byte_for_column, line_at};
use crate::{parse_tua_source, DefinitionKind, Diagnostic};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticToken {
    pub span: SourceSpan,
    pub kind: SemanticTokenKind,
    pub modifiers: Vec<SemanticTokenModifier>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticTokenKind {
    TypeAlias,
    Function,
    Variable,
    Parameter,
    Property,
    BuiltinType,
    UnresolvedReference,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticTokenModifier {
    Declaration,
}

pub fn semantic_tokens_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<Vec<SemanticToken>, Vec<Diagnostic>> {
    let parsed = parse_tua_source(path, source)?;

    semantic_tokens_parsed_source(&parsed)
}

pub fn semantic_tokens_parsed_source(
    parsed: &ParsedSource,
) -> Result<Vec<SemanticToken>, Vec<Diagnostic>> {
    let declarations = DeclarationIndex::new(parsed);
    let mut tokens = parsed
        .ast
        .tokens()
        .filter(|token| is_identifier(token))
        .filter_map(|token| semantic_token_for_identifier(parsed, &declarations, token))
        .collect::<Vec<_>>();

    tokens.sort_by_key(|token| position_key(token.span.start));
    tokens.dedup_by_key(|token| {
        (
            token.span.start.line,
            token.span.start.column,
            token.span.end.line,
            token.span.end.column,
            token.kind,
        )
    });

    Ok(tokens)
}

fn semantic_token_for_identifier(
    parsed: &ParsedSource,
    declarations: &DeclarationIndex,
    token: &TokenReference,
) -> Option<SemanticToken> {
    let span = node_span(token)?;
    let text = token_text(token);

    if let Some(classification) = declarations.by_span.get(&span_key(span)) {
        return Some(classification.to_token(span));
    }

    if is_primitive_type(&text) && is_type_reference(&parsed.source, span.start) {
        return Some(SemanticToken {
            span,
            kind: SemanticTokenKind::BuiltinType,
            modifiers: Vec::new(),
        });
    }

    if let Ok(Some(definition)) = definition_parsed_source(parsed, span.start) {
        return Some(SemanticToken {
            span,
            kind: semantic_kind_for_definition(definition.kind),
            modifiers: Vec::new(),
        });
    }

    if let Some(kind) = declarations.nearest_value_kind(&text, span.start) {
        return Some(SemanticToken {
            span,
            kind,
            modifiers: Vec::new(),
        });
    }

    (!is_known_global(&text)).then_some(SemanticToken {
        span,
        kind: SemanticTokenKind::UnresolvedReference,
        modifiers: Vec::new(),
    })
}

fn semantic_kind_for_definition(kind: DefinitionKind) -> SemanticTokenKind {
    match kind {
        DefinitionKind::TypeAlias => SemanticTokenKind::TypeAlias,
        DefinitionKind::Local => SemanticTokenKind::Variable,
        DefinitionKind::Function => SemanticTokenKind::Function,
        DefinitionKind::Parameter => SemanticTokenKind::Parameter,
        DefinitionKind::TableField => SemanticTokenKind::Property,
    }
}

#[derive(Default)]
struct DeclarationIndex {
    by_span: BTreeMap<SpanKey, Classification>,
    values: BTreeMap<String, Vec<ValueDeclaration>>,
}

impl DeclarationIndex {
    fn new(parsed: &ParsedSource) -> Self {
        let mut index = Self::default();

        for alias in &parsed.type_aliases {
            if let Some(span) = alias.name_span.or(alias.span) {
                index.insert_declaration(span, &alias.name, SemanticTokenKind::TypeAlias);
            }

            collect_type_table_fields(&alias.ty, &mut index);
        }

        collect_block_declarations(parsed.ast.nodes(), &mut index);

        index
    }

    fn insert_declaration(&mut self, span: SourceSpan, name: &str, kind: SemanticTokenKind) {
        self.by_span.insert(
            span_key(span),
            Classification {
                kind,
                declaration: true,
            },
        );

        if matches!(
            kind,
            SemanticTokenKind::Function
                | SemanticTokenKind::Variable
                | SemanticTokenKind::Parameter
        ) {
            self.values
                .entry(name.to_owned())
                .or_default()
                .push(ValueDeclaration { span, kind });
        }
    }

    fn insert_property(&mut self, span: SourceSpan) {
        self.by_span
            .entry(span_key(span))
            .or_insert(Classification {
                kind: SemanticTokenKind::Property,
                declaration: false,
            });
    }

    fn nearest_value_kind(
        &self,
        name: &str,
        position: SourcePosition,
    ) -> Option<SemanticTokenKind> {
        self.values
            .get(name)?
            .iter()
            .filter(|declaration| position_key(declaration.span.start) <= position_key(position))
            .max_by_key(|declaration| position_key(declaration.span.start))
            .map(|declaration| declaration.kind)
    }
}

#[derive(Clone, Copy)]
struct Classification {
    kind: SemanticTokenKind,
    declaration: bool,
}

impl Classification {
    fn to_token(self, span: SourceSpan) -> SemanticToken {
        let modifiers = self
            .declaration
            .then_some(SemanticTokenModifier::Declaration)
            .into_iter()
            .collect();

        SemanticToken {
            span,
            kind: self.kind,
            modifiers,
        }
    }
}

struct ValueDeclaration {
    span: SourceSpan,
    kind: SemanticTokenKind,
}

fn collect_type_table_fields(ty: &TuaType, index: &mut DeclarationIndex) {
    match ty {
        TuaType::Table(fields) => {
            for field in fields {
                if let Some(span) = field.name_span.or(field.span) {
                    index.insert_declaration(span, &field.name, SemanticTokenKind::Property);
                }

                collect_type_table_fields(&field.ty, index);
            }
        }
        TuaType::Union(types) => {
            for ty in types {
                collect_type_table_fields(ty, index);
            }
        }
        TuaType::Named(_) => {}
    }
}

fn collect_block_declarations(block: &ast::Block, index: &mut DeclarationIndex) {
    for stmt in block.stmts() {
        collect_stmt_declarations(stmt, index);
    }

    if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
        for expression in return_stmt.returns() {
            collect_expression_properties(expression, index);
        }
    }
}

fn collect_stmt_declarations(stmt: &ast::Stmt, index: &mut DeclarationIndex) {
    match stmt {
        ast::Stmt::Assignment(assignment) => {
            for expression in assignment.expressions() {
                collect_expression_properties(expression, index);
            }
        }
        ast::Stmt::Do(block) => collect_block_declarations(block.block(), index),
        ast::Stmt::FunctionCall(function_call) => {
            for suffix in function_call.suffixes() {
                if let Suffix::Call(_) = suffix {
                    break;
                }
            }
        }
        ast::Stmt::FunctionDeclaration(function) => {
            if let Some(span) = node_span(function.name()) {
                index.insert_declaration(
                    span,
                    &function.name().to_string(),
                    SemanticTokenKind::Function,
                );
            }

            collect_function_body_declarations(function.body(), index);
        }
        ast::Stmt::GenericFor(for_stmt) => collect_block_declarations(for_stmt.block(), index),
        ast::Stmt::If(if_stmt) => {
            collect_block_declarations(if_stmt.block(), index);

            if let Some(else_ifs) = if_stmt.else_if() {
                for else_if in else_ifs {
                    collect_block_declarations(else_if.block(), index);
                }
            }

            if let Some(else_block) = if_stmt.else_block() {
                collect_block_declarations(else_block, index);
            }
        }
        ast::Stmt::LocalAssignment(local) => {
            for name in local.names() {
                if let Some(span) = node_span(name) {
                    index.insert_declaration(span, &token_text(name), SemanticTokenKind::Variable);
                }
            }

            for expression in local.expressions() {
                collect_expression_properties(expression, index);
            }
        }
        ast::Stmt::LocalFunction(function) => {
            if let Some(span) = node_span(function.name()) {
                index.insert_declaration(
                    span,
                    &token_text(function.name()),
                    SemanticTokenKind::Function,
                );
            }

            collect_function_body_declarations(function.body(), index);
        }
        ast::Stmt::NumericFor(for_stmt) => collect_block_declarations(for_stmt.block(), index),
        ast::Stmt::Repeat(repeat) => collect_block_declarations(repeat.block(), index),
        ast::Stmt::While(while_stmt) => collect_block_declarations(while_stmt.block(), index),
        _ => {}
    }
}

fn collect_function_body_declarations(body: &FunctionBody, index: &mut DeclarationIndex) {
    for parameter in body.parameters() {
        let Parameter::Name(name) = parameter else {
            continue;
        };
        let Some(span) = node_span(name) else {
            continue;
        };

        index.insert_declaration(span, &token_text(name), SemanticTokenKind::Parameter);
    }

    collect_block_declarations(body.block(), index);
}

fn collect_expression_properties(expression: &Expression, index: &mut DeclarationIndex) {
    match expression {
        Expression::BinaryOperator { lhs, rhs, .. } => {
            collect_expression_properties(lhs, index);
            collect_expression_properties(rhs, index);
        }
        Expression::Function(function) => {
            collect_function_body_declarations(function.body(), index);
        }
        Expression::FunctionCall(function_call) => {
            for suffix in function_call.suffixes() {
                if let Suffix::Call(_) = suffix {
                    break;
                }
            }
        }
        Expression::Parentheses { expression, .. } => {
            collect_expression_properties(expression, index)
        }
        Expression::TableConstructor(table) => {
            for field in table.fields() {
                match field {
                    Field::ExpressionKey { key, value, .. } => {
                        collect_expression_properties(key, index);
                        collect_expression_properties(value, index);
                    }
                    Field::NameKey { key, value, .. } => {
                        if let Some(span) = node_span(key) {
                            index.insert_property(span);
                        }

                        collect_expression_properties(value, index);
                    }
                    Field::NoKey(value) => collect_expression_properties(value, index),
                    _ => {}
                }
            }
        }
        Expression::TypeAssertion { expression, .. } => {
            collect_expression_properties(expression, index);
        }
        Expression::UnaryOperator { expression, .. } => {
            collect_expression_properties(expression, index)
        }
        Expression::Var(var) => collect_var_properties(var, index),
        _ => {}
    }
}

fn collect_var_properties(var: &Var, index: &mut DeclarationIndex) {
    let Var::Expression(expression) = var else {
        return;
    };

    for suffix in expression.suffixes() {
        if let Suffix::Index(Index::Dot { name, .. }) = suffix {
            if let Some(span) = node_span(name) {
                index.insert_property(span);
            }
        }
    }
}

fn is_type_reference(source: &str, position: SourcePosition) -> bool {
    let Some(line) = line_at(source, position.line) else {
        return false;
    };
    let cursor = byte_for_column(line, position.column).min(line.len());
    let prefix = &line[..cursor];
    let Some(marker_index) = prefix.rfind([':', '|', '=']) else {
        return false;
    };
    let marker = prefix.as_bytes()[marker_index];
    let before_marker = &prefix[..marker_index];

    match marker {
        b':' => true,
        b'|' => before_marker.contains(':') || before_marker.contains("type "),
        b'=' => before_marker.contains("type "),
        _ => false,
    }
}

fn is_known_global(name: &str) -> bool {
    matches!(
        name,
        "assert"
            | "coroutine"
            | "debug"
            | "error"
            | "getmetatable"
            | "io"
            | "ipairs"
            | "love"
            | "math"
            | "next"
            | "os"
            | "package"
            | "pairs"
            | "pcall"
            | "print"
            | "rawequal"
            | "rawget"
            | "rawset"
            | "require"
            | "select"
            | "setmetatable"
            | "string"
            | "table"
            | "tonumber"
            | "tostring"
            | "type"
            | "unpack"
            | "xpcall"
    )
}

fn is_primitive_type(text: &str) -> bool {
    matches!(text, "number" | "string" | "boolean" | "nil" | "any")
}

fn is_identifier(token: &TokenReference) -> bool {
    matches!(token.token().token_type(), TokenType::Identifier { .. })
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn token_text(token: &TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn span_key(span: SourceSpan) -> SpanKey {
    (
        span.start.line,
        span.start.column,
        span.end.line,
        span.end.column,
    )
}

type SpanKey = (usize, usize, usize, usize);

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests;
