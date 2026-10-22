use std::collections::{BTreeMap, BTreeSet};

use full_moon::ast::{Call, FunctionCall, Index, Prefix, Stmt, Suffix};
use full_moon::node::Node;
use full_moon::tokenizer::TokenReference;
use full_moon::visitors::Visitor;

use crate::deprecation::{
    leading_lua_doc_annotations, normalized_symbol_path, starts_at_first_code_column,
};
use crate::hir::{lower_parsed_source, HirSymbolKind};
use crate::scope::{ScopeDeclarationKind, ScopeMap, ScopeResolution, ScopeSymbolId};
use crate::types::TypeEnvironment;
use crate::{LuaDocAnnotationKind, ParsedSource, SourceSpan};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MustUseCall {
    pub span: SourceSpan,
    pub name: String,
}

#[derive(Clone, Debug)]
struct MustUseDeclaration {
    path: Vec<String>,
    kind: HirSymbolKind,
    name_span: SourceSpan,
    origin_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum AccessRoot {
    Symbol(ScopeSymbolId),
    Global(String),
}

#[derive(Clone, Debug)]
struct Access {
    parts: Vec<String>,
    root_span: SourceSpan,
    access_span: SourceSpan,
}

#[derive(Clone, Debug, Default)]
struct AccessIndex {
    entries: BTreeSet<(AccessRoot, Vec<String>)>,
    reference_roots: BTreeMap<(usize, usize), AccessRoot>,
}

impl AccessIndex {
    fn new(parsed: &ParsedSource, environment: &TypeEnvironment, scope: &ScopeMap) -> Self {
        let mut index = Self {
            reference_roots: scope
                .references()
                .iter()
                .filter_map(|reference| {
                    access_root(&reference.resolution).map(|root| {
                        (
                            (reference.span.start.line, reference.span.start.column),
                            root,
                        )
                    })
                })
                .collect(),
            ..Self::default()
        };

        for declaration in must_use_declarations(parsed) {
            if !matches!(
                declaration.kind,
                HirSymbolKind::Local | HirSymbolKind::Function
            ) {
                continue;
            }

            let Some((root_name, fields)) = declaration.path.split_first() else {
                continue;
            };
            let root = scope
                .declaration_at_span(declaration.name_span)
                .map(|declaration| AccessRoot::Symbol(declaration.id))
                .or_else(|| {
                    scope.references().iter().find_map(|reference| {
                        (reference.name == *root_name
                            && span_contains(declaration.origin_span, reference.span))
                        .then(|| access_root(&reference.resolution))
                        .flatten()
                    })
                })
                .or_else(|| {
                    scope
                        .globals()
                        .contains_key(root_name)
                        .then(|| AccessRoot::Global(root_name.clone()))
                });
            let Some(root) = root else {
                continue;
            };

            index.entries.insert((root, fields.to_vec()));
        }

        for path in environment.must_use_accesses() {
            let Some((root_name, fields)) = path.split_first() else {
                continue;
            };

            for declaration in scope.declarations().iter().filter(|declaration| {
                declaration.name == *root_name
                    && matches!(declaration.kind, ScopeDeclarationKind::ModuleAlias { .. })
            }) {
                index
                    .entries
                    .insert((AccessRoot::Symbol(declaration.id), fields.to_vec()));
            }
        }

        index
    }

    fn contains(&self, access: &Access) -> bool {
        let Some(root) = self
            .reference_roots
            .get(&(access.root_span.start.line, access.root_span.start.column))
        else {
            return false;
        };

        self.entries
            .contains(&(root.clone(), access.parts[1..].to_vec()))
    }
}

struct Collector {
    index: AccessIndex,
    calls: Vec<MustUseCall>,
}

impl Visitor for Collector {
    fn visit_stmt(&mut self, statement: &Stmt) {
        let Stmt::FunctionCall(call) = statement else {
            return;
        };
        let Some(access) = access_from_function_call(call) else {
            return;
        };
        if self.index.contains(&access) {
            self.calls.push(MustUseCall {
                span: access.access_span,
                name: access.parts.join("."),
            });
        }
    }
}

pub(crate) fn discarded_must_use_calls(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    scope: &ScopeMap,
) -> Vec<MustUseCall> {
    let mut collector = Collector {
        index: AccessIndex::new(parsed, environment, scope),
        calls: Vec::new(),
    };
    collector.visit_ast(&parsed.ast);
    collector.calls
}

pub(crate) fn must_use_paths(parsed: &ParsedSource) -> BTreeSet<Vec<String>> {
    must_use_declarations(parsed)
        .into_iter()
        .map(|declaration| declaration.path)
        .collect()
}

fn must_use_declarations(parsed: &ParsedSource) -> Vec<MustUseDeclaration> {
    lower_parsed_source(parsed)
        .symbols
        .into_iter()
        .filter(|symbol| {
            matches!(
                symbol.kind,
                HirSymbolKind::TableField | HirSymbolKind::Local | HirSymbolKind::Function
            ) && starts_at_first_code_column(parsed, symbol.origin_span)
        })
        .filter_map(|symbol| {
            let annotations = leading_lua_doc_annotations(parsed, symbol.origin_span.start.line);
            let marked = annotations
                .iter()
                .any(|annotation| matches!(annotation.kind, LuaDocAnnotationKind::NoDiscard(_)));
            let path = normalized_symbol_path(&symbol.name);

            (marked && !path.is_empty()).then_some(MustUseDeclaration {
                path,
                kind: symbol.kind,
                name_span: symbol.name_span,
                origin_span: symbol.origin_span,
            })
        })
        .collect()
}

fn access_root(resolution: &ScopeResolution) -> Option<AccessRoot> {
    match resolution {
        ScopeResolution::Symbol(id) => Some(AccessRoot::Symbol(*id)),
        ScopeResolution::Global(global) => Some(AccessRoot::Global(global.name.clone())),
        ScopeResolution::Unresolved => None,
    }
}

fn access_from_function_call(call: &FunctionCall) -> Option<Access> {
    let Prefix::Name(root) = call.prefix() else {
        return None;
    };
    let root_span = node_span(root)?;
    let mut access_span = root_span;
    let mut parts = vec![token_text(root)];

    for suffix in call.suffixes() {
        match suffix {
            Suffix::Index(Index::Dot { name, .. }) => {
                parts.push(token_text(name));
                access_span = node_span(name)?;
            }
            Suffix::Call(Call::MethodCall(method)) => {
                parts.push(token_text(method.name()));
                access_span = node_span(method.name())?;
                break;
            }
            Suffix::Call(Call::AnonymousCall(_)) => break,
            _ => return None,
        }
    }

    Some(Access {
        parts,
        root_span,
        access_span,
    })
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn token_text(token: &TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

fn span_contains(outer: SourceSpan, inner: SourceSpan) -> bool {
    let outer_start = (outer.start.line, outer.start.column);
    let outer_end = (outer.end.line, outer.end.column);
    let inner_start = (inner.start.line, inner.start.column);
    let inner_end = (inner.end.line, inner.end.column);

    outer_start <= inner_start && inner_end <= outer_end
}
