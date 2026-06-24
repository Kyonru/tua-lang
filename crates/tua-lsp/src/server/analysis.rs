use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use tua_core::{
    build_type_environment, call_hierarchy_calls_parsed_source,
    call_hierarchy_exports_parsed_source, call_hierarchy_prepare_parsed_source,
    call_hierarchy_symbols_parsed_source, code_actions_parsed_source, complete_tua_source,
    document_highlights_parsed_source, document_symbols_parsed_source,
    global_function_definition_count, global_function_name_for_definition,
    global_references_to_definition_parsed_source, global_type_alias_name_for_definition,
    global_type_alias_references_to_definition_parsed_source, infer_module_exports,
    inlay_hints_parsed_source, is_valid_rename_identifier, module_links_tua_source,
    parse_lua_source, parse_tua_source, reference_kind_for_rename,
    references_to_definition_parsed_source, rename_target_for_definition,
    require_bindings_for_parsed_source, semantic_tokens_parsed_source, semantic_tokens_tua_source,
    signature_help_parsed_source, signature_help_tua_source, type_alias_definition_count,
    AnalysisHost as CoreAnalysisHost, AnalysisSnapshot as CoreAnalysisSnapshot, AnalysisSourceKind,
    CallHierarchyCallSite, CallHierarchyCallTarget, CallHierarchySymbol, CallHierarchySymbolKind,
    CallHierarchySymbolTarget, CodeAction, CodeActionRange, Completion, Definition, Diagnostic,
    DocumentHighlight, DocumentSymbol, InlayHint, ModuleExports, ModuleLink, ParsedSource,
    ProjectModel, ProjectModelOptions, ProjectWorkspace, Reference, RenameEdit, RenameTarget,
    SemanticToken, SignatureHelp, SourcePosition, SourceSpan, TextEdit, TuaType, TypeEnvironment,
    VfsSourceKind, WorkspaceSymbol,
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
    source: String,
    parsed: Result<ParsedSource, Vec<Diagnostic>>,
    environment: Option<TypeEnvironment>,
    exports: Option<ModuleExports>,
    require_edges: Vec<ModuleLink>,
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
            let mut config = workspace.config().clone();

            config.diagnostics = settings.diagnostics_config(config.diagnostics);
            self.core.set_config(config);
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
        self.core.hover(path, position).or_else(|| {
            self.parsed(path).and_then(|parsed| {
                tua_core::hover_parsed_source(parsed, position)
                    .ok()
                    .flatten()
            })
        })
    }

    pub(super) fn completion(&self, path: &Path, position: SourcePosition) -> Vec<Completion> {
        let Some(file) = self.file(path) else {
            return Vec::new();
        };

        let mut completions = if self.core.source(path).is_some() {
            self.core.completion(path, position)
        } else {
            match &file.parsed {
                Ok(parsed) => {
                    tua_core::complete_parsed_source(parsed, position).unwrap_or_default()
                }
                Err(_) => {
                    complete_tua_source(path, file.source.clone(), position).unwrap_or_default()
                }
            }
        };
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

    fn auto_import_completions(
        &self,
        path: &Path,
        position: SourcePosition,
        existing_labels: &BTreeSet<String>,
    ) -> Vec<Completion> {
        let Some(file) = self.file(path) else {
            return Vec::new();
        };
        let Ok(parsed) = &file.parsed else {
            return Vec::new();
        };
        let Some(prefix) = identifier_prefix_at(&file.source, position) else {
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
                current_source: &file.source,
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

    pub(super) fn definition(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<tua_core::Definition> {
        if let Some(definition) = self.core.definition(path, position) {
            return Some(definition);
        }

        let file = self.file(path)?;

        tua_core::definition_tua_source(path, file.source.clone(), position)
            .ok()
            .flatten()
    }

    pub(super) fn document_links(&self, path: &Path) -> Vec<ModuleLink> {
        self.file(path)
            .map(|file| file.require_edges.clone())
            .unwrap_or_default()
    }

    pub(super) fn document_highlights(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Vec<DocumentHighlight> {
        self.parsed(path)
            .and_then(|parsed| document_highlights_parsed_source(parsed, position).ok())
            .unwrap_or_default()
    }

    pub(super) fn document_symbols(&self, path: &Path) -> Vec<DocumentSymbol> {
        self.parsed(path)
            .map(document_symbols_parsed_source)
            .unwrap_or_default()
    }

    pub(super) fn prepare_call_hierarchy(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Vec<CallHierarchyItem> {
        let Some(parsed) = self.parsed(path) else {
            return Vec::new();
        };

        if let Some(symbol) = call_hierarchy_prepare_parsed_source(parsed, position) {
            return vec![call_hierarchy_item_from_symbol(&symbol)];
        }

        call_hierarchy_calls_parsed_source(parsed)
            .into_iter()
            .find(|call| span_contains(call.span, position))
            .and_then(|call| self.resolve_call_target(path, parsed, &call))
            .map(|symbol| vec![call_hierarchy_item_from_symbol(&symbol)])
            .unwrap_or_default()
    }

    pub(super) fn incoming_calls(
        &self,
        identity: &CallHierarchyIdentity,
    ) -> Vec<CallHierarchyIncomingCall> {
        let Some(target) = self.symbol_for_identity(identity) else {
            return Vec::new();
        };
        let mut calls = Vec::new();

        for (path, parsed) in self.parsed_files() {
            for call in call_hierarchy_calls_parsed_source(parsed) {
                let Some(callee) = self.resolve_call_target(path, parsed, &call) else {
                    continue;
                };

                if !same_call_symbol(&callee, &target) {
                    continue;
                }

                let Some(caller) = call
                    .caller
                    .as_ref()
                    .and_then(|definition| self.symbol_for_definition(definition))
                else {
                    continue;
                };

                push_incoming_call(
                    &mut calls,
                    call_hierarchy_item_from_symbol(&caller),
                    call.span,
                );
            }
        }

        sort_incoming_calls(&mut calls);
        calls
    }

    pub(super) fn outgoing_calls(
        &self,
        identity: &CallHierarchyIdentity,
    ) -> Vec<CallHierarchyOutgoingCall> {
        let Some(caller) = self.symbol_for_identity(identity) else {
            return Vec::new();
        };
        let Some(parsed) = self.parsed(&caller.path) else {
            return Vec::new();
        };
        let mut calls = Vec::new();

        for call in call_hierarchy_calls_parsed_source(parsed) {
            if !call
                .caller
                .as_ref()
                .is_some_and(|definition| definition_matches_symbol(definition, &caller))
            {
                continue;
            }

            let Some(callee) = self.resolve_call_target(&caller.path, parsed, &call) else {
                continue;
            };

            push_outgoing_call(
                &mut calls,
                call_hierarchy_item_from_symbol(&callee),
                call.span,
            );
        }

        sort_outgoing_calls(&mut calls);
        calls
    }

    pub(super) fn code_actions(
        &self,
        path: &Path,
        range: CodeActionRange,
        diagnostics: &[Diagnostic],
    ) -> Vec<CodeAction> {
        let Some(parsed) = self.parsed(path) else {
            return Vec::new();
        };
        let (source_dir, extension) = self.project_source_for_path(path).unwrap_or_else(|| {
            (
                path.parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf(),
                "tua".to_owned(),
            )
        });

        code_actions_parsed_source(parsed, diagnostics, range, &source_dir, &extension)
    }

    pub(super) fn references(
        &self,
        path: &Path,
        position: SourcePosition,
        include_declaration: bool,
    ) -> Vec<Reference> {
        let Some(target) = self.definition(path, position) else {
            return Vec::new();
        };
        let global_name = self
            .parsed(&target.path)
            .and_then(|parsed| global_function_name_for_definition(parsed, &target));
        let alias_name = self
            .parsed(&target.path)
            .and_then(|parsed| global_type_alias_name_for_definition(parsed, &target));
        let mut references = Vec::new();

        for (path, file) in &self.files {
            if self.settings.is_ignored(path, &self.workspace_folders) {
                continue;
            }

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

    pub(super) fn prepare_rename(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<RenameTarget> {
        let target = self.definition(path, position)?;
        let target_parsed = self.parsed(&target.path)?;
        let mut rename_target = rename_target_for_definition(target_parsed, target)?;
        let reference_kind = reference_kind_for_rename(rename_target.kind);
        let current_reference =
            self.references(path, position, true)
                .into_iter()
                .find(|reference| {
                    reference.path == path
                        && reference.kind == reference_kind
                        && span_contains(reference.span, position)
                })?;

        rename_target.path = current_reference.path;
        rename_target.span = current_reference.span;

        Some(rename_target)
    }

    pub(super) fn rename(
        &self,
        path: &Path,
        position: SourcePosition,
        new_name: &str,
    ) -> Option<RenameEdit> {
        if !is_valid_rename_identifier(new_name) {
            return None;
        }

        let target = self.definition(path, position)?;
        let target_parsed = self.parsed(&target.path)?;
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

    pub(super) fn inlay_hints(&self, path: &Path) -> Vec<InlayHint> {
        self.parsed(path)
            .and_then(|parsed| inlay_hints_parsed_source(parsed).ok())
            .unwrap_or_default()
    }

    pub(super) fn semantic_tokens(&self, path: &Path) -> Vec<SemanticToken> {
        let Some(file) = self.file(path) else {
            return Vec::new();
        };

        match &file.parsed {
            Ok(parsed) => semantic_tokens_parsed_source(parsed).unwrap_or_default(),
            Err(_) => semantic_tokens_tua_source(path, file.source.clone()).unwrap_or_default(),
        }
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

    fn parsed(&self, path: &Path) -> Option<&ParsedSource> {
        self.file(path).and_then(|file| file.parsed.as_ref().ok())
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

    fn project_source_for_path(&self, path: &Path) -> Option<(PathBuf, String)> {
        let model = project_model_for_roots(&self.workspace_folders, &self.settings).ok()?;
        let workspace = model.workspace_for_path(path)?;

        Some((
            workspace.primary_source_root().to_path_buf(),
            workspace.config().extension.clone(),
        ))
    }

    fn parsed_files(&self) -> Vec<(&PathBuf, &ParsedSource)> {
        let mut files = self
            .files
            .iter()
            .filter(|(path, _)| !self.settings.is_ignored(path, &self.workspace_folders))
            .filter_map(|(path, file)| file.parsed.as_ref().ok().map(|parsed| (path, parsed)))
            .collect::<Vec<_>>();

        files.sort_by(|(left, _), (right, _)| left.cmp(right));
        files
    }

    fn all_call_symbols(&self) -> Vec<CallHierarchySymbol> {
        let mut symbols = self
            .parsed_files()
            .into_iter()
            .flat_map(|(_, parsed)| call_hierarchy_symbols_parsed_source(parsed))
            .collect::<Vec<_>>();

        symbols.sort_by_key(call_hierarchy_symbol_key);
        symbols
    }

    fn symbol_for_identity(&self, identity: &CallHierarchyIdentity) -> Option<CallHierarchySymbol> {
        self.parsed(&identity.path)
            .into_iter()
            .flat_map(call_hierarchy_symbols_parsed_source)
            .find(|symbol| {
                symbol.path == identity.path && symbol.selection_range == identity.selection_range
            })
    }

    fn symbol_for_definition(&self, definition: &Definition) -> Option<CallHierarchySymbol> {
        self.parsed(&definition.path)
            .into_iter()
            .flat_map(call_hierarchy_symbols_parsed_source)
            .find(|symbol| definition_matches_symbol(definition, symbol))
    }

    fn resolve_call_target(
        &self,
        current_path: &Path,
        parsed: &ParsedSource,
        call: &CallHierarchyCallSite,
    ) -> Option<CallHierarchySymbol> {
        match &call.target {
            CallHierarchyCallTarget::Name(name) => {
                resolve_local_function_symbol(parsed, name, call.span.start)
            }
            CallHierarchyCallTarget::Field(parts) => self
                .resolve_required_module_symbol(current_path, parsed, parts)
                .or_else(|| resolve_qualified_function_symbol(parsed, parts)),
            CallHierarchyCallTarget::Method { receiver, method } => {
                let receiver_type = self.receiver_type_name(parsed, receiver, call)?;

                self.resolve_method_symbol(current_path, &receiver_type, method)
            }
        }
    }

    fn resolve_required_module_symbol(
        &self,
        current_path: &Path,
        parsed: &ParsedSource,
        parts: &[String],
    ) -> Option<CallHierarchySymbol> {
        let (receiver, fields) = parts.split_first()?;
        let field = fields.first()?;
        let module = require_bindings_for_parsed_source(parsed)
            .into_iter()
            .find(|binding| binding.alias == *receiver)?
            .module;
        let module_path = self.cached_required_module_path(current_path, &module)?;

        self.exported_symbol(&module_path, field)
    }

    fn cached_required_module_path(&self, current_path: &Path, module: &str) -> Option<PathBuf> {
        let model = project_model_for_roots(&self.workspace_folders, &self.settings).ok()?;
        let workspace = model.workspace_for_path(current_path)?;
        let mut matches = self
            .files
            .keys()
            .filter(|path| !self.settings.is_ignored(path, &self.workspace_folders))
            .filter(|path| workspace.module_name_for_path(path).as_deref() == Some(module))
            .cloned()
            .collect::<Vec<_>>();

        matches.sort();
        matches.into_iter().next()
    }

    fn exported_symbol(&self, module_path: &Path, field: &str) -> Option<CallHierarchySymbol> {
        let parsed = self.parsed(module_path)?;
        let symbols = call_hierarchy_symbols_parsed_source(parsed);

        if let Some(symbol) = symbols
            .iter()
            .find(|symbol| symbol.target == CallHierarchySymbolTarget::Export(field.to_owned()))
        {
            return Some(symbol.clone());
        }

        let exports = call_hierarchy_exports_parsed_source(parsed);
        let target_name = exports
            .iter()
            .find(|export| export.field == field)
            .and_then(|export| export.target_name.as_deref())?;

        symbols
            .into_iter()
            .find(|symbol| symbol_targets_name(symbol, target_name))
    }

    fn receiver_type_name(
        &self,
        parsed: &ParsedSource,
        receiver: &str,
        call: &CallHierarchyCallSite,
    ) -> Option<String> {
        let local = parsed
            .local_annotations
            .iter()
            .filter(|annotation| annotation.name == receiver)
            .filter(|annotation| {
                annotation
                    .name_span
                    .or(annotation.span)
                    .is_some_and(|span| position_key(span.start) <= position_key(call.span.start))
            })
            .max_by_key(|annotation| {
                annotation
                    .name_span
                    .or(annotation.span)
                    .map(|span| position_key(span.start))
                    .unwrap_or((0, 0))
            })
            .and_then(|annotation| type_name_from_tua_type(&annotation.ty));

        if local.is_some() {
            return local;
        }

        let caller = call.caller.as_ref()?;

        parsed
            .function_annotations
            .iter()
            .find(|function| function.name_span == Some(caller.span))
            .and_then(|function| {
                function
                    .parameters
                    .iter()
                    .find(|parameter| parameter.name == receiver)
                    .and_then(|parameter| type_name_from_tua_type(&parameter.ty))
            })
    }

    fn resolve_method_symbol(
        &self,
        current_path: &Path,
        receiver_type: &str,
        method: &str,
    ) -> Option<CallHierarchySymbol> {
        let mut matches = self
            .all_call_symbols()
            .into_iter()
            .filter(|symbol| {
                symbol.target
                    == CallHierarchySymbolTarget::Method {
                        receiver: receiver_type.to_owned(),
                        method: method.to_owned(),
                    }
            })
            .collect::<Vec<_>>();

        let same_file = matches
            .iter()
            .filter(|symbol| symbol.path == current_path)
            .cloned()
            .collect::<Vec<_>>();

        if same_file.len() == 1 {
            return same_file.into_iter().next();
        }

        (matches.len() == 1).then(|| matches.remove(0))
    }

    fn is_ambiguous_project_rename(
        &self,
        target: &tua_core::Definition,
        rename_target: &RenameTarget,
    ) -> bool {
        match rename_target.kind {
            tua_core::RenameKind::Function => {
                let Some(global_name) = self
                    .parsed(&target.path)
                    .and_then(|parsed| global_function_name_for_definition(parsed, target))
                else {
                    return false;
                };

                self.files
                    .iter()
                    .filter(|(path, _)| !self.settings.is_ignored(path, &self.workspace_folders))
                    .filter_map(|(_, file)| file.parsed.as_ref().ok())
                    .map(|parsed| global_function_definition_count(parsed, &global_name))
                    .sum::<usize>()
                    > 1
            }
            tua_core::RenameKind::TypeAlias => {
                let Some(alias_name) = self
                    .parsed(&target.path)
                    .and_then(|parsed| global_type_alias_name_for_definition(parsed, target))
                else {
                    return false;
                };

                self.files
                    .iter()
                    .filter(|(path, _)| !self.settings.is_ignored(path, &self.workspace_folders))
                    .filter_map(|(_, file)| file.parsed.as_ref().ok())
                    .map(|parsed| type_alias_definition_count(parsed, &alias_name))
                    .sum::<usize>()
                    > 1
            }
            tua_core::RenameKind::Local | tua_core::RenameKind::Parameter => false,
        }
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

fn push_incoming_call(
    calls: &mut Vec<CallHierarchyIncomingCall>,
    from: CallHierarchyItem,
    from_range: SourceSpan,
) {
    if let Some(existing) = calls
        .iter_mut()
        .find(|call| same_call_hierarchy_item(&call.from, &from))
    {
        existing.from_ranges.push(from_range);
        return;
    }

    calls.push(CallHierarchyIncomingCall {
        from,
        from_ranges: vec![from_range],
    });
}

fn push_outgoing_call(
    calls: &mut Vec<CallHierarchyOutgoingCall>,
    to: CallHierarchyItem,
    from_range: SourceSpan,
) {
    if let Some(existing) = calls
        .iter_mut()
        .find(|call| same_call_hierarchy_item(&call.to, &to))
    {
        existing.from_ranges.push(from_range);
        return;
    }

    calls.push(CallHierarchyOutgoingCall {
        to,
        from_ranges: vec![from_range],
    });
}

fn sort_incoming_calls(calls: &mut [CallHierarchyIncomingCall]) {
    for call in calls.iter_mut() {
        sort_source_ranges(&mut call.from_ranges);
    }

    calls.sort_by_key(|call| call_hierarchy_item_key(&call.from));
}

fn sort_outgoing_calls(calls: &mut [CallHierarchyOutgoingCall]) {
    for call in calls.iter_mut() {
        sort_source_ranges(&mut call.from_ranges);
    }

    calls.sort_by_key(|call| call_hierarchy_item_key(&call.to));
}

fn sort_source_ranges(ranges: &mut [SourceSpan]) {
    ranges.sort_by_key(source_span_key);
}

fn same_call_hierarchy_item(left: &CallHierarchyItem, right: &CallHierarchyItem) -> bool {
    left.path == right.path && left.selection_range == right.selection_range
}

fn same_call_symbol(left: &CallHierarchySymbol, right: &CallHierarchySymbol) -> bool {
    left.path == right.path && left.selection_range == right.selection_range
}

fn definition_matches_symbol(definition: &Definition, symbol: &CallHierarchySymbol) -> bool {
    definition.path == symbol.path && definition.span == symbol.selection_range
}

fn resolve_local_function_symbol(
    parsed: &ParsedSource,
    name: &str,
    position: SourcePosition,
) -> Option<CallHierarchySymbol> {
    call_hierarchy_symbols_parsed_source(parsed)
        .into_iter()
        .filter(|symbol| symbol_targets_name(symbol, name))
        .filter(|symbol| position_key(symbol.selection_range.start) <= position_key(position))
        .max_by_key(call_hierarchy_symbol_key)
}

fn resolve_qualified_function_symbol(
    parsed: &ParsedSource,
    parts: &[String],
) -> Option<CallHierarchySymbol> {
    call_hierarchy_symbols_parsed_source(parsed)
        .into_iter()
        .find(|symbol| symbol.target == CallHierarchySymbolTarget::Qualified(parts.to_vec()))
}

fn symbol_targets_name(symbol: &CallHierarchySymbol, name: &str) -> bool {
    match &symbol.target {
        CallHierarchySymbolTarget::Local(candidate) => candidate == name,
        CallHierarchySymbolTarget::Qualified(parts) => {
            parts.len() == 1 && parts.first().is_some_and(|candidate| candidate == name)
        }
        CallHierarchySymbolTarget::Method { .. } | CallHierarchySymbolTarget::Export(_) => false,
    }
}

fn type_name_from_tua_type(ty: &TuaType) -> Option<String> {
    match ty {
        TuaType::Named(name) if name == "any" => None,
        TuaType::Named(name) => Some(name.clone()),
        TuaType::Union(types) => {
            let names = types
                .iter()
                .filter_map(type_name_from_tua_type)
                .filter(|name| name != "nil")
                .collect::<BTreeSet<_>>();

            (names.len() == 1)
                .then(|| names.into_iter().next())
                .flatten()
        }
        TuaType::Table(_) => None,
    }
}

fn call_hierarchy_item_key(item: &CallHierarchyItem) -> (PathBuf, usize, usize, usize, usize) {
    (
        item.path.clone(),
        item.selection_range.start.line,
        item.selection_range.start.column,
        item.selection_range.end.line,
        item.selection_range.end.column,
    )
}

fn call_hierarchy_symbol_key(
    symbol: &CallHierarchySymbol,
) -> (PathBuf, usize, usize, usize, usize) {
    (
        symbol.path.clone(),
        symbol.selection_range.start.line,
        symbol.selection_range.start.column,
        symbol.selection_range.end.line,
        symbol.selection_range.end.column,
    )
}

fn source_span_key(span: &SourceSpan) -> (usize, usize, usize, usize) {
    (
        span.start.line,
        span.start.column,
        span.end.line,
        span.end.column,
    )
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
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

fn text_edit_key(edit: &TextEdit) -> (PathBuf, usize, usize, usize, usize) {
    (
        edit.path.clone(),
        edit.span.start.line,
        edit.span.start.column,
        edit.span.end.line,
        edit.span.end.column,
    )
}

fn span_contains(span: SourceSpan, position: SourcePosition) -> bool {
    let start = (span.start.line, span.start.column);
    let end = (span.end.line, span.end.column);
    let position = (position.line, position.column);

    start <= position && position < end
}

fn reference_kind_key(kind: tua_core::ReferenceKind) -> u8 {
    match kind {
        tua_core::ReferenceKind::Module => 0,
        tua_core::ReferenceKind::TypeAlias => 1,
        tua_core::ReferenceKind::Local => 2,
        tua_core::ReferenceKind::Function => 3,
        tua_core::ReferenceKind::Parameter => 4,
        tua_core::ReferenceKind::TableField => 5,
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
        let require_edges = module_links_tua_source(&path, source.clone());

        Self {
            revision,
            source,
            parsed,
            environment,
            exports,
            require_edges,
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
