use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::asset_pipeline::{asset_pipeline_input_files, asset_pipeline_watch_roots};
use crate::build::{
    has_errors, output_path_for_asset, output_paths_for_tua_source, BuildOutcome, WatchBuildSession,
};
use crate::diagnostics::sorted_diagnostics;
use crate::error::CliError;
use crate::source::{
    discover_build_files, discover_lua_files, io_diagnostic, paths_same_location, SourceKind,
};
use crate::source_map::{debugger_source_map_path_for, source_map_path_for};
use backend::{ReceiveError, WatchBackend, WatchBatch};
use tua_core::{
    format_diagnostics_for_cli, load_config_from_dir, Diagnostic, LoadedConfig, ProjectWorkspace,
    CONFIG_FILE,
};

mod backend;

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(250);
const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(150);
const DEFAULT_RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const POLL_INTERVAL_ENV: &str = "TUA_INTERNAL_WATCH_POLL_MS";
const DEBOUNCE_ENV: &str = "TUA_INTERNAL_WATCH_DEBOUNCE_MS";
const RECONCILE_INTERVAL_ENV: &str = "TUA_INTERNAL_WATCH_RECONCILE_MS";
const EXIT_AFTER_REBUILDS_ENV: &str = "TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS";
const FORCE_POLL_ENV: &str = "TUA_INTERNAL_WATCH_FORCE_POLL";
const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

#[derive(Clone, Copy, Debug)]
struct WatchOptions {
    poll_interval: Duration,
    debounce: Duration,
    reconcile_interval: Duration,
    exit_after_rebuilds: Option<usize>,
    force_poll: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceFingerprint {
    len: u64,
    modified: SystemTime,
    content_hash: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceSnapshot {
    files: BTreeMap<PathBuf, SourceFingerprint>,
    pipeline_inputs: BTreeSet<PathBuf>,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct ChangeSet {
    changed: Vec<PathBuf>,
    deleted: Vec<PathBuf>,
}

#[derive(Debug, Default)]
struct RemovalOutcome {
    removed: usize,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
struct WatchLayout {
    project_root: PathBuf,
    project_config: tua_core::ProjectConfig,
    config_path: PathBuf,
    source_dir: PathBuf,
    out_dir: PathBuf,
    in_place: bool,
    extension: String,
    type_library_roots: Vec<PathBuf>,
    pipeline_watch_roots: Vec<PathBuf>,
}

struct ReloadedWatch {
    loaded: LoadedConfig,
    layout: WatchLayout,
    session: WatchBuildSession,
    watcher: WatchBackend,
    snapshot: SourceSnapshot,
    outcome: BuildOutcome,
    fallback_error: Option<notify::Error>,
}

impl WatchOptions {
    fn from_env() -> Self {
        Self {
            poll_interval: env_duration(POLL_INTERVAL_ENV, DEFAULT_POLL_INTERVAL),
            debounce: env_duration(DEBOUNCE_ENV, DEFAULT_DEBOUNCE),
            reconcile_interval: env_duration(RECONCILE_INTERVAL_ENV, DEFAULT_RECONCILE_INTERVAL),
            exit_after_rebuilds: env_usize(EXIT_AFTER_REBUILDS_ENV),
            force_poll: env_flag(FORCE_POLL_ENV),
        }
    }
}

impl WatchLayout {
    fn new(cwd: &Path, loaded: &LoadedConfig) -> Self {
        let workspace = ProjectWorkspace::from_loaded(cwd, loaded.clone());
        let source_dir = workspace.primary_source_root().to_path_buf();
        let out_dir = workspace.primary_output_root().to_path_buf();

        Self {
            project_root: cwd.to_path_buf(),
            project_config: loaded.config.clone(),
            config_path: cwd.join(CONFIG_FILE),
            in_place: paths_same_location(&source_dir, &out_dir),
            extension: workspace.config().extension.clone(),
            type_library_roots: workspace
                .type_libraries()
                .iter()
                .map(|library| library.root().to_path_buf())
                .collect(),
            pipeline_watch_roots: asset_pipeline_watch_roots(cwd, &loaded.config),
            source_dir,
            out_dir,
        }
    }

    fn is_type_library_path(&self, path: &Path) -> bool {
        self.type_library_roots
            .iter()
            .any(|root| path.starts_with(root))
    }

    fn additional_watch_roots(&self) -> Vec<PathBuf> {
        self.type_library_roots
            .iter()
            .chain(&self.pipeline_watch_roots)
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}

impl SourceSnapshot {
    fn diff(&self, next: &Self) -> ChangeSet {
        let mut changes = ChangeSet::default();

        for (path, fingerprint) in &next.files {
            if self.files.get(path) != Some(fingerprint) {
                changes.changed.push(path.clone());
            }
        }

        for path in self.files.keys() {
            if !next.files.contains_key(path) {
                changes.deleted.push(path.clone());
            }
        }

        changes
    }
}

impl ChangeSet {
    fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.deleted.is_empty()
    }

    fn len(&self) -> usize {
        self.changed.len() + self.deleted.len()
    }

    fn take_config_change(&mut self, config_path: &Path) -> bool {
        let changed = remove_path(&mut self.changed, config_path);
        let deleted = remove_path(&mut self.deleted, config_path);

        changed || deleted
    }
}

pub(crate) fn run_watch(cwd: impl AsRef<Path>, loaded: LoadedConfig) -> Result<(), CliError> {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();

    run_watch_with_options(
        cwd.as_ref(),
        loaded,
        WatchOptions::from_env(),
        &mut stdout,
        &mut stderr,
    )
}

fn run_watch_with_options<W: Write, E: Write>(
    cwd: &Path,
    mut loaded: LoadedConfig,
    options: WatchOptions,
    stdout: &mut W,
    stderr: &mut E,
) -> Result<(), CliError> {
    let mut layout = WatchLayout::new(cwd, &loaded);
    write_watch_target(cwd, &layout, stdout)?;

    let (mut watcher, fallback_error) = WatchBackend::start(
        cwd,
        &layout.source_dir,
        &layout.additional_watch_roots(),
        options.poll_interval,
        options.force_poll,
    )
    .map_err(watch_backend_error)?;

    if let Some(error) = fallback_error {
        write_stdout(
            stdout,
            &format!("native file watcher unavailable ({error}); using polling fallback"),
        )?;
    } else if options.force_poll {
        write_stdout(stdout, "using polling file watcher")?;
    }

    let (mut build_session, initial) = WatchBuildSession::start(cwd, loaded.clone())?;
    let mut snapshot = snapshot_watch_tree(&layout, None, &WatchBatch::rescan_all())
        .map_err(|diagnostic| watch_error("failed to read source directory", diagnostic))?;
    write_outcome(stdout, stderr, "initial build", "built", &initial)?;

    let mut rebuilds = 0usize;

    loop {
        let batch = match watcher.receive_batch(options.debounce, options.reconcile_interval) {
            Ok(batch) => batch,
            Err(ReceiveError::Backend(error)) if !watcher.is_polling() => {
                write_stdout(
                    stdout,
                    &format!("native file watcher failed ({error}); using polling fallback"),
                )?;
                watcher = WatchBackend::poll(
                    cwd,
                    &layout.source_dir,
                    &layout.additional_watch_roots(),
                    options.poll_interval,
                )
                .map_err(watch_backend_error)?;
                WatchBatch::rescan_all()
            }
            Err(ReceiveError::Backend(error)) => return Err(watch_backend_error(error)),
            Err(ReceiveError::Disconnected(error)) => {
                return Err(CliError::usage(format!(
                    "file watcher channel disconnected: {error}"
                )));
            }
        };

        let next = snapshot_watch_tree(&layout, Some(&snapshot), &batch)
            .map_err(|diagnostic| watch_error("failed to read watched files", diagnostic))?;
        let mut changes = snapshot.diff(&next);
        let pipeline_changed = changes.changed.iter().chain(&changes.deleted).any(|path| {
            snapshot.pipeline_inputs.contains(path) || next.pipeline_inputs.contains(path)
        });
        snapshot = next;

        if changes.is_empty() {
            continue;
        }

        if pipeline_changed {
            let (next_session, outcome) = WatchBuildSession::start(cwd, loaded.clone())?;
            build_session = next_session;
            snapshot = snapshot_watch_tree(&layout, None, &WatchBatch::rescan_all())
                .map_err(|diagnostic| watch_error("failed to read source directory", diagnostic))?;
            write_outcome(
                stdout,
                stderr,
                "asset pipeline rebuild",
                "rebuilt",
                &outcome,
            )?;

            rebuilds += 1;
            if reached_rebuild_limit(rebuilds, options.exit_after_rebuilds) {
                return Ok(());
            }
            continue;
        }

        write_stdout(stdout, &format!("changed {} file(s)", changes.len()))?;

        if changes.take_config_change(&layout.config_path) {
            match reload_watch(cwd, options) {
                Ok(next) => {
                    loaded = next.loaded;
                    layout = next.layout;
                    build_session = next.session;
                    watcher = next.watcher;
                    snapshot = next.snapshot;

                    write_watch_target(cwd, &layout, stdout)?;
                    write_outcome(
                        stdout,
                        stderr,
                        "configuration rebuild",
                        "rebuilt",
                        &next.outcome,
                    )?;

                    if let Some(error) = next.fallback_error {
                        write_stdout(
                            stdout,
                            &format!(
                                "native file watcher unavailable ({error}); using polling fallback"
                            ),
                        )?;
                    }

                    rebuilds += 1;
                    if reached_rebuild_limit(rebuilds, options.exit_after_rebuilds) {
                        return Ok(());
                    }
                    continue;
                }
                Err(error) => {
                    write_stderr(stderr, &error.format_stderr())?;
                    write_stdout(
                        stdout,
                        "configuration reload failed; keeping previous configuration",
                    )?;
                }
            }
        }

        if changes.is_empty() {
            continue;
        }

        let removed = if layout.in_place {
            RemovalOutcome::default()
        } else {
            remove_deleted_outputs(cwd, &loaded, &changes.deleted)
        };
        let mut outcome = build_session.rebuild(changes.changed, changes.deleted);
        outcome.diagnostics.extend(removed.diagnostics);

        if removed.removed > 0 {
            write_stdout(
                stdout,
                &format!("removed {} output file(s)", removed.removed),
            )?;
        }

        write_outcome(stdout, stderr, "rebuild", "rebuilt", &outcome)?;

        rebuilds += 1;
        if reached_rebuild_limit(rebuilds, options.exit_after_rebuilds) {
            return Ok(());
        }
    }
}

fn reload_watch(cwd: &Path, options: WatchOptions) -> Result<ReloadedWatch, CliError> {
    let loaded = load_config_from_dir(cwd).map_err(CliError::from_config_error)?;
    let layout = WatchLayout::new(cwd, &loaded);
    let (watcher, fallback_error) = WatchBackend::start(
        cwd,
        &layout.source_dir,
        &layout.additional_watch_roots(),
        options.poll_interval,
        options.force_poll,
    )
    .map_err(watch_backend_error)?;
    let (session, outcome) = WatchBuildSession::start(cwd, loaded.clone())?;
    let snapshot = snapshot_watch_tree(&layout, None, &WatchBatch::rescan_all())
        .map_err(|diagnostic| watch_error("failed to read source directory", diagnostic))?;

    Ok(ReloadedWatch {
        loaded,
        layout,
        session,
        watcher,
        snapshot,
        outcome,
        fallback_error,
    })
}

fn snapshot_watch_tree(
    layout: &WatchLayout,
    previous: Option<&SourceSnapshot>,
    batch: &WatchBatch,
) -> Result<SourceSnapshot, Diagnostic> {
    let mut files = BTreeMap::new();
    let pipeline_inputs = asset_pipeline_input_files(&layout.project_root, &layout.project_config)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let source_files = match fs::metadata(&layout.source_dir) {
        Ok(metadata) if metadata.is_dir() => discover_build_files(
            &layout.source_dir,
            &layout.extension,
            (!layout.in_place).then_some(layout.out_dir.as_path()),
        )?,
        Ok(_) => {
            return Err(io_diagnostic(
                &layout.source_dir,
                io::Error::new(
                    io::ErrorKind::NotADirectory,
                    "source root is not a directory",
                ),
            ));
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(source) => return Err(io_diagnostic(&layout.source_dir, source)),
    };

    for source_file in source_files {
        if layout.is_type_library_path(&source_file.path) {
            continue;
        }
        if layout.in_place && source_file.kind == SourceKind::Asset {
            continue;
        }

        let path = source_file.path;
        let fingerprint = fingerprint_path(&path, previous, batch)?;

        files.insert(path, fingerprint);
    }

    for root in &layout.type_library_roots {
        let paths = match fs::metadata(root) {
            Ok(metadata) if metadata.is_dir() => discover_lua_files(root)?,
            Ok(_) => {
                return Err(io_diagnostic(
                    root,
                    io::Error::new(
                        io::ErrorKind::NotADirectory,
                        "type library root is not a directory",
                    ),
                ));
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(source) => return Err(io_diagnostic(root, source)),
        };

        for path in paths {
            let fingerprint = fingerprint_path(&path, previous, batch)?;
            files.insert(path, fingerprint);
        }
    }

    if layout.config_path.is_file() {
        let fingerprint = fingerprint_path(&layout.config_path, previous, batch)?;
        files.insert(layout.config_path.clone(), fingerprint);
    }

    for path in &pipeline_inputs {
        if files.contains_key(path) {
            continue;
        }
        let fingerprint = fingerprint_path(path, previous, batch)?;
        files.insert(path.clone(), fingerprint);
    }

    Ok(SourceSnapshot {
        files,
        pipeline_inputs,
    })
}

fn fingerprint_path(
    path: &Path,
    previous: Option<&SourceSnapshot>,
    batch: &WatchBatch,
) -> Result<SourceFingerprint, Diagnostic> {
    let metadata = fs::metadata(path).map_err(|source| io_diagnostic(path, source))?;
    let len = metadata.len();
    let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
    let previous = previous.and_then(|snapshot| snapshot.files.get(path));
    let event_may_affect_path = batch.rescan_all
        || batch
            .paths
            .iter()
            .any(|event_path| path == event_path || path.starts_with(event_path));

    let content_hash = match previous {
        Some(previous)
            if previous.len == len && previous.modified == modified && !event_may_affect_path =>
        {
            previous.content_hash
        }
        _ => hash_file(path)?,
    };

    Ok(SourceFingerprint {
        len,
        modified,
        content_hash,
    })
}

fn hash_file(path: &Path) -> Result<u64, Diagnostic> {
    let mut file = File::open(path).map_err(|source| io_diagnostic(path, source))?;
    let mut buffer = [0u8; 16 * 1024];
    let mut hash = FNV_OFFSET_BASIS;

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| io_diagnostic(path, source))?;
        if read == 0 {
            return Ok(hash);
        }

        for byte in &buffer[..read] {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
}

fn remove_deleted_outputs(
    cwd: &Path,
    loaded: &LoadedConfig,
    deleted: &[PathBuf],
) -> RemovalOutcome {
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded.clone());
    let mut outcome = RemovalOutcome::default();
    let output_guard = match crate::build::OutputGuard::new(&workspace) {
        Ok(guard) => guard,
        Err(diagnostic) => {
            outcome.diagnostics.push(diagnostic);
            return outcome;
        }
    };

    for source_path in deleted {
        let source_kind = SourceKind::for_path(source_path, &workspace.config().extension);
        let output_paths = match source_kind {
            SourceKind::Tua => output_paths_for_tua_source(&workspace, source_path),
            SourceKind::Asset => vec![output_path_for_asset(&workspace, source_path)],
        };
        let output_paths = output_paths
            .into_iter()
            .flat_map(|output_path| {
                let mut paths = vec![output_path.clone()];

                if source_kind == SourceKind::Tua {
                    paths.push(source_map_path_for(&output_path));
                    paths.push(debugger_source_map_path_for(&output_path));
                }

                paths
            })
            .collect::<Vec<_>>();

        for output_path in output_paths {
            if let Err(diagnostic) = output_guard.check(&output_path) {
                outcome.diagnostics.push(diagnostic);
                continue;
            }
            match fs::remove_file(&output_path) {
                Ok(()) => outcome.removed += 1,
                Err(source) if source.kind() == io::ErrorKind::NotFound => {}
                Err(source) => outcome
                    .diagnostics
                    .push(io_diagnostic(&output_path, source)),
            }
        }
    }

    outcome
}

fn write_outcome<W: Write, E: Write>(
    stdout: &mut W,
    stderr: &mut E,
    label: &str,
    clean_verb: &str,
    outcome: &BuildOutcome,
) -> Result<(), CliError> {
    if outcome.diagnostics.is_empty() {
        write_stdout(
            stdout,
            &format!("{label}: {clean_verb} {} file(s)", outcome.emitted),
        )
    } else if has_errors(&outcome.diagnostics) {
        let diagnostics = sorted_diagnostics(outcome.diagnostics.clone());

        write_stderr(stderr, &format_diagnostics_for_cli(&diagnostics))?;
        write_stdout(
            stdout,
            &format!("{label}: {} file(s) emitted with errors", outcome.emitted),
        )
    } else {
        let diagnostics = sorted_diagnostics(outcome.diagnostics.clone());

        write_stderr(stderr, &format_diagnostics_for_cli(&diagnostics))?;
        write_stdout(
            stdout,
            &format!("{label}: {} file(s) emitted with warnings", outcome.emitted),
        )
    }
}

fn write_stdout<W: Write>(stdout: &mut W, message: &str) -> Result<(), CliError> {
    writeln!(stdout, "{message}").map_err(watch_output_error)?;
    stdout.flush().map_err(watch_output_error)
}

fn write_stderr<E: Write>(stderr: &mut E, message: &str) -> Result<(), CliError> {
    writeln!(stderr, "{message}").map_err(watch_output_error)?;
    stderr.flush().map_err(watch_output_error)
}

fn watch_error(message: &str, diagnostic: Diagnostic) -> CliError {
    CliError::from_diagnostics(message, vec![diagnostic])
}

fn watch_output_error(source: io::Error) -> CliError {
    CliError::usage(format!("failed to write watch output: {source}"))
}

fn watch_backend_error(source: notify::Error) -> CliError {
    CliError::usage(format!("failed to watch project files: {source}"))
}

fn env_duration(name: &str, default: Duration) -> Duration {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .map(Duration::from_millis)
        .unwrap_or(default)
}

fn env_usize(name: &str) -> Option<usize> {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
}

fn reached_rebuild_limit(rebuilds: usize, limit: Option<usize>) -> bool {
    limit.is_some_and(|limit| rebuilds >= limit)
}

fn remove_path(paths: &mut Vec<PathBuf>, target: &Path) -> bool {
    let original_len = paths.len();
    paths.retain(|path| path != target);
    paths.len() != original_len
}

fn display_path(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn watch_glob(extension: &str, include_assets: bool) -> String {
    if include_assets {
        "**/*".to_owned()
    } else if extension == "lua" {
        "**/*.lua".to_owned()
    } else {
        format!("**/*.{extension}")
    }
}

fn write_watch_target<W: Write>(
    cwd: &Path,
    layout: &WatchLayout,
    stdout: &mut W,
) -> Result<(), CliError> {
    write_stdout(
        stdout,
        &format!(
            "watching {}",
            display_path(
                cwd,
                &layout
                    .source_dir
                    .join(watch_glob(&layout.extension, !layout.in_place)),
            )
        ),
    )
}

#[cfg(test)]
mod tests;
