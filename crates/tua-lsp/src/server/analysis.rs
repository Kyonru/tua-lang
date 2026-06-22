use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use tua_core::{
    build_type_environment, check_parsed_source_with_diagnostics_config, complete_parsed_source,
    complete_tua_source, definition_tua_source, document_highlights_parsed_source,
    document_symbols_parsed_source, hover_parsed_source, infer_module_exports,
    inlay_hints_parsed_source, load_config_from_dir, module_links_tua_source, parse_lua_source,
    parse_tua_source, prepare_rename_parsed_source, references_parsed_source, rename_parsed_source,
    semantic_tokens_parsed_source, semantic_tokens_tua_source, signature_help_parsed_source,
    signature_help_tua_source, workspace_symbols_from_dir, Completion, Diagnostic,
    DiagnosticsConfig, DocumentHighlight, DocumentSymbol, Hover, InlayHint, ModuleExports,
    ModuleLink, ParsedSource, Reference, RenameEdit, RenameTarget, SemanticToken, SignatureHelp,
    SourcePosition, TypeEnvironment, WorkspaceSymbol,
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

    pub(super) fn references(
        &self,
        path: &Path,
        position: SourcePosition,
        include_declaration: bool,
    ) -> Vec<Reference> {
        self.parsed(path)
            .and_then(|parsed| references_parsed_source(parsed, position, include_declaration).ok())
            .unwrap_or_default()
    }

    pub(super) fn prepare_rename(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<RenameTarget> {
        self.parsed(path).and_then(|parsed| {
            prepare_rename_parsed_source(parsed, position)
                .ok()
                .flatten()
        })
    }

    pub(super) fn rename(
        &self,
        path: &Path,
        position: SourcePosition,
        new_name: &str,
    ) -> Option<RenameEdit> {
        self.parsed(path).and_then(|parsed| {
            rename_parsed_source(parsed, position, new_name)
                .ok()
                .flatten()
        })
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
