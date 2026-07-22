use std::collections::BTreeMap;

use full_moon::ast::{Call, FunctionCall, Index, Prefix, Suffix, Var};
use full_moon::node::Node;
use full_moon::tokenizer::TokenReference;
use full_moon::visitors::Visitor;

use crate::hir::{lower_parsed_source, HirSymbolKind};
use crate::scope::{ScopeDeclarationKind, ScopeMap, ScopeResolution, ScopeSymbolId};
use crate::types::TypeEnvironment;
use crate::{LuaDocAnnotation, LuaDocAnnotationKind, ParsedSource, SourceSpan};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deprecation {
    pub guidance: Option<String>,
}

impl Deprecation {
    pub(crate) fn from_annotations(annotations: &[&LuaDocAnnotation]) -> Option<Self> {
        annotations.iter().find_map(|annotation| {
            matches!(annotation.kind, LuaDocAnnotationKind::Deprecated(_)).then(|| Self {
                guidance: annotation
                    .documentation
                    .as_deref()
                    .map(str::trim)
                    .filter(|guidance| !guidance.is_empty())
                    .map(str::to_owned),
            })
        })
    }

    pub(crate) fn markdown(&self) -> String {
        match self.guidance.as_deref() {
            Some(guidance) => format!("**Deprecated.** {guidance}"),
            None => "**Deprecated.**".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeprecatedDeclaration {
    pub path: Vec<String>,
    pub kind: HirSymbolKind,
    pub name_span: SourceSpan,
    pub origin_span: SourceSpan,
    pub deprecation: Deprecation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeprecatedApiUse {
    pub span: SourceSpan,
    pub name: String,
    pub deprecation: Deprecation,
}

pub(crate) fn deprecated_api_uses(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    scope: &ScopeMap,
) -> Vec<DeprecatedApiUse> {
    let mut collector = DeprecatedApiUseCollector {
        index: DeprecatedAccessIndex::new(parsed, environment, scope),
        uses: Vec::new(),
    };

    collector.visit_ast(&parsed.ast);
    collector.uses
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DeprecatedAccessRoot {
    Symbol(ScopeSymbolId),
    Global(String),
}

#[derive(Clone, Debug)]
struct DeprecatedAccess {
    parts: Vec<String>,
    root_span: SourceSpan,
    access_span: SourceSpan,
}

#[derive(Clone, Debug, Default)]
struct DeprecatedAccessIndex {
    entries: BTreeMap<(DeprecatedAccessRoot, Vec<String>), Deprecation>,
    reference_roots: BTreeMap<(usize, usize), DeprecatedAccessRoot>,
}

impl DeprecatedAccessIndex {
    fn new(parsed: &ParsedSource, environment: &TypeEnvironment, scope: &ScopeMap) -> Self {
        let mut index = Self {
            reference_roots: scope
                .references()
                .iter()
                .filter_map(|reference| {
                    deprecated_access_root(&reference.resolution).map(|root| {
                        (
                            (reference.span.start.line, reference.span.start.column),
                            root,
                        )
                    })
                })
                .collect(),
            ..Self::default()
        };

        for declaration in deprecated_declarations(parsed) {
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
                .map(|declaration| DeprecatedAccessRoot::Symbol(declaration.id))
                .or_else(|| {
                    scope.references().iter().find_map(|reference| {
                        (reference.name == *root_name
                            && span_contains(declaration.origin_span, reference.span))
                        .then(|| deprecated_access_root(&reference.resolution))
                        .flatten()
                    })
                })
                .or_else(|| {
                    scope
                        .globals()
                        .contains_key(root_name)
                        .then(|| DeprecatedAccessRoot::Global(root_name.clone()))
                });
            let Some(root) = root else {
                continue;
            };

            index
                .entries
                .insert((root, fields.to_vec()), declaration.deprecation);
        }

        for (path, deprecation) in environment.deprecated_accesses() {
            let Some((root_name, fields)) = path.split_first() else {
                continue;
            };

            for declaration in scope.declarations().iter().filter(|declaration| {
                declaration.name == *root_name
                    && matches!(declaration.kind, ScopeDeclarationKind::ModuleAlias { .. })
            }) {
                index.entries.insert(
                    (
                        DeprecatedAccessRoot::Symbol(declaration.id),
                        fields.to_vec(),
                    ),
                    deprecation.clone(),
                );
            }
        }

        index
    }

    fn get(&self, access: &DeprecatedAccess) -> Option<&Deprecation> {
        let root = self
            .reference_roots
            .get(&(access.root_span.start.line, access.root_span.start.column))?;

        self.entries
            .get(&(root.clone(), access.parts[1..].to_vec()))
    }
}

struct DeprecatedApiUseCollector {
    index: DeprecatedAccessIndex,
    uses: Vec<DeprecatedApiUse>,
}

impl DeprecatedApiUseCollector {
    fn record(&mut self, access: DeprecatedAccess) {
        let Some(deprecation) = self.index.get(&access).cloned() else {
            return;
        };

        self.uses.push(DeprecatedApiUse {
            span: access.access_span,
            name: access.parts.join("."),
            deprecation,
        });
    }
}

impl Visitor for DeprecatedApiUseCollector {
    fn visit_function_call(&mut self, call: &FunctionCall) {
        if let Some(access) = access_from_function_call(call) {
            self.record(access);
        }
    }

    fn visit_var(&mut self, var: &Var) {
        if let Some(access) = access_from_var(var) {
            self.record(access);
        }
    }
}

fn deprecated_access_root(resolution: &ScopeResolution) -> Option<DeprecatedAccessRoot> {
    match resolution {
        ScopeResolution::Symbol(id) => Some(DeprecatedAccessRoot::Symbol(*id)),
        ScopeResolution::Global(global) => Some(DeprecatedAccessRoot::Global(global.name.clone())),
        ScopeResolution::Unresolved => None,
    }
}

fn access_from_var(var: &Var) -> Option<DeprecatedAccess> {
    match var {
        Var::Name(name) => {
            let span = node_span(name)?;
            Some(DeprecatedAccess {
                parts: vec![token_text(name)],
                root_span: span,
                access_span: span,
            })
        }
        Var::Expression(expression) => {
            let Prefix::Name(root) = expression.prefix() else {
                return None;
            };
            let root_span = node_span(root)?;
            let mut access_span = root_span;
            let mut parts = vec![token_text(root)];

            for suffix in expression.suffixes() {
                let Suffix::Index(Index::Dot { name, .. }) = suffix else {
                    return None;
                };

                parts.push(token_text(name));
                access_span = node_span(name)?;
            }

            Some(DeprecatedAccess {
                parts,
                root_span,
                access_span,
            })
        }
        _ => None,
    }
}

fn access_from_function_call(call: &FunctionCall) -> Option<DeprecatedAccess> {
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

    Some(DeprecatedAccess {
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
    token.to_string().trim().to_owned()
}

fn span_contains(outer: SourceSpan, inner: SourceSpan) -> bool {
    let outer_start = (outer.start.line, outer.start.column);
    let outer_end = (outer.end.line, outer.end.column);
    let inner_start = (inner.start.line, inner.start.column);
    let inner_end = (inner.end.line, inner.end.column);

    outer_start <= inner_start && inner_end <= outer_end
}

pub(crate) fn deprecated_declarations(parsed: &ParsedSource) -> Vec<DeprecatedDeclaration> {
    lower_parsed_source(parsed)
        .symbols
        .into_iter()
        .filter(|symbol| {
            matches!(
                symbol.kind,
                HirSymbolKind::Class
                    | HirSymbolKind::TypeAlias
                    | HirSymbolKind::TableField
                    | HirSymbolKind::Local
                    | HirSymbolKind::Function
            ) && starts_at_first_code_column(parsed, symbol.origin_span)
        })
        .filter_map(|symbol| {
            let annotations = leading_lua_doc_annotations(parsed, symbol.origin_span.start.line);
            let deprecation = Deprecation::from_annotations(&annotations)?;
            let path = normalized_symbol_path(&symbol.name);

            (!path.is_empty()).then_some(DeprecatedDeclaration {
                path,
                kind: symbol.kind,
                name_span: symbol.name_span,
                origin_span: symbol.origin_span,
                deprecation,
            })
        })
        .collect()
}

fn starts_at_first_code_column(parsed: &ParsedSource, span: SourceSpan) -> bool {
    let Some(line) = parsed.source.lines().nth(span.start.line.saturating_sub(1)) else {
        return false;
    };
    let first_code_column = line
        .char_indices()
        .find_map(|(index, character)| (!character.is_whitespace()).then_some(index + 1))
        .unwrap_or(1);

    span.start.column == first_code_column
}

pub(crate) fn leading_lua_doc_annotations(
    parsed: &ParsedSource,
    line: usize,
) -> Vec<&LuaDocAnnotation> {
    if line <= 1 {
        return Vec::new();
    }

    let lines = parsed.source.lines().collect::<Vec<_>>();
    let mut current = line - 1;
    let mut first_doc_line = None;

    while current > 0 {
        let text = lines.get(current - 1).copied().unwrap_or("").trim_start();

        if text.is_empty() {
            current -= 1;
            continue;
        }

        if text.starts_with("---") {
            first_doc_line = Some(current);
            current -= 1;
            continue;
        }

        break;
    }

    let first = first_doc_line.unwrap_or(line);

    parsed
        .lua_doc_annotations
        .iter()
        .filter(|annotation| annotation.span.start.line >= first && annotation.span.end.line < line)
        .collect()
}

fn normalized_symbol_path(name: &str) -> Vec<String> {
    name.replace(':', ".")
        .split('.')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}
