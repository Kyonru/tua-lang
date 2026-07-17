use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use super::*;

impl AnalysisSnapshot {
    pub fn module_dependency_graph(&self) -> ModuleDependencyGraph {
        let _profile = profile::span("analysis.query.module_dependency_graph");
        let graph = self.module_graph();
        let cycles = graph.cycles().to_vec();
        let mut edges = Vec::new();
        let mut incoming = BTreeMap::<PathBuf, usize>::new();

        for module in graph.modules() {
            for edge in &module.require_edges {
                let target = resolved_dependency_path(&edge.resolution);

                if let Some(target) = target {
                    *incoming.entry(target.to_path_buf()).or_default() += 1;
                }

                edges.push(ModuleDependencyEdge {
                    source: edge.source.clone(),
                    specifier: edge.module.clone(),
                    kind: edge.kind,
                    span: edge.span,
                    resolution: edge.resolution.clone(),
                    in_cycle: target.is_some_and(|target| {
                        dependency_is_in_cycle(&edge.source, target, &cycles)
                    }),
                });
            }
        }

        let roots = graph
            .modules()
            .filter(|module| !incoming.contains_key(&module.path))
            .map(|module| module.path.clone())
            .collect::<BTreeSet<_>>();
        let inclusion_paths = inclusion_paths(&edges, &roots);
        let modules = graph
            .modules()
            .map(|module| {
                let unresolved_dependency_count = module
                    .require_edges
                    .iter()
                    .filter(|edge| {
                        matches!(
                            edge.resolution,
                            RequireResolution::Missing { .. } | RequireResolution::Invalid { .. }
                        )
                    })
                    .count();
                let exports = module
                    .exports
                    .iter()
                    .map(|entry| ModuleDependencyExport {
                        name: entry.public_name.clone(),
                        type_label: entry.field.ty.to_string(),
                        documentation: entry.field.documentation.clone(),
                        origin: entry.origin,
                        definition_span: entry.definition_span,
                        export_span: entry.export_span,
                    })
                    .collect();

                ModuleDependencyModule {
                    path: module.path.clone(),
                    source_kind: module.source_kind,
                    is_root: roots.contains(&module.path),
                    in_cycle: cycles
                        .iter()
                        .any(|cycle| cycle.iter().any(|path| path == &module.path)),
                    dependency_count: module.require_edges.len(),
                    unresolved_dependency_count,
                    reverse_dependency_count: incoming.get(&module.path).copied().unwrap_or(0),
                    exports,
                    inclusion_path: inclusion_paths.get(&module.path).cloned(),
                }
            })
            .collect();

        ModuleDependencyGraph {
            modules,
            edges,
            cycles,
        }
    }
}

fn inclusion_paths(
    edges: &[ModuleDependencyEdge],
    roots: &BTreeSet<PathBuf>,
) -> BTreeMap<PathBuf, ModuleInclusionPath> {
    let mut outgoing = BTreeMap::<PathBuf, Vec<&ModuleDependencyEdge>>::new();

    for edge in edges {
        if resolved_dependency_path(&edge.resolution).is_some() {
            outgoing.entry(edge.source.clone()).or_default().push(edge);
        }
    }

    let mut paths = roots
        .iter()
        .map(|root| {
            (
                root.clone(),
                ModuleInclusionPath {
                    root: root.clone(),
                    steps: Vec::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut pending = roots.iter().cloned().collect::<VecDeque<_>>();

    while let Some(source) = pending.pop_front() {
        let Some(source_path) = paths.get(&source).cloned() else {
            continue;
        };

        for edge in outgoing.get(&source).into_iter().flatten() {
            let Some(target) = resolved_dependency_path(&edge.resolution) else {
                continue;
            };

            if paths.contains_key(target) {
                continue;
            }

            let mut path = source_path.clone();
            path.steps.push(ModuleInclusionStep {
                source: edge.source.clone(),
                target: target.to_path_buf(),
                specifier: edge.specifier.clone(),
                kind: edge.kind,
                span: edge.span,
            });
            paths.insert(target.to_path_buf(), path);
            pending.push_back(target.to_path_buf());
        }
    }

    paths
}

fn resolved_dependency_path(resolution: &RequireResolution) -> Option<&Path> {
    match resolution {
        RequireResolution::Resolved { path } => Some(path),
        RequireResolution::Duplicate { selected, .. } => Some(selected),
        RequireResolution::Missing { .. } | RequireResolution::Invalid { .. } => None,
    }
}

fn dependency_is_in_cycle(source: &Path, target: &Path, cycles: &[Vec<PathBuf>]) -> bool {
    cycles.iter().any(|cycle| {
        cycle.iter().any(|path| path == source) && cycle.iter().any(|path| path == target)
    })
}
