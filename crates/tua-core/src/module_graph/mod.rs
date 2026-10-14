use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use full_moon::ast::{self, Expression, Field, FunctionArgs, LastStmt, Prefix, Suffix};
use full_moon::node::Node;

use crate::ast_utils::{plain_field_chain_from_var, token_text};
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
    declaration_modules: BTreeMap<String, PathBuf>,
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
    pub declaration_modules: BTreeMap<String, PathBuf>,
}

#[derive(Clone, Copy)]
struct ModuleResolutionContext<'a> {
    indexed_paths: &'a BTreeSet<PathBuf>,
    search_roots: &'a [PathBuf],
    source_roots: &'a [PathBuf],
    output_roots: &'a [PathBuf],
    extension: &'a str,
    separator: ModulePathSeparator,
    declaration_modules: &'a BTreeMap<String, PathBuf>,
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
    pub type_only: bool,
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

#[derive(Clone, Debug)]
struct RuntimeExportForward {
    source_path: PathBuf,
    module: String,
    public_name: Option<String>,
    fields: Vec<String>,
    span: SourceSpan,
}

impl ModuleGraph {
    pub fn build<'a>(
        inputs: impl IntoIterator<Item = ModuleGraphInput<'a>>,
        options: ModuleGraphOptions,
    ) -> Self {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        let mut graph = Self {
            declaration_modules: options.declaration_modules.clone(),
            ..Self::default()
        };
        let indexed_paths = inputs
            .iter()
            .map(|input| normalize_path(input.path))
            .collect::<BTreeSet<_>>();
        let search_roots = search_roots(&inputs, &options);
        let resolution_context = ModuleResolutionContext {
            indexed_paths: &indexed_paths,
            search_roots: &search_roots,
            source_roots: &options.source_roots,
            output_roots: &options.output_roots,
            extension: &options.extension,
            separator: options.require_path_separator,
            declaration_modules: &options.declaration_modules,
        };

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
                module.require_edges = require_edges_for_source(&path, parsed, resolution_context);
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

        let forwards = inputs
            .iter()
            .filter_map(|input| {
                input.parsed.map(|parsed| {
                    runtime_export_forwards(&normalize_path(input.path), parsed, input.hir)
                })
            })
            .flatten()
            .collect::<Vec<_>>();
        graph.propagate_runtime_export_forwards(&forwards);

        graph.collect_require_diagnostics();
        graph.collect_cycle_diagnostics();
        graph.cycles.sort();
        graph
    }

    pub fn module(&self, path: impl AsRef<Path>) -> Option<&ModuleGraphModule> {
        self.modules.get(&normalize_path(path))
    }

    pub fn declaration_module(&self, module: &str) -> Option<&Path> {
        self.declaration_modules.get(module).map(PathBuf::as_path)
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

    fn propagate_runtime_export_forwards(&mut self, forwards: &[RuntimeExportForward]) {
        // Every successful pass resolves at least one facade field. Limiting the
        // passes to the number of forwarding expressions makes require cycles
        // terminate without turning unresolved dynamic values into guessed types.
        for _ in 0..forwards.len() {
            let mut changed = false;

            for forward in forwards {
                let Some(target_path) =
                    self.resolved_require_path(&forward.source_path, &forward.module)
                else {
                    continue;
                };

                if let Some(public_name) = &forward.public_name {
                    let Some(field) = self.forwarded_field(&target_path, &forward.fields) else {
                        continue;
                    };
                    let should_replace = self
                        .export(&forward.source_path, public_name)
                        .is_none_or(|entry| entry.field.ty == crate::Type::Any);
                    if !should_replace {
                        continue;
                    }

                    let existing = self.export(&forward.source_path, public_name).cloned();
                    let entry = ModuleExportEntry {
                        module_path: forward.source_path.clone(),
                        public_name: public_name.clone(),
                        symbol_id: existing.as_ref().and_then(|entry| entry.symbol_id),
                        definition_span: existing
                            .as_ref()
                            .map(|entry| entry.definition_span)
                            .unwrap_or(forward.span),
                        export_span: existing
                            .as_ref()
                            .map(|entry| entry.export_span)
                            .unwrap_or(forward.span),
                        field: merge_forwarded_field(
                            existing.as_ref().map(|entry| &entry.field),
                            field,
                        ),
                        origin: self
                            .modules
                            .get(&forward.source_path)
                            .map(|module| export_origin(module.source_kind, None))
                            .unwrap_or(ModuleExportOrigin::Tua),
                    };
                    changed |= self.upsert_forwarded_export(entry);
                } else {
                    let target_exports = self
                        .modules
                        .get(&target_path)
                        .map(|module| module.exports.clone())
                        .unwrap_or_default();

                    for target in target_exports {
                        let existing = self
                            .export(&forward.source_path, &target.public_name)
                            .cloned();
                        let should_replace = existing
                            .as_ref()
                            .is_none_or(|entry| entry.field.ty == crate::Type::Any);
                        if !should_replace {
                            continue;
                        }
                        let entry = ModuleExportEntry {
                            module_path: forward.source_path.clone(),
                            public_name: target.public_name,
                            symbol_id: existing.as_ref().and_then(|entry| entry.symbol_id),
                            definition_span: existing
                                .as_ref()
                                .map(|entry| entry.definition_span)
                                .unwrap_or(forward.span),
                            export_span: existing
                                .as_ref()
                                .map(|entry| entry.export_span)
                                .unwrap_or(forward.span),
                            field: merge_forwarded_field(
                                existing.as_ref().map(|entry| &entry.field),
                                target.field,
                            ),
                            origin: self
                                .modules
                                .get(&forward.source_path)
                                .map(|module| export_origin(module.source_kind, None))
                                .unwrap_or(ModuleExportOrigin::Tua),
                        };
                        changed |= self.upsert_forwarded_export(entry);
                    }
                }
            }

            if !changed {
                break;
            }
        }
    }

    fn resolved_require_path(&self, source_path: &Path, module: &str) -> Option<PathBuf> {
        match self.resolve_require(source_path, module)? {
            RequireResolution::Resolved { path }
            | RequireResolution::Duplicate { selected: path, .. } => Some(path.clone()),
            RequireResolution::Missing { .. } | RequireResolution::Invalid { .. } => None,
        }
    }

    fn forwarded_field(&self, target_path: &Path, fields: &[String]) -> Option<ModuleExportField> {
        let (first, rest) = fields.split_first()?;
        let entry = self.export(target_path, first)?;
        if rest.is_empty() {
            return Some(entry.field.clone());
        }

        forwarded_nested_type(&entry.field.ty, rest).map(|ty| ModuleExportField {
            ty,
            documentation: None,
            deprecation: None,
            parameter_names: Vec::new(),
            parameter_documentation: Vec::new(),
        })
    }

    fn upsert_forwarded_export(&mut self, entry: ModuleExportEntry) -> bool {
        let key = (entry.module_path.clone(), entry.public_name.clone());
        if self.export_index.get(&key) == Some(&entry) {
            return false;
        }

        self.export_index.insert(key, entry.clone());
        if let Some(module) = self.modules.get_mut(&entry.module_path) {
            if let Some(existing) = module
                .exports
                .iter_mut()
                .find(|existing| existing.public_name == entry.public_name)
            {
                *existing = entry;
            } else {
                module.exports.push(entry);
                module
                    .exports
                    .sort_by(|left, right| left.public_name.cmp(&right.public_name));
            }
        }
        true
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
        let mut visited = BTreeSet::new();
        let mut reported = BTreeSet::new();

        for path in self.modules.keys().cloned().collect::<Vec<_>>() {
            let path = normalize_path(path);
            if !visited.insert(path.clone()) {
                continue;
            }

            let mut visiting = vec![path.clone()];
            let mut active = BTreeMap::from([(path.clone(), 0usize)]);
            let mut stack = vec![(path.clone(), self.runtime_dependencies(&path), 0usize)];

            while let Some((current_path, dependencies, next_dependency)) = stack.last_mut() {
                let Some(dependency) = dependencies.get(*next_dependency).cloned() else {
                    active.remove(current_path);
                    stack.pop();
                    visiting.pop();
                    continue;
                };
                *next_dependency += 1;

                if let Some(&index) = active.get(&dependency) {
                    let cycle = visiting[index..].to_vec();
                    if reported.insert(cycle.clone()) {
                        self.cycles.push(cycle.clone());
                        let display = cycle
                            .iter()
                            .chain(std::iter::once(&dependency))
                            .map(|path| path.to_string_lossy().replace('\\', "/"))
                            .collect::<Vec<_>>()
                            .join(" -> ");

                        self.diagnostics
                            .entry(dependency.clone())
                            .or_default()
                            .push(
                                Diagnostic::new(
                                    dependency,
                                    1,
                                    1,
                                    DiagnosticSeverity::Warning,
                                    format!("require cycle detected: {display}"),
                                )
                                .with_code(DiagnosticCode::LINT_REQUIRE_CYCLE),
                            );
                    }
                    continue;
                }

                if !visited.insert(dependency.clone()) {
                    continue;
                }

                let child_dependencies = self.runtime_dependencies(&dependency);
                active.insert(dependency.clone(), visiting.len());
                visiting.push(dependency.clone());
                stack.push((dependency, child_dependencies, 0));
            }
        }
    }

    fn runtime_dependencies(&self, path: &Path) -> Vec<PathBuf> {
        self.module(path)
            .map(|module| {
                module
                    .require_edges
                    .iter()
                    .filter(|edge| !edge.type_only)
                    .filter_map(|edge| match &edge.resolution {
                        RequireResolution::Resolved { path }
                        | RequireResolution::Duplicate { selected: path, .. } => Some(path.clone()),
                        RequireResolution::Missing { .. } | RequireResolution::Invalid { .. } => {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn require_edges_for_source(
    source_path: &Path,
    parsed: &ParsedSource,
    context: ModuleResolutionContext<'_>,
) -> Vec<RequireGraphEdge> {
    module_specifier_literals(&parsed.source)
        .into_iter()
        .map(|literal| {
            let type_only = parsed.type_imports.iter().any(|import| {
                import.module == literal.module && import.module_span == literal.span
            });
            let resolution = match literal.kind {
                ModuleSpecifierKind::Require => {
                    let candidates = context
                        .declaration_modules
                        .get(&literal.module)
                        .cloned()
                        .map(|path| vec![path])
                        .unwrap_or_else(|| existing_candidates(&literal.module, context));

                    resolution_for_candidates(candidates, context.search_roots.to_vec())
                }
                ModuleSpecifierKind::Import => {
                    match imported_module_candidates(
                        source_path,
                        &literal.module,
                        context,
                        type_only,
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
                type_only,
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
    context: ModuleResolutionContext<'_>,
    include_declarations: bool,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    if is_child_relative_import_specifier(specifier) {
        let search_roots = source_path
            .parent()
            .map(|path| vec![normalize_path(path)])
            .unwrap_or_default();
        let candidates = child_relative_import_candidates(
            source_path,
            specifier,
            search_roots,
            context,
            include_declarations,
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

    let candidates =
        existing_candidates_with_declarations(specifier, context, include_declarations);

    Ok((candidates, context.search_roots.to_vec()))
}

fn child_relative_import_candidates(
    source_path: &Path,
    specifier: &str,
    search_roots: Vec<PathBuf>,
    context: ModuleResolutionContext<'_>,
    include_declarations: bool,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    let parent = source_path
        .parent()
        .ok_or_else(|| "import specifier needs a source file parent directory".to_owned())?;
    let base = normalize_path(parent.join(specifier));

    if let Some(file_extension) = base.extension().and_then(|extension| extension.to_str()) {
        if file_extension != "lua" && file_extension != context.extension {
            return Err(format!(
                "import specifier must be extensionless or end in `.{}`/`.lua`",
                context.extension
            ));
        }
    }

    let mut candidates = Vec::new();
    let import_candidates = if base.extension().is_some() {
        vec![base]
    } else {
        import_module_candidates_with_declarations(
            &base,
            &module_extensions(context.extension),
            include_declarations,
        )
    };

    for candidate in import_candidates {
        let candidate = normalize_path(candidate);

        if module_candidate_exists(&candidate, context.indexed_paths)
            && module_name_for_any_root(&search_roots, &candidate, context.separator).is_some()
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

fn import_module_candidates_with_declarations(
    base: &Path,
    extensions: &[&str],
    include_declarations: bool,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    for extension in extensions {
        if include_declarations && *extension != "lua" {
            candidates.push(base.with_extension(format!("d.{extension}")));
            candidates.push(base.join(format!("init.d.{extension}")));
        }
        candidates.push(base.with_extension(extension));
        candidates.push(base.join(format!("init.{extension}")));
    }

    candidates
}

fn existing_candidates(module: &str, context: ModuleResolutionContext<'_>) -> Vec<PathBuf> {
    existing_candidates_with_declarations(module, context, false)
}

fn existing_candidates_with_declarations(
    module: &str,
    context: ModuleResolutionContext<'_>,
    include_declarations: bool,
) -> Vec<PathBuf> {
    let relative = module_relative_path(module, context.separator);
    let extensions = module_extensions(context.extension);
    let mut candidates = Vec::new();

    for root in context.search_roots {
        for candidate in
            module_candidates_with_declarations(root, &relative, &extensions, include_declarations)
        {
            let candidate = normalize_path(candidate);

            if module_candidate_exists(&candidate, context.indexed_paths)
                && !is_shadowed_output_candidate(
                    &candidate,
                    context.indexed_paths,
                    context.source_roots,
                    context.output_roots,
                    context.extension,
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

fn runtime_export_forwards(
    path: &Path,
    parsed: &ParsedSource,
    hir: Option<&HirModule>,
) -> Vec<RuntimeExportForward> {
    let owned_hir;
    let hir = match hir {
        Some(hir) => hir,
        None => {
            owned_hir = lower_parsed_source(parsed);
            &owned_hir
        }
    };
    let bindings = hir
        .require_bindings
        .iter()
        .map(|binding| (binding.alias.as_str(), binding.module.as_str()))
        .collect::<BTreeMap<_, _>>();
    let Some(LastStmt::Return(return_stmt)) = parsed.ast.nodes().last_stmt() else {
        return Vec::new();
    };
    let Some(expression) = return_stmt.returns().iter().next() else {
        return Vec::new();
    };

    if let Some(module) = directly_returned_module(expression, &bindings) {
        return node_span(expression)
            .map(|span| {
                vec![RuntimeExportForward {
                    source_path: path.to_path_buf(),
                    module,
                    public_name: None,
                    fields: Vec::new(),
                    span,
                }]
            })
            .unwrap_or_default();
    }

    let Expression::TableConstructor(table) = unparenthesized_expression(expression) else {
        return Vec::new();
    };

    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NameKey { key, value, .. } = field else {
                return None;
            };
            let chain = plain_field_chain_from_expression(value)?;
            let (root, fields) = chain.split_first()?;
            if fields.is_empty() {
                return None;
            }
            let module = bindings.get(root.as_str())?.to_string();

            Some(RuntimeExportForward {
                source_path: path.to_path_buf(),
                module,
                public_name: Some(token_text(key)),
                fields: fields.to_vec(),
                span: node_span(field)?,
            })
        })
        .collect()
}

fn directly_returned_module(
    expression: &Expression,
    bindings: &BTreeMap<&str, &str>,
) -> Option<String> {
    let expression = unparenthesized_expression(expression);
    if let Expression::Var(var) = expression {
        let chain = plain_field_chain_from_var(var)?;
        let [name] = chain.as_slice() else {
            return None;
        };
        return bindings
            .get(name.as_str())
            .map(|module| (*module).to_owned());
    }

    let Expression::FunctionCall(call) = expression else {
        return None;
    };
    let Prefix::Name(name) = call.prefix() else {
        return None;
    };
    if token_text(name) != "require" {
        return None;
    }
    let mut suffixes = call.suffixes();
    let call = suffixes.next()?;
    if suffixes.next().is_some() {
        return None;
    }
    let Suffix::Call(ast::Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })) = call
    else {
        return None;
    };
    let Expression::String(module) = arguments.iter().next()? else {
        return None;
    };

    Some(unquote_string(&module.to_string()))
}

fn unparenthesized_expression(mut expression: &Expression) -> &Expression {
    while let Expression::Parentheses {
        expression: inner, ..
    }
    | Expression::TypeAssertion {
        expression: inner, ..
    } = expression
    {
        expression = inner;
    }
    expression
}

fn plain_field_chain_from_expression(expression: &Expression) -> Option<Vec<String>> {
    match unparenthesized_expression(expression) {
        Expression::Var(var) => plain_field_chain_from_var(var),
        _ => None,
    }
}

fn forwarded_nested_type(ty: &crate::Type, fields: &[String]) -> Option<crate::Type> {
    let mut current = ty.clone();
    for field in fields {
        current = match current {
            crate::Type::Table(table) => table.semantic_field(field)?.ty.clone(),
            crate::Type::Union(types) => {
                let mut resolved = types
                    .iter()
                    .filter_map(|ty| forwarded_nested_type(ty, std::slice::from_ref(field)))
                    .collect::<Vec<_>>();
                match resolved.len() {
                    0 => return None,
                    1 => resolved.remove(0),
                    _ => crate::Type::union(resolved),
                }
            }
            crate::Type::Any
            | crate::Type::Alias(_)
            | crate::Type::Function(_)
            | crate::Type::Primitive(_)
            | crate::Type::StringLiteral(_)
            | crate::Type::Array(_)
            | crate::Type::Tuple(_)
            | crate::Type::Dictionary { .. } => return None,
        };
    }
    Some(current)
}

fn merge_forwarded_field(
    existing: Option<&ModuleExportField>,
    mut forwarded: ModuleExportField,
) -> ModuleExportField {
    let Some(existing) = existing else {
        return forwarded;
    };
    if existing.documentation.is_some() {
        forwarded.documentation.clone_from(&existing.documentation);
    }
    if existing.deprecation.is_some() {
        forwarded.deprecation.clone_from(&existing.deprecation);
    }
    if !existing.parameter_names.is_empty() {
        forwarded
            .parameter_names
            .clone_from(&existing.parameter_names);
    }
    if !existing.parameter_documentation.is_empty() {
        forwarded
            .parameter_documentation
            .clone_from(&existing.parameter_documentation);
    }
    forwarded
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn unquote_string(value: &str) -> String {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
        .to_owned()
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

fn module_candidates_with_declarations(
    root: &Path,
    relative: &Path,
    extensions: &[&str],
    include_declarations: bool,
) -> Vec<PathBuf> {
    let base = root.join(relative);
    let mut candidates = Vec::new();

    for extension in extensions {
        if include_declarations && *extension != "lua" {
            candidates.push(base.with_extension(format!("d.{extension}")));
            candidates.push(base.join(format!("init.d.{extension}")));
        }
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
