use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use tua_core::{
    build_type_environment, check_parsed_source_with_diagnostics_config,
    code_actions_parsed_source, complete_parsed_source, complete_tua_source, definition_tua_source,
    document_highlights_parsed_source, document_symbols_parsed_source,
    global_function_definition_count, global_function_name_for_definition,
    global_references_to_definition_parsed_source, global_type_alias_name_for_definition,
    global_type_alias_references_to_definition_parsed_source, hover_parsed_source,
    infer_module_exports, inlay_hints_parsed_source, is_valid_rename_identifier,
    load_config_from_dir, module_links_tua_source, parse_lua_source, parse_tua_source,
    reference_kind_for_rename, references_to_definition_parsed_source,
    rename_target_for_definition, semantic_tokens_parsed_source, semantic_tokens_tua_source,
    signature_help_parsed_source, signature_help_tua_source, type_alias_definition_count,
    workspace_symbols_from_dir, CodeAction, CodeActionRange, Completion, Diagnostic,
    DiagnosticsConfig, DocumentHighlight, DocumentSymbol, Hover, InlayHint, ModuleExports,
    ModuleLink, ParsedSource, Reference, RenameEdit, RenameTarget, SemanticToken, SignatureHelp,
    SourcePosition, SourceSpan, TextEdit, TypeEnvironment, WorkspaceSymbol,
};

use super::uri::path_from_uri;

#[derive(Default)]
pub(super) struct AnalysisHost {
    documents: HashMap<String, OpenDocument>,
    files: HashMap<PathBuf, CachedFile>,
    next_revision: u64,
}

#[derive(Clone, Debug)]
pub(super) struct AnalysisSnapshot {
    documents: HashMap<String, OpenDocument>,
    files: HashMap<PathBuf, CachedFile>,
    workspace_folders: Vec<PathBuf>,
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
    pub(super) fn snapshot(&self, workspace_folders: &[PathBuf]) -> AnalysisSnapshot {
        AnalysisSnapshot {
            documents: self.documents.clone(),
            files: self.files.clone(),
            workspace_folders: workspace_folders.to_vec(),
        }
    }

    pub(super) fn open_document(&mut self, uri: impl Into<String>, source: impl Into<String>) {
        let uri = uri.into();
        let path = path_from_uri(&uri);
        let source = source.into();

        self.documents.insert(
            uri,
            OpenDocument {
                path: path.clone(),
                source: source.clone(),
            },
        );
        self.cache_source(path, source);
    }

    pub(super) fn update_document(&mut self, uri: impl Into<String>, source: impl Into<String>) {
        self.open_document(uri, source);
    }

    pub(super) fn close_document(&mut self, uri: &str) {
        if let Some(document) = self.documents.remove(uri) {
            self.files.remove(&document.path);
        }
    }

    pub(super) fn remove_file(&mut self, uri: &str) {
        let path = path_from_uri(uri);

        self.documents.remove(uri);
        self.files.remove(&path);
    }

    pub(super) fn cache_disk_file(&mut self, uri: &str) {
        let path = path_from_uri(uri);

        if self
            .documents
            .values()
            .any(|document| document.path == path)
        {
            return;
        }

        if let Ok(source) = fs::read_to_string(&path) {
            self.cache_source(path, source);
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

    pub(super) fn refresh_open_documents(&mut self) {
        let documents = self
            .documents
            .values()
            .map(|document| (document.path.clone(), document.source.clone()))
            .collect::<Vec<_>>();

        for (path, source) in documents {
            self.cache_source(path, source);
        }
    }

    pub(super) fn reindex_workspace(&mut self, roots: &[PathBuf]) {
        let open_paths = self
            .documents
            .values()
            .map(|document| document.path.clone())
            .collect::<BTreeSet<_>>();
        let roots = if roots.is_empty() {
            vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))]
        } else {
            roots.to_vec()
        };

        self.files.retain(|path, _| {
            open_paths.contains(path) || !roots.iter().any(|root| path.starts_with(root))
        });

        for root in &roots {
            let Ok(loaded) = load_config_from_dir(root) else {
                continue;
            };
            let source_dir = root.join(&loaded.config.source);

            for path in discover_project_sources(&source_dir, &loaded.config.extension) {
                if open_paths.contains(&path) {
                    continue;
                }

                if let Ok(source) = fs::read_to_string(&path) {
                    self.cache_source(path, source);
                }
            }
        }
    }

    fn cache_source(&mut self, path: PathBuf, source: String) {
        self.next_revision += 1;
        self.files.insert(
            path.clone(),
            CachedFile::new(path, source, self.next_revision),
        );
    }

    #[cfg(test)]
    fn cache_revision_for_uri(&self, uri: &str) -> Option<u64> {
        let path = path_from_uri(uri);

        self.files.get(&path).map(|file| file.revision)
    }
}

impl AnalysisSnapshot {
    pub(super) fn diagnostics_for_uri(
        &self,
        uri: &str,
        diagnostics_config: &DiagnosticsConfig,
    ) -> Vec<Diagnostic> {
        let path = path_from_uri(uri);

        self.file(&path)
            .map(|file| file.diagnostics(diagnostics_config))
            .unwrap_or_default()
    }

    pub(super) fn hover(&self, path: &Path, position: SourcePosition) -> Option<Hover> {
        self.parsed(path)
            .and_then(|parsed| hover_parsed_source(parsed, position).ok().flatten())
    }

    pub(super) fn completion(&self, path: &Path, position: SourcePosition) -> Vec<Completion> {
        let Some(file) = self.file(path) else {
            return Vec::new();
        };

        match &file.parsed {
            Ok(parsed) => complete_parsed_source(parsed, position).unwrap_or_default(),
            Err(_) => complete_tua_source(path, file.source.clone(), position).unwrap_or_default(),
        }
    }

    pub(super) fn resolve_completion(
        &self,
        path: &Path,
        position: SourcePosition,
        label: &str,
        kind: Option<tua_core::CompletionKind>,
    ) -> Option<Completion> {
        self.completion(path, position)
            .into_iter()
            .find(|completion| {
                completion.label == label && kind.is_none_or(|kind| completion.kind == kind)
            })
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
        let file = self.file(path)?;

        definition_tua_source(path, file.source.clone(), position)
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
        let roots = if self.workspace_folders.is_empty() {
            vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))]
        } else {
            self.workspace_folders.clone()
        };

        roots
            .into_iter()
            .flat_map(|root| workspace_symbols_from_dir(root, query).unwrap_or_default())
            .collect()
    }

    fn parsed(&self, path: &Path) -> Option<&ParsedSource> {
        self.file(path).and_then(|file| file.parsed.as_ref().ok())
    }

    fn file(&self, path: &Path) -> Option<&CachedFile> {
        self.files.get(path).or_else(|| {
            self.documents
                .values()
                .find(|document| document.path == path)
                .and_then(|document| self.files.get(&document.path))
        })
    }

    fn project_source_for_path(&self, path: &Path) -> Option<(PathBuf, String)> {
        let mut roots = if self.workspace_folders.is_empty() {
            vec![std::env::current_dir().ok()?]
        } else {
            self.workspace_folders.clone()
        };

        roots.sort_by_key(|root| std::cmp::Reverse(root.components().count()));

        for root in roots {
            if !path.starts_with(&root) {
                continue;
            }

            let Ok(loaded) = load_config_from_dir(&root) else {
                continue;
            };

            return Some((root.join(loaded.config.source), loaded.config.extension));
        }

        None
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
                    .values()
                    .filter_map(|file| file.parsed.as_ref().ok())
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
                    .values()
                    .filter_map(|file| file.parsed.as_ref().ok())
                    .map(|parsed| type_alias_definition_count(parsed, &alias_name))
                    .sum::<usize>()
                    > 1
            }
            tua_core::RenameKind::Local | tua_core::RenameKind::Parameter => false,
        }
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
    fn new(path: PathBuf, source: String, revision: u64) -> Self {
        let parsed = parse_source(&path, source.clone());
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

    fn diagnostics(&self, diagnostics_config: &DiagnosticsConfig) -> Vec<Diagnostic> {
        let parsed = match &self.parsed {
            Ok(parsed) => parsed,
            Err(diagnostics) => return diagnostics.clone(),
        };

        check_parsed_source_with_diagnostics_config(parsed, diagnostics_config)
            .err()
            .unwrap_or_default()
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

fn parse_source(path: &Path, source: String) -> Result<ParsedSource, Vec<Diagnostic>> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("lua") => parse_lua_source(path, source),
        _ => parse_tua_source(path, source),
    }
}

fn discover_project_sources(source_dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();

    discover_project_sources_into(source_dir, extension, &mut files);
    files.sort();
    files
}

fn discover_project_sources_into(dir: &Path, extension: &str, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            discover_project_sources_into(&path, extension, files);
        } else if file_type.is_file() && is_project_source(&path, extension) {
            files.push(path);
        }
    }
}

fn is_project_source(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value == extension || value == "lua")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updating_one_open_document_invalidates_only_that_cached_file() {
        let mut host = AnalysisHost::default();
        let player_uri = "file:///tmp/analysis-player.tua";
        let enemy_uri = "file:///tmp/analysis-enemy.tua";

        host.open_document(player_uri, "local hp: number = 1\n");
        host.open_document(enemy_uri, "local hp: number = 2\n");

        let player_before = host
            .cache_revision_for_uri(player_uri)
            .expect("player should be cached");
        let enemy_before = host
            .cache_revision_for_uri(enemy_uri)
            .expect("enemy should be cached");

        host.update_document(player_uri, "local hp: number = 3\n");

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
