use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::hir::{lower_parsed_source, HirExportValue, HirModule, HirSymbolId};
use crate::module_index::{ModuleExportField, ModuleExports};
use crate::module_link::{module_specifier_literals, ModuleSpecifierKind};
use crate::vfs::{module_name_for_path, normalize_path, ModulePathSeparator};
use crate::{
    AnalysisSourceKind, Diagnostic, DiagnosticCode, DiagnosticSeverity, ParsedSource, SourceSpan,
};

mod inspection;

pub use inspection::{
    ModuleDependencyEdge, ModuleDependencyExport, ModuleDependencyGraph, ModuleDependencyModule,
    ModuleInclusionPath, ModuleInclusionStep,
};

#[derive(Clone, Debug, Default)]
pub struct ModuleGraph {
    modules: BTreeMap<PathBuf, ModuleGraphModule>,
    export_index: BTreeMap<(PathBuf, String), ModuleExportEntry>,
    exports_by_symbol: BTreeMap<(PathBuf, HirSymbolId), Vec<ModuleExportEntry>>,
    diagnostics: BTreeMap<PathBuf, Vec<Diagnostic>>,
    cycles: Vec<Vec<PathBuf>>,
}

#[derive(Clone, Debug)]
pub struct ModuleGraphInput<'a> {
    pub path: &'a Path,
    pub parsed: Option<&'a ParsedSource>,
    pub hir: Option<&'a HirModule>,
    pub exports: Option<&'a ModuleExports>,
    pub source_kind: AnalysisSourceKind,
}

#[derive(Clone, Debug)]
pub struct ModuleGraphOptions {
    pub source_roots: Vec<PathBuf>,
    pub output_roots: Vec<PathBuf>,
    pub extension: String,
    pub require_path_separator: ModulePathSeparator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleGraphModule {
    pub path: PathBuf,
    pub source_kind: AnalysisSourceKind,
    pub require_edges: Vec<RequireGraphEdge>,
    pub exports: Vec<ModuleExportEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequireGraphEdge {
    pub source: PathBuf,
    pub module: String,
    pub kind: ModuleSpecifierKind,
    pub span: SourceSpan,
    pub resolution: RequireResolution,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequireResolution {
    Resolved {
        path: PathBuf,
    },
    Missing {
        search_roots: Vec<PathBuf>,
    },
    Invalid {
        reason: String,
    },
    Duplicate {
        candidates: Vec<PathBuf>,
        selected: PathBuf,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleExportEntry {
    pub module_path: PathBuf,
    pub public_name: String,
    pub symbol_id: Option<HirSymbolId>,
    pub definition_span: SourceSpan,
    pub export_span: SourceSpan,
    pub field: ModuleExportField,
    pub origin: ModuleExportOrigin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleExportOrigin {
    Tua,
    Lua,
    LuaDoc,
    GeneratedBuiltin,
}

impl ModuleGraph {
    pub fn build<'a>(
        inputs: impl IntoIterator<Item = ModuleGraphInput<'a>>,
        options: ModuleGraphOptions,
    ) -> Self {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        let mut graph = Self::default();
        let indexed_paths = inputs
            .iter()
            .map(|input| normalize_path(input.path))
            .collect::<BTreeSet<_>>();
        let search_roots = search_roots(&inputs, &options);

        for input in &inputs {
            let path = normalize_path(input.path);
            let hir = input
                .hir
                .cloned()
                .or_else(|| input.parsed.map(lower_parsed_source));
            let mut module = ModuleGraphModule {
                path: path.clone(),
                source_kind: input.source_kind,
                require_edges: Vec::new(),
                exports: Vec::new(),
            };

            if let Some(parsed) = input.parsed {
                module.require_edges = require_edges_for_source(
                    &path,
                    &parsed.source,
                    &indexed_paths,
                    &search_roots,
                    &options.source_roots,
                    &options.output_roots,
                    &options.extension,
                    options.require_path_separator,
                );
            }

            if let Some(exports) = input.exports {
                let entries = export_entries_for_module(
                    &path,
                    input.parsed,
                    hir.as_ref(),
                    exports,
                    input.source_kind,
                );

                module.exports = entries.clone();
                for entry in entries {
                    if let Some(symbol_id) = entry.symbol_id {
                        graph
                            .exports_by_symbol
                            .entry((path.clone(), symbol_id))
                            .or_default()
                            .push(entry.clone());
                    }
                    graph
                        .export_index
                        .insert((path.clone(), entry.public_name.clone()), entry);
                }
            }

            graph.modules.insert(path, module);
        }

        graph.collect_require_diagnostics();
        graph.collect_cycle_diagnostics();
        graph.cycles.sort();
        graph
    }

    pub fn module(&self, path: impl AsRef<Path>) -> Option<&ModuleGraphModule> {
        self.modules.get(&normalize_path(path))
    }

    pub fn modules(&self) -> impl Iterator<Item = &ModuleGraphModule> {
        self.modules.values()
    }

    /// Returns the roots and every module that transitively depends on them.
    pub fn reverse_dependency_closure<I, P>(&self, roots: I) -> BTreeSet<PathBuf>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let mut importers = BTreeMap::<PathBuf, BTreeSet<PathBuf>>::new();

        for module in self.modules.values() {
            for edge in &module.require_edges {
                let dependency = match &edge.resolution {
                    RequireResolution::Resolved { path } => path,
                    RequireResolution::Duplicate { selected, .. } => selected,
                    RequireResolution::Missing { .. } | RequireResolution::Invalid { .. } => {
                        continue;
                    }
                };

                importers
                    .entry(normalize_path(dependency))
                    .or_default()
                    .insert(module.path.clone());
            }
        }

        let mut closure = roots
            .into_iter()
            .map(|path| normalize_path(path.as_ref()))
            .collect::<BTreeSet<_>>();
        let mut pending = closure.iter().cloned().collect::<Vec<_>>();

        while let Some(path) = pending.pop() {
            let Some(dependents) = importers.get(&path) else {
                continue;
            };

            for dependent in dependents {
                if closure.insert(dependent.clone()) {
                    pending.push(dependent.clone());
                }
            }
        }

        closure
    }

    pub fn export(
        &self,
        module_path: impl AsRef<Path>,
        public_name: &str,
    ) -> Option<&ModuleExportEntry> {
        self.export_index
            .get(&(normalize_path(module_path), public_name.to_owned()))
    }

    pub fn exports_for_symbol(
        &self,
        module_path: impl AsRef<Path>,
        symbol_id: HirSymbolId,
    ) -> &[ModuleExportEntry] {
        self.exports_by_symbol
            .get(&(normalize_path(module_path), symbol_id))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn resolve_require(
        &self,
        source_path: impl AsRef<Path>,
        module: &str,
    ) -> Option<&RequireResolution> {
        self.module(source_path)?
            .require_edges
            .iter()
            .find(|edge| edge.module == module)
            .map(|edge| &edge.resolution)
    }

    pub fn diagnostics(&self, path: impl AsRef<Path>) -> &[Diagnostic] {
        self.diagnostics
            .get(&normalize_path(path))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn all_diagnostics(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .values()
            .flat_map(|diagnostics| diagnostics.iter())
    }

    pub fn cycles(&self) -> &[Vec<PathBuf>] {
        &self.cycles
    }

    fn collect_require_diagnostics(&mut self) {
        for module in self.modules.values() {
            for edge in &module.require_edges {
                match &edge.resolution {
                    RequireResolution::Missing { search_roots } => self
                        .diagnostics
                        .entry(module.path.clone())
                        .or_default()
                        .push(missing_module_diagnostic(edge, search_roots)),
                    RequireResolution::Invalid { reason } => self
                        .diagnostics
                        .entry(module.path.clone())
                        .or_default()
                        .push(invalid_module_specifier_diagnostic(edge, reason)),
                    RequireResolution::Duplicate {
                        candidates,
                        selected,
                    } => self
                        .diagnostics
                        .entry(module.path.clone())
                        .or_default()
                        .push(duplicate_module_diagnostic(edge, candidates, selected)),
                    RequireResolution::Resolved { .. } => {}
                }
            }
        }
    }

    fn collect_cycle_diagnostics(&mut self) {
        let mut visiting = Vec::new();
        let mut visited = BTreeSet::new();
        let mut reported = BTreeSet::new();

        for path in self.modules.keys().cloned().collect::<Vec<_>>() {
            self.visit_cycles(&path, &mut visiting, &mut visited, &mut reported);
        }
    }

    fn visit_cycles(
        &mut self,
        path: &Path,
        visiting: &mut Vec<PathBuf>,
        visited: &mut BTreeSet<PathBuf>,
        reported: &mut BTreeSet<Vec<PathBuf>>,
    ) {
        let path = normalize_path(path);

        if let Some(index) = visiting.iter().position(|candidate| candidate == &path) {
            let cycle = visiting[index..].to_vec();
            if reported.insert(cycle.clone()) {
                self.cycles.push(cycle.clone());
                let display = cycle
                    .iter()
                    .chain(std::iter::once(&path))
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .collect::<Vec<_>>()
                    .join(" -> ");

                self.diagnostics.entry(path.clone()).or_default().push(
                    Diagnostic::new(
                        path,
                        1,
                        1,
                        DiagnosticSeverity::Warning,
                        format!("require cycle detected: {display}"),
                    )
                    .with_code(DiagnosticCode::LINT_REQUIRE_CYCLE),
                );
            }
            return;
        }

        if !visited.insert(path.clone()) {
            return;
        }

        visiting.push(path.clone());
        let dependencies = self
            .module(&path)
            .map(|module| {
                module
                    .require_edges
                    .iter()
                    .filter_map(|edge| match &edge.resolution {
                        RequireResolution::Resolved { path }
                        | RequireResolution::Duplicate { selected: path, .. } => Some(path.clone()),
                        RequireResolution::Missing { .. } | RequireResolution::Invalid { .. } => {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for dependency in dependencies {
            self.visit_cycles(&dependency, visiting, visited, reported);
        }

        visiting.pop();
    }
}

fn require_edges_for_source(
    source_path: &Path,
    source: &str,
    indexed_paths: &BTreeSet<PathBuf>,
    search_roots: &[PathBuf],
    source_roots: &[PathBuf],
    output_roots: &[PathBuf],
    extension: &str,
    separator: ModulePathSeparator,
) -> Vec<RequireGraphEdge> {
    module_specifier_literals(source)
        .into_iter()
        .map(|literal| {
            let resolution = match literal.kind {
                ModuleSpecifierKind::Require => {
                    let candidates = existing_candidates(
                        &literal.module,
                        indexed_paths,
                        search_roots,
                        source_roots,
                        output_roots,
                        extension,
                        separator,
                    );

                    resolution_for_candidates(candidates, search_roots.to_vec())
                }
                ModuleSpecifierKind::Import => {
                    match imported_module_candidates(
                        source_path,
                        &literal.module,
                        indexed_paths,
                        search_roots,
                        source_roots,
                        output_roots,
                        extension,
                        separator,
                    ) {
                        Ok((candidates, search_roots)) => {
                            resolution_for_candidates(candidates, search_roots)
                        }
                        Err(reason) => RequireResolution::Invalid { reason },
                    }
                }
            };

            RequireGraphEdge {
                source: source_path.to_path_buf(),
                module: literal.module,
                kind: literal.kind,
                span: literal.span,
                resolution,
            }
        })
        .collect()
}

fn resolution_for_candidates(
    candidates: Vec<PathBuf>,
    search_roots: Vec<PathBuf>,
) -> RequireResolution {
    match candidates.as_slice() {
        [] => RequireResolution::Missing { search_roots },
        [path] => RequireResolution::Resolved { path: path.clone() },
        [selected, ..] => {
            let selected = selected.clone();

            RequireResolution::Duplicate {
                candidates,
                selected,
            }
        }
    }
}

fn imported_module_candidates(
    source_path: &Path,
    specifier: &str,
    indexed_paths: &BTreeSet<PathBuf>,
    search_roots: &[PathBuf],
    source_roots: &[PathBuf],
    output_roots: &[PathBuf],
    extension: &str,
    separator: ModulePathSeparator,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    if is_child_relative_import_specifier(specifier) {
        let search_roots = source_path
            .parent()
            .map(|path| vec![normalize_path(path)])
            .unwrap_or_default();
        let candidates = child_relative_import_candidates(
            source_path,
            specifier,
            indexed_paths,
            search_roots,
            extension,
            separator,
        )?;

        return Ok(candidates);
    }

    if is_parent_relative_import_specifier(specifier) {
        return Err(
            "import specifier cannot traverse parent directories; use a project module name instead"
                .to_owned(),
        );
    }

    if !is_project_import_specifier(specifier) {
        return Err(
            "import specifier must be child-relative (`./foo`) or a project module (`engine.foo`)"
                .to_owned(),
        );
    }

    let candidates = existing_candidates(
        specifier,
        indexed_paths,
        search_roots,
        source_roots,
        output_roots,
        extension,
        separator,
    );

    Ok((candidates, search_roots.to_vec()))
}

fn child_relative_import_candidates(
    source_path: &Path,
    specifier: &str,
    indexed_paths: &BTreeSet<PathBuf>,
    search_roots: Vec<PathBuf>,
    extension: &str,
    separator: ModulePathSeparator,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    let parent = source_path
        .parent()
        .ok_or_else(|| "import specifier needs a source file parent directory".to_owned())?;
    let base = normalize_path(parent.join(specifier));

    if let Some(file_extension) = base.extension().and_then(|extension| extension.to_str()) {
        if file_extension != "lua" && file_extension != extension {
            return Err(format!(
                "import specifier must be extensionless or end in `.{extension}`/`.lua`"
            ));
        }
    }

    let mut candidates = Vec::new();
    let import_candidates = if base.extension().is_some() {
        vec![base]
    } else {
        import_module_candidates(&base, &module_extensions(extension))
    };

    for candidate in import_candidates {
        let candidate = normalize_path(candidate);

        if module_candidate_exists(&candidate, indexed_paths)
            && module_name_for_any_root(&search_roots, &candidate, separator).is_some()
            && !candidates.contains(&candidate)
        {
            candidates.push(candidate);
        }
    }

    Ok((candidates, search_roots))
}

fn is_child_relative_import_specifier(specifier: &str) -> bool {
    specifier.starts_with("./") && !has_parent_path_component(specifier)
}

fn is_project_import_specifier(specifier: &str) -> bool {
    !specifier.is_empty() && !specifier.starts_with('.') && !has_parent_path_component(specifier)
}

fn is_parent_relative_import_specifier(specifier: &str) -> bool {
    specifier.starts_with("../")
        || specifier.starts_with("./") && has_parent_path_component(specifier)
}

fn has_parent_path_component(specifier: &str) -> bool {
    Path::new(specifier)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
}

fn module_name_for_any_root(
    roots: &[PathBuf],
    path: &Path,
    separator: ModulePathSeparator,
) -> Option<String> {
    roots
        .iter()
        .filter(|root| path.starts_with(root))
        .max_by_key(|root| root.components().count())
        .and_then(|root| module_name_for_path(root, path, separator))
}

fn import_module_candidates(base: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    for extension in extensions {
        candidates.push(base.with_extension(extension));
        candidates.push(base.join(format!("init.{extension}")));
    }

    candidates
}

fn existing_candidates(
    module: &str,
    indexed_paths: &BTreeSet<PathBuf>,
    search_roots: &[PathBuf],
    source_roots: &[PathBuf],
    output_roots: &[PathBuf],
    extension: &str,
    separator: ModulePathSeparator,
) -> Vec<PathBuf> {
    let relative = module_relative_path(module, separator);
    let extensions = module_extensions(extension);
    let mut candidates = Vec::new();

    for root in search_roots {
        for candidate in module_candidates(root, &relative, &extensions) {
            let candidate = normalize_path(candidate);

            if module_candidate_exists(&candidate, indexed_paths)
                && !is_shadowed_output_candidate(
                    &candidate,
                    indexed_paths,
                    source_roots,
                    output_roots,
                    extension,
                )
                && !candidates.contains(&candidate)
            {
                candidates.push(candidate);
            }
        }
    }

    candidates
}

fn is_shadowed_output_candidate(
    candidate: &Path,
    indexed_paths: &BTreeSet<PathBuf>,
    source_roots: &[PathBuf],
    output_roots: &[PathBuf],
    extension: &str,
) -> bool {
    if extension == "lua"
        || candidate
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("lua")
    {
        return false;
    }

    let Some(output_root) = output_roots
        .iter()
        .filter(|root| candidate.starts_with(root))
        .max_by_key(|root| root.components().count())
    else {
        return false;
    };
    let Ok(relative) = candidate.strip_prefix(output_root) else {
        return false;
    };

    let source_extensions = module_extensions(extension);

    source_roots.iter().any(|source_root| {
        let source_base = source_root.join(relative);

        source_extensions.iter().any(|source_extension| {
            let source_candidate = normalize_path(source_base.with_extension(source_extension));

            source_candidate != candidate
                && module_candidate_exists(&source_candidate, indexed_paths)
        })
    })
}

fn export_entries_for_module(
    path: &Path,
    parsed: Option<&ParsedSource>,
    hir: Option<&HirModule>,
    exports: &ModuleExports,
    source_kind: AnalysisSourceKind,
) -> Vec<ModuleExportEntry> {
    exports
        .fields
        .iter()
        .filter_map(|(public_name, field)| {
            let export = hir.and_then(|hir| {
                hir.exports
                    .iter()
                    .find(|export| export.name == *public_name)
            });
            let symbol_id = export.and_then(|export| match export.value {
                HirExportValue::Symbol(symbol_id) => Some(symbol_id),
                HirExportValue::Unknown => None,
            });
            let definition_span = symbol_id
                .and_then(|symbol_id| hir?.symbol(symbol_id).map(|symbol| symbol.name_span))
                .or_else(|| export_definition_span(hir, public_name))
                .or_else(|| export.map(|export| export.name_span))?;
            let export_span = export
                .map(|export| export.name_span)
                .unwrap_or(definition_span);

            Some(ModuleExportEntry {
                module_path: path.to_path_buf(),
                public_name: public_name.clone(),
                symbol_id,
                definition_span,
                export_span,
                field: field.clone(),
                origin: export_origin(source_kind, parsed),
            })
        })
        .collect()
}

fn export_definition_span(hir: Option<&HirModule>, public_name: &str) -> Option<SourceSpan> {
    let hir = hir?;
    hir.symbols()
        .iter()
        .find(|symbol| {
            symbol.name == public_name
                || symbol
                    .name
                    .strip_suffix(public_name)
                    .is_some_and(|prefix| prefix.ends_with('.'))
        })
        .map(|symbol| symbol.name_span)
}

fn export_origin(
    source_kind: AnalysisSourceKind,
    parsed: Option<&ParsedSource>,
) -> ModuleExportOrigin {
    if parsed.is_some_and(|parsed| !parsed.lua_doc_annotations.is_empty()) {
        return ModuleExportOrigin::LuaDoc;
    }

    match source_kind {
        AnalysisSourceKind::Tua => ModuleExportOrigin::Tua,
        AnalysisSourceKind::Lua => ModuleExportOrigin::Lua,
    }
}

fn search_roots(inputs: &[ModuleGraphInput<'_>], options: &ModuleGraphOptions) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    for root in &options.source_roots {
        push_unique_path(&mut roots, root);
    }
    for root in &options.output_roots {
        push_unique_path(&mut roots, root);
    }

    if roots.is_empty() {
        for input in inputs {
            if let Some(parent) = input.path.parent() {
                push_unique_path(&mut roots, parent);
            }
        }
    }

    roots
}

fn module_relative_path(module: &str, separator: ModulePathSeparator) -> PathBuf {
    let separator = match separator {
        ModulePathSeparator::Dot => '.',
        ModulePathSeparator::Slash => '/',
    };
    let mut relative = PathBuf::new();

    for part in module.split(separator).filter(|part| !part.is_empty()) {
        relative.push(part);
    }

    relative
}

fn module_extensions(project_extension: &str) -> Vec<&str> {
    let mut extensions = vec![project_extension];

    if project_extension != "lua" {
        extensions.push("lua");
    }

    extensions
}

fn module_candidates(root: &Path, relative: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let base = root.join(relative);
    let mut candidates = Vec::new();

    for extension in extensions {
        candidates.push(base.with_extension(extension));
        candidates.push(base.join(format!("init.{extension}")));
    }

    candidates
}

fn module_candidate_exists(candidate: &Path, indexed_paths: &BTreeSet<PathBuf>) -> bool {
    indexed_paths.contains(candidate) || candidate.is_file()
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: &Path) {
    let path = normalize_path(path);

    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn missing_module_diagnostic(edge: &RequireGraphEdge, search_roots: &[PathBuf]) -> Diagnostic {
    let roots = search_roots
        .iter()
        .map(|root| root.to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join(", ");

    Diagnostic::new(
        edge.source.clone(),
        edge.span.start.line,
        edge.span.start.column,
        DiagnosticSeverity::Warning,
        format!(
            "{} file not found: `{}` (searched: {})",
            edge.kind.label(),
            edge.module,
            roots
        ),
    )
    .with_code(DiagnosticCode::LINT_MISSING_REQUIRE)
}

fn invalid_module_specifier_diagnostic(edge: &RequireGraphEdge, reason: &str) -> Diagnostic {
    Diagnostic::new(
        edge.source.clone(),
        edge.span.start.line,
        edge.span.start.column,
        DiagnosticSeverity::Warning,
        format!(
            "invalid {} specifier `{}`: {reason}",
            edge.kind.label(),
            edge.module
        ),
    )
    .with_code(DiagnosticCode::LINT_BAD_REQUIRE)
}

fn duplicate_module_diagnostic(
    edge: &RequireGraphEdge,
    candidates: &[PathBuf],
    selected: &Path,
) -> Diagnostic {
    let candidates = candidates
        .iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join(", ");

    Diagnostic::new(
        edge.source.clone(),
        edge.span.start.line,
        edge.span.start.column,
        DiagnosticSeverity::Warning,
        format!(
            "duplicate {} candidates for `{}`: {candidates} (using {})",
            edge.kind.label(),
            edge.module,
            selected.to_string_lossy().replace('\\', "/")
        ),
    )
    .with_code(DiagnosticCode::LINT_DUPLICATE_REQUIRE_CANDIDATE)
}

#[cfg(test)]
mod tests;
