use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use tua_core::{
    build_type_environment, infer_module_exports, parse_lua_source, parse_tua_source,
    require_bindings_for_parsed_source, AnalysisCacheStats, AnalysisHost as CoreAnalysisHost,
    AnalysisSnapshot as CoreAnalysisSnapshot, AnalysisSourceKind,
    CallHierarchyIncomingCall as CoreCallHierarchyIncomingCall,
    CallHierarchyOutgoingCall as CoreCallHierarchyOutgoingCall, CallHierarchySymbol,
    CallHierarchySymbolKind, CodeAction, CodeActionRange, Completion, Diagnostic,
    DocumentHighlight, DocumentSymbol, FoldingRange, InlayHint, ModuleExports, ModuleLink,
    ParsedSource, ProjectModel, ProjectModelOptions, ProjectWorkspace, Reference, RenameEdit,
    RenameTarget, SelectionRange, SemanticToken, SignatureHelp, SourcePosition, SourceSpan,
    TypeEnvironment, VfsSourceKind, WorkspaceSymbol,
};

use super::auto_import::{
    auto_import_completion, identifier_prefix_at, unique_import_alias, AutoImportCandidate,
};
use super::configuration::{SourceKind, WorkspaceSettings};
use super::uri::path_from_uri;

#[derive(Default)]
pub(super) struct AnalysisHost {
    core: CoreAnalysisHost,
    documents: HashMap<String, OpenDocument>,
    files: HashMap<PathBuf, CachedFile>,
    next_revision: u64,
}

#[derive(Clone, Debug)]
pub(super) struct AnalysisSnapshot {
    core: CoreAnalysisSnapshot,
    documents: HashMap<String, OpenDocument>,
    files: HashMap<PathBuf, CachedFile>,
    workspace_folders: Vec<PathBuf>,
    settings: WorkspaceSettings,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CallHierarchyItem {
    pub(super) name: String,
    pub(super) kind: CallHierarchyItemKind,
    pub(super) path: PathBuf,
    pub(super) range: SourceSpan,
    pub(super) selection_range: SourceSpan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CallHierarchyItemKind {
    Function,
    Method,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CallHierarchyIdentity {
    pub(super) path: PathBuf,
    pub(super) selection_range: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CallHierarchyIncomingCall {
    pub(super) from: CallHierarchyItem,
    pub(super) from_ranges: Vec<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CallHierarchyOutgoingCall {
    pub(super) to: CallHierarchyItem,
    pub(super) from_ranges: Vec<SourceSpan>,
}

#[derive(Clone, Debug)]
struct OpenDocument {
    path: PathBuf,
    source: String,
}

#[derive(Clone, Debug)]
struct CachedFile {
    #[cfg_attr(not(test), allow(dead_code))]
    revision: u64,
    parsed: Result<ParsedSource, Vec<Diagnostic>>,
    environment: Option<TypeEnvironment>,
    exports: Option<ModuleExports>,
}

impl AnalysisHost {
    pub(super) fn snapshot(
        &self,
        workspace_folders: &[PathBuf],
        settings: &WorkspaceSettings,
    ) -> AnalysisSnapshot {
        AnalysisSnapshot {
            core: self.core.snapshot(),
            documents: self.documents.clone(),
            files: self.files.clone(),
            workspace_folders: workspace_folders.to_vec(),
            settings: settings.clone(),
        }
    }

    pub(super) fn cache_stats(&self) -> AnalysisCacheStats {
        self.core.cache_stats()
    }

    pub(super) fn open_document(
        &mut self,
        uri: impl Into<String>,
        source: impl Into<String>,
        roots: &[PathBuf],
        settings: &WorkspaceSettings,
    ) {
        let uri = uri.into();
        let path = path_from_uri(&uri);
        let source = source.into();

        self.sync_core_project_inputs(roots, settings);
        self.documents.insert(
            uri,
            OpenDocument {
                path: path.clone(),
                source: source.clone(),
            },
        );
        if let Some(kind) = self.cache_source(path.clone(), source.clone(), roots, settings) {
            self.core
                .open_document_with_kind(path, source, core_source_kind(kind));
        } else {
            self.core.remove_file(path);
        }
    }

    pub(super) fn update_document(
        &mut self,
        uri: impl Into<String>,
        source: impl Into<String>,
        roots: &[PathBuf],
        settings: &WorkspaceSettings,
    ) {
        self.open_document(uri, source, roots, settings);
    }

    pub(super) fn close_document(&mut self, uri: &str) {
        if let Some(document) = self.documents.remove(uri) {
            self.files.remove(&document.path);
            self.core.remove_file(&document.path);
        }
    }

    pub(super) fn remove_file(&mut self, uri: &str) {
        let path = path_from_uri(uri);

        self.documents.remove(uri);
        self.files.remove(&path);
        self.core.remove_file(&path);
    }

    pub(super) fn cache_disk_file(
        &mut self,
        uri: &str,
        roots: &[PathBuf],
        settings: &WorkspaceSettings,
    ) {
        let path = path_from_uri(uri);

        self.sync_core_project_inputs(roots, settings);
        if self
            .documents
            .values()
            .any(|document| document.path == path)
        {
            return;
        }

        if settings.is_ignored(&path, roots) {
            self.files.remove(&path);
            return;
        }

        if let Ok(source) = fs::read_to_string(&path) {
            self.cache_source(path, source, roots, settings);
        }
    }

    pub(super) fn document_text(&self, uri: &str) -> Option<&str> {
        self.documents
            .get(uri)
            .map(|document| document.source.as_str())
    }

    pub(super) fn contains_document(&self, uri: &str) -> bool {
        self.documents.contains_key(uri)
    }

    pub(super) fn open_uris(&self) -> Vec<String> {
        self.documents.keys().cloned().collect()
    }

    pub(super) fn refresh_open_documents(
        &mut self,
        roots: &[PathBuf],
        settings: &WorkspaceSettings,
    ) {
        self.sync_core_project_inputs(roots, settings);
        let documents = self
            .documents
            .values()
            .map(|document| (document.path.clone(), document.source.clone()))
            .collect::<Vec<_>>();

        for (path, source) in documents {
            self.cache_source(path, source, roots, settings);
        }
    }

    pub(super) fn reindex_workspace(&mut self, roots: &[PathBuf], settings: &WorkspaceSettings) {
        self.sync_core_project_inputs(roots, settings);
        let open_paths = self
            .documents
            .values()
            .map(|document| document.path.clone())
            .collect::<BTreeSet<_>>();
        let model = project_model_for_roots(roots, settings).ok();

        self.files.retain(|path, _| {
            (open_paths.contains(path) && !settings.is_ignored(path, roots))
                || !project_contains_path(model.as_ref(), path)
        });

        let Some(model) = model else {
            return;
        };

        for workspace in model.workspaces() {
            for source_dir in workspace.source_roots() {
                for path in discover_project_sources(source_dir, workspace, settings) {
                    if open_paths.contains(&path) {
                        continue;
                    }

                    if let Ok(source) = fs::read_to_string(&path) {
                        self.cache_source(path, source, roots, settings);
                    }
                }
            }
        }
    }

    fn cache_source(
        &mut self,
        path: PathBuf,
        source: String,
        roots: &[PathBuf],
        settings: &WorkspaceSettings,
    ) -> Option<SourceKind> {
        self.sync_core_project_inputs(roots, settings);
        if settings.is_ignored(&path, roots) {
            self.files.remove(&path);
            self.core.remove_file(&path);
            return None;
        }

        let Some(kind) = source_kind_for_path(&path, roots, settings) else {
            self.files.remove(&path);
            self.core.remove_file(&path);
            return None;
        };

        self.next_revision += 1;
        let core_source = source.clone();
        self.files.insert(
            path.clone(),
            CachedFile::new(path.clone(), source, self.next_revision, kind),
        );
        self.core
            .set_file_with_kind(path, core_source, core_source_kind(kind));

        Some(kind)
    }

    fn sync_core_project_inputs(&mut self, roots: &[PathBuf], settings: &WorkspaceSettings) {
        let Ok(model) = project_model_for_roots(roots, settings) else {
            return;
        };
        let source_roots = model
            .workspaces()
            .iter()
            .flat_map(|workspace| workspace.source_roots().iter().cloned())
            .collect::<Vec<_>>();
        let output_roots = model
            .workspaces()
            .iter()
            .flat_map(|workspace| workspace.output_roots().iter().cloned())
            .collect::<Vec<_>>();
        let ignored_paths = model
            .workspaces()
            .iter()
            .flat_map(|workspace| workspace.ignored_paths().iter().cloned())
            .collect::<Vec<_>>();
        let asset_roots = model
            .workspaces()
            .iter()
            .flat_map(|workspace| workspace.asset_roots())
            .collect::<Vec<_>>();

        self.core.set_source_roots(source_roots);
        self.core.set_output_roots(output_roots);
        self.core.set_ignored_paths(ignored_paths);
        self.core.set_asset_roots(asset_roots);

        if let Some(workspace) = model.primary_workspace() {
            self.core
                .set_config(settings.apply_project_config(workspace.config().clone()));
        }
    }

    #[cfg(test)]
    fn cache_revision_for_uri(&self, uri: &str) -> Option<u64> {
        let path = path_from_uri(uri);

        self.files.get(&path).map(|file| file.revision)
    }
}

impl AnalysisSnapshot {
    pub(super) fn diagnostics_for_uri(&self, uri: &str) -> Vec<Diagnostic> {
        let path = path_from_uri(uri);

        if self.core.source(&path).is_some() {
            return self.core.pipeline_diagnostics(&path);
        }

        self.file(&path)
            .map(CachedFile::diagnostics)
            .unwrap_or_default()
    }

    pub(super) fn hover(&self, path: &Path, position: SourcePosition) -> Option<tua_core::Hover> {
        self.core.hover(path, position)
    }

    pub(super) fn completion(&self, path: &Path, position: SourcePosition) -> Vec<Completion> {
        if self.core.source(path).is_none() {
            return Vec::new();
        }

        let mut completions = self.core.completion(path, position);
        let existing_labels = completions
            .iter()
            .map(|completion| completion.label.clone())
            .collect::<BTreeSet<_>>();

        completions.extend(self.auto_import_completions(path, position, &existing_labels));
        completions
    }

    pub(super) fn resolve_completion(
        &self,
        path: &Path,
        position: SourcePosition,
        label: &str,
        kind: Option<tua_core::CompletionKind>,
        module_path: Option<&Path>,
        source_module: Option<&str>,
    ) -> Option<Completion> {
        self.core
            .resolve_completion(path, position, label, kind, module_path, source_module)
            .or_else(|| {
                self.completion(path, position)
                    .into_iter()
                    .find(|completion| {
                        completion.label == label
                            && kind.is_none_or(|kind| completion.kind == kind)
                            && module_path
                                .is_none_or(|path| completion.module_path.as_deref() == Some(path))
                            && source_module.is_none_or(|module| {
                                completion.source_module.as_deref() == Some(module)
                            })
                    })
            })
    }

    fn auto_import_completions(
        &self,
        path: &Path,
        position: SourcePosition,
        existing_labels: &BTreeSet<String>,
    ) -> Vec<Completion> {
        let Some(source) = self.core.source(path) else {
            return Vec::new();
        };
        let Some(Ok(parsed)) = self.core.parse(path) else {
            return Vec::new();
        };
        let Some(prefix) = identifier_prefix_at(source, position) else {
            return Vec::new();
        };

        if prefix.is_empty() {
            return Vec::new();
        }

        let Some(model) = project_model_for_roots(&self.workspace_folders, &self.settings).ok()
        else {
            return Vec::new();
        };
        let Some(workspace) = model.workspace_for_path(path) else {
            return Vec::new();
        };

        let require_aliases = require_bindings_for_parsed_source(parsed)
            .into_iter()
            .map(|binding| (binding.module, binding.alias))
            .collect::<BTreeMap<_, _>>();
        let reserved_aliases = existing_labels
            .iter()
            .chain(require_aliases.values())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut completions = Vec::new();
        for symbol in self.core.importable_symbols(&prefix) {
            if symbol.module_path == path
                || !project_contains_path(Some(&model), &symbol.module_path)
                || self
                    .settings
                    .is_ignored(&symbol.module_path, &self.workspace_folders)
            {
                continue;
            }

            let Some(export) = symbol.export.as_ref() else {
                continue;
            };
            let module = symbol
                .module_name
                .clone()
                .or_else(|| workspace.module_name_for_path(&symbol.module_path));
            let Some(module) = module else {
                continue;
            };

            let existing_alias = require_aliases.get(&module).cloned();
            let alias = existing_alias
                .clone()
                .unwrap_or_else(|| unique_import_alias(&module, &reserved_aliases));

            if existing_labels.contains(&symbol.name) {
                continue;
            }

            completions.push(auto_import_completion(AutoImportCandidate {
                current_path: path,
                current_source: source,
                module_path: &symbol.module_path,
                module: &module,
                alias: &alias,
                already_imported: existing_alias.is_some(),
                name: &export.public_name,
                field: &export.field,
            }));
        }

        completions.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.source_module.cmp(&right.source_module))
        });
        completions
    }

    pub(super) fn signature_help(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<SignatureHelp> {
        self.core.signature_help(path, position)
    }

    pub(super) fn definition(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<tua_core::Definition> {
        self.core.definition(path, position)
    }

    pub(super) fn document_links(&self, path: &Path) -> Vec<ModuleLink> {
        self.core.document_links(path)
    }

    pub(super) fn document_highlights(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Vec<DocumentHighlight> {
        self.core.document_highlights(path, position)
    }

    pub(super) fn document_symbols(&self, path: &Path) -> Vec<DocumentSymbol> {
        self.core.document_symbols(path)
    }

    pub(super) fn selection_ranges(
        &self,
        path: &Path,
        positions: &[SourcePosition],
    ) -> Vec<Option<SelectionRange>> {
        self.core.selection_ranges(path, positions)
    }

    pub(super) fn folding_ranges(&self, path: &Path) -> Vec<FoldingRange> {
        self.core.folding_ranges(path)
    }

    pub(super) fn prepare_call_hierarchy(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Vec<CallHierarchyItem> {
        self.core
            .prepare_call_hierarchy(path, position)
            .into_iter()
            .map(|symbol| call_hierarchy_item_from_symbol(&symbol))
            .collect()
    }

    pub(super) fn incoming_calls(
        &self,
        identity: &CallHierarchyIdentity,
    ) -> Vec<CallHierarchyIncomingCall> {
        self.core
            .call_hierarchy_incoming(&identity.path, identity.selection_range)
            .into_iter()
            .map(call_hierarchy_incoming_from_core)
            .collect()
    }

    pub(super) fn outgoing_calls(
        &self,
        identity: &CallHierarchyIdentity,
    ) -> Vec<CallHierarchyOutgoingCall> {
        self.core
            .call_hierarchy_outgoing(&identity.path, identity.selection_range)
            .into_iter()
            .map(call_hierarchy_outgoing_from_core)
            .collect()
    }

    pub(super) fn code_actions(
        &self,
        path: &Path,
        range: CodeActionRange,
        diagnostics: &[Diagnostic],
    ) -> Vec<CodeAction> {
        self.core.code_actions(path, range, diagnostics)
    }

    pub(super) fn references(
        &self,
        path: &Path,
        position: SourcePosition,
        include_declaration: bool,
    ) -> Vec<Reference> {
        self.core.references(path, position, include_declaration)
    }

    pub(super) fn prepare_rename(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<RenameTarget> {
        self.core.prepare_rename(path, position)
    }

    pub(super) fn rename(
        &self,
        path: &Path,
        position: SourcePosition,
        new_name: &str,
    ) -> Option<RenameEdit> {
        self.core.rename(path, position, new_name)
    }

    pub(super) fn inlay_hints(&self, path: &Path) -> Vec<InlayHint> {
        self.core.inlay_hints(path)
    }

    pub(super) fn semantic_tokens(&self, path: &Path) -> Vec<SemanticToken> {
        self.core.semantic_tokens(path)
    }

    pub(super) fn workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let model = project_model_for_roots(&self.workspace_folders, &self.settings).ok();

        self.core
            .workspace_symbols(query)
            .into_iter()
            .filter(|symbol| {
                project_contains_path(model.as_ref(), &symbol.path)
                    && !self
                        .settings
                        .is_ignored(&symbol.path, &self.workspace_folders)
            })
            .collect()
    }

    fn file(&self, path: &Path) -> Option<&CachedFile> {
        if self.settings.is_ignored(path, &self.workspace_folders) {
            return None;
        }

        self.files.get(path).or_else(|| {
            self.documents
                .values()
                .find(|document| document.path == path)
                .and_then(|document| self.files.get(&document.path))
        })
    }
}

fn call_hierarchy_item_from_symbol(symbol: &CallHierarchySymbol) -> CallHierarchyItem {
    CallHierarchyItem {
        name: symbol.name.clone(),
        kind: match symbol.kind {
            CallHierarchySymbolKind::Function => CallHierarchyItemKind::Function,
            CallHierarchySymbolKind::Method => CallHierarchyItemKind::Method,
        },
        path: symbol.path.clone(),
        range: symbol.range,
        selection_range: symbol.selection_range,
    }
}

fn call_hierarchy_incoming_from_core(
    call: CoreCallHierarchyIncomingCall,
) -> CallHierarchyIncomingCall {
    CallHierarchyIncomingCall {
        from: call_hierarchy_item_from_symbol(&call.from),
        from_ranges: call.from_ranges,
    }
}

fn call_hierarchy_outgoing_from_core(
    call: CoreCallHierarchyOutgoingCall,
) -> CallHierarchyOutgoingCall {
    CallHierarchyOutgoingCall {
        to: call_hierarchy_item_from_symbol(&call.to),
        from_ranges: call.from_ranges,
    }
}

impl CachedFile {
    fn new(path: PathBuf, source: String, revision: u64, kind: SourceKind) -> Self {
        let parsed = parse_source(&path, source.clone(), kind);
        let environment = parsed
            .as_ref()
            .ok()
            .and_then(|parsed| build_type_environment(parsed).ok());
        let exports = parsed.as_ref().ok().and_then(infer_module_exports);

        Self {
            revision,
            parsed,
            environment,
            exports,
        }
    }

    fn diagnostics(&self) -> Vec<Diagnostic> {
        match &self.parsed {
            Ok(_) => Vec::new(),
            Err(diagnostics) => diagnostics.clone(),
        }
    }

    #[allow(dead_code)]
    fn environment(&self) -> Option<&TypeEnvironment> {
        self.environment.as_ref()
    }

    #[allow(dead_code)]
    fn exports(&self) -> Option<&ModuleExports> {
        self.exports.as_ref()
    }
}

fn parse_source(
    path: &Path,
    source: String,
    kind: SourceKind,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    match kind {
        SourceKind::Lua => parse_lua_source(path, source),
        SourceKind::Tua => parse_tua_source(path, source),
    }
}

fn core_source_kind(kind: SourceKind) -> AnalysisSourceKind {
    match kind {
        SourceKind::Lua => AnalysisSourceKind::Lua,
        SourceKind::Tua => AnalysisSourceKind::Tua,
    }
}

fn source_kind_from_vfs(kind: VfsSourceKind) -> SourceKind {
    match kind {
        VfsSourceKind::Lua => SourceKind::Lua,
        VfsSourceKind::Tua => SourceKind::Tua,
    }
}

fn project_model_for_roots(
    roots: &[PathBuf],
    settings: &WorkspaceSettings,
) -> Result<ProjectModel, tua_core::ConfigError> {
    ProjectModel::load_with_options(ProjectModelOptions {
        workspace_folders: roots.to_vec(),
        ignored_paths: settings.ignored_paths.clone(),
        file_associations: settings.project_file_associations(),
        require_path_separator: settings.module_path_separator(),
        ..ProjectModelOptions::default()
    })
}

fn project_contains_path(model: Option<&ProjectModel>, path: &Path) -> bool {
    model.is_some_and(|model| {
        model
            .workspaces()
            .iter()
            .any(|workspace| path.starts_with(workspace.root()))
    })
}

fn discover_project_sources(
    source_dir: &Path,
    workspace: &ProjectWorkspace,
    settings: &WorkspaceSettings,
) -> Vec<PathBuf> {
    let mut files = Vec::new();

    discover_project_sources_into(source_dir, workspace, settings, &mut files);
    files.sort();
    files
}

fn discover_project_sources_into(
    dir: &Path,
    workspace: &ProjectWorkspace,
    settings: &WorkspaceSettings,
    files: &mut Vec<PathBuf>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            if !workspace.is_ignored(&path)
                && !settings.is_ignored(&path, &[workspace.root().to_path_buf()])
            {
                discover_project_sources_into(&path, workspace, settings, files);
            }
        } else if file_type.is_file()
            && !workspace.is_ignored(&path)
            && !settings.is_ignored(&path, &[workspace.root().to_path_buf()])
            && workspace.source_kind_for_path(&path).is_some()
        {
            files.push(path);
        }
    }
}

fn source_kind_for_path(
    path: &Path,
    roots: &[PathBuf],
    settings: &WorkspaceSettings,
) -> Option<SourceKind> {
    project_model_for_roots(roots, settings)
        .ok()
        .and_then(|model| {
            model
                .workspace_for_path(path)
                .and_then(|workspace| workspace.source_kind_for_path(path))
        })
        .map(source_kind_from_vfs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updating_one_open_document_invalidates_only_that_cached_file() {
        let mut host = AnalysisHost::default();
        let roots = Vec::new();
        let settings = WorkspaceSettings::default();
        let player_uri = "file:///tmp/analysis-player.tua";
        let enemy_uri = "file:///tmp/analysis-enemy.tua";

        host.open_document(player_uri, "local hp: number = 1\n", &roots, &settings);
        host.open_document(enemy_uri, "local hp: number = 2\n", &roots, &settings);

        let player_before = host
            .cache_revision_for_uri(player_uri)
            .expect("player should be cached");
        let enemy_before = host
            .cache_revision_for_uri(enemy_uri)
            .expect("enemy should be cached");

        host.update_document(player_uri, "local hp: number = 3\n", &roots, &settings);

        let player_after = host
            .cache_revision_for_uri(player_uri)
            .expect("player should stay cached");
        let enemy_after = host
            .cache_revision_for_uri(enemy_uri)
            .expect("enemy should stay cached");

        assert!(player_after > player_before);
        assert_eq!(enemy_after, enemy_before);
    }
}
