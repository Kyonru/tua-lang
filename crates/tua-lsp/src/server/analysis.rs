use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use tua_core::{
    require_bindings_for_parsed_source, AnalysisCacheStats, AnalysisHost as CoreAnalysisHost,
    AnalysisSnapshot as CoreAnalysisSnapshot, AnalysisSourceKind, AssetLoaderKind,
    CallHierarchyIncomingCall as CoreCallHierarchyIncomingCall,
    CallHierarchyOutgoingCall as CoreCallHierarchyOutgoingCall, CallHierarchySymbol,
    CallHierarchySymbolKind, CodeAction, CodeActionRange, CodeLens, Completion,
    CompletionAdaptiveStats, Diagnostic, DocumentHighlight, DocumentSymbol, FoldingRange,
    FormatOptions, InlayHint, InspectorItem, ModuleLink, ModuleSpecifierKind, ProjectModel,
    ProjectModelOptions, ProjectWorkspace, Reference, RenameEdit, RenameTarget, SelectionRange,
    SemanticToken, SignalGraph, SignatureHelp, SourcePosition, SourceSpan, TextEdit, VfsSourceKind,
    WorkspaceSymbol, CONFIG_FILE,
};

use super::auto_import::{
    auto_import_completion, identifier_prefix_at, unique_import_alias, AutoImportCandidate,
};
use super::completion_stats::{
    current_epoch, file_scope_hash, load_completion_stats, remove_completion_stats,
    retain_observed_file_scopes, save_completion_stats,
};
use super::configuration::{SourceKind, WorkspaceSettings};
use super::uri::path_from_uri;

const MIN_AUTO_IMPORT_PREFIX_CHARS: usize = 2;

#[derive(Clone, Default)]
pub(super) struct AnalysisHost {
    core: CoreAnalysisHost,
    documents: HashMap<String, OpenDocument>,
    project_model: Option<ProjectModel>,
}

#[derive(Clone, Debug)]
pub(super) struct AnalysisSnapshot {
    core: CoreAnalysisSnapshot,
    workspace_folders: Vec<PathBuf>,
    settings: WorkspaceSettings,
    project_model: Option<ProjectModel>,
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
    config_root: Option<PathBuf>,
}

impl AnalysisHost {
    pub(super) fn snapshot(
        &self,
        workspace_folders: &[PathBuf],
        settings: &WorkspaceSettings,
    ) -> AnalysisSnapshot {
        AnalysisSnapshot {
            core: self.core.snapshot(),
            workspace_folders: self.effective_workspace_folders(workspace_folders),
            settings: settings.clone(),
            project_model: self.project_model.clone(),
        }
    }

    pub(super) fn cache_stats(&self) -> AnalysisCacheStats {
        self.core.cache_stats()
    }

    pub(super) fn reset_completion_adaptive_stats(
        &mut self,
        roots: &[PathBuf],
        settings: &WorkspaceSettings,
    ) -> usize {
        self.sync_core_project_inputs(roots, settings);
        self.core.reset_completion_adaptive_stats();

        let Some(model) = self.project_model.as_ref() else {
            return 0;
        };

        model
            .workspaces()
            .iter()
            .filter_map(|workspace| remove_completion_stats(workspace.root()).ok())
            .filter(|removed| *removed)
            .count()
    }

    pub(super) fn fast_identifier_completion_for_uri(
        &self,
        uri: &str,
        position: SourcePosition,
    ) -> Option<(SourcePosition, Vec<Completion>)> {
        let document = self.documents.get(uri)?;
        let source = &document.source;
        let position = identifier_completion_position(source, position);

        identifier_prefix_at(source, position)?;

        if position_is_inside_open_table(source, position) {
            return None;
        }

        let completions = self.core.current_file_completion(&document.path, position);

        Some((position, completions))
    }

    pub(super) fn fast_field_completion_for_uri(
        &self,
        uri: &str,
        position: SourcePosition,
    ) -> Option<Vec<Completion>> {
        let document = self.documents.get(uri)?;

        self.core
            .current_file_field_completion(&document.path, position)
    }

    pub(super) fn fast_hover_for_uri(
        &self,
        uri: &str,
        position: SourcePosition,
    ) -> Option<tua_core::Hover> {
        let document = self.documents.get(uri)?;

        self.core.current_file_hover(&document.path, position)
    }

    pub(super) fn fast_diagnostics_for_uri(&self, uri: &str) -> Vec<Diagnostic> {
        if !self.contains_document(uri) {
            return Vec::new();
        }

        self.core.fast_diagnostics(path_from_uri(uri))
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

    pub(super) fn semantic_tokens(&self, path: &Path) -> Vec<SemanticToken> {
        self.core.semantic_tokens(path)
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
        let previous_roots = self.effective_workspace_folders(roots);
        let config_root = self
            .documents
            .get(&uri)
            .and_then(|document| document.config_root.clone())
            .or_else(|| nearest_config_root(&path));

        {
            let _trace = tua_core::profile::diagnostics_span(
                tua_core::profile::DiagnosticsTraceCategory::ReadVfsUpdate,
            );

            self.sync_core_project_inputs(roots, settings);
            self.documents.insert(
                uri,
                OpenDocument {
                    path: path.clone(),
                    source: source.clone(),
                    config_root,
                },
            );
        }
        let current_roots = self.effective_workspace_folders(roots);
        if path_sets_differ(&previous_roots, &current_roots) {
            self.reindex_workspace(roots, settings);
        }
        let kind = {
            let _trace = tua_core::profile::diagnostics_span(
                tua_core::profile::DiagnosticsTraceCategory::ReadVfsUpdate,
            );

            self.cache_source_after_sync(path.clone(), source.clone(), &current_roots, settings)
        };
        if let Some(kind) = kind {
            self.core
                .open_document_with_kind(path.clone(), source.clone(), core_source_kind(kind));
        } else {
            self.remove_completion_adaptive_source(&path);
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
        let uri = uri.into();
        let source = source.into();

        let Some(document) = self.documents.get_mut(&uri) else {
            self.open_document(uri, source, roots, settings);
            return;
        };

        document.source = source.clone();
        let path = document.path.clone();
        let effective_roots = self.effective_workspace_folders(roots);

        if settings.is_ignored(&path, &effective_roots) {
            self.remove_completion_adaptive_source(&path);
            self.core.remove_file(path);
            return;
        }

        let Some(kind) = source_kind_for_path(&path, self.project_model.as_ref()) else {
            self.remove_completion_adaptive_source(&path);
            self.core.remove_file(path);
            return;
        };

        self.core
            .open_document_with_kind(path, source, core_source_kind(kind));
    }

    pub(super) fn save_document(
        &mut self,
        uri: impl Into<String>,
        source: impl Into<String>,
        roots: &[PathBuf],
        settings: &WorkspaceSettings,
    ) {
        let uri = uri.into();
        let source = source.into();

        let Some(document) = self.documents.get_mut(&uri) else {
            self.open_document(uri, source, roots, settings);
            return;
        };

        document.source = source.clone();
        let path = document.path.clone();
        let effective_roots = self.effective_workspace_folders(roots);
        let kind = {
            let _trace = tua_core::profile::diagnostics_span(
                tua_core::profile::DiagnosticsTraceCategory::ReadVfsUpdate,
            );

            self.cache_source_after_sync(path.clone(), source.clone(), &effective_roots, settings)
        };

        if let Some(kind) = kind {
            self.core
                .open_document_with_kind(path.clone(), source.clone(), core_source_kind(kind));
            self.persist_completion_adaptive_source(&path, &source);
        } else {
            self.remove_completion_adaptive_source(&path);
            self.core.remove_file(path);
        }
    }

    pub(super) fn close_document(&mut self, uri: &str) {
        if let Some(document) = self.documents.remove(uri) {
            self.core.close_document(&document.path);
        }
    }

    pub(super) fn remove_file(&mut self, uri: &str) {
        let path = path_from_uri(uri);

        self.documents.remove(uri);
        self.remove_completion_adaptive_source(&path);
        self.core.remove_file(&path);
    }

    pub(super) fn cache_disk_file(
        &mut self,
        uri: &str,
        roots: &[PathBuf],
        settings: &WorkspaceSettings,
    ) {
        let _trace = tua_core::profile::diagnostics_span(
            tua_core::profile::DiagnosticsTraceCategory::ReadVfsUpdate,
        );
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
            self.remove_completion_adaptive_source(&path);
            self.core.remove_file(&path);
            return;
        }

        if let Ok(source) = fs::read_to_string(&path) {
            let effective_roots = self.effective_workspace_folders(roots);

            if self
                .cache_source_after_sync(path.clone(), source.clone(), &effective_roots, settings)
                .is_some()
            {
                self.persist_completion_adaptive_source(&path, &source);
            }
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
        let _trace = tua_core::profile::diagnostics_span(
            tua_core::profile::DiagnosticsTraceCategory::ReadVfsUpdate,
        );

        self.sync_core_project_inputs(roots, settings);
        let effective_roots = self.effective_workspace_folders(roots);
        let documents = self
            .documents
            .values()
            .map(|document| (document.path.clone(), document.source.clone()))
            .collect::<Vec<_>>();

        for (path, source) in documents {
            self.core.invalidate_file_analysis(&path);
            self.cache_source_after_sync(path, source, &effective_roots, settings);
        }
    }

    pub(super) fn reindex_workspace(&mut self, roots: &[PathBuf], settings: &WorkspaceSettings) {
        let (effective_roots, open_paths, open_documents, model) = {
            let _trace = tua_core::profile::diagnostics_span(
                tua_core::profile::DiagnosticsTraceCategory::ReadVfsUpdate,
            );

            self.sync_core_project_inputs(roots, settings);
            let effective_roots = self.effective_workspace_folders(roots);
            let open_documents = self
                .documents
                .values()
                .map(|document| (document.path.clone(), document.source.clone()))
                .collect::<Vec<_>>();
            let open_paths = open_documents
                .iter()
                .map(|(path, _)| path.clone())
                .collect::<BTreeSet<_>>();
            let model = self.project_model.clone();

            (effective_roots, open_paths, open_documents, model)
        };

        let Some(model) = model else {
            self.core.reset_completion_adaptive_stats();
            return;
        };

        let now = current_epoch();
        let mut combined_adaptive_stats = CompletionAdaptiveStats::default();

        for workspace in model.workspaces() {
            let mut workspace_stats = load_completion_stats(workspace.root());
            let mut observed_file_scopes = BTreeSet::new();

            for source_dir in workspace.source_roots() {
                let sources = {
                    let _trace = tua_core::profile::diagnostics_span(
                        tua_core::profile::DiagnosticsTraceCategory::DiscoverFiles,
                    );

                    discover_project_sources(source_dir, workspace, settings)
                };

                for (path, kind) in sources {
                    if open_paths.contains(&path) {
                        continue;
                    }

                    if let Ok(source) = fs::read_to_string(&path) {
                        let file_scope = file_scope_hash(workspace.root(), &path);

                        observed_file_scopes.insert(file_scope);

                        let _trace = tua_core::profile::diagnostics_span(
                            tua_core::profile::DiagnosticsTraceCategory::ReadVfsUpdate,
                        );

                        self.cache_source_with_kind_after_sync(
                            path,
                            source,
                            &effective_roots,
                            settings,
                            kind,
                        );
                    }
                }
            }

            for (path, _source) in &open_documents {
                if workspace.source_kind_for_path(path).is_none()
                    || workspace.is_ignored(path)
                    || settings.is_ignored(path, &effective_roots)
                {
                    continue;
                }

                let file_scope = file_scope_hash(workspace.root(), path);

                observed_file_scopes.insert(file_scope);
            }

            retain_observed_file_scopes(&mut workspace_stats, &observed_file_scopes);
            workspace_stats.evict(now);
            let _ = save_completion_stats(workspace.root(), &workspace_stats);
            combined_adaptive_stats.merge(workspace_stats);
        }

        self.core
            .set_completion_adaptive_stats(combined_adaptive_stats);
    }

    fn persist_completion_adaptive_source(&mut self, path: &Path, source: &str) {
        let Some(root) = self.completion_stats_workspace_root(path) else {
            return;
        };
        let file_scope = file_scope_hash(&root, path);
        let now = current_epoch();

        self.core
            .replace_completion_adaptive_file_observations(file_scope, source, now);

        let mut stats = load_completion_stats(&root);

        stats.replace_file_observations(file_scope, source, now);
        let _ = save_completion_stats(&root, &stats);
    }

    fn remove_completion_adaptive_source(&mut self, path: &Path) {
        let Some(root) = self.completion_stats_workspace_root(path) else {
            return;
        };
        let file_scope = file_scope_hash(&root, path);

        self.core
            .remove_completion_adaptive_file_observations(file_scope);

        let mut stats = load_completion_stats(&root);

        stats.remove_file_observations(file_scope);
        let _ = save_completion_stats(&root, &stats);
    }

    fn completion_stats_workspace_root(&self, path: &Path) -> Option<PathBuf> {
        self.project_model
            .as_ref()?
            .workspace_for_path(path)
            .map(|workspace| workspace.root().to_path_buf())
    }

    fn cache_source_after_sync(
        &mut self,
        path: PathBuf,
        source: String,
        roots: &[PathBuf],
        settings: &WorkspaceSettings,
    ) -> Option<SourceKind> {
        if settings.is_ignored(&path, roots) {
            self.core.remove_file(&path);
            return None;
        }

        let Some(kind) = source_kind_for_path(&path, self.project_model.as_ref()) else {
            self.core.remove_file(&path);
            return None;
        };

        self.cache_source_with_kind_after_sync(path, source, roots, settings, kind)
    }

    fn cache_source_with_kind_after_sync(
        &mut self,
        path: PathBuf,
        source: String,
        roots: &[PathBuf],
        settings: &WorkspaceSettings,
        kind: SourceKind,
    ) -> Option<SourceKind> {
        if settings.is_ignored(&path, roots) {
            self.core.remove_file(&path);
            return None;
        }

        self.core
            .set_file_with_kind(path, source, core_source_kind(kind));

        Some(kind)
    }

    fn sync_core_project_inputs(&mut self, roots: &[PathBuf], settings: &WorkspaceSettings) {
        let effective_roots = self.effective_workspace_folders(roots);
        let Ok(model) = project_model_for_roots(&effective_roots, settings) else {
            self.project_model = None;
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
        self.core
            .set_require_path_separator(settings.module_path_separator());
        self.core
            .set_completion_sequence_ranking(settings.completion_sequence_ranking_config());
        self.core
            .set_completion_multi_token(settings.completion_multi_token_config());
        self.core.set_ignored_paths(ignored_paths);
        self.core.set_asset_roots(asset_roots);

        if let Some(workspace) = model.primary_workspace() {
            self.core
                .set_config(settings.apply_project_config(workspace.config().clone()));
        }
        self.core.refresh_asset_index_from_disk();

        self.project_model = Some(model);
    }

    pub(super) fn format_options_for_path(
        &self,
        path: &Path,
        settings: &WorkspaceSettings,
    ) -> FormatOptions {
        let format_config = self
            .project_model
            .as_ref()
            .and_then(|model| model.workspace_for_path(path))
            .map(|workspace| workspace.config().format.clone())
            .unwrap_or_default();

        settings.format_options(format_config)
    }

    pub(super) fn is_source_uri(
        &self,
        uri: &str,
        roots: &[PathBuf],
        settings: &WorkspaceSettings,
    ) -> bool {
        let path = path_from_uri(uri);

        if settings.is_ignored(&path, roots) {
            return false;
        }

        source_kind_for_path(&path, self.project_model.as_ref()).is_some()
    }

    pub(super) fn is_asset_uri(
        &self,
        uri: &str,
        roots: &[PathBuf],
        settings: &WorkspaceSettings,
    ) -> bool {
        let path = path_from_uri(uri);

        if settings.is_ignored(&path, roots) || !is_love_asset_path(&path) {
            return false;
        }

        self.project_model
            .as_ref()
            .map(|model| {
                model.workspaces().iter().any(|workspace| {
                    workspace
                        .asset_roots()
                        .into_iter()
                        .any(|root| path.starts_with(root))
                })
            })
            .unwrap_or_else(|| roots.iter().any(|root| path.starts_with(root)))
    }

    fn effective_workspace_folders(&self, roots: &[PathBuf]) -> Vec<PathBuf> {
        let mut effective = roots.to_vec();

        for document in self.documents.values() {
            if let Some(root) = document.config_root.clone() {
                push_unique_path(&mut effective, root);
            }
        }

        effective
    }

    pub(super) fn revision(&self) -> u64 {
        self.core.revision()
    }
}

fn position_is_inside_open_table(source: &str, position: SourcePosition) -> bool {
    let Some(cursor) = byte_offset_for_position(source, position) else {
        return false;
    };
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let bytes = &source.as_bytes()[..cursor.min(source.len())];
    let mut index = 0usize;

    while index < bytes.len() {
        let byte = bytes[index];

        if let Some(quote_byte) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote_byte {
                quote = None;
            }
            index += 1;
            continue;
        }

        if byte == b'-' && bytes.get(index + 1) == Some(&b'-') {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }

        match byte {
            b'\'' | b'"' => quote = Some(byte),
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }

        index += 1;
    }

    depth > 0
}

fn byte_offset_for_position(source: &str, position: SourcePosition) -> Option<usize> {
    let mut offset = 0usize;

    for (index, line) in source.split_inclusive('\n').enumerate() {
        let line_number = index + 1;
        let text = line
            .strip_suffix('\n')
            .unwrap_or(line)
            .strip_suffix('\r')
            .unwrap_or_else(|| line.strip_suffix('\n').unwrap_or(line));

        if line_number == position.line {
            return Some(offset + byte_for_column(text, position.column));
        }

        offset += line.len();
    }

    if position.line == source.lines().count().max(1) {
        let line = source.lines().last().unwrap_or_default();

        return Some(offset + byte_for_column(line, position.column));
    }

    None
}

fn identifier_completion_position(source: &str, position: SourcePosition) -> SourcePosition {
    let Some(line) = source_line_at(source, position.line) else {
        return position;
    };
    let cursor = byte_for_column(line, position.column).min(line.len());
    let bytes = line.as_bytes();

    if cursor >= bytes.len() || !is_identifier_continue_byte(bytes[cursor]) {
        return position;
    }

    let mut end = cursor;

    while end < bytes.len() && is_identifier_continue_byte(bytes[end]) {
        end += 1;
    }

    SourcePosition {
        line: position.line,
        column: column_for_byte(line, end),
    }
}

fn source_line_at(source: &str, line: usize) -> Option<&str> {
    source
        .split('\n')
        .nth(line.checked_sub(1)?)
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

fn column_for_byte(line: &str, byte: usize) -> usize {
    line[..byte.min(line.len())].chars().count() + 1
}

fn is_identifier_continue_byte(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

fn byte_for_column(line: &str, column: usize) -> usize {
    let target = column.saturating_sub(1);

    line.char_indices()
        .nth(target)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

impl AnalysisSnapshot {
    pub(super) fn revision(&self) -> u64 {
        self.core.revision()
    }

    pub(super) fn diagnostics_for_uri(&self, uri: &str) -> Vec<Diagnostic> {
        let path = path_from_uri(uri);

        if self.core.source(&path).is_some() {
            return self.core.fresh_diagnostics(&path);
        }

        Vec::new()
    }

    pub(super) fn file_rename_edits(&self, old_path: &Path, new_path: &Path) -> Vec<TextEdit> {
        let Some(model) = self.project_model.as_ref() else {
            return Vec::new();
        };
        let Some(old_workspace) = model.workspace_for_path(old_path) else {
            return Vec::new();
        };
        let Some(new_workspace) = model.workspace_for_path(new_path) else {
            return Vec::new();
        };

        if old_workspace.root() != new_workspace.root()
            || old_workspace.is_ignored(old_path)
            || new_workspace.is_ignored(new_path)
            || old_workspace.source_kind_for_path(old_path).is_none()
            || new_workspace.source_kind_for_path(new_path).is_none()
        {
            return Vec::new();
        }

        self.core.file_rename_edits(old_path, new_path)
    }

    pub(super) fn hover(&self, path: &Path, position: SourcePosition) -> Option<tua_core::Hover> {
        self.core.hover(path, position)
    }

    pub(super) fn inspect(
        &self,
        path: &Path,
        position: Option<SourcePosition>,
    ) -> Vec<InspectorItem> {
        self.core.inspect(path, position)
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

    pub(super) fn multi_token_completion(
        &self,
        path: &Path,
        position: SourcePosition,
        primary_candidate_count: usize,
    ) -> Vec<Completion> {
        if self.core.source(path).is_none() {
            return Vec::new();
        }

        self.core
            .multi_token_completion(path, position, primary_candidate_count)
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

        if prefix.chars().count() < MIN_AUTO_IMPORT_PREFIX_CHARS {
            return Vec::new();
        }

        let Some(model) = self.project_model.as_ref() else {
            return Vec::new();
        };
        let Some(workspace) = model.workspace_for_path(path) else {
            return Vec::new();
        };

        let require_aliases = require_bindings_for_parsed_source(parsed)
            .into_iter()
            .filter_map(|binding| {
                let module = match binding.kind {
                    ModuleSpecifierKind::Require => Some(binding.module),
                    ModuleSpecifierKind::Import => {
                        model.module_name_for_import(path, &binding.module)
                    }
                }?;

                Some((module, binding.alias))
            })
            .collect::<BTreeMap<_, _>>();
        let reserved_aliases = existing_labels
            .iter()
            .chain(require_aliases.values())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut completions = Vec::new();
        for symbol in self.core.importable_symbols(&prefix) {
            if symbol.module_path == path
                || !project_contains_path(Some(model), &symbol.module_path)
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
            let Some(import_specifier) = model.import_specifier_for_path(path, &symbol.module_path)
            else {
                continue;
            };

            let existing_alias = require_aliases.get(&module).cloned();
            let alias = existing_alias
                .clone()
                .unwrap_or_else(|| unique_import_alias(&module, &reserved_aliases));

            if existing_labels.contains(&symbol.name) {
                continue;
            }

            let mut completion = auto_import_completion(AutoImportCandidate {
                current_path: path,
                current_source: source,
                module_path: &symbol.module_path,
                module: &module,
                import_specifier: &import_specifier,
                alias: &alias,
                already_imported: existing_alias.is_some(),
                name: &export.public_name,
                field: &export.field,
            });
            completion.filter_text.get_or_insert_with(|| prefix.clone());
            completions.push(completion);
        }

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

    pub(super) fn type_definition(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<tua_core::Definition> {
        self.core.type_definition(path, position)
    }

    pub(super) fn implementation(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Vec<tua_core::Definition> {
        self.core.implementation(path, position)
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
        requested_kinds: &[tua_core::AssistKind],
    ) -> Vec<CodeAction> {
        self.core
            .code_actions(path, range, diagnostics, requested_kinds)
    }

    pub(super) fn code_lenses(&self, path: &Path) -> Vec<CodeLens> {
        self.core.code_lenses(path)
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

    pub(super) fn inlay_hints_in_range(&self, path: &Path, range: SourceSpan) -> Vec<InlayHint> {
        self.core.inlay_hints_in_range(path, range)
    }

    pub(super) fn workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let model = self.project_model.as_ref();

        self.core
            .workspace_symbols(query)
            .into_iter()
            .filter(|symbol| {
                project_contains_path(model, &symbol.path)
                    && !self
                        .settings
                        .is_ignored(&symbol.path, &self.workspace_folders)
            })
            .collect()
    }

    pub(super) fn signal_graph(&self) -> SignalGraph {
        self.core.signal_graph()
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
) -> Vec<(PathBuf, SourceKind)> {
    let mut files = Vec::new();

    discover_project_sources_into(source_dir, workspace, settings, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn discover_project_sources_into(
    dir: &Path,
    workspace: &ProjectWorkspace,
    settings: &WorkspaceSettings,
    files: &mut Vec<(PathBuf, SourceKind)>,
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
        {
            if let Some(kind) = workspace.source_kind_for_path(&path) {
                files.push((path, source_kind_from_vfs(kind)));
            }
        }
    }
}

fn source_kind_for_path(path: &Path, model: Option<&ProjectModel>) -> Option<SourceKind> {
    model
        .and_then(|model| {
            model
                .workspace_for_path(path)
                .and_then(|workspace| workspace.source_kind_for_path(path))
        })
        .map(source_kind_from_vfs)
        .or_else(|| source_kind_from_extension(path))
}

fn source_kind_from_extension(path: &Path) -> Option<SourceKind> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("lua") => Some(SourceKind::Lua),
        Some("tua") => Some(SourceKind::Tua),
        _ => None,
    }
}

fn is_love_asset_path(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    let extension = extension.to_ascii_lowercase();

    [
        AssetLoaderKind::Image,
        AssetLoaderKind::Sound,
        AssetLoaderKind::Music,
        AssetLoaderKind::Font,
        AssetLoaderKind::Shader,
        AssetLoaderKind::ImageData,
        AssetLoaderKind::ImageFont,
        AssetLoaderKind::SoundData,
        AssetLoaderKind::Video,
        AssetLoaderKind::VideoStream,
    ]
    .into_iter()
    .any(|kind| kind.supported_extensions().contains(&extension.as_str()))
}

fn nearest_config_root(path: &Path) -> Option<PathBuf> {
    path.parent()?
        .ancestors()
        .find(|dir| dir.join(CONFIG_FILE).is_file())
        .map(Path::to_path_buf)
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn path_sets_differ(left: &[PathBuf], right: &[PathBuf]) -> bool {
    left.len() != right.len()
        || left
            .iter()
            .any(|path| !right.iter().any(|other| other == path))
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
        let player_initial = "local hp: number = 1\n";
        let player_changed = "local hp: number = 3\n";

        host.open_document(player_uri, player_initial, &roots, &settings);
        host.open_document(enemy_uri, "local hp: number = 2\n", &roots, &settings);

        let before = host.revision();
        let snapshot = host.snapshot(&roots, &settings);

        assert!(snapshot.core.source(path_from_uri(player_uri)).is_some());
        assert!(snapshot.core.source(path_from_uri(enemy_uri)).is_some());
        assert_eq!(host.cache_stats().parse_misses, 2);
        assert_eq!(host.cache_stats().parse_hits, 0);

        host.update_document(player_uri, "local hp: number = 1\n", &roots, &settings);
        assert_eq!(host.revision(), before);

        host.update_document(player_uri, player_changed, &roots, &settings);

        let after = host.revision();
        let snapshot = host.snapshot(&roots, &settings);

        assert_eq!(after, before + 1);
        assert_eq!(
            snapshot.core.source(path_from_uri(player_uri)),
            Some(player_changed)
        );
        assert!(snapshot.core.source(path_from_uri(player_uri)).is_some());
        assert!(snapshot.core.source(path_from_uri(enemy_uri)).is_some());
        assert_eq!(host.cache_stats().parse_misses, 3);
        assert_eq!(host.cache_stats().parse_hits, 1);
    }
}
