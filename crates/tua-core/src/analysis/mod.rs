use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::hir::{HirModule, HirRequireBinding};
use crate::module_link::require_module_at_position;
use crate::source_text::dotted_chain_at_position;
use crate::vfs::{normalize_path, VfsSnapshot, VfsSourceKind, VirtualFileSystem};
use crate::{
    build_type_environment, check_parsed_source_with_asset_roots_and_diagnostics_config,
    complete_parsed_source, complete_tua_source, definition_parsed_source,
    exported_field_for_fields, hover_parsed_source, infer_module_exports, lower_parsed_source,
    module_links_tua_source, parse_lua_source, parse_tua_source, Completion, Definition,
    DefinitionKind, Diagnostic, Hover, ModuleExportField, ModuleExports, ModuleGraph,
    ModuleGraphInput, ModuleGraphOptions, ModuleLink, ParsedSource, ProjectConfig,
    RequireResolution, SourcePosition, SourceSpan, Type, TypeEnvironment, WorkspaceSymbol,
    WorkspaceSymbolKind,
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
    builtin_catalogs: BuiltinCatalogs,
    cache: RefCell<AnalysisQueryCache>,
}

#[derive(Clone, Debug)]
pub struct AnalysisSnapshot {
    config: ProjectConfig,
    vfs: VfsSnapshot,
    files: BTreeMap<PathBuf, AnalysisFile>,
    builtin_catalogs: BuiltinCatalogs,
    module_graph: ModuleGraph,
}

#[derive(Clone, Debug)]
struct AnalysisFile {
    source: String,
    kind: AnalysisSourceKind,
    parsed: Result<ParsedSource, Vec<Diagnostic>>,
    hir: Option<HirModule>,
    environment: Option<TypeEnvironment>,
    diagnostics: Vec<Diagnostic>,
    exports: Option<ModuleExports>,
    require_edges: Vec<ModuleLink>,
    symbols: Vec<WorkspaceSymbol>,
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
        self.config = config;
        self.cache.borrow_mut().invalidate_all();
    }

    pub fn source_roots(&self) -> &[PathBuf] {
        &self.vfs.options().source_roots
    }

    pub fn set_source_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        self.vfs.set_source_roots(roots);
        self.cache.borrow_mut().invalidate_all();
    }

    pub fn set_output_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        self.vfs.set_output_roots(roots);
        self.cache.borrow_mut().invalidate_all();
    }

    pub fn set_ignored_paths(&mut self, patterns: impl IntoIterator<Item = String>) {
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

        AnalysisSnapshot {
            config: self.config.clone(),
            vfs,
            files,
            builtin_catalogs: self.builtin_catalogs.clone(),
            module_graph,
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
        self.diagnostics_with_asset_roots(path, self.source_roots())
    }

    pub fn diagnostics_with_asset_roots(
        &self,
        path: impl AsRef<Path>,
        asset_roots: &[PathBuf],
    ) -> Vec<Diagnostic> {
        self.diagnostics_with_config(path, asset_roots, &self.config.diagnostics)
    }

    pub fn diagnostics_with_config(
        &self,
        path: impl AsRef<Path>,
        asset_roots: &[PathBuf],
        diagnostics_config: &crate::DiagnosticsConfig,
    ) -> Vec<Diagnostic> {
        let Some(file) = self.file(path.as_ref()) else {
            return Vec::new();
        };

        if asset_roots == self.source_roots() && diagnostics_config == &self.config.diagnostics {
            return file.diagnostics.clone();
        }

        let parsed = match &file.parsed {
            Ok(parsed) => parsed,
            Err(diagnostics) => return diagnostics.clone(),
        };

        check_parsed_source_with_asset_roots_and_diagnostics_config(
            parsed,
            asset_roots,
            diagnostics_config,
        )
        .err()
        .unwrap_or_default()
    }

    pub fn hover(&self, path: impl AsRef<Path>, position: SourcePosition) -> Option<Hover> {
        let path = path.as_ref();
        let parsed = self.parse(path).and_then(Result::ok)?;

        if let Some(hover) = self.module_hover(parsed, position) {
            return Some(hover);
        }

        if self.is_module_reference(parsed, position) {
            return None;
        }

        hover_parsed_source(parsed, position).ok().flatten()
    }

    pub fn completion(&self, path: impl AsRef<Path>, position: SourcePosition) -> Vec<Completion> {
        let path = path.as_ref();
        let Some(file) = self.file(path) else {
            return Vec::new();
        };

        match &file.parsed {
            Ok(parsed) => complete_parsed_source(parsed, position).unwrap_or_default(),
            Err(_) => complete_tua_source(path, file.source.clone(), position).unwrap_or_default(),
        }
    }

    pub fn definition(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Option<Definition> {
        let path = path.as_ref();
        let file = self.file(path)?;

        let parsed = file.parsed.as_ref().ok()?;

        if let Some(definition) = self.module_definition(parsed, position) {
            return Some(definition);
        }

        if self.is_module_reference(parsed, position) {
            return None;
        }

        definition_parsed_source(parsed, position).ok().flatten()
    }

    fn file(&self, path: &Path) -> Option<&AnalysisFile> {
        self.files.get(&normalize_path(path))
    }

    pub fn type_environment(&self, path: impl AsRef<Path>) -> Option<&TypeEnvironment> {
        self.file(path.as_ref())?.environment.as_ref()
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

    pub fn symbol_index(&self, path: impl AsRef<Path>) -> &[WorkspaceSymbol] {
        self.file(path.as_ref())
            .map(|file| file.symbols.as_slice())
            .unwrap_or_default()
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
            documentation: export.documentation,
            context: vec![format!("module: {}", path.display())],
        })
    }

    fn module_definition(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Definition> {
        if let Some(module) = require_module_at_position(&parsed.source, position) {
            let path = self.resolved_module_path(&parsed.path, &module)?;

            return Some(module_file_definition(path));
        }

        let chain = dotted_chain_at_position(&parsed.source, position)?;
        let root = chain.parts.first()?;
        let binding = self.require_binding(parsed, &root.text, position)?;
        let path = self.resolved_module_path(&parsed.path, &binding.module)?;

        if chain.cursor_index == 0 {
            return Some(module_file_definition(path));
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
}

impl AnalysisQueryCache {
    fn cached_file(
        &mut self,
        path: &Path,
        source: &str,
        kind: AnalysisSourceKind,
        config: &ProjectConfig,
        source_roots: &[PathBuf],
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

        let file = build_analysis_file(&path, source.to_owned(), kind, config, source_roots);
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
) -> AnalysisFile {
    let parsed = parse_analysis_source(path, source.clone(), kind);
    let hir = parsed.as_ref().ok().map(lower_parsed_source);
    let environment = parsed
        .as_ref()
        .ok()
        .and_then(|parsed| build_type_environment(parsed).ok());
    let diagnostics = match &parsed {
        Ok(parsed) => check_parsed_source_with_asset_roots_and_diagnostics_config(
            parsed,
            source_roots,
            &config.diagnostics,
        )
        .err()
        .unwrap_or_default(),
        Err(diagnostics) => diagnostics.clone(),
    };
    let exports = parsed.as_ref().ok().and_then(infer_module_exports);
    let require_edges = module_links_tua_source(path, source.clone());
    let symbols = parsed
        .as_ref()
        .ok()
        .map(symbol_index_for_parsed)
        .unwrap_or_default();

    AnalysisFile {
        source,
        kind,
        parsed,
        hir,
        environment,
        diagnostics,
        exports,
        require_edges,
        symbols,
    }
}

fn module_file_definition(path: &Path) -> Definition {
    Definition {
        path: path.to_path_buf(),
        span: default_span(),
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

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn symbol_index_for_parsed(parsed: &ParsedSource) -> Vec<WorkspaceSymbol> {
    let module_name = parsed
        .path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("module")
        .to_owned();
    let module_span = parsed.span.unwrap_or_else(default_span);
    let mut symbols = vec![WorkspaceSymbol {
        name: module_name.clone(),
        kind: WorkspaceSymbolKind::Module,
        path: parsed.path.clone(),
        range: module_span,
        selection_range: module_span,
        container_name: None,
    }];

    symbols.extend(parsed.type_aliases.iter().filter_map(|alias| {
        let range = alias.span?;
        let selection_range = alias.name_span.unwrap_or(range);

        Some(WorkspaceSymbol {
            name: alias.name.clone(),
            kind: WorkspaceSymbolKind::TypeAlias,
            path: parsed.path.clone(),
            range,
            selection_range,
            container_name: Some(module_name.clone()),
        })
    }));

    symbols.extend(parsed.function_annotations.iter().filter_map(|function| {
        let name = function.name.clone()?;
        let range = function.span?;
        let selection_range = function.name_span.unwrap_or(range);

        Some(WorkspaceSymbol {
            name,
            kind: WorkspaceSymbolKind::Function,
            path: parsed.path.clone(),
            range,
            selection_range,
            container_name: Some(module_name.clone()),
        })
    }));

    symbols
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
