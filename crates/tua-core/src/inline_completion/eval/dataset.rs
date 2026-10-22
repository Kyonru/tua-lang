//! Evaluation datasets: the committed manifest, the project split, and
//! de-duplication across sets.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{AnalysisSourceKind, ProjectConfig, ProjectModel};

/// What a set is for. The role decides de-duplication priority and whether the
/// set is a quality gate that requires committed thresholds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SetRole {
    /// Held-out projects and frozen fixtures. Never used for tuning.
    Holdout,
    /// Pinned third-party projects, fetched into `target/`.
    External,
    /// Small committed fixtures for CI.
    Smoke,
    /// Projects and files used while developing and tuning thresholds.
    Tune,
    /// Paths given on the command line.
    Adhoc,
}

impl SetRole {
    pub fn is_gate(self) -> bool {
        matches!(self, Self::Holdout | Self::External)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    /// The directory set paths are relative to, itself relative to the
    /// manifest's directory. Defaults to the manifest's directory.
    #[serde(default)]
    pub root: Option<PathBuf>,
    pub sets: BTreeMap<String, ManifestSet>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestSet {
    pub role: SetRole,
    #[serde(default)]
    pub description: String,
    /// Project directories. A directory with a `tua.toml` uses its source
    /// roots; one without uses the whole directory as its source root.
    #[serde(default)]
    pub projects: Vec<PathBuf>,
    /// Loose source files, each evaluated as its own single-file project.
    #[serde(default)]
    pub files: Vec<PathBuf>,
    /// Path fragments to skip inside projects, such as generated output
    /// (`/generated/`) that nobody types by hand.
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let manifest: Self =
            toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        if manifest.version != 1 {
            return Err(format!(
                "{}: unsupported manifest version {}",
                path.display(),
                manifest.version
            ));
        }
        let mut manifest = manifest;
        let directory = path.parent().unwrap_or(Path::new("."));
        let base = directory.join(manifest.root.clone().unwrap_or_else(|| PathBuf::from(".")));
        for set in manifest.sets.values_mut() {
            set.resolve(&base);
        }
        check_split(manifest.sets.iter().map(|(name, set)| (name.as_str(), set)))?;
        Ok(manifest)
    }
}

impl ManifestSet {
    /// Resolves every path against `base`, canonicalizing the ones that exist.
    pub fn resolve(&mut self, base: &Path) {
        for path in self.projects.iter_mut().chain(self.files.iter_mut()) {
            *path = resolve_path(base, path);
        }
    }
}

/// `path` resolved against `base`: canonical when it exists, so `./x`,
/// `x`, and a symbolic link to `x` are the same path, and otherwise
/// normalized lexically.
pub fn resolve_path(base: &Path, path: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    joined
        .canonicalize()
        .unwrap_or_else(|_| lexically_normalize(&joined))
}

fn lexically_normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir
                if matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                ) =>
            {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

/// The split is by project: no path may appear twice, and no project may
/// contain another, within one set or across sets. Otherwise a held-out file
/// could also be read during tuning, or a file counted twice.
pub fn check_split<'a>(
    sets: impl IntoIterator<Item = (&'a str, &'a ManifestSet)>,
) -> Result<(), String> {
    let entries = sets
        .into_iter()
        .flat_map(|(name, set)| {
            set.projects
                .iter()
                .chain(&set.files)
                .map(move |path| (name, path.as_path()))
        })
        .collect::<Vec<_>>();
    for (index, (left_set, left)) in entries.iter().enumerate() {
        for (right_set, right) in &entries[index + 1..] {
            if left.starts_with(right) || right.starts_with(left) {
                return Err(format!(
                    "{} (`{left_set}`) and {} (`{right_set}`) overlap; every project and file must belong to exactly one set",
                    left.display(),
                    right.display()
                ));
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalFile {
    pub path: PathBuf,
    pub kind: AnalysisSourceKind,
    pub source: String,
}

impl EvalFile {
    pub fn language(&self) -> &'static str {
        match self.kind {
            AnalysisSourceKind::Tua => "tua",
            AnalysisSourceKind::Lua => "lua",
        }
    }
}

/// One project as the evaluator loads it into an `AnalysisHost`.
#[derive(Clone, Debug)]
pub struct EvalProject {
    pub name: String,
    pub config: ProjectConfig,
    pub source_roots: Vec<PathBuf>,
    pub output_roots: Vec<PathBuf>,
    pub asset_roots: Vec<PathBuf>,
    pub files: Vec<EvalFile>,
}

#[derive(Clone, Debug)]
pub struct EvalSet {
    pub name: String,
    pub role: SetRole,
    pub projects: Vec<EvalProject>,
}

/// Files dropped because identical content already appeared in a set with
/// higher priority, or earlier in the same set.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct DuplicateReport {
    pub dropped: Vec<String>,
}

/// Loads every set, then removes duplicate files before evaluation. Content is
/// compared after normalizing line endings and surrounding whitespace. A file
/// is kept in the set with the highest-priority role (holdout first, tune
/// last), so shared content can never appear on both sides of the split.
pub fn load_sets(
    sets: impl IntoIterator<Item = (String, ManifestSet)>,
) -> Result<(Vec<EvalSet>, DuplicateReport), String> {
    let sets = sets.into_iter().collect::<Vec<_>>();
    check_split(sets.iter().map(|(name, set)| (name.as_str(), set)))?;
    let mut loaded = sets
        .into_iter()
        .map(|(name, set)| load_set(name, &set))
        .collect::<Result<Vec<_>, _>>()?;
    loaded.sort_by(|left, right| left.role.cmp(&right.role).then(left.name.cmp(&right.name)));

    let mut seen = BTreeSet::new();
    let mut report = DuplicateReport::default();
    for set in &mut loaded {
        for project in &mut set.projects {
            project.files.retain(|file| {
                let fresh = seen.insert(content_key(&file.source));
                if !fresh {
                    report
                        .dropped
                        .push(format!("{}: {}", set.name, display_name(&file.path)));
                }
                fresh
            });
        }
        set.projects.retain(|project| !project.files.is_empty());
    }

    Ok((loaded, report))
}

fn load_set(name: String, set: &ManifestSet) -> Result<EvalSet, String> {
    let mut projects = Vec::new();
    for directory in &set.projects {
        let mut project = load_project(directory)?;
        project.files.retain(|file| {
            let path = file.path.to_string_lossy();
            !set.exclude
                .iter()
                .any(|fragment| path.contains(fragment.as_str()))
        });
        projects.push(project);
    }
    for file in &set.files {
        projects.push(load_single_file(file)?);
    }

    Ok(EvalSet {
        name,
        role: set.role,
        projects,
    })
}

pub fn load_project(directory: &Path) -> Result<EvalProject, String> {
    if !directory.is_dir() {
        return Err(format!(
            "{} is not a directory; external projects are fetched by `make inline-eval-external`",
            directory.display()
        ));
    }
    // Manifest project entries name workspace directories, not files inside
    // them. Loading "for" the directory starts the config search at its
    // parent, which can silently miss a `tua.toml` in the named workspace and
    // make generated output part of the evaluation corpus.
    let model = ProjectModel::load([directory.to_path_buf()])
        .map_err(|error| format!("{}: {error:?}", directory.display()))?;
    let workspace = model
        .primary_workspace()
        .ok_or_else(|| format!("{}: no workspace", directory.display()))?;

    let (config, source_roots, output_roots, asset_roots) = if workspace.config_found()
        && same_directory(
            workspace.config_path().parent().unwrap_or(directory),
            directory,
        ) {
        (
            workspace.config().clone(),
            workspace.source_roots().to_vec(),
            workspace.output_roots().to_vec(),
            workspace.asset_roots(),
        )
    } else {
        (
            ProjectConfig::default(),
            vec![directory.to_path_buf()],
            Vec::new(),
            vec![directory.to_path_buf()],
        )
    };

    let mut files = Vec::new();
    for root in &source_roots {
        collect_sources(root, &output_roots, &mut files)?;
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(EvalProject {
        name: display_name(directory),
        config,
        source_roots,
        output_roots,
        asset_roots,
        files,
    })
}

fn load_single_file(path: &Path) -> Result<EvalProject, String> {
    let kind =
        source_kind(path).ok_or_else(|| format!("{}: not a .tua or .lua file", path.display()))?;
    let source = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();

    Ok(EvalProject {
        name: display_name(path),
        config: ProjectConfig::default(),
        source_roots: vec![root.clone()],
        output_roots: Vec::new(),
        asset_roots: vec![root],
        files: vec![EvalFile {
            path: path.to_path_buf(),
            kind,
            source,
        }],
    })
}

fn collect_sources(
    directory: &Path,
    excluded: &[PathBuf],
    files: &mut Vec<EvalFile>,
) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Ok(());
    };
    let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let hidden = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('.'));
        if hidden
            || excluded
                .iter()
                .any(|excluded| same_directory(excluded, &path))
        {
            continue;
        }
        if path.is_dir() {
            collect_sources(&path, excluded, files)?;
        } else if let Some(kind) = source_kind(&path) {
            let source = fs::read_to_string(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            files.push(EvalFile { path, kind, source });
        }
    }

    Ok(())
}

/// `path` relative to the working directory when it is inside it, for
/// reports that read the same on every machine.
pub fn display_name(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|directory| directory.canonicalize().ok())
        .and_then(|directory| path.strip_prefix(directory).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

fn source_kind(path: &Path) -> Option<AnalysisSourceKind> {
    match path.extension()?.to_str()? {
        "tua" => Some(AnalysisSourceKind::Tua),
        "lua" => Some(AnalysisSourceKind::Lua),
        _ => None,
    }
}

fn same_directory(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

/// Whole normalized content, compared exactly so a collision can never drop a
/// distinct file.
fn content_key(source: &str) -> String {
    source.replace("\r\n", "\n").trim().to_owned()
}
