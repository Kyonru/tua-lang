use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::{
    check_parsed_source_with_asset_roots_and_diagnostics_config, complete_parsed_source,
    complete_tua_source, definition_tua_source, hover_parsed_source, parse_lua_source,
    parse_tua_source, Completion, Definition, Diagnostic, Hover, ParsedSource, ProjectConfig,
    SourcePosition,
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
    source_roots: Vec<PathBuf>,
    files: BTreeMap<PathBuf, AnalysisSource>,
    open_documents: BTreeMap<PathBuf, AnalysisSource>,
    builtin_catalogs: BuiltinCatalogs,
}

#[derive(Clone, Debug)]
pub struct AnalysisSnapshot {
    config: ProjectConfig,
    source_roots: Vec<PathBuf>,
    files: BTreeMap<PathBuf, AnalysisFile>,
    open_documents: BTreeMap<PathBuf, AnalysisSource>,
    builtin_catalogs: BuiltinCatalogs,
}

#[derive(Clone, Debug)]
struct AnalysisFile {
    source: String,
    parsed: Result<ParsedSource, Vec<Diagnostic>>,
}

#[derive(Clone, Debug)]
struct AnalysisSource {
    source: String,
    kind: Option<AnalysisSourceKind>,
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
    }

    pub fn source_roots(&self) -> &[PathBuf] {
        &self.source_roots
    }

    pub fn set_source_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        self.source_roots = roots.into_iter().collect();
    }

    pub fn builtin_catalogs(&self) -> &BuiltinCatalogs {
        &self.builtin_catalogs
    }

    pub fn set_builtin_catalogs(&mut self, catalogs: BuiltinCatalogs) {
        self.builtin_catalogs = catalogs;
    }

    pub fn set_file(&mut self, path: impl Into<PathBuf>, source: impl Into<String>) {
        self.files.insert(
            path.into(),
            AnalysisSource {
                source: source.into(),
                kind: None,
            },
        );
    }

    pub fn set_file_with_kind(
        &mut self,
        path: impl Into<PathBuf>,
        source: impl Into<String>,
        kind: AnalysisSourceKind,
    ) {
        self.files.insert(
            path.into(),
            AnalysisSource {
                source: source.into(),
                kind: Some(kind),
            },
        );
    }

    pub fn remove_file(&mut self, path: impl AsRef<Path>) {
        self.files.remove(path.as_ref());
        self.open_documents.remove(path.as_ref());
    }

    pub fn open_document(&mut self, path: impl Into<PathBuf>, source: impl Into<String>) {
        self.open_documents.insert(
            path.into(),
            AnalysisSource {
                source: source.into(),
                kind: None,
            },
        );
    }

    pub fn open_document_with_kind(
        &mut self,
        path: impl Into<PathBuf>,
        source: impl Into<String>,
        kind: AnalysisSourceKind,
    ) {
        self.open_documents.insert(
            path.into(),
            AnalysisSource {
                source: source.into(),
                kind: Some(kind),
            },
        );
    }

    pub fn update_open_document(&mut self, path: impl Into<PathBuf>, source: impl Into<String>) {
        self.open_document(path, source);
    }

    pub fn close_document(&mut self, path: impl AsRef<Path>) {
        self.open_documents.remove(path.as_ref());
    }

    pub fn snapshot(&self) -> AnalysisSnapshot {
        let mut sources = self.files.clone();

        for (path, source) in &self.open_documents {
            sources.insert(path.clone(), source.clone());
        }

        let files = sources
            .into_iter()
            .map(|(path, analysis_source)| {
                let parsed = parse_analysis_source(
                    &path,
                    analysis_source.source.clone(),
                    analysis_source.kind,
                    &self.config,
                );

                (
                    path,
                    AnalysisFile {
                        source: analysis_source.source,
                        parsed,
                    },
                )
            })
            .collect();

        AnalysisSnapshot {
            config: self.config.clone(),
            source_roots: self.source_roots.clone(),
            files,
            open_documents: self.open_documents.clone(),
            builtin_catalogs: self.builtin_catalogs.clone(),
        }
    }
}

impl AnalysisSnapshot {
    pub fn config(&self) -> &ProjectConfig {
        &self.config
    }

    pub fn source_roots(&self) -> &[PathBuf] {
        &self.source_roots
    }

    pub fn builtin_catalogs(&self) -> &BuiltinCatalogs {
        &self.builtin_catalogs
    }

    pub fn is_open_document(&self, path: impl AsRef<Path>) -> bool {
        self.open_documents.contains_key(path.as_ref())
    }

    pub fn source(&self, path: impl AsRef<Path>) -> Option<&str> {
        self.file(path.as_ref()).map(|file| file.source.as_str())
    }

    pub fn parse(&self, path: impl AsRef<Path>) -> Option<Result<&ParsedSource, &[Diagnostic]>> {
        self.file(path.as_ref())
            .map(|file| file.parsed.as_ref().map_err(Vec::as_slice))
    }

    pub fn diagnostics(&self, path: impl AsRef<Path>) -> Vec<Diagnostic> {
        self.diagnostics_with_asset_roots(path, &self.source_roots)
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
        self.parse(path)
            .and_then(Result::ok)
            .and_then(|parsed| hover_parsed_source(parsed, position).ok().flatten())
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

        definition_tua_source(path, file.source.clone(), position)
            .ok()
            .flatten()
    }

    fn file(&self, path: &Path) -> Option<&AnalysisFile> {
        self.files.get(path)
    }
}

fn parse_analysis_source(
    path: &Path,
    source: String,
    kind: Option<AnalysisSourceKind>,
    config: &ProjectConfig,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    match kind.unwrap_or_else(|| infer_source_kind(path, config)) {
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

#[cfg(test)]
mod tests;
