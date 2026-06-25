use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::hir::{HirModule, HirSymbolId, HirSymbolKind};
use crate::module_graph::ModuleGraph;
use crate::module_index::ModuleExportField;
use crate::vfs::{module_name_for_path, normalize_path, ModulePathSeparator};
use crate::{SourcePosition, SourceSpan, WorkspaceSymbol, WorkspaceSymbolKind};

#[derive(Clone, Debug, Default)]
pub struct SymbolIndex {
    entries: Vec<SymbolIndexEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolIndexEntry {
    pub id: SymbolIndexId,
    pub name: String,
    pub kind: SymbolIndexKind,
    pub path: PathBuf,
    pub module_path: PathBuf,
    pub module_name: Option<String>,
    pub range: SourceSpan,
    pub selection_range: SourceSpan,
    pub container_name: Option<String>,
    pub export: Option<SymbolIndexExport>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SymbolIndexId {
    Module { path: PathBuf },
    Hir { path: PathBuf, symbol: HirSymbolId },
    Export { path: PathBuf, name: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolIndexKind {
    TypeAlias,
    Function,
    Module,
    Local,
    Parameter,
    Field,
    Export,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolIndexExport {
    pub public_name: String,
    pub field: ModuleExportField,
}

impl SymbolIndex {
    pub fn build<'a>(
        files: impl IntoIterator<Item = (&'a PathBuf, Option<&'a HirModule>)>,
        module_graph: &ModuleGraph,
        source_roots: &[PathBuf],
    ) -> Self {
        let mut entries = Vec::new();

        for (path, hir) in files {
            let path = normalize_path(path);
            let module_name = module_name_for_index(source_roots, &path);
            let module_span = hir
                .and_then(|hir| {
                    hir.symbols()
                        .iter()
                        .map(|symbol| symbol.origin_span)
                        .min_by_key(|span| position_key(span.start))
                })
                .unwrap_or_else(default_span);

            entries.push(SymbolIndexEntry {
                id: SymbolIndexId::Module { path: path.clone() },
                name: module_name.clone().unwrap_or_else(|| module_stem(&path)),
                kind: SymbolIndexKind::Module,
                path: path.clone(),
                module_path: path.clone(),
                module_name: None,
                range: module_span,
                selection_range: module_span,
                container_name: None,
                export: None,
            });

            if let Some(hir) = hir {
                for symbol in hir.symbols() {
                    entries.push(SymbolIndexEntry {
                        id: SymbolIndexId::Hir {
                            path: path.clone(),
                            symbol: symbol.id,
                        },
                        name: symbol.name.clone(),
                        kind: symbol_index_kind(symbol.kind),
                        path: path.clone(),
                        module_path: path.clone(),
                        module_name: module_name.clone(),
                        range: symbol.origin_span,
                        selection_range: symbol.name_span,
                        container_name: symbol
                            .parent
                            .and_then(|parent| hir.symbol(parent))
                            .map(|parent| parent.name.clone())
                            .or_else(|| module_name.clone()),
                        export: None,
                    });
                }
            }
        }

        for module in module_graph.modules() {
            let module_path = normalize_path(&module.path);
            let module_name = module_name_for_index(source_roots, &module_path);

            for export in &module.exports {
                entries.push(SymbolIndexEntry {
                    id: SymbolIndexId::Export {
                        path: module_path.clone(),
                        name: export.public_name.clone(),
                    },
                    name: export.public_name.clone(),
                    kind: SymbolIndexKind::Export,
                    path: module_path.clone(),
                    module_path: module_path.clone(),
                    module_name: module_name.clone(),
                    range: export.export_span,
                    selection_range: export.export_span,
                    container_name: module_name.clone(),
                    export: Some(SymbolIndexExport {
                        public_name: export.public_name.clone(),
                        field: export.field.clone(),
                    }),
                });
            }
        }

        entries.sort_by_key(symbol_entry_key);
        entries.dedup_by_key(|entry| entry.id.clone());

        Self { entries }
    }

    pub fn entries(&self) -> &[SymbolIndexEntry] {
        &self.entries
    }

    pub fn entries_for_path(&self, path: impl AsRef<Path>) -> Vec<&SymbolIndexEntry> {
        let path = normalize_path(path);

        self.entries
            .iter()
            .filter(|entry| entry.path == path)
            .collect()
    }

    pub fn workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let mut matches = self
            .entries
            .iter()
            .filter(|entry| entry.is_workspace_symbol())
            .filter_map(|entry| {
                symbol_score(entry, query).map(|score| ScoredSymbolEntry { score, entry })
            })
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| {
            left.score
                .cmp(&right.score)
                .then_with(|| left.entry.path.cmp(&right.entry.path))
                .then_with(|| {
                    position_key(left.entry.selection_range.start)
                        .cmp(&position_key(right.entry.selection_range.start))
                })
                .then_with(|| left.entry.name.cmp(&right.entry.name))
        });

        matches
            .into_iter()
            .map(|matched| matched.entry.to_workspace_symbol())
            .collect()
    }

    pub fn importable_symbols(&self, prefix: &str) -> Vec<&SymbolIndexEntry> {
        let mut symbols = self
            .entries
            .iter()
            .filter(|entry| entry.kind == SymbolIndexKind::Export)
            .filter(|entry| entry.name.starts_with(prefix))
            .filter(|entry| entry.export.is_some())
            .collect::<Vec<_>>();

        symbols.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.module_name.cmp(&right.module_name))
                .then_with(|| left.path.cmp(&right.path))
        });

        symbols
    }

    pub fn names_for_kind(&self, kind: SymbolIndexKind) -> BTreeSet<String> {
        self.entries
            .iter()
            .filter(|entry| entry.kind == kind)
            .map(|entry| entry.name.clone())
            .collect()
    }

    pub fn export_names_for_module_path(&self, path: impl AsRef<Path>) -> BTreeSet<String> {
        let path = normalize_path(path.as_ref());

        self.entries
            .iter()
            .filter(|entry| entry.kind == SymbolIndexKind::Export)
            .filter(|entry| entry.module_path == path)
            .map(|entry| entry.name.clone())
            .collect()
    }
}

impl SymbolIndexEntry {
    fn is_workspace_symbol(&self) -> bool {
        matches!(
            self.kind,
            SymbolIndexKind::Module
                | SymbolIndexKind::TypeAlias
                | SymbolIndexKind::Function
                | SymbolIndexKind::Export
        )
    }

    fn to_workspace_symbol(&self) -> WorkspaceSymbol {
        WorkspaceSymbol {
            name: self.name.clone(),
            kind: match self.kind {
                SymbolIndexKind::TypeAlias => WorkspaceSymbolKind::TypeAlias,
                SymbolIndexKind::Function => WorkspaceSymbolKind::Function,
                SymbolIndexKind::Module => WorkspaceSymbolKind::Module,
                SymbolIndexKind::Local => WorkspaceSymbolKind::Local,
                SymbolIndexKind::Parameter => WorkspaceSymbolKind::Parameter,
                SymbolIndexKind::Field => WorkspaceSymbolKind::Field,
                SymbolIndexKind::Export => WorkspaceSymbolKind::Export,
            },
            path: self.path.clone(),
            range: self.range,
            selection_range: self.selection_range,
            container_name: self.container_name.clone(),
        }
    }
}

struct ScoredSymbolEntry<'a> {
    score: usize,
    entry: &'a SymbolIndexEntry,
}

fn symbol_index_kind(kind: HirSymbolKind) -> SymbolIndexKind {
    match kind {
        HirSymbolKind::TypeAlias => SymbolIndexKind::TypeAlias,
        HirSymbolKind::TableField => SymbolIndexKind::Field,
        HirSymbolKind::Local => SymbolIndexKind::Local,
        HirSymbolKind::Function => SymbolIndexKind::Function,
        HirSymbolKind::Parameter => SymbolIndexKind::Parameter,
    }
}

fn symbol_score(symbol: &SymbolIndexEntry, query: &str) -> Option<usize> {
    let query = query.trim();

    if query.is_empty() {
        return Some(0);
    }

    let qualified = symbol
        .container_name
        .as_ref()
        .map(|container| format!("{container}.{}", symbol.name));

    fuzzy_score(query, &symbol.name).or_else(|| {
        qualified
            .as_deref()
            .and_then(|qualified| fuzzy_score(query, qualified).map(|score| score + 25))
    })
}

fn fuzzy_score(query: &str, candidate: &str) -> Option<usize> {
    let query = query.to_ascii_lowercase();
    let candidate = candidate.to_ascii_lowercase();

    if let Some(index) = candidate.find(&query) {
        return Some(index);
    }

    let mut last_match = None;
    let mut score = 100usize;
    let mut search_start = 0usize;

    for query_char in query.chars() {
        let (offset, _) = candidate[search_start..]
            .char_indices()
            .find(|(_, candidate_char)| *candidate_char == query_char)?;
        let index = search_start + offset;

        if let Some(last_match) = last_match {
            score += index.saturating_sub(last_match + 1);
        } else {
            score += index;
        }

        last_match = Some(index);
        search_start = index + query_char.len_utf8();
    }

    Some(score)
}

fn symbol_entry_key(entry: &SymbolIndexEntry) -> (PathBuf, u8, usize, usize, String) {
    (
        entry.path.clone(),
        symbol_kind_key(entry.kind),
        entry.selection_range.start.line,
        entry.selection_range.start.column,
        entry.name.clone(),
    )
}

fn symbol_kind_key(kind: SymbolIndexKind) -> u8 {
    match kind {
        SymbolIndexKind::Module => 0,
        SymbolIndexKind::TypeAlias => 1,
        SymbolIndexKind::Function => 2,
        SymbolIndexKind::Local => 3,
        SymbolIndexKind::Parameter => 4,
        SymbolIndexKind::Field => 5,
        SymbolIndexKind::Export => 6,
    }
}

fn module_name_for_index(source_roots: &[PathBuf], path: &Path) -> Option<String> {
    source_roots
        .iter()
        .filter(|root| path.starts_with(root))
        .max_by_key(|root| root.components().count())
        .and_then(|root| module_name_for_path(root, path, ModulePathSeparator::Dot))
}

fn module_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("module")
        .to_owned()
}

fn default_span() -> SourceSpan {
    SourceSpan {
        start: SourcePosition { line: 1, column: 1 },
        end: SourcePosition { line: 1, column: 1 },
    }
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests;
