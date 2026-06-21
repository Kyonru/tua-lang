use std::collections::BTreeMap;
use std::path::Path;

use full_moon::ast::{
    self, Expression, Field, FunctionBody, Index, LastStmt, LuaVersion, Parameter, Suffix, Var,
};
use full_moon::node::Node;
use full_moon::tokenizer::{Lexer, LexerResult, Symbol, Token, TokenReference, TokenType};

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
    let source = source.into();
    let parsed = match parse_tua_source(path, source.clone()) {
        Ok(parsed) => parsed,
        Err(_) => return Ok(lossy_semantic_tokens(&source)),
    };

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
        DefinitionKind::Module => SemanticTokenKind::Variable,
        DefinitionKind::TypeAlias => SemanticTokenKind::TypeAlias,
        DefinitionKind::Local => SemanticTokenKind::Variable,
        DefinitionKind::Function => SemanticTokenKind::Function,
        DefinitionKind::Parameter => SemanticTokenKind::Parameter,
        DefinitionKind::TableField => SemanticTokenKind::Property,
    }
}

fn lossy_semantic_tokens(source: &str) -> Vec<SemanticToken> {
    let tokens = lossy_tokens(source);
    let mut semantic_tokens = tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| lossy_semantic_token_for_identifier(&tokens, index, token))
        .collect::<Vec<_>>();

    semantic_tokens.sort_by_key(|token| position_key(token.span.start));
    semantic_tokens.dedup_by_key(|token| {
        (
            token.span.start.line,
            token.span.start.column,
            token.span.end.line,
            token.span.end.column,
            token.kind,
        )
    });

    semantic_tokens
}

fn lossy_tokens(source: &str) -> Vec<Token> {
    match Lexer::new(source, LuaVersion::luau()).collect() {
        LexerResult::Ok(tokens) | LexerResult::Recovered(tokens, _) => tokens
            .into_iter()
            .filter(|token| !token.token_type().is_trivia())
            .filter(|token| !matches!(token.token_type(), TokenType::Eof))
            .collect(),
        LexerResult::Fatal(_) => Vec::new(),
    }
}

fn lossy_semantic_token_for_identifier(
    tokens: &[Token],
    index: usize,
    token: &Token,
) -> Option<SemanticToken> {
    let text = token_identifier_text(token)?;
    let span = token_span(token);

    if text == "type" {
        return None;
    }

    if is_lossy_type_alias_declaration(tokens, index) {
        return Some(semantic_token(span, SemanticTokenKind::TypeAlias, true));
    }

    if is_lossy_function_declaration(tokens, index) {
        return Some(semantic_token(span, SemanticTokenKind::Function, true));
    }

    if is_lossy_property(tokens, index) {
        return Some(semantic_token(span, SemanticTokenKind::Property, false));
    }

    if is_lossy_local_declaration(tokens, index) {
        return Some(semantic_token(span, SemanticTokenKind::Variable, true));
    }

    if is_lossy_parameter_declaration(tokens, index) {
        return Some(semantic_token(span, SemanticTokenKind::Parameter, true));
    }

    if is_lossy_type_reference(tokens, index) {
        let kind = if is_primitive_type(&text) {
            SemanticTokenKind::BuiltinType
        } else {
            SemanticTokenKind::TypeAlias
        };

        return Some(semantic_token(span, kind, false));
    }

    None
}

fn semantic_token(span: SourceSpan, kind: SemanticTokenKind, declaration: bool) -> SemanticToken {
    SemanticToken {
        span,
        kind,
        modifiers: declaration
            .then_some(SemanticTokenModifier::Declaration)
            .into_iter()
            .collect(),
    }
}

fn is_lossy_type_alias_declaration(tokens: &[Token], index: usize) -> bool {
    previous_identifier(tokens, index).as_deref() == Some("type")
        && next_symbol(tokens, index) == Some(Symbol::Equal)
}

fn is_lossy_property(tokens: &[Token], index: usize) -> bool {
    previous_symbol(tokens, index) == Some(Symbol::Dot)
        || (next_symbol(tokens, index) == Some(Symbol::Colon)
            && is_inside_type_table(tokens, index))
        || (next_symbol(tokens, index) == Some(Symbol::Equal) && is_inside_table(tokens, index))
}

fn is_lossy_function_declaration(tokens: &[Token], index: usize) -> bool {
    (previous_symbol(tokens, index) == Some(Symbol::Function)
        && matches!(next_symbol(tokens, index), Some(Symbol::LeftParen) | None))
        || (previous_symbol(tokens, index) == Some(Symbol::Dot)
            && previous_symbol_before(tokens, index, Symbol::Dot) == Some(Symbol::Function))
}

fn is_lossy_local_declaration(tokens: &[Token], index: usize) -> bool {
    previous_symbol(tokens, index) == Some(Symbol::Local)
        || (previous_symbol(tokens, index) == Some(Symbol::Comma)
            && is_since_line_start(tokens, index, Symbol::Local)
            && !is_since_line_start(tokens, index, Symbol::Equal))
}

fn is_lossy_parameter_declaration(tokens: &[Token], index: usize) -> bool {
    next_symbol(tokens, index) == Some(Symbol::Colon)
        && is_inside_function_parameters(tokens, index)
}

fn is_lossy_type_reference(tokens: &[Token], index: usize) -> bool {
    matches!(
        previous_symbol(tokens, index),
        Some(Symbol::Colon | Symbol::Pipe)
    ) || (previous_symbol(tokens, index) == Some(Symbol::Equal)
        && is_after_type_alias_equal(tokens, index))
        || (previous_symbol(tokens, index) == Some(Symbol::QuestionMark)
            && previous_symbol_before(tokens, index, Symbol::QuestionMark) == Some(Symbol::Colon))
}

fn is_after_type_alias_equal(tokens: &[Token], index: usize) -> bool {
    let line = token_span(&tokens[index]).start.line;
    let mut saw_equal = false;

    for token in tokens[..index].iter().rev() {
        if token_span(token).start.line != line {
            break;
        }

        if token_symbol(token) == Some(Symbol::Equal) {
            saw_equal = true;
            continue;
        }

        if saw_equal && token_identifier_text(token).as_deref() == Some("type") {
            return true;
        }
    }

    false
}

fn is_inside_type_table(tokens: &[Token], index: usize) -> bool {
    let mut depth = 0usize;

    for token in tokens[..index].iter().rev() {
        match token_symbol(token) {
            Some(Symbol::RightBrace) => depth += 1,
            Some(Symbol::LeftBrace) if depth > 0 => depth -= 1,
            Some(Symbol::LeftBrace) => return previous_type_declaration_marker(tokens, token),
            _ => {}
        }
    }

    false
}

fn is_inside_table(tokens: &[Token], index: usize) -> bool {
    tokens[..index]
        .iter()
        .rev()
        .find_map(|token| match token_symbol(token) {
            Some(Symbol::LeftBrace) => Some(true),
            Some(Symbol::RightBrace) => Some(false),
            _ => None,
        })
        .unwrap_or(false)
}

fn is_inside_function_parameters(tokens: &[Token], index: usize) -> bool {
    let mut depth = 0usize;

    for token in tokens[..index].iter().rev() {
        match token_symbol(token) {
            Some(Symbol::RightParen) => depth += 1,
            Some(Symbol::LeftParen) if depth > 0 => depth -= 1,
            Some(Symbol::LeftParen) => {
                return previous_symbol_for_token(tokens, token) == Some(Symbol::Function)
            }
            _ => {}
        }
    }

    false
}

fn previous_type_declaration_marker(tokens: &[Token], left_brace: &Token) -> bool {
    let Some(left_brace_index) = token_index(tokens, left_brace) else {
        return false;
    };

    previous_symbol(tokens, left_brace_index) == Some(Symbol::Equal)
        && previous_identifier(tokens, left_brace_index).is_some()
}

fn is_since_line_start(tokens: &[Token], index: usize, symbol: Symbol) -> bool {
    let line = token_span(&tokens[index]).start.line;

    tokens[..index]
        .iter()
        .rev()
        .any(|token| token_span(token).start.line == line && token_symbol(token) == Some(symbol))
}

fn previous_symbol(tokens: &[Token], index: usize) -> Option<Symbol> {
    tokens[..index].iter().rev().find_map(token_symbol)
}

fn previous_symbol_before(tokens: &[Token], index: usize, marker: Symbol) -> Option<Symbol> {
    let marker_index =
        tokens[..index]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(candidate, token)| {
                (token_symbol(token) == Some(marker)).then_some(candidate)
            })?;

    previous_symbol(tokens, marker_index)
}

fn previous_symbol_for_token(tokens: &[Token], token: &Token) -> Option<Symbol> {
    let index = token_index(tokens, token)?;

    previous_symbol(tokens, index)
}

fn previous_identifier(tokens: &[Token], index: usize) -> Option<String> {
    tokens[..index].iter().rev().find_map(token_identifier_text)
}

fn next_symbol(tokens: &[Token], index: usize) -> Option<Symbol> {
    tokens.get(index + 1..)?.iter().find_map(token_symbol)
}

fn token_symbol(token: &Token) -> Option<Symbol> {
    match token.token_type() {
        TokenType::Symbol { symbol } => Some(*symbol),
        _ => None,
    }
}

fn token_identifier_text(token: &Token) -> Option<String> {
    match token.token_type() {
        TokenType::Identifier { identifier } => Some(identifier.to_string()),
        _ => None,
    }
}

fn token_span(token: &Token) -> SourceSpan {
    SourceSpan::from_range(token.start_position(), token.end_position())
}

fn token_index(tokens: &[Token], needle: &Token) -> Option<usize> {
    tokens.iter().position(|token| {
        token.start_position() == needle.start_position()
            && token.end_position() == needle.end_position()
            && token.token_type() == needle.token_type()
    })
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
