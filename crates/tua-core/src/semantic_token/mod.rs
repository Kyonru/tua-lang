use std::collections::BTreeMap;
use std::path::Path;

use full_moon::ast::{
    self, Call, Expression, Field, FunctionBody, Index, LastStmt, LuaVersion, Parameter, Prefix,
    Suffix, Var,
};
use full_moon::node::Node;
use full_moon::tokenizer::{Lexer, LexerResult, Symbol, Token, TokenReference, TokenType};

use crate::hir::{lower_parsed_source, HirSymbolKind};
use crate::parser::{ParsedSource, SourcePosition, SourceSpan, TypeAliasKind};
use crate::source_text::{byte_for_column, line_at};
use crate::{
    ecs, lua, object_pool, parse_tua_source, signal, sprite, state_machine, vector, Diagnostic,
};

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
    Global,
    Module,
    Method,
    AnnotationTag,
    Class,
    Enum,
    EnumMember,
    Namespace,
    Event,
    Label,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticTokenModifier {
    Declaration,
    Readonly,
    Deprecated,
    Modification,
    DefaultLibrary,
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
    let source_tokens = lossy_tokens(&parsed.source);
    let declarations = DeclarationIndex::new(parsed, &source_tokens);
    let mut tokens = source_tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            semantic_token_for_identifier(parsed, &declarations, &source_tokens, index, token)
        })
        .collect::<Vec<_>>();

    tokens.extend(lua_doc_semantic_tokens(parsed));
    tokens.extend(annotation_tag_semantic_tokens(&parsed.source));
    tokens.extend(module_semantic_tokens(&source_tokens));
    tokens.extend(signal_event_semantic_tokens(&source_tokens));
    tokens.extend(enum_member_semantic_tokens(parsed, &source_tokens));

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
    tokens: &[Token],
    index: usize,
    token: &Token,
) -> Option<SemanticToken> {
    let text = token_identifier_text(token)?;
    let span = token_span(token);

    if is_language_keyword(&text) {
        return None;
    }

    if let Some(classification) = declarations.by_span.get(&span_key(span)) {
        let mut token = classification.to_token(span);

        if matches!(
            token.kind,
            SemanticTokenKind::Method | SemanticTokenKind::Property
        ) && member_uses_builtin_namespace(tokens, index)
            && !token
                .modifiers
                .contains(&SemanticTokenModifier::DefaultLibrary)
        {
            token.modifiers.push(SemanticTokenModifier::DefaultLibrary);
        }
        if is_modification_reference(tokens, index)
            && !token
                .modifiers
                .contains(&SemanticTokenModifier::Modification)
        {
            token.modifiers.push(SemanticTokenModifier::Modification);
        }

        return Some(token);
    }

    if is_type_reference(&parsed.source, span.start) {
        let kind = declarations
            .type_kind(&text)
            .unwrap_or_else(|| type_reference_kind(&text));
        let mut modifiers = semantic_modifiers_for_name(&text, false);

        if kind == SemanticTokenKind::BuiltinType {
            modifiers.push(SemanticTokenModifier::DefaultLibrary);
        }

        return Some(SemanticToken {
            span,
            kind,
            modifiers,
        });
    }

    if let Some(kind) = member_kind(tokens, index) {
        let mut modifiers = semantic_modifiers_for_name(&text, false);

        if member_uses_builtin_namespace(tokens, index) {
            modifiers.push(SemanticTokenModifier::DefaultLibrary);
        }
        if is_modification_reference(tokens, index) {
            modifiers.push(SemanticTokenModifier::Modification);
        }

        return Some(SemanticToken {
            span,
            kind,
            modifiers,
        });
    }

    if let Some(kind) = declarations.nearest_value_kind(&text, span.start) {
        let mut modifiers = semantic_modifiers_for_name(&text, false);

        if is_modification_reference(tokens, index) {
            modifiers.push(SemanticTokenModifier::Modification);
        }

        return Some(SemanticToken {
            span,
            kind,
            modifiers,
        });
    }

    if let Some(kind) = builtin_global_kind(&text) {
        let mut modifiers = semantic_modifiers_for_name(&text, false);
        modifiers.push(SemanticTokenModifier::DefaultLibrary);

        return Some(SemanticToken {
            span,
            kind,
            modifiers,
        });
    }

    if text == "self" {
        return Some(SemanticToken {
            span,
            kind: SemanticTokenKind::Parameter,
            modifiers: Vec::new(),
        });
    }

    if let Some(kind) = declarations.type_kind(&text) {
        return Some(SemanticToken {
            span,
            kind,
            modifiers: semantic_modifiers_for_name(&text, false),
        });
    }

    if is_label(tokens, index) {
        return Some(semantic_token(
            span,
            SemanticTokenKind::Label,
            is_label_declaration(tokens, index),
        ));
    }

    Some(SemanticToken {
        span,
        kind: SemanticTokenKind::UnresolvedReference,
        modifiers: semantic_modifiers_for_name(&text, false),
    })
}

fn lossy_semantic_tokens(source: &str) -> Vec<SemanticToken> {
    let tokens = lossy_tokens(source);
    let mut semantic_tokens = tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| lossy_semantic_token_for_identifier(&tokens, index, token))
        .collect::<Vec<_>>();

    semantic_tokens.extend(annotation_tag_semantic_tokens(source));
    semantic_tokens.extend(module_semantic_tokens(&tokens));
    semantic_tokens.extend(signal_event_semantic_tokens(&tokens));
    semantic_tokens.extend(lossy_enum_member_semantic_tokens(&tokens));

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

    if is_language_keyword(&text) {
        return None;
    }

    if is_lossy_type_alias_declaration(tokens, index) {
        return Some(semantic_token(span, SemanticTokenKind::TypeAlias, true));
    }

    if is_lossy_enum_declaration(tokens, index) {
        return Some(semantic_token(span, SemanticTokenKind::Enum, true));
    }

    if is_lossy_class_declaration(tokens, index) {
        return Some(semantic_token(span, SemanticTokenKind::Class, true));
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

    if let Some(kind) = member_kind(tokens, index) {
        let mut token = semantic_token_with_default_library(
            span,
            kind,
            member_uses_builtin_namespace(tokens, index),
        );
        if is_modification_reference(tokens, index) {
            token.modifiers.push(SemanticTokenModifier::Modification);
        }
        return Some(token);
    }

    if is_lossy_type_reference(tokens, index) {
        let kind = type_reference_kind(&text);

        return Some(semantic_token_with_default_library(
            span,
            kind,
            kind == SemanticTokenKind::BuiltinType,
        ));
    }

    if let Some(kind) = builtin_global_kind(&text) {
        return Some(semantic_token_with_default_library(span, kind, true));
    }

    if text == "self" {
        return Some(semantic_token(span, SemanticTokenKind::Parameter, false));
    }

    if is_label(tokens, index) {
        return Some(semantic_token(
            span,
            SemanticTokenKind::Label,
            is_label_declaration(tokens, index),
        ));
    }

    None
}

fn semantic_token(span: SourceSpan, kind: SemanticTokenKind, declaration: bool) -> SemanticToken {
    SemanticToken {
        span,
        kind,
        modifiers: semantic_modifiers_for_name("", declaration),
    }
}

fn semantic_token_with_default_library(
    span: SourceSpan,
    kind: SemanticTokenKind,
    default_library: bool,
) -> SemanticToken {
    let mut token = semantic_token(span, kind, false);

    if default_library {
        token.modifiers.push(SemanticTokenModifier::DefaultLibrary);
    }

    token
}

fn is_lossy_type_alias_declaration(tokens: &[Token], index: usize) -> bool {
    previous_identifier(tokens, index).as_deref() == Some("type")
        && next_symbol(tokens, index) == Some(Symbol::Equal)
}

fn is_lossy_enum_declaration(tokens: &[Token], index: usize) -> bool {
    previous_identifier_immediate(tokens, index).as_deref() == Some("enum")
}

fn is_lossy_class_declaration(tokens: &[Token], index: usize) -> bool {
    previous_identifier_immediate(tokens, index).as_deref() == Some("class")
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

fn previous_identifier_immediate(tokens: &[Token], index: usize) -> Option<String> {
    tokens
        .get(index.checked_sub(1)?)
        .and_then(token_identifier_text)
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
    types: BTreeMap<String, SemanticTokenKind>,
}

impl DeclarationIndex {
    fn new(parsed: &ParsedSource, tokens: &[Token]) -> Self {
        let mut index = Self::default();
        let hir = lower_parsed_source(parsed);

        for symbol in hir.symbols() {
            match symbol.kind {
                HirSymbolKind::Class => {
                    index.insert_declaration(
                        symbol.name_span,
                        &symbol.name,
                        SemanticTokenKind::Class,
                    );
                }
                HirSymbolKind::TypeAlias => {
                    let kind = parsed
                        .type_aliases
                        .iter()
                        .find(|alias| alias.name_span == Some(symbol.name_span))
                        .map(|alias| match alias.kind {
                            TypeAliasKind::Alias => SemanticTokenKind::TypeAlias,
                            TypeAliasKind::Enum => SemanticTokenKind::Enum,
                        })
                        .unwrap_or(SemanticTokenKind::TypeAlias);
                    index.insert_declaration(symbol.name_span, &symbol.name, kind);
                }
                HirSymbolKind::TableField => {
                    index.insert_property_declaration(symbol.name_span, &symbol.name)
                }
                HirSymbolKind::Local => index.insert_declaration(
                    symbol.name_span,
                    &symbol.name,
                    SemanticTokenKind::Variable,
                ),
                HirSymbolKind::Function => index.insert_declaration(
                    symbol.name_span,
                    &symbol.name,
                    SemanticTokenKind::Function,
                ),
                HirSymbolKind::Parameter => index.insert_declaration(
                    symbol.name_span,
                    &symbol.name,
                    SemanticTokenKind::Parameter,
                ),
            }
        }

        collect_block_declarations(parsed.ast.nodes(), &mut index);
        collect_class_declarations(parsed, tokens, &mut index);
        mark_deprecated_declarations(parsed, &mut index);

        index
    }

    fn insert_declaration(&mut self, span: SourceSpan, name: &str, kind: SemanticTokenKind) {
        self.by_span.insert(
            span_key(span),
            Classification {
                kind,
                declaration: true,
                readonly: looks_like_const_name(name),
                deprecated: false,
                modification: false,
                default_library: false,
            },
        );

        if matches!(
            kind,
            SemanticTokenKind::TypeAlias | SemanticTokenKind::Class | SemanticTokenKind::Enum
        ) {
            self.types.insert(name.to_owned(), kind);
        }

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
                readonly: false,
                deprecated: false,
                modification: false,
                default_library: false,
            });
    }

    fn insert_property_declaration(&mut self, span: SourceSpan, name: &str) {
        self.insert_declaration(span, name, SemanticTokenKind::Property);
    }

    fn insert_method(&mut self, span: SourceSpan, name: &str, declaration: bool) {
        self.by_span.insert(
            span_key(span),
            Classification {
                kind: SemanticTokenKind::Method,
                declaration,
                readonly: looks_like_const_name(name),
                deprecated: false,
                modification: false,
                default_library: false,
            },
        );
    }

    fn insert_reference(&mut self, span: SourceSpan, kind: SemanticTokenKind, modification: bool) {
        self.by_span.insert(
            span_key(span),
            Classification {
                kind,
                declaration: false,
                readonly: false,
                deprecated: false,
                modification,
                default_library: false,
            },
        );
    }

    fn mark_deprecated_on_line(&mut self, line: usize) {
        for (span, classification) in &mut self.by_span {
            if span.0 == line && classification.declaration {
                classification.deprecated = true;
            }
        }
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

    fn type_kind(&self, name: &str) -> Option<SemanticTokenKind> {
        self.types.get(name).copied()
    }
}

#[derive(Clone, Copy)]
struct Classification {
    kind: SemanticTokenKind,
    declaration: bool,
    readonly: bool,
    deprecated: bool,
    modification: bool,
    default_library: bool,
}

impl Classification {
    fn to_token(self, span: SourceSpan) -> SemanticToken {
        let mut modifiers = self
            .declaration
            .then_some(SemanticTokenModifier::Declaration)
            .into_iter()
            .collect::<Vec<_>>();

        if self.readonly {
            modifiers.push(SemanticTokenModifier::Readonly);
        }

        if self.deprecated {
            modifiers.push(SemanticTokenModifier::Deprecated);
        }

        if self.modification {
            modifiers.push(SemanticTokenModifier::Modification);
        }

        if self.default_library {
            modifiers.push(SemanticTokenModifier::DefaultLibrary);
        }

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

fn collect_class_declarations(
    parsed: &ParsedSource,
    tokens: &[Token],
    index: &mut DeclarationIndex,
) {
    for class in &parsed.class_declarations {
        index.insert_declaration(class.name_span, &class.name, SemanticTokenKind::Class);

        if let Some(parent_span) = class.parent_span {
            index.insert_reference(parent_span, SemanticTokenKind::Class, false);
        }

        for field in &class.fields {
            index.insert_declaration(field.name_span, &field.name, SemanticTokenKind::Property);
        }

        for method in class.init.iter().chain(class.methods.iter()) {
            index.insert_method(method.name_span, &method.name, true);
            collect_class_parameter_declarations(tokens, method.parameters_span, index);
        }
    }
}

fn collect_class_parameter_declarations(
    tokens: &[Token],
    parameters_span: SourceSpan,
    index: &mut DeclarationIndex,
) {
    let mut expect_parameter = true;
    let mut nested_depth = 0usize;

    for token in tokens
        .iter()
        .filter(|token| span_contains(parameters_span, token_span(token)))
    {
        match token_symbol(token) {
            Some(Symbol::LeftParen | Symbol::LeftBrace | Symbol::LeftBracket) => {
                nested_depth += 1;
            }
            Some(Symbol::RightParen | Symbol::RightBrace | Symbol::RightBracket) => {
                nested_depth = nested_depth.saturating_sub(1);
            }
            Some(Symbol::Comma) if nested_depth == 0 => expect_parameter = true,
            _ => {
                let Some(name) = token_identifier_text(token) else {
                    continue;
                };

                if expect_parameter && nested_depth == 0 {
                    index.insert_declaration(
                        token_span(token),
                        &name,
                        SemanticTokenKind::Parameter,
                    );
                    expect_parameter = false;
                }
            }
        }
    }
}

fn collect_block_declarations(block: &ast::Block, index: &mut DeclarationIndex) {
    for stmt in block.stmts() {
        collect_stmt_declarations(stmt, index);
    }

    if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
        for expression in return_stmt.returns() {
            collect_return_expression_properties(expression, index);
        }
    }
}

fn collect_stmt_declarations(stmt: &ast::Stmt, index: &mut DeclarationIndex) {
    match stmt {
        ast::Stmt::Assignment(assignment) => {
            for variable in assignment.variables() {
                collect_assignment_target(variable, index);
            }

            for expression in assignment.expressions() {
                collect_expression_properties(expression, index);
            }
        }
        ast::Stmt::CompoundAssignment(assignment) => {
            collect_assignment_target(assignment.lhs(), index);
            collect_expression_properties(assignment.rhs(), index);
        }
        ast::Stmt::Do(block) => collect_block_declarations(block.block(), index),
        ast::Stmt::FunctionCall(function_call) => {
            collect_function_call_methods(function_call, index)
        }
        ast::Stmt::FunctionDeclaration(function) => {
            collect_function_name_declaration(function.name(), index);

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
            collect_function_call_methods(function_call, index)
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

fn collect_return_expression_properties(expression: &Expression, index: &mut DeclarationIndex) {
    match expression {
        Expression::TableConstructor(table) => {
            for field in table.fields() {
                match field {
                    Field::ExpressionKey { key, value, .. } => {
                        collect_expression_properties(key, index);
                        collect_expression_properties(value, index);
                    }
                    Field::NameKey { key, value, .. } => {
                        if let Some(span) = node_span(key) {
                            index.insert_property_declaration(span, &token_text(key));
                        }

                        collect_expression_properties(value, index);
                    }
                    Field::NoKey(value) => collect_expression_properties(value, index),
                    _ => {}
                }
            }
        }
        _ => collect_expression_properties(expression, index),
    }
}

fn collect_function_name_declaration(name: &ast::FunctionName, index: &mut DeclarationIndex) {
    if let Some(method_name) = name.method_name() {
        if let Some(span) = node_span(method_name) {
            index.insert_method(span, &token_text(method_name), true);
        }

        return;
    }

    if let Some(function_name) = name.names().iter().last() {
        if let Some(span) = node_span(function_name) {
            let function_name = token_text(function_name);

            if name.names().iter().count() > 1 {
                index.insert_method(span, &function_name, true);
            } else {
                index.insert_declaration(span, &function_name, SemanticTokenKind::Function);
            }
        }
    }
}

fn collect_function_call_methods(function_call: &ast::FunctionCall, index: &mut DeclarationIndex) {
    let suffixes = function_call.suffixes().collect::<Vec<_>>();

    for (offset, suffix) in suffixes.iter().enumerate() {
        match suffix {
            Suffix::Call(Call::MethodCall(method)) => {
                if let Some(span) = node_span(method.name()) {
                    index.insert_method(span, &token_text(method.name()), false);
                }
            }
            Suffix::Index(Index::Dot { name, .. }) => {
                let Some(span) = node_span(name) else {
                    continue;
                };
                let called = matches!(
                    suffixes.get(offset + 1),
                    Some(Suffix::Call(Call::AnonymousCall(_)))
                );

                if called {
                    index.insert_method(span, &token_text(name), false);
                } else {
                    index.insert_property(span);
                }
            }
            _ => {}
        }
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

fn collect_assignment_target(var: &Var, index: &mut DeclarationIndex) {
    match var {
        Var::Name(name) => {
            if let Some(span) = node_span(name) {
                index.insert_reference(span, SemanticTokenKind::Variable, true);
            }
        }
        Var::Expression(expression) => {
            if expression.suffixes().next().is_none() {
                if let Prefix::Name(name) = expression.prefix() {
                    if let Some(span) = node_span(name) {
                        index.insert_reference(span, SemanticTokenKind::Variable, true);
                    }
                }
                return;
            }

            let properties = expression
                .suffixes()
                .filter_map(|suffix| match suffix {
                    Suffix::Index(Index::Dot { name, .. }) => node_span(name),
                    _ => None,
                })
                .collect::<Vec<_>>();

            for (offset, span) in properties.iter().copied().enumerate() {
                index.insert_reference(
                    span,
                    SemanticTokenKind::Property,
                    offset + 1 == properties.len(),
                );
            }
        }
        _ => {}
    }
}

fn mark_deprecated_declarations(parsed: &ParsedSource, index: &mut DeclarationIndex) {
    let mut pending = false;

    for (offset, line) in parsed.source.lines().enumerate() {
        let line_number = offset + 1;
        let trimmed = line.trim_start();

        if trimmed.starts_with("---@deprecated") {
            pending = true;
            continue;
        }

        if trimmed.starts_with("---") || trimmed.is_empty() {
            continue;
        }

        if pending {
            index.mark_deprecated_on_line(line_number);
            pending = false;
        }
    }
}

fn lua_doc_semantic_tokens(parsed: &ParsedSource) -> Vec<SemanticToken> {
    let mut tokens = Vec::new();

    for annotation in &parsed.lua_doc_annotations {
        match &annotation.kind {
            crate::LuaDocAnnotationKind::Class(class) => {
                tokens.push(semantic_token(
                    class.name_span,
                    SemanticTokenKind::TypeAlias,
                    true,
                ));

                if let Some(span) = class.extends_span {
                    tokens.push(semantic_token(span, SemanticTokenKind::TypeAlias, false));
                }
            }
            crate::LuaDocAnnotationKind::Field(field) => {
                tokens.push(semantic_token(
                    field.name_span,
                    SemanticTokenKind::Property,
                    true,
                ));
                tokens.extend(type_tokens_in_span(&parsed.source, field.type_span));
            }
            crate::LuaDocAnnotationKind::Alias(alias) => {
                tokens.push(semantic_token(
                    alias.name_span,
                    SemanticTokenKind::TypeAlias,
                    true,
                ));
                tokens.extend(type_tokens_in_span(&parsed.source, alias.type_span));
            }
            crate::LuaDocAnnotationKind::Type(type_annotation) => {
                tokens.extend(type_tokens_in_span(
                    &parsed.source,
                    type_annotation.type_span,
                ));
            }
            crate::LuaDocAnnotationKind::Param(param) => {
                tokens.push(semantic_token(
                    param.name_span,
                    SemanticTokenKind::Parameter,
                    true,
                ));
                tokens.extend(type_tokens_in_span(&parsed.source, param.type_span));
            }
            crate::LuaDocAnnotationKind::Return(return_annotation) => {
                for return_ty in &return_annotation.returns {
                    tokens.extend(type_tokens_in_span(&parsed.source, return_ty.type_span));
                }
            }
            crate::LuaDocAnnotationKind::Generic(generic) => {
                for parameter in &generic.parameters {
                    tokens.push(semantic_token(
                        parameter.name_span,
                        SemanticTokenKind::TypeAlias,
                        true,
                    ));

                    if let Some(bound_span) = parameter.bound_span {
                        tokens.extend(type_tokens_in_span(&parsed.source, bound_span));
                    }
                }
            }
            crate::LuaDocAnnotationKind::As(annotation) => {
                tokens.extend(type_tokens_in_span(&parsed.source, annotation.type_span));
            }
            crate::LuaDocAnnotationKind::Overload(annotation) => {
                tokens.extend(type_tokens_in_span(&parsed.source, annotation.type_span));
            }
            crate::LuaDocAnnotationKind::Vararg(annotation) => {
                tokens.extend(type_tokens_in_span(&parsed.source, annotation.type_span));
            }
            crate::LuaDocAnnotationKind::Enum(annotation) => {
                if let Some(span) = annotation.name_span {
                    tokens.push(semantic_token(span, SemanticTokenKind::TypeAlias, true));
                }
            }
            crate::LuaDocAnnotationKind::Meta(_)
            | crate::LuaDocAnnotationKind::Module(_)
            | crate::LuaDocAnnotationKind::Diagnostic(_)
            | crate::LuaDocAnnotationKind::Cast(_)
            | crate::LuaDocAnnotationKind::Deprecated(_)
            | crate::LuaDocAnnotationKind::NoDiscard(_)
            | crate::LuaDocAnnotationKind::Version(_) => {}
        }
    }

    tokens
}

fn annotation_tag_semantic_tokens(source: &str) -> Vec<SemanticToken> {
    let mut tokens = Vec::new();

    for (offset, line) in source.lines().enumerate() {
        let line_number = offset + 1;
        let Some(marker_start) = line.find("---@") else {
            continue;
        };
        let tag_start = marker_start + 3;
        let tag_end = line[tag_start..]
            .char_indices()
            .find_map(|(offset, ch)| {
                (!ch.is_ascii_alphanumeric() && ch != '@' && ch != '_')
                    .then_some(tag_start + offset)
            })
            .unwrap_or(line.len());

        if tag_end <= tag_start + 1 {
            continue;
        }

        let text = &line[tag_start..tag_end];
        let mut token = semantic_token(
            SourceSpan {
                start: SourcePosition {
                    line: line_number,
                    column: tag_start + 1,
                },
                end: SourcePosition {
                    line: line_number,
                    column: tag_end + 1,
                },
            },
            SemanticTokenKind::AnnotationTag,
            false,
        );

        if text == "@deprecated" {
            token.modifiers.push(SemanticTokenModifier::Deprecated);
        }

        tokens.push(token);
    }

    tokens
}

fn type_tokens_in_span(source: &str, span: SourceSpan) -> Vec<SemanticToken> {
    let Some(line) = line_at(source, span.start.line) else {
        return Vec::new();
    };
    let start = byte_for_column(line, span.start.column);
    let end = byte_for_column(line, span.end.column).min(line.len());
    let text = &line[start..end];
    let mut tokens = Vec::new();
    let mut cursor = 0usize;

    while cursor < text.len() {
        let Some((relative_start, _)) = text[cursor..]
            .char_indices()
            .find(|(_, ch)| is_identifier_start(*ch))
        else {
            break;
        };
        let token_start = cursor + relative_start;
        let token_end = text[token_start..]
            .char_indices()
            .find_map(|(offset, ch)| (!is_identifier_continue(ch)).then_some(token_start + offset))
            .unwrap_or(text.len());
        let name = &text[token_start..token_end];
        let after = text[token_end..].trim_start();
        let kind = if after.starts_with(':') || after.starts_with("?:") {
            SemanticTokenKind::Property
        } else if is_builtin_type(name) {
            SemanticTokenKind::BuiltinType
        } else {
            SemanticTokenKind::TypeAlias
        };

        let mut token = semantic_token(
            SourceSpan {
                start: SourcePosition {
                    line: span.start.line,
                    column: span.start.column + token_start,
                },
                end: SourcePosition {
                    line: span.start.line,
                    column: span.start.column + token_end,
                },
            },
            kind,
            kind == SemanticTokenKind::Property,
        );

        if kind == SemanticTokenKind::BuiltinType {
            token.modifiers.push(SemanticTokenModifier::DefaultLibrary);
        }

        tokens.push(token);

        cursor = token_end;
    }

    tokens
}

fn module_semantic_tokens(tokens: &[Token]) -> Vec<SemanticToken> {
    tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            let name = token_identifier_text(token)?;

            if !matches!(name.as_str(), "require" | "import")
                || matches!(
                    index
                        .checked_sub(1)
                        .and_then(|previous| tokens.get(previous))
                        .and_then(token_symbol),
                    Some(Symbol::Colon | Symbol::Dot)
                )
            {
                return None;
            }

            let span = string_literal_content_span(first_string_argument(tokens, index)?)?;
            Some(semantic_token(span, SemanticTokenKind::Module, false))
        })
        .collect()
}

fn signal_event_semantic_tokens(tokens: &[Token]) -> Vec<SemanticToken> {
    tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            let name = token_identifier_text(token)?;

            if !signal::is_event_name_method(&name)
                || !matches!(
                    tokens.get(index.checked_sub(1)?).and_then(token_symbol),
                    Some(Symbol::Colon | Symbol::Dot)
                )
            {
                return None;
            }

            let span = string_literal_content_span(first_string_argument(tokens, index)?)?;
            let mut token = semantic_token(
                span,
                SemanticTokenKind::Event,
                signal::is_event_schema_method(&name),
            );
            token.modifiers.push(SemanticTokenModifier::Readonly);
            Some(token)
        })
        .collect()
}

fn first_string_argument(tokens: &[Token], function_index: usize) -> Option<&Token> {
    let mut cursor = function_index + 1;

    if token_symbol(tokens.get(cursor)?) == Some(Symbol::LeftParen) {
        cursor += 1;
    }

    let token = tokens.get(cursor)?;
    matches!(token.token_type(), TokenType::StringLiteral { .. }).then_some(token)
}

fn string_literal_content_span(token: &Token) -> Option<SourceSpan> {
    let text = token.to_string();
    let quote = text.chars().next()?;

    if !matches!(quote, '\'' | '"') || !text.ends_with(quote) || text.len() < 2 {
        return None;
    }

    let span = token_span(token);
    (span.start.line == span.end.line).then_some(SourceSpan {
        start: SourcePosition {
            line: span.start.line,
            column: span.start.column + 1,
        },
        end: SourcePosition {
            line: span.end.line,
            column: span.end.column - 1,
        },
    })
}

fn enum_member_semantic_tokens(parsed: &ParsedSource, tokens: &[Token]) -> Vec<SemanticToken> {
    let enum_spans = parsed
        .type_aliases
        .iter()
        .filter(|alias| alias.kind == TypeAliasKind::Enum)
        .filter_map(|alias| alias.span)
        .collect::<Vec<_>>();

    tokens
        .iter()
        .filter(|token| matches!(token.token_type(), TokenType::StringLiteral { .. }))
        .filter(|token| {
            let span = token_span(token);
            enum_spans
                .iter()
                .any(|enum_span| span_contains(*enum_span, span))
        })
        .filter_map(string_literal_content_span)
        .map(|span| {
            let mut token = semantic_token(span, SemanticTokenKind::EnumMember, true);
            token.modifiers.push(SemanticTokenModifier::Readonly);
            token
        })
        .collect()
}

fn lossy_enum_member_semantic_tokens(tokens: &[Token]) -> Vec<SemanticToken> {
    let mut in_enum = false;
    let mut semantic_tokens = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        if token_identifier_text(token).as_deref() == Some("enum") {
            in_enum = true;
            continue;
        }

        if in_enum && token_symbol(token) == Some(Symbol::End) {
            in_enum = false;
            continue;
        }

        if !in_enum || !matches!(token.token_type(), TokenType::StringLiteral { .. }) {
            continue;
        }

        if let Some(span) = string_literal_content_span(&tokens[index]) {
            let mut token = semantic_token(span, SemanticTokenKind::EnumMember, true);
            token.modifiers.push(SemanticTokenModifier::Readonly);
            semantic_tokens.push(token);
        }
    }

    semantic_tokens
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

    if marker == b':' && before_marker.ends_with(':') {
        return false;
    }

    match marker {
        b':' => line[marker_index + 1..cursor]
            .chars()
            .all(char::is_whitespace),
        b'|' => before_marker.contains(':') || before_marker.contains("type "),
        b'=' => before_marker.contains("type "),
        _ => false,
    }
}

fn builtin_global_kind(name: &str) -> Option<SemanticTokenKind> {
    if matches!(name, "ecs" | "love") {
        return Some(SemanticTokenKind::Namespace);
    }

    if name == "import" {
        return Some(SemanticTokenKind::Function);
    }

    lua::top_level_items(name)
        .into_iter()
        .find(|item| item.label == name)
        .map(|item| match item.kind {
            lua::LuaItemKind::Function => SemanticTokenKind::Function,
            lua::LuaItemKind::Module => SemanticTokenKind::Namespace,
            lua::LuaItemKind::Value => SemanticTokenKind::Global,
            lua::LuaItemKind::Keyword => SemanticTokenKind::UnresolvedReference,
        })
}

fn type_reference_kind(name: &str) -> SemanticTokenKind {
    if is_builtin_type(name) {
        SemanticTokenKind::BuiltinType
    } else {
        SemanticTokenKind::TypeAlias
    }
}

fn is_builtin_type(name: &str) -> bool {
    is_primitive_type(name)
        || name == "Self"
        || lua::love_type_names().contains(&name)
        || vector::builtin_type_names().contains(&name)
        || signal::builtin_type_names().contains(&name)
        || object_pool::builtin_type_names().contains(&name)
        || state_machine::builtin_type_names().contains(&name)
        || sprite::builtin_type_names().contains(&name)
        || ecs::builtin_type_names().contains(&name)
}

fn is_language_keyword(name: &str) -> bool {
    matches!(
        name,
        "and"
            | "break"
            | "class"
            | "do"
            | "else"
            | "elseif"
            | "end"
            | "enum"
            | "extends"
            | "false"
            | "final"
            | "for"
            | "function"
            | "goto"
            | "if"
            | "in"
            | "is"
            | "local"
            | "nil"
            | "not"
            | "open"
            | "or"
            | "override"
            | "repeat"
            | "return"
            | "super"
            | "then"
            | "true"
            | "type"
            | "until"
            | "while"
    )
}

fn member_kind(tokens: &[Token], index: usize) -> Option<SemanticTokenKind> {
    let accessor = tokens.get(index.checked_sub(1)?).and_then(token_symbol)?;

    if !matches!(accessor, Symbol::Dot | Symbol::Colon) {
        return None;
    }

    let called = tokens.get(index + 1).is_some_and(|token| {
        token_symbol(token) == Some(Symbol::LeftParen)
            || matches!(token.token_type(), TokenType::StringLiteral { .. })
            || token_symbol(token) == Some(Symbol::LeftBrace)
    });

    if accessor == Symbol::Colon && !called {
        return None;
    }

    Some(if called {
        SemanticTokenKind::Method
    } else {
        SemanticTokenKind::Property
    })
}

fn member_uses_builtin_namespace(tokens: &[Token], index: usize) -> bool {
    let mut cursor = index;
    let mut root = None;

    while cursor >= 2 {
        let Some(accessor) = tokens.get(cursor - 1).and_then(token_symbol) else {
            break;
        };
        if !matches!(accessor, Symbol::Dot | Symbol::Colon) {
            break;
        }

        let Some(name) = tokens.get(cursor - 2).and_then(token_identifier_text) else {
            break;
        };
        root = Some(name);
        cursor -= 2;
    }

    root.is_some_and(|name| builtin_global_kind(&name) == Some(SemanticTokenKind::Namespace))
}

fn is_modification_reference(tokens: &[Token], index: usize) -> bool {
    matches!(
        tokens.get(index + 1).and_then(token_symbol),
        Some(
            Symbol::Equal
                | Symbol::PlusEqual
                | Symbol::MinusEqual
                | Symbol::StarEqual
                | Symbol::SlashEqual
                | Symbol::PercentEqual
        )
    )
}

fn is_label(tokens: &[Token], index: usize) -> bool {
    is_label_declaration(tokens, index)
        || previous_identifier_immediate(tokens, index).as_deref() == Some("goto")
}

fn is_label_declaration(tokens: &[Token], index: usize) -> bool {
    tokens.get(index.wrapping_sub(1)).and_then(token_symbol) == Some(Symbol::TwoColons)
        && tokens.get(index + 1).and_then(token_symbol) == Some(Symbol::TwoColons)
}

fn is_primitive_type(text: &str) -> bool {
    matches!(text, "number" | "string" | "boolean" | "nil" | "any")
}

fn semantic_modifiers_for_name(name: &str, declaration: bool) -> Vec<SemanticTokenModifier> {
    let mut modifiers = declaration
        .then_some(SemanticTokenModifier::Declaration)
        .into_iter()
        .collect::<Vec<_>>();

    if looks_like_const_name(name) {
        modifiers.push(SemanticTokenModifier::Readonly);
    }

    modifiers
}

fn looks_like_const_name(name: &str) -> bool {
    let mut has_uppercase = false;
    let mut has_lowercase = false;

    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            has_uppercase = true;
        } else if ch.is_ascii_lowercase() {
            has_lowercase = true;
        } else if ch != '_' && !ch.is_ascii_digit() {
            return false;
        }
    }

    has_uppercase && !has_lowercase
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
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

fn span_contains(container: SourceSpan, candidate: SourceSpan) -> bool {
    position_key(container.start) <= position_key(candidate.start)
        && position_key(candidate.end) <= position_key(container.end)
}

type SpanKey = (usize, usize, usize, usize);

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests;
