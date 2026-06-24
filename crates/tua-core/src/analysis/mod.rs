use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::completion::complete_field_access_in_source_with_environment;
use crate::folding_range::folding_ranges_source;
use crate::hir::{HirModule, HirRequireBinding};
use crate::ide_db::SymbolIndexEntry;
use crate::module_link::require_module_at_position;
use crate::scope_map_parsed_source;
use crate::selection_range::selection_ranges_source;
use crate::semantic::SemanticModel;
use crate::source_text::{dotted_chain_at_position, line_at};
use crate::types::build_type_environment_with_diagnostics;
use crate::vfs::{normalize_path, VfsSnapshot, VfsSourceKind, VirtualFileSystem};
use crate::{
    assists_parsed_source, build_type_environment,
    check_parsed_source_with_asset_roots_and_diagnostics_config, complete_parsed_source,
    complete_tua_source, definition_parsed_source, document_highlights_parsed_source,
    document_symbols_parsed_source, emit_lua, exported_field_for_fields,
    global_function_definition_count, global_function_name_for_definition,
    global_references_to_definition_parsed_source, global_type_alias_name_for_definition,
    global_type_alias_references_to_definition_parsed_source, hover_parsed_source,
    infer_module_exports, inlay_hints_parsed_source, is_valid_rename_identifier,
    lower_parsed_source, module_links_tua_source, parse_lua_source, parse_tua_source,
    reference_kind_for_rename, references_to_definition_parsed_source,
    rename_target_for_definition, require_bindings_for_parsed_source,
    semantic_tokens_parsed_source, semantic_tokens_tua_source, signature_help_parsed_source,
    signature_help_tua_source, type_alias_definition_count, Assist, AssistRange, Completion,
    CompletionKind, Definition, DefinitionKind, Diagnostic, DocumentHighlight, DocumentSymbol,
    FoldingRange, Hover, InlayHint, ModuleExportField, ModuleExports, ModuleGraph,
    ModuleGraphInput, ModuleGraphOptions, ModuleLink, ParsedSource, ProjectConfig,
    ProjectTypeDatabase, ProjectTypeDatabaseInput, Reference, RenameEdit, RenameTarget,
    RequireResolution, ScopeMap, SelectionRange, SemanticToken, SignatureHelp, SourcePosition,
    SourceSpan, SymbolIndex, TableField, TextEdit, TuaType, Type, TypeEnvironment, WorkspaceSymbol,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltinCatalogs {
    pub lua: bool,
    pub love: bool,
}

impl Default for BuiltinCatalogs {
    fn default() -> Self {
        Self {
            lua: true,
            love: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisSourceKind {
    Tua,
    Lua,
}

#[derive(Clone, Debug, Default)]
pub struct AnalysisHost {
    config: ProjectConfig,
    vfs: VirtualFileSystem,
    asset_roots: Vec<PathBuf>,
    builtin_catalogs: BuiltinCatalogs,
    cache: RefCell<AnalysisQueryCache>,
}

#[derive(Clone, Debug)]
pub struct AnalysisSnapshot {
    config: ProjectConfig,
    vfs: VfsSnapshot,
    asset_roots: Vec<PathBuf>,
    files: BTreeMap<PathBuf, AnalysisFile>,
    builtin_catalogs: BuiltinCatalogs,
    module_graph: ModuleGraph,
    type_database: ProjectTypeDatabase,
    symbol_index: SymbolIndex,
}

#[derive(Clone, Debug)]
struct AnalysisFile {
    source: String,
    kind: AnalysisSourceKind,
    parsed: Result<ParsedSource, Vec<Diagnostic>>,
    hir: Option<HirModule>,
    scope: Option<ScopeMap>,
    environment: Option<TypeEnvironment>,
    diagnostics: Vec<Diagnostic>,
    exports: Option<ModuleExports>,
    require_edges: Vec<ModuleLink>,
    #[allow(dead_code)]
    symbols: Vec<SymbolIndexEntry>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnalysisCacheStats {
    pub parse_hits: usize,
    pub parse_misses: usize,
    pub type_environment_hits: usize,
    pub type_environment_misses: usize,
    pub diagnostics_hits: usize,
    pub diagnostics_misses: usize,
    pub module_export_hits: usize,
    pub module_export_misses: usize,
    pub require_edge_hits: usize,
    pub require_edge_misses: usize,
    pub symbol_index_hits: usize,
    pub symbol_index_misses: usize,
    pub invalidations: usize,
    pub dependent_invalidations: usize,
}

#[derive(Clone, Debug, Default)]
struct AnalysisQueryCache {
    files: BTreeMap<PathBuf, CachedAnalysisFile>,
    dependencies: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
    importers: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
    stats: AnalysisCacheStats,
}

#[derive(Clone, Debug)]
struct CachedAnalysisFile {
    source: String,
    kind: AnalysisSourceKind,
    file: AnalysisFile,
}

impl AnalysisHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: ProjectConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    pub fn config(&self) -> &ProjectConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: ProjectConfig) {
        if self.config == config {
            return;
        }

        self.config = config;
        self.cache.borrow_mut().invalidate_all();
    }

    pub fn source_roots(&self) -> &[PathBuf] {
        &self.vfs.options().source_roots
    }

    pub fn set_source_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        let roots = roots.into_iter().map(normalize_path).collect::<Vec<_>>();

        if self.vfs.options().source_roots == roots {
            return;
        }

        self.vfs.set_source_roots(roots);
        self.cache.borrow_mut().invalidate_all();
    }

    pub fn set_output_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        let roots = roots.into_iter().map(normalize_path).collect::<Vec<_>>();

        if self.vfs.options().output_roots == roots {
            return;
        }

        self.vfs.set_output_roots(roots);
        self.cache.borrow_mut().invalidate_all();
    }

    pub fn asset_roots(&self) -> &[PathBuf] {
        &self.asset_roots
    }

    pub fn set_asset_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        let roots = roots.into_iter().map(normalize_path).collect::<Vec<_>>();

        if self.asset_roots == roots {
            return;
        }

        self.asset_roots = roots;
        self.cache.borrow_mut().invalidate_all();
    }

    pub fn set_ignored_paths(&mut self, patterns: impl IntoIterator<Item = String>) {
        let patterns = patterns.into_iter().collect::<Vec<_>>();

        if self.vfs.options().ignored_paths == patterns {
            return;
        }

        self.vfs.set_ignored_paths(patterns);
        self.cache.borrow_mut().invalidate_all();
    }

    pub fn builtin_catalogs(&self) -> &BuiltinCatalogs {
        &self.builtin_catalogs
    }

    pub fn set_builtin_catalogs(&mut self, catalogs: BuiltinCatalogs) {
        self.builtin_catalogs = catalogs;
        self.cache.borrow_mut().invalidate_all();
    }

    pub fn set_file(&mut self, path: impl Into<PathBuf>, source: impl Into<String>) {
        let path = path.into();
        let kind = infer_source_kind(&path, &self.config);

        self.cache.borrow_mut().invalidate_file(&path);
        self.vfs.set_disk_file(path, source, vfs_source_kind(kind));
    }

    pub fn set_file_with_kind(
        &mut self,
        path: impl Into<PathBuf>,
        source: impl Into<String>,
        kind: AnalysisSourceKind,
    ) {
        let path = path.into();

        self.cache.borrow_mut().invalidate_file(&path);
        self.vfs.set_disk_file(path, source, vfs_source_kind(kind));
    }

    pub fn remove_file(&mut self, path: impl AsRef<Path>) {
        self.cache.borrow_mut().invalidate_file(path.as_ref());
        self.vfs.remove_file(path);
    }

    pub fn open_document(&mut self, path: impl Into<PathBuf>, source: impl Into<String>) {
        let path = path.into();
        let kind = infer_source_kind(&path, &self.config);

        self.cache.borrow_mut().invalidate_file(&path);
        self.vfs.open_file(path, source, vfs_source_kind(kind));
    }

    pub fn open_document_with_kind(
        &mut self,
        path: impl Into<PathBuf>,
        source: impl Into<String>,
        kind: AnalysisSourceKind,
    ) {
        let path = path.into();

        self.cache.borrow_mut().invalidate_file(&path);
        self.vfs.open_file(path, source, vfs_source_kind(kind));
    }

    pub fn update_open_document(&mut self, path: impl Into<PathBuf>, source: impl Into<String>) {
        self.open_document(path, source);
    }

    pub fn close_document(&mut self, path: impl AsRef<Path>) {
        self.cache.borrow_mut().invalidate_file(path.as_ref());
        self.vfs.close_file(path);
    }

    pub fn snapshot(&self) -> AnalysisSnapshot {
        let vfs = self.vfs.snapshot();
        let source_roots = vfs.source_roots().to_vec();
        let live_paths = vfs
            .files()
            .map(|file| file.path().to_path_buf())
            .collect::<BTreeSet<_>>();
        let mut cache = self.cache.borrow_mut();

        cache.retain_paths(&live_paths);

        let files = vfs
            .files()
            .map(|file| {
                let analysis_file = cache.cached_file(
                    file.path(),
                    file.source(),
                    analysis_source_kind(file.kind()),
                    &self.config,
                    &source_roots,
                    &self.asset_roots,
                );

                (file.path().to_path_buf(), analysis_file)
            })
            .collect::<BTreeMap<_, _>>();
        let module_graph = ModuleGraph::build(
            files.iter().map(|(path, file)| ModuleGraphInput {
                path,
                parsed: file.parsed.as_ref().ok(),
                hir: file.hir.as_ref(),
                exports: file.exports.as_ref(),
                source_kind: file.kind,
            }),
            ModuleGraphOptions {
                source_roots: vfs.source_roots().to_vec(),
                output_roots: vfs.options().output_roots.clone(),
                extension: self.config.extension.clone(),
            },
        );
        let type_database = ProjectTypeDatabase::build(
            files.iter().map(|(path, file)| ProjectTypeDatabaseInput {
                path,
                source_kind: file.kind,
                environment: file.environment.as_ref(),
                scope: file.scope.as_ref(),
            }),
            &module_graph,
        );
        let symbol_index = SymbolIndex::build(
            files.iter().map(|(path, file)| (path, file.hir.as_ref())),
            &module_graph,
            vfs.source_roots(),
        );

        AnalysisSnapshot {
            config: self.config.clone(),
            vfs,
            asset_roots: self.asset_roots.clone(),
            files,
            builtin_catalogs: self.builtin_catalogs.clone(),
            module_graph,
            type_database,
            symbol_index,
        }
    }

    pub fn cache_stats(&self) -> AnalysisCacheStats {
        self.cache.borrow().stats.clone()
    }
}

impl AnalysisSnapshot {
    pub fn config(&self) -> &ProjectConfig {
        &self.config
    }

    pub fn source_roots(&self) -> &[PathBuf] {
        self.vfs.source_roots()
    }

    pub fn asset_roots(&self) -> &[PathBuf] {
        &self.asset_roots
    }

    pub fn builtin_catalogs(&self) -> &BuiltinCatalogs {
        &self.builtin_catalogs
    }

    pub fn is_open_document(&self, path: impl AsRef<Path>) -> bool {
        self.vfs.is_open(path)
    }

    pub fn source(&self, path: impl AsRef<Path>) -> Option<&str> {
        self.file(path.as_ref()).map(|file| file.source.as_str())
    }

    pub fn parse(&self, path: impl AsRef<Path>) -> Option<Result<&ParsedSource, &[Diagnostic]>> {
        self.file(path.as_ref())
            .map(|file| file.parsed.as_ref().map_err(Vec::as_slice))
    }

    pub fn diagnostics(&self, path: impl AsRef<Path>) -> Vec<Diagnostic> {
        self.pipeline_diagnostics(path)
    }

    pub fn pipeline_diagnostics(&self, path: impl AsRef<Path>) -> Vec<Diagnostic> {
        let Some(file) = self.file(path.as_ref()) else {
            return Vec::new();
        };
        let mut diagnostics = file.diagnostics.clone();

        diagnostics.extend(
            self.module_graph
                .diagnostics(path.as_ref())
                .iter()
                .filter(|diagnostic| self.project_diagnostic_enabled(diagnostic))
                .cloned(),
        );

        self.project_aware_diagnostics(path.as_ref(), deduplicate_diagnostics(diagnostics))
    }

    pub fn hover(&self, path: impl AsRef<Path>, position: SourcePosition) -> Option<Hover> {
        let path = path.as_ref();
        let file = self.file(path)?;

        match &file.parsed {
            Ok(parsed) => self.hover_parsed_snapshot(parsed, position),
            Err(_) => {
                let recovered = source_without_incomplete_field_access_lines(&file.source);
                let parsed = parse_analysis_source(path, recovered, file.kind).ok()?;

                self.hover_parsed_snapshot(&parsed, position)
            }
        }
    }

    fn hover_parsed_snapshot(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Hover> {
        if let Some(hover) = self.project_type_hover(parsed, position) {
            return Some(hover);
        }

        if let Some(hover) = self.module_hover(parsed, position) {
            return Some(hover);
        }

        if self.is_module_reference(parsed, position) {
            return None;
        }

        if let Some(hover) = self.project_annotated_local_hover(parsed, position) {
            return Some(hover);
        }

        hover_parsed_source(parsed, position)
            .ok()
            .flatten()
            .or_else(|| self.project_symbol_hover(parsed, position))
    }

    pub fn completion(&self, path: impl AsRef<Path>, position: SourcePosition) -> Vec<Completion> {
        let path = path.as_ref();
        let Some(file) = self.file(path) else {
            return Vec::new();
        };

        match &file.parsed {
            Ok(parsed) => {
                let completions = self.project_completions(parsed, &file.source, position);

                if !completions.is_empty() {
                    return completions;
                }

                complete_parsed_source(parsed, position).unwrap_or_default()
            }
            Err(_) => {
                let recovered = source_without_incomplete_field_access_lines(&file.source);

                if let Ok(parsed) = parse_analysis_source(path, recovered, file.kind) {
                    let completions = self.project_completions(&parsed, &file.source, position);

                    if !completions.is_empty() {
                        return completions;
                    }
                }

                complete_tua_source(path, file.source.clone(), position).unwrap_or_default()
            }
        }
    }

    pub fn definition(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Option<Definition> {
        let path = path.as_ref();
        let file = self.file(path)?;

        match &file.parsed {
            Ok(parsed) => self.definition_parsed_snapshot(parsed, position),
            Err(_) => {
                let recovered = source_without_incomplete_field_access_lines(&file.source);
                let parsed = parse_analysis_source(path, recovered, file.kind).ok()?;

                self.definition_parsed_snapshot(&parsed, position)
            }
        }
    }

    fn definition_parsed_snapshot(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Definition> {
        if let Some(definition) = self.module_definition(parsed, position) {
            return Some(definition);
        }

        if self.is_module_reference(parsed, position) {
            return None;
        }

        definition_parsed_source(parsed, position).ok().flatten()
    }

    pub fn resolve_completion(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
        label: &str,
        kind: Option<CompletionKind>,
        module_path: Option<&Path>,
        source_module: Option<&str>,
    ) -> Option<Completion> {
        self.completion(path, position)
            .into_iter()
            .find(|completion| {
                completion.label == label
                    && kind.is_none_or(|kind| completion.kind == kind)
                    && module_path
                        .is_none_or(|path| completion.module_path.as_deref() == Some(path))
                    && source_module
                        .is_none_or(|module| completion.source_module.as_deref() == Some(module))
            })
    }

    pub fn signature_help(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Option<SignatureHelp> {
        let path = path.as_ref();
        let file = self.file(path)?;

        match &file.parsed {
            Ok(parsed) => signature_help_parsed_source(parsed, position)
                .ok()
                .flatten(),
            Err(_) => signature_help_tua_source(path, file.source.clone(), position)
                .ok()
                .flatten(),
        }
    }

    pub fn document_links(&self, path: impl AsRef<Path>) -> Vec<ModuleLink> {
        self.file(path.as_ref())
            .map(|file| file.require_edges.clone())
            .unwrap_or_default()
    }

    pub fn document_highlights(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Vec<DocumentHighlight> {
        self.file(path.as_ref())
            .and_then(|file| file.parsed.as_ref().ok())
            .and_then(|parsed| document_highlights_parsed_source(parsed, position).ok())
            .unwrap_or_default()
    }

    pub fn document_symbols(&self, path: impl AsRef<Path>) -> Vec<DocumentSymbol> {
        self.file(path.as_ref())
            .and_then(|file| file.parsed.as_ref().ok())
            .map(document_symbols_parsed_source)
            .unwrap_or_default()
    }

    pub fn selection_ranges(
        &self,
        path: impl AsRef<Path>,
        positions: &[SourcePosition],
    ) -> Vec<Option<SelectionRange>> {
        let Some(file) = self.file(path.as_ref()) else {
            return Vec::new();
        };
        let file_span = file.parsed.as_ref().ok().and_then(|parsed| parsed.span);

        selection_ranges_source(&file.source, file_span, positions)
    }

    pub fn folding_ranges(&self, path: impl AsRef<Path>) -> Vec<FoldingRange> {
        let Some(file) = self.file(path.as_ref()) else {
            return Vec::new();
        };
        let type_alias_spans = file
            .parsed
            .as_ref()
            .ok()
            .map(|parsed| {
                parsed
                    .type_aliases
                    .iter()
                    .filter_map(|alias| alias.span)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        folding_ranges_source(&file.source, &type_alias_spans)
    }

    pub fn assists(
        &self,
        path: impl AsRef<Path>,
        range: AssistRange,
        diagnostics: &[Diagnostic],
    ) -> Vec<Assist> {
        let path = path.as_ref();
        let Some(parsed) = self.file(path).and_then(|file| file.parsed.as_ref().ok()) else {
            return Vec::new();
        };
        let source_dir = self.assist_source_root_for_path(path);

        assists_parsed_source(
            parsed,
            diagnostics,
            range,
            &source_dir,
            &self.config.extension,
        )
    }

    pub fn code_actions(
        &self,
        path: impl AsRef<Path>,
        range: AssistRange,
        diagnostics: &[Diagnostic],
    ) -> Vec<Assist> {
        self.assists(path, range, diagnostics)
    }

    pub fn references(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
        include_declaration: bool,
    ) -> Vec<Reference> {
        let path = path.as_ref();
        let Some(target) = self.definition(path, position) else {
            return Vec::new();
        };
        let global_name = self
            .file(&target.path)
            .and_then(|file| file.parsed.as_ref().ok())
            .and_then(|parsed| global_function_name_for_definition(parsed, &target));
        let alias_name = self
            .file(&target.path)
            .and_then(|file| file.parsed.as_ref().ok())
            .and_then(|parsed| global_type_alias_name_for_definition(parsed, &target));
        let mut references = Vec::new();

        for file in self.files.values() {
            let Ok(parsed) = &file.parsed else {
                continue;
            };
            let Ok(mut file_references) =
                references_to_definition_parsed_source(parsed, &target, include_declaration)
            else {
                continue;
            };

            references.append(&mut file_references);

            if let Some(global_name) = global_name.as_deref() {
                let Ok(mut global_references) = global_references_to_definition_parsed_source(
                    parsed,
                    &target,
                    global_name,
                    include_declaration,
                ) else {
                    continue;
                };

                references.append(&mut global_references);
            }

            if let Some(alias_name) = alias_name.as_deref() {
                let Ok(mut alias_references) =
                    global_type_alias_references_to_definition_parsed_source(
                        parsed,
                        &target,
                        alias_name,
                        include_declaration,
                    )
                else {
                    continue;
                };

                references.append(&mut alias_references);
            }
        }

        references.sort_by_key(reference_key);
        references.dedup_by_key(|reference| reference_key(reference));
        references
    }

    pub fn prepare_rename(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Option<RenameTarget> {
        let path = path.as_ref();
        let target = self.definition(path, position)?;
        let target_parsed = self.file(&target.path)?.parsed.as_ref().ok()?;
        let mut rename_target = rename_target_for_definition(target_parsed, target)?;
        let reference_kind = reference_kind_for_rename(rename_target.kind);
        let current_reference =
            self.references(path, position, true)
                .into_iter()
                .find(|reference| {
                    reference.path == path
                        && reference.kind == reference_kind
                        && contains_position(reference.span, position)
                })?;

        rename_target.path = current_reference.path;
        rename_target.span = current_reference.span;

        Some(rename_target)
    }

    pub fn rename(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
        new_name: &str,
    ) -> Option<RenameEdit> {
        let path = path.as_ref();

        if !is_valid_rename_identifier(new_name) {
            return None;
        }

        let target = self.definition(path, position)?;
        let target_parsed = self.file(&target.path)?.parsed.as_ref().ok()?;
        let rename_target = rename_target_for_definition(target_parsed, target.clone())?;

        if self.is_ambiguous_project_rename(&target, &rename_target) {
            return None;
        }

        let reference_kind = reference_kind_for_rename(rename_target.kind);
        let mut edits = self
            .references(path, position, true)
            .into_iter()
            .filter(|reference| reference.kind == reference_kind)
            .map(|reference| TextEdit {
                path: reference.path,
                span: reference.span,
                new_text: new_name.to_owned(),
            })
            .collect::<Vec<_>>();

        edits.sort_by_key(text_edit_key);
        edits.dedup_by_key(|edit| text_edit_key(edit));

        (!edits.is_empty()).then_some(RenameEdit { edits })
    }

    pub fn inlay_hints(&self, path: impl AsRef<Path>) -> Vec<InlayHint> {
        self.file(path.as_ref())
            .and_then(|file| file.parsed.as_ref().ok())
            .and_then(|parsed| inlay_hints_parsed_source(parsed).ok())
            .unwrap_or_default()
    }

    pub fn semantic_tokens(&self, path: impl AsRef<Path>) -> Vec<SemanticToken> {
        let path = path.as_ref();
        let Some(file) = self.file(path) else {
            return Vec::new();
        };

        match &file.parsed {
            Ok(parsed) => semantic_tokens_parsed_source(parsed).unwrap_or_default(),
            Err(_) => semantic_tokens_tua_source(path, file.source.clone()).unwrap_or_default(),
        }
    }

    fn file(&self, path: &Path) -> Option<&AnalysisFile> {
        self.files.get(&normalize_path(path))
    }

    pub fn type_environment(&self, path: impl AsRef<Path>) -> Option<&TypeEnvironment> {
        self.file(path.as_ref())?.environment.as_ref()
    }

    pub fn scope_map(&self, path: impl AsRef<Path>) -> Option<&ScopeMap> {
        self.file(path.as_ref())?.scope.as_ref()
    }

    pub fn module_exports(&self, path: impl AsRef<Path>) -> Option<&ModuleExports> {
        self.file(path.as_ref())?.exports.as_ref()
    }

    pub fn require_edges(&self, path: impl AsRef<Path>) -> &[ModuleLink] {
        self.file(path.as_ref())
            .map(|file| file.require_edges.as_slice())
            .unwrap_or_default()
    }

    pub fn module_graph(&self) -> &ModuleGraph {
        &self.module_graph
    }

    pub fn type_database(&self) -> &ProjectTypeDatabase {
        &self.type_database
    }

    pub fn symbol_index(&self) -> &SymbolIndex {
        &self.symbol_index
    }

    pub fn symbol_index_for_path(&self, path: impl AsRef<Path>) -> Vec<&SymbolIndexEntry> {
        self.symbol_index.entries_for_path(path)
    }

    pub fn workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        self.symbol_index.workspace_symbols(query)
    }

    pub fn importable_symbols(&self, prefix: &str) -> Vec<&SymbolIndexEntry> {
        self.symbol_index.importable_symbols(prefix)
    }

    fn module_hover(&self, parsed: &ParsedSource, position: SourcePosition) -> Option<Hover> {
        if let Some(module) = require_module_at_position(&parsed.source, position) {
            let path = self.resolved_module_path(&parsed.path, &module)?;

            return Some(Hover {
                contents: format!("(module) {module} -> {}", path.display()),
                span: None,
                documentation: None,
                context: Vec::new(),
            });
        }

        let chain = dotted_chain_at_position(&parsed.source, position)?;
        let current_part = chain.parts.get(chain.cursor_index)?;
        let root = chain.parts.first()?;
        let binding = self.require_binding(parsed, &root.text, position)?;

        if chain.cursor_index == 0 {
            let path = self.resolved_module_path(&parsed.path, &binding.module)?;

            return Some(Hover {
                contents: format!(
                    "(module) {} = require(\"{}\") -> {}",
                    binding.alias,
                    binding.module,
                    path.display()
                ),
                span: Some(current_part.span),
                documentation: None,
                context: Vec::new(),
            });
        }

        let fields = chain
            .parts
            .iter()
            .skip(1)
            .take(chain.cursor_index)
            .map(|part| part.text.clone())
            .collect::<Vec<_>>();
        let path = self.resolved_module_path(&parsed.path, &binding.module)?;
        let export = self.module_export_for_fields(path, &fields)?;
        let documentation = module_export_documentation(
            &export,
            self.module_export_source_documentation(path, &fields),
        );
        let kind = if matches!(export.ty, Type::Function(_)) {
            "function"
        } else {
            "field"
        };

        Some(Hover {
            contents: format_module_export_hover(
                &chain
                    .parts
                    .iter()
                    .take(chain.cursor_index + 1)
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
                &export.ty,
            ),
            span: Some(current_part.span),
            documentation,
            context: vec![
                format!("kind: {kind}"),
                format!("module: {}", path.display()),
            ],
        })
    }

    fn module_definition(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Definition> {
        if let Some(module) = require_module_at_position(&parsed.source, position) {
            let path = self.resolved_module_path(&parsed.path, &module)?;

            return Some(module_file_definition(
                path,
                self.module_definition_span(path),
            ));
        }

        let chain = dotted_chain_at_position(&parsed.source, position)?;
        let root = chain.parts.first()?;
        let binding = self.require_binding(parsed, &root.text, position)?;
        let path = self.resolved_module_path(&parsed.path, &binding.module)?;

        if chain.cursor_index == 0 {
            return Some(module_file_definition(
                path,
                self.module_definition_span(path),
            ));
        }

        let public_name = chain.parts.get(1)?.text.as_str();
        let entry = self.module_graph.export(path, public_name)?;

        Some(Definition {
            path: path.to_path_buf(),
            span: entry.definition_span,
            kind: if matches!(entry.field.ty, Type::Function(_)) {
                DefinitionKind::Function
            } else {
                DefinitionKind::TableField
            },
        })
    }

    fn module_definition_span(&self, path: &Path) -> SourceSpan {
        self.module_graph
            .module(path)
            .and_then(|module| {
                module
                    .exports
                    .iter()
                    .map(|entry| entry.definition_span)
                    .min_by_key(|span| position_key(span.start))
            })
            .unwrap_or_else(default_span)
    }

    fn module_export_source_documentation(&self, path: &Path, fields: &[String]) -> Option<String> {
        let public_name = fields.first()?;
        let entry = self.module_graph.export(path, public_name)?;
        let source = &self.file(path)?.source;

        leading_plain_documentation(source, entry.definition_span.start.line)
    }

    fn is_module_reference(&self, parsed: &ParsedSource, position: SourcePosition) -> bool {
        if require_module_at_position(&parsed.source, position).is_some() {
            return true;
        }

        let Some(chain) = dotted_chain_at_position(&parsed.source, position) else {
            return false;
        };
        let Some(root) = chain.parts.first() else {
            return false;
        };

        self.require_binding(parsed, &root.text, position).is_some()
    }

    fn require_binding(
        &self,
        parsed: &ParsedSource,
        name: &str,
        position: SourcePosition,
    ) -> Option<HirRequireBinding> {
        self.file(&parsed.path)?
            .hir
            .as_ref()?
            .require_bindings
            .iter()
            .filter(|binding| {
                binding.alias == name
                    && position_key(binding.alias_span.start) <= position_key(position)
            })
            .max_by_key(|binding| position_key(binding.alias_span.start))
            .cloned()
    }

    fn resolved_module_path(&self, source_path: &Path, module: &str) -> Option<&Path> {
        match self.module_graph.resolve_require(source_path, module)? {
            RequireResolution::Resolved { path }
            | RequireResolution::Duplicate { selected: path, .. } => Some(path.as_path()),
            RequireResolution::Missing { .. } => None,
        }
    }

    fn module_export_for_fields(
        &self,
        module_path: &Path,
        fields: &[String],
    ) -> Option<ModuleExportField> {
        let exports = self.module_exports(module_path)?;

        exported_field_for_fields(exports, fields)
    }

    fn project_type_hover(&self, parsed: &ParsedSource, position: SourcePosition) -> Option<Hover> {
        if !is_type_annotation_position(parsed, position) {
            return None;
        }

        let chain = dotted_chain_at_position(&parsed.source, position)?;
        let current_part = chain.parts.get(chain.cursor_index)?;
        let name = chain
            .parts
            .iter()
            .take(chain.cursor_index + 1)
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join(".");
        let binding =
            self.type_database
                .resolve_type_name(&self.module_graph, &parsed.path, &name)?;
        let mut hover = Hover {
            contents: format!("type {name} = {}", binding.ty),
            span: Some(current_part.span),
            documentation: None,
            context: Vec::new(),
        };

        if binding.module_path != normalize_path(&parsed.path) {
            hover
                .context
                .push(format!("module: {}", binding.module_path.display()));
        }

        Some(hover)
    }

    fn project_symbol_hover(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Hover> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;

        if chain.cursor_index != 0 || chain.parts.len() != 1 {
            return None;
        }

        let current_part = chain.parts.first()?;
        let environment = self.project_type_environment(parsed);
        let semantic = SemanticModel::new(parsed, &environment);
        let ty = semantic.type_of_name(&current_part.text, position)?;

        if matches!(ty, Type::Any) {
            return None;
        }

        Some(Hover {
            contents: format!("local {}: {ty}", current_part.text),
            span: Some(current_part.span),
            documentation: None,
            context: Vec::new(),
        })
    }

    fn project_annotated_local_hover(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Hover> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;

        if chain.cursor_index != 0 || chain.parts.len() != 1 {
            return None;
        }

        let current_part = chain.parts.first()?;
        let annotation = parsed
            .local_annotations
            .iter()
            .filter(|annotation| {
                annotation.name == current_part.text
                    && tua_type_uses_qualified_name(&annotation.ty)
                    && annotation
                        .name_span
                        .is_some_and(|span| position_key(span.start) <= position_key(position))
            })
            .max_by_key(|annotation| annotation.name_span.map(|span| position_key(span.start)))?;
        let ty = self.project_type_from_tua_type(parsed, &annotation.ty)?;

        if matches!(ty, Type::Any) {
            return None;
        }

        Some(Hover {
            contents: format!("local {}: {ty}", current_part.text),
            span: Some(current_part.span),
            documentation: None,
            context: Vec::new(),
        })
    }

    fn project_completions(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
    ) -> Vec<Completion> {
        let environment = self.project_type_environment(parsed);
        let field_completions = complete_field_access_in_source_with_environment(
            source,
            parsed,
            position,
            &environment,
        );

        if !field_completions.is_empty() {
            return field_completions;
        }

        self.project_table_literal_completions(parsed, source, position)
    }

    fn project_type_environment(&self, parsed: &ParsedSource) -> TypeEnvironment {
        let (environment, _) = build_type_environment_with_diagnostics(parsed);

        environment.with_local_types(self.project_local_annotation_types(parsed))
    }

    fn project_local_annotation_types(&self, parsed: &ParsedSource) -> Vec<(String, Type)> {
        parsed
            .local_annotations
            .iter()
            .filter_map(|annotation| {
                self.project_type_from_tua_type(parsed, &annotation.ty)
                    .map(|ty| (annotation.name.clone(), ty))
            })
            .collect()
    }

    fn project_type_from_tua_type(&self, parsed: &ParsedSource, ty: &TuaType) -> Option<Type> {
        match ty {
            TuaType::Named(name) => self
                .type_database
                .resolve_type_name(&self.module_graph, &parsed.path, name)
                .map(|binding| binding.ty)
                .or_else(|| self.required_module_type(parsed, name))
                .or_else(|| Some(Type::named(name.trim()))),
            TuaType::Union(types) => {
                Some(Type::union(types.iter().filter_map(|ty| {
                    self.project_type_from_tua_type(parsed, ty)
                })))
            }
            TuaType::Table(fields) => Some(Type::table(fields.iter().filter_map(|field| {
                self.project_type_from_tua_type(parsed, &field.ty)
                    .map(|ty| {
                        (
                            field.name.clone(),
                            TableField {
                                ty,
                                optional: field.optional,
                            },
                        )
                    })
            }))),
        }
    }

    fn required_module_type(&self, parsed: &ParsedSource, name: &str) -> Option<Type> {
        let (receiver, member) = name.trim().split_once('.')?;

        if let Some(path) = self.indexed_required_module_path(parsed, receiver) {
            let binding = self.type_database.module(path)?.alias(member.trim())?;

            return Some(self.type_database.resolve_type(
                &self.module_graph,
                &binding.module_path,
                &binding.ty,
            ));
        }

        None
    }

    fn indexed_required_module_path(&self, parsed: &ParsedSource, receiver: &str) -> Option<&Path> {
        let module = require_bindings_for_parsed_source(parsed)
            .into_iter()
            .find(|binding| binding.alias == receiver)?
            .module;

        for candidate in self.indexed_module_candidates(&module) {
            if self.type_database.module(&candidate).is_some() {
                return self
                    .type_database
                    .module(&candidate)
                    .map(|module| module.path.as_path());
            }
        }

        None
    }

    fn indexed_module_candidates(&self, module: &str) -> Vec<PathBuf> {
        let mut relative = PathBuf::new();

        for part in module.split('.').filter(|part| !part.is_empty()) {
            relative.push(part);
        }

        let mut candidates = Vec::new();
        let extensions = if self.config.extension == "lua" {
            vec!["lua"]
        } else {
            vec![self.config.extension.as_str(), "lua"]
        };

        for root in self.source_roots() {
            for extension in &extensions {
                candidates.push(normalize_path(
                    root.join(&relative).with_extension(extension),
                ));
                candidates.push(normalize_path(
                    root.join(&relative).join(format!("init.{extension}")),
                ));
            }
        }

        candidates
    }

    fn project_table_literal_completions(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
    ) -> Vec<Completion> {
        let Some((ty, prefix)) = self.table_literal_annotation_type(parsed, source, position)
        else {
            return Vec::new();
        };
        let mut fields = BTreeMap::new();

        collect_type_fields(&ty, &mut fields);

        fields
            .into_iter()
            .filter(|(name, _)| name.starts_with(&prefix))
            .map(|(name, ty)| Completion {
                label: name,
                kind: CompletionKind::Field,
                detail: Some(format!("field: {ty}")),
                documentation: None,
                insert_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect()
    }

    fn table_literal_annotation_type(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
    ) -> Option<(Type, String)> {
        let prefix = identifier_prefix_before(source, position)?;
        let annotation = parsed
            .local_annotations
            .iter()
            .filter(|annotation| {
                let Some(name_span) = annotation.name_span else {
                    return false;
                };

                name_span.start.line <= position.line
                    && typed_initializer_is_open(source, name_span.start.line, position)
            })
            .max_by_key(|annotation| annotation.name_span.map(|span| position_key(span.start)))?;
        let ty = self.project_type_from_tua_type(parsed, &annotation.ty)?;

        Some((ty, prefix))
    }

    fn project_aware_diagnostics(
        &self,
        path: &Path,
        diagnostics: Vec<Diagnostic>,
    ) -> Vec<Diagnostic> {
        deduplicate_diagnostics(diagnostics)
            .into_iter()
            .filter(|diagnostic| {
                if diagnostic.code != Some(crate::DiagnosticCode::TYPE_UNKNOWN_NAME) {
                    return true;
                }

                let Some(name) = unknown_type_name(&diagnostic.message) else {
                    return true;
                };

                self.type_database
                    .resolve_type_name(&self.module_graph, path, name)
                    .is_none()
            })
            .collect()
    }

    fn project_diagnostic_enabled(&self, diagnostic: &Diagnostic) -> bool {
        match diagnostic.code {
            Some(crate::DiagnosticCode::LINT_MISSING_REQUIRE) => {
                self.config.diagnostics.missing_requires
            }
            Some(crate::DiagnosticCode::LINT_DUPLICATE_REQUIRE_CANDIDATE)
            | Some(crate::DiagnosticCode::LINT_REQUIRE_CYCLE) => {
                self.config.diagnostics.bad_requires
            }
            _ => true,
        }
    }

    fn assist_source_root_for_path(&self, path: &Path) -> PathBuf {
        self.source_roots()
            .iter()
            .filter(|root| path.starts_with(root))
            .max_by_key(|root| root.components().count())
            .cloned()
            .or_else(|| path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn is_ambiguous_project_rename(
        &self,
        target: &Definition,
        rename_target: &RenameTarget,
    ) -> bool {
        match rename_target.kind {
            crate::RenameKind::Function => {
                let Some(global_name) = self
                    .file(&target.path)
                    .and_then(|file| file.parsed.as_ref().ok())
                    .and_then(|parsed| global_function_name_for_definition(parsed, target))
                else {
                    return false;
                };

                self.files
                    .values()
                    .filter_map(|file| file.parsed.as_ref().ok())
                    .map(|parsed| global_function_definition_count(parsed, &global_name))
                    .sum::<usize>()
                    > 1
            }
            crate::RenameKind::TypeAlias => {
                let Some(alias_name) = self
                    .file(&target.path)
                    .and_then(|file| file.parsed.as_ref().ok())
                    .and_then(|parsed| global_type_alias_name_for_definition(parsed, target))
                else {
                    return false;
                };

                self.files
                    .values()
                    .filter_map(|file| file.parsed.as_ref().ok())
                    .map(|parsed| type_alias_definition_count(parsed, &alias_name))
                    .sum::<usize>()
                    > 1
            }
            crate::RenameKind::Local | crate::RenameKind::Parameter => false,
        }
    }
}

fn deduplicate_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();

    for diagnostic in diagnostics {
        let key = diagnostic_key(&diagnostic);

        if seen.insert(key) {
            output.push(diagnostic);
        }
    }

    output
}

fn diagnostic_key(diagnostic: &Diagnostic) -> (PathBuf, usize, usize, u8, Option<u16>) {
    if diagnostic.code == Some(crate::DiagnosticCode::LINT_MISSING_REQUIRE) {
        return (
            normalize_path(&diagnostic.file),
            diagnostic.line,
            0,
            diagnostic_severity_key(diagnostic.severity),
            diagnostic.code.map(crate::DiagnosticCode::as_u16),
        );
    }

    (
        normalize_path(&diagnostic.file),
        diagnostic.line,
        diagnostic.column,
        diagnostic_severity_key(diagnostic.severity),
        diagnostic.code.map(crate::DiagnosticCode::as_u16),
    )
}

fn diagnostic_severity_key(severity: crate::DiagnosticSeverity) -> u8 {
    match severity {
        crate::DiagnosticSeverity::Error => 1,
        crate::DiagnosticSeverity::Warning => 2,
        crate::DiagnosticSeverity::Info => 3,
        crate::DiagnosticSeverity::Hint => 4,
    }
}

fn reference_key(reference: &Reference) -> (PathBuf, usize, usize, usize, usize, u8) {
    (
        reference.path.clone(),
        reference.span.start.line,
        reference.span.start.column,
        reference.span.end.line,
        reference.span.end.column,
        reference_kind_key(reference.kind),
    )
}

fn reference_kind_key(kind: crate::ReferenceKind) -> u8 {
    match kind {
        crate::ReferenceKind::Module => 0,
        crate::ReferenceKind::TypeAlias => 1,
        crate::ReferenceKind::Local => 2,
        crate::ReferenceKind::Function => 3,
        crate::ReferenceKind::Parameter => 4,
        crate::ReferenceKind::TableField => 5,
    }
}

fn text_edit_key(edit: &TextEdit) -> (PathBuf, usize, usize, usize, usize) {
    (
        edit.path.clone(),
        edit.span.start.line,
        edit.span.start.column,
        edit.span.end.line,
        edit.span.end.column,
    )
}

fn unknown_type_name(message: &str) -> Option<&str> {
    Some(
        message
            .strip_prefix("unknown type `")?
            .strip_suffix('`')?
            .trim(),
    )
}

fn is_type_annotation_position(parsed: &ParsedSource, position: SourcePosition) -> bool {
    parsed.local_annotations.iter().any(|annotation| {
        annotation
            .span
            .is_some_and(|span| contains_position(span, position))
    }) || parsed.function_annotations.iter().any(|function| {
        function.parameters.iter().any(|parameter| {
            parameter
                .span
                .is_some_and(|span| contains_position(span, position))
        })
    })
}

fn tua_type_uses_qualified_name(ty: &TuaType) -> bool {
    match ty {
        TuaType::Named(name) => name.contains('.'),
        TuaType::Union(types) => types.iter().any(tua_type_uses_qualified_name),
        TuaType::Table(fields) => fields
            .iter()
            .any(|field| tua_type_uses_qualified_name(&field.ty)),
    }
}

fn contains_position(span: SourceSpan, position: SourcePosition) -> bool {
    let start = position_key(span.start);
    let end = position_key(span.end);
    let position = position_key(position);

    start <= position && position < end
}

fn collect_type_fields(ty: &Type, fields: &mut BTreeMap<String, Type>) {
    match ty {
        Type::Table(table) => {
            fields.extend(
                table
                    .fields()
                    .map(|(name, field)| (name.to_owned(), field.ty.clone())),
            );
        }
        Type::Union(types) => {
            for ty in types {
                collect_type_fields(ty, fields);
            }
        }
        Type::Any | Type::Alias(_) | Type::Function(_) | Type::Primitive(_) => {}
    }
}

fn identifier_prefix_before(source: &str, position: SourcePosition) -> Option<String> {
    let line = line_at(source, position.line)?;
    let cursor = byte_for_position_column(line, position.column);
    let mut start = cursor;

    while start > 0 {
        let byte = line.as_bytes()[start - 1];

        if byte == b'_' || byte.is_ascii_alphanumeric() {
            start -= 1;
        } else {
            break;
        }
    }

    Some(line[start..cursor].to_owned())
}

fn typed_initializer_is_open(source: &str, start_line: usize, position: SourcePosition) -> bool {
    let Some(first_line) = line_at(source, start_line) else {
        return false;
    };

    if !first_line.contains("local ") || !first_line.contains(':') || !first_line.contains("= {") {
        return false;
    }

    let mut depth = 0isize;

    for line_number in start_line..=position.line {
        let Some(line) = line_at(source, line_number) else {
            continue;
        };
        let end = if line_number == position.line {
            byte_for_position_column(line, position.column)
        } else {
            line.len()
        };

        for byte in line[..end].bytes() {
            match byte {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
        }
    }

    depth > 0
}

fn byte_for_position_column(line: &str, column: usize) -> usize {
    let target = column.saturating_sub(1);

    line.char_indices()
        .nth(target)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

fn source_without_incomplete_field_access_lines(source: &str) -> String {
    let mut recovered = String::with_capacity(source.len());

    for line in source.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let content = content.strip_suffix('\r').unwrap_or(content);
        let trimmed = content.trim_end();

        if trimmed.ends_with('.') || trimmed.ends_with(':') {
            recovered.push('\n');
        } else {
            recovered.push_str(line);
        }
    }

    recovered
}

impl AnalysisQueryCache {
    fn cached_file(
        &mut self,
        path: &Path,
        source: &str,
        kind: AnalysisSourceKind,
        config: &ProjectConfig,
        source_roots: &[PathBuf],
        asset_roots: &[PathBuf],
    ) -> AnalysisFile {
        let path = normalize_path(path);

        if let Some(cached) = self.files.get(&path) {
            if cached.source == source && cached.kind == kind {
                let file = cached.file.clone();

                self.record_hit();
                return file;
            }
        }

        self.record_miss();

        let file = build_analysis_file(
            &path,
            source.to_owned(),
            kind,
            config,
            source_roots,
            asset_roots,
        );
        self.update_dependencies(&path, &file.require_edges);
        self.files.insert(
            path,
            CachedAnalysisFile {
                source: source.to_owned(),
                kind,
                file: file.clone(),
            },
        );

        file
    }

    fn invalidate_file(&mut self, path: &Path) {
        let path = normalize_path(path);
        let mut invalidated = BTreeSet::new();

        self.invalidate_one(&path, &mut invalidated, false);

        if let Some(importers) = self.importers.get(&path).cloned() {
            for importer in importers {
                self.invalidate_one(&importer, &mut invalidated, true);
            }
        }
    }

    fn invalidate_all(&mut self) {
        self.stats.invalidations += self.files.len();
        self.files.clear();
        self.dependencies.clear();
        self.importers.clear();
    }

    fn retain_paths(&mut self, live_paths: &BTreeSet<PathBuf>) {
        let removed = self
            .files
            .keys()
            .filter(|path| !live_paths.contains(*path))
            .cloned()
            .collect::<Vec<_>>();

        for path in removed {
            self.invalidate_file(&path);
        }
    }

    fn invalidate_one(
        &mut self,
        path: &Path,
        invalidated: &mut BTreeSet<PathBuf>,
        dependent: bool,
    ) {
        let path = normalize_path(path);

        if !invalidated.insert(path.clone()) {
            return;
        }

        if self.files.remove(&path).is_some() {
            self.stats.invalidations += 1;
            if dependent {
                self.stats.dependent_invalidations += 1;
            }
        }

        if let Some(dependencies) = self.dependencies.remove(&path) {
            for dependency in dependencies {
                if let Some(importers) = self.importers.get_mut(&dependency) {
                    importers.remove(&path);
                    if importers.is_empty() {
                        self.importers.remove(&dependency);
                    }
                }
            }
        }
    }

    fn update_dependencies(&mut self, path: &Path, require_edges: &[ModuleLink]) {
        let path = normalize_path(path);

        if let Some(old_dependencies) = self.dependencies.remove(&path) {
            for dependency in old_dependencies {
                if let Some(importers) = self.importers.get_mut(&dependency) {
                    importers.remove(&path);
                    if importers.is_empty() {
                        self.importers.remove(&dependency);
                    }
                }
            }
        }

        let dependencies = require_edges
            .iter()
            .map(|edge| normalize_path(&edge.target))
            .collect::<BTreeSet<_>>();

        for dependency in &dependencies {
            self.importers
                .entry(dependency.clone())
                .or_default()
                .insert(path.clone());
        }

        self.dependencies.insert(path, dependencies);
    }

    fn record_hit(&mut self) {
        self.stats.parse_hits += 1;
        self.stats.type_environment_hits += 1;
        self.stats.diagnostics_hits += 1;
        self.stats.module_export_hits += 1;
        self.stats.require_edge_hits += 1;
        self.stats.symbol_index_hits += 1;
    }

    fn record_miss(&mut self) {
        self.stats.parse_misses += 1;
        self.stats.type_environment_misses += 1;
        self.stats.diagnostics_misses += 1;
        self.stats.module_export_misses += 1;
        self.stats.require_edge_misses += 1;
        self.stats.symbol_index_misses += 1;
    }
}

fn build_analysis_file(
    path: &Path,
    source: String,
    kind: AnalysisSourceKind,
    config: &ProjectConfig,
    source_roots: &[PathBuf],
    asset_roots: &[PathBuf],
) -> AnalysisFile {
    let parsed = parse_analysis_source(path, source.clone(), kind);
    let hir = parsed.as_ref().ok().map(lower_parsed_source);
    let scope = parsed.as_ref().ok().map(scope_map_parsed_source);
    let environment = parsed
        .as_ref()
        .ok()
        .and_then(|parsed| build_type_environment(parsed).ok());
    let mut diagnostics = match &parsed {
        Ok(parsed) => check_parsed_source_with_asset_roots_and_diagnostics_config(
            parsed,
            analysis_asset_roots(source_roots, asset_roots),
            &config.diagnostics,
        )
        .err()
        .unwrap_or_default(),
        Err(diagnostics) => diagnostics.clone(),
    };
    if kind == AnalysisSourceKind::Tua {
        if let Ok(parsed) = &parsed {
            if let Err(mut emit_diagnostics) = emit_lua(parsed) {
                diagnostics.append(&mut emit_diagnostics);
            }
        }
    }
    let diagnostics = deduplicate_diagnostics(diagnostics);
    let exports = parsed.as_ref().ok().and_then(infer_module_exports);
    let require_edges = module_links_tua_source(path, source.clone());
    let symbols = parsed
        .as_ref()
        .ok()
        .map(|parsed| symbol_entries_for_parsed(parsed, hir.as_ref(), source_roots))
        .unwrap_or_default();

    AnalysisFile {
        source,
        kind,
        parsed,
        hir,
        scope,
        environment,
        diagnostics,
        exports,
        require_edges,
        symbols,
    }
}

fn analysis_asset_roots<'a>(
    source_roots: &'a [PathBuf],
    asset_roots: &'a [PathBuf],
) -> &'a [PathBuf] {
    if asset_roots.is_empty() {
        source_roots
    } else {
        asset_roots
    }
}

fn module_file_definition(path: &Path, span: SourceSpan) -> Definition {
    Definition {
        path: path.to_path_buf(),
        span,
        kind: DefinitionKind::Module,
    }
}

fn format_module_export_hover(name: &str, ty: &Type) -> String {
    let ty = ty.to_string();

    if let Some(signature) = ty.strip_prefix("function") {
        format!("function {name}{signature}")
    } else {
        format!("(field) {name}: {ty}")
    }
}

fn module_export_documentation(
    export: &ModuleExportField,
    source_documentation: Option<String>,
) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(documentation) = source_documentation.as_deref() {
        push_unique_documentation(&mut parts, documentation);
    }

    if let Some(documentation) = export.documentation.as_deref() {
        push_unique_documentation(&mut parts, documentation);
    }

    for documentation in export
        .parameter_documentation
        .iter()
        .filter_map(|documentation| documentation.as_deref())
    {
        push_unique_documentation(&mut parts, documentation);
    }

    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn push_unique_documentation(parts: &mut Vec<String>, documentation: &str) {
    for line in documentation
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if !parts.iter().any(|part| part == line) {
            parts.push(line.to_owned());
        }
    }
}

fn leading_plain_documentation(source: &str, definition_line: usize) -> Option<String> {
    let mut docs = Vec::new();

    let lines = source
        .lines()
        .take(definition_line.saturating_sub(1))
        .collect::<Vec<_>>();

    for line in lines.into_iter().rev() {
        let trimmed = line.trim();

        if let Some(doc) = trimmed.strip_prefix("---") {
            let doc = doc.trim();

            if !doc.starts_with('@') && !doc.is_empty() {
                docs.push(doc.to_owned());
            }

            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        break;
    }

    docs.reverse();

    (!docs.is_empty()).then(|| docs.join("\n"))
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn symbol_entries_for_parsed(
    parsed: &ParsedSource,
    hir: Option<&HirModule>,
    source_roots: &[PathBuf],
) -> Vec<SymbolIndexEntry> {
    SymbolIndex::build([(&parsed.path, hir)], &ModuleGraph::default(), source_roots)
        .entries()
        .to_vec()
}

fn default_span() -> SourceSpan {
    SourceSpan {
        start: SourcePosition { line: 1, column: 1 },
        end: SourcePosition { line: 1, column: 1 },
    }
}

fn parse_analysis_source(
    path: &Path,
    source: String,
    kind: AnalysisSourceKind,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    match kind {
        AnalysisSourceKind::Lua => parse_lua_source(path, source),
        AnalysisSourceKind::Tua => parse_tua_source(path, source),
    }
}

fn infer_source_kind(path: &Path, config: &ProjectConfig) -> AnalysisSourceKind {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("lua") => AnalysisSourceKind::Lua,
        Some(extension) if extension == config.extension => AnalysisSourceKind::Tua,
        _ => AnalysisSourceKind::Tua,
    }
}

fn vfs_source_kind(kind: AnalysisSourceKind) -> VfsSourceKind {
    match kind {
        AnalysisSourceKind::Lua => VfsSourceKind::Lua,
        AnalysisSourceKind::Tua => VfsSourceKind::Tua,
    }
}

fn analysis_source_kind(kind: VfsSourceKind) -> AnalysisSourceKind {
    match kind {
        VfsSourceKind::Lua => AnalysisSourceKind::Lua,
        VfsSourceKind::Tua => AnalysisSourceKind::Tua,
    }
}

#[cfg(test)]
mod tests;
