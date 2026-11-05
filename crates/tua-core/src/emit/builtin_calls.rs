//! Lower callable helpers while their AST tokens retain lexical identity.
use std::collections::BTreeMap;

use full_moon::ast::{
    self,
    punctuated::{Pair, Punctuated},
    span::ContainedSpan,
    Call, Expression, FunctionArgs, Prefix, Suffix,
};
use full_moon::tokenizer::{StringLiteralQuoteType, Token, TokenReference, TokenType};
use full_moon::visitors::VisitorMut;

use crate::scope::{scope_map_parsed_source, ScopeGlobalKind, ScopeMap, ScopeResolution};
use crate::{
    asset, audio, input, object_pool, signal, state_machine, store, vector, ParsedSource,
    SourcePosition, SourceSpan,
};

pub(super) fn lower(
    ast: ast::Ast,
    parsed: &ParsedSource,
) -> (ast::Ast, BTreeMap<String, SourceSpan>) {
    lower_with_outer_scope(ast, parsed, None)
}

pub(super) fn lower_with_outer_scope(
    ast: ast::Ast,
    parsed: &ParsedSource,
    outer_scope: Option<(&ScopeMap, SourcePosition)>,
) -> (ast::Ast, BTreeMap<String, SourceSpan>) {
    let mut lowerer = Lowerer {
        parsed,
        scope: scope_map_parsed_source(parsed),
        outer_scope,
        used: BTreeMap::new(),
    };
    let ast = lowerer.visit_ast(ast);
    (ast, lowerer.used)
}

struct Lowerer<'a> {
    parsed: &'a ParsedSource,
    scope: ScopeMap,
    outer_scope: Option<(&'a ScopeMap, SourcePosition)>,
    used: BTreeMap<String, SourceSpan>,
}

pub(super) fn runtime_callee(name: &str) -> Option<&'static str> {
    [
        (vector::CONSTRUCTOR_NAME, vector::EMITTED_CONSTRUCTOR_LOCAL),
        (signal::CONSTRUCTOR_NAME, signal::EMITTED_CONSTRUCTOR_LOCAL),
        (
            object_pool::CONSTRUCTOR_NAME,
            object_pool::EMITTED_CONSTRUCTOR_LOCAL,
        ),
        (
            state_machine::CONSTRUCTOR_NAME,
            state_machine::EMITTED_CONSTRUCTOR_LOCAL,
        ),
        (store::CONSTRUCTOR_NAME, store::EMITTED_CONSTRUCTOR_LOCAL),
        (input::CONSTRUCTOR_NAME, input::EMITTED_CONSTRUCTOR_LOCAL),
        (audio::CONSTRUCTOR_NAME, audio::EMITTED_CONSTRUCTOR_LOCAL),
    ]
    .into_iter()
    .find_map(|(constructor, callee)| (constructor == name).then_some(callee))
}

impl VisitorMut for Lowerer<'_> {
    fn visit_function_call_end(&mut self, call: ast::FunctionCall) -> ast::FunctionCall {
        let Prefix::Name(name) = call.prefix() else {
            return call;
        };
        let original = name.token().to_string();
        let asset_kind = asset::loader_kind(&original);
        let Some(callee) = asset_kind
            .map(|kind| kind.emitted_callee())
            .or_else(|| runtime_callee(&original))
        else {
            return call;
        };
        let Some(Suffix::Call(Call::AnonymousCall(args))) = call.suffixes().next() else {
            return call;
        };
        let position = SourcePosition::from_full_moon(name.start_position());
        let synthetic_vector =
            original == vector::CONSTRUCTOR_NAME && name.start_position().line() == 0;
        if !synthetic_vector {
            let Some(reference) =
                self.scope.references().iter().find(|reference| {
                    reference.name == original && reference.span.start == position
                })
            else {
                return call;
            };
            if !matches!(
                &reference.resolution,
                ScopeResolution::Unresolved
                    | ScopeResolution::Global(crate::scope::ScopeGlobal {
                        kind: ScopeGlobalKind::LuaBuiltin,
                        ..
                    })
            ) {
                return call;
            }
            if self
                .outer_scope
                .is_some_and(|(scope, position)| shadowed_at(scope, &original, position))
            {
                return call;
            }
        }
        let string_sugar = matches!(args, FunctionArgs::String(_));
        let args = match args {
            FunctionArgs::Parentheses { .. } => args.clone(),
            FunctionArgs::String(text)
                if asset_kind.is_some_and(|kind| kind.supports_string_sugar()) =>
            {
                parentheses_with_trailing(
                    [Expression::String(with_trailing(text, Vec::new()))],
                    text.trailing_trivia().cloned().collect(),
                )
            }
            _ => return call,
        };
        let args =
            if let Some(kind @ (asset::AssetLoaderKind::Sound | asset::AssetLoaderKind::Music)) =
                asset_kind
            {
                let FunctionArgs::Parentheses { arguments, .. } = &args else {
                    return call;
                };
                let Some(first) = arguments.iter().next() else {
                    return call;
                };
                let mode = if kind == asset::AssetLoaderKind::Sound {
                    "static"
                } else {
                    "stream"
                };
                parentheses_with_trailing(
                    [
                        first.clone(),
                        Expression::String(TokenReference::new(
                            Vec::new(),
                            Token::new(TokenType::StringLiteral {
                                literal: mode.into(),
                                multi_line_depth: 0,
                                quote_type: StringLiteralQuoteType::Double,
                            }),
                            Vec::new(),
                        )),
                    ],
                    match &args {
                        FunctionArgs::Parentheses { parentheses, .. } => {
                            parentheses.tokens().1.trailing_trivia().cloned().collect()
                        }
                        _ => Vec::new(),
                    },
                )
            } else {
                args
            };
        let source_span = self
            .parsed
            .raw
            .parser_provenance
            .get(position.line.saturating_sub(1))
            .map(|origin| origin.source_span)
            .or(self.parsed.span)
            .unwrap_or(SourceSpan {
                start: position,
                end: position,
            });
        self.used.entry(original).or_insert(source_span);
        let mut pieces = callee.split('.').peekable();
        let root_name = pieces.next().unwrap();
        let mut trailing = name.trailing_trivia().cloned().collect::<Vec<_>>();
        // Parentheses replace the plain separator of string-call sugar. Keep
        // comments and line breaks, which carry documentation and provenance.
        if string_sugar
            && trailing
                .iter()
                .all(|token| token.to_string().chars().all(|ch| ch == ' ' || ch == '\t'))
        {
            trailing.clear();
        }
        let root_trailing = if pieces.peek().is_none() {
            trailing.clone()
        } else {
            Vec::new()
        };
        let root = TokenReference::new(
            name.leading_trivia().cloned().collect(),
            Token::new(TokenType::Identifier {
                identifier: root_name.into(),
            }),
            root_trailing,
        );
        let mut suffixes = pieces
            .map(|piece| {
                Suffix::Index(ast::Index::Dot {
                    dot: TokenReference::symbol(".").unwrap(),
                    name: identifier(piece),
                })
            })
            .collect::<Vec<_>>();
        if let Some(Suffix::Index(ast::Index::Dot { name, .. })) = suffixes.last_mut() {
            *name = with_trailing(name, trailing);
        }
        suffixes.push(Suffix::Call(Call::AnonymousCall(args)));
        suffixes.extend(call.suffixes().skip(1).cloned());
        call.with_prefix(Prefix::Name(root)).with_suffixes(suffixes)
    }
}

fn identifier(name: &str) -> TokenReference {
    TokenReference::new(
        Vec::new(),
        Token::new(TokenType::Identifier {
            identifier: name.into(),
        }),
        Vec::new(),
    )
}

fn parentheses_with_trailing(
    arguments: impl IntoIterator<Item = Expression>,
    trailing: Vec<Token>,
) -> FunctionArgs {
    let mut arguments = arguments.into_iter().peekable();
    let mut values = Punctuated::new();
    while let Some(value) = arguments.next() {
        values.push(Pair::new(
            value,
            arguments
                .peek()
                .map(|_| TokenReference::symbol(", ").unwrap()),
        ));
    }
    FunctionArgs::Parentheses {
        parentheses: ContainedSpan::new(
            TokenReference::symbol("(").unwrap(),
            with_trailing(&TokenReference::symbol(")").unwrap(), trailing),
        ),
        arguments: values,
    }
}

// Resolve inherited bindings for a separately parsed default guard. The guard
// owns its inner lexical scopes; this position is the enclosing parameter
// default's original parser position.
pub(super) fn shadowed_at(scope: &ScopeMap, name: &str, position: SourcePosition) -> bool {
    let key = |position: SourcePosition| (position.line, position.column);
    scope.declarations().iter().any(|declaration| {
        declaration.name == name
            && !matches!(
                declaration.kind,
                crate::scope::ScopeDeclarationKind::TableField
            )
            && key(declaration.scope_span.start) <= key(position)
            && key(position) <= key(declaration.scope_span.end)
            // Defaults execute in the function body, where every parameter is
            // already bound, including parameters later in the header. Ordinary
            // outer locals still become visible only after their declaration.
            && (declaration.kind == crate::scope::ScopeDeclarationKind::Parameter
                || key(declaration.name_span.start) <= key(position))
    }) || scope
        .globals()
        .get(name)
        .is_some_and(|global| global.kind == ScopeGlobalKind::Assigned)
}

fn with_trailing(token: &TokenReference, trailing: Vec<Token>) -> TokenReference {
    TokenReference::new(
        token.leading_trivia().cloned().collect(),
        token.token().clone(),
        trailing,
    )
}
