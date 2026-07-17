use std::path::PathBuf;

use crate::{
    AnalysisSourceKind, ModuleExportOrigin, ModuleSpecifierKind, RequireResolution, SourceSpan,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleDependencyGraph {
    pub modules: Vec<ModuleDependencyModule>,
    pub edges: Vec<ModuleDependencyEdge>,
    pub cycles: Vec<Vec<PathBuf>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDependencyModule {
    pub path: PathBuf,
    pub source_kind: AnalysisSourceKind,
    pub is_root: bool,
    pub in_cycle: bool,
    pub dependency_count: usize,
    pub unresolved_dependency_count: usize,
    pub reverse_dependency_count: usize,
    pub exports: Vec<ModuleDependencyExport>,
    pub inclusion_path: Option<ModuleInclusionPath>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDependencyEdge {
    pub source: PathBuf,
    pub specifier: String,
    pub kind: ModuleSpecifierKind,
    pub span: SourceSpan,
    pub resolution: RequireResolution,
    pub in_cycle: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDependencyExport {
    pub name: String,
    pub type_label: String,
    pub documentation: Option<String>,
    pub origin: ModuleExportOrigin,
    pub definition_span: SourceSpan,
    pub export_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleInclusionPath {
    pub root: PathBuf,
    pub steps: Vec<ModuleInclusionStep>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleInclusionStep {
    pub source: PathBuf,
    pub target: PathBuf,
    pub specifier: String,
    pub kind: ModuleSpecifierKind,
    pub span: SourceSpan,
}
