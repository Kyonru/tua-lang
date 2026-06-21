use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::build::{build_changed_files, build_project, output_path_for, BuildOutcome};
use crate::diagnostics::sorted_diagnostics;
use crate::error::CliError;
use crate::source::{discover_source_files, io_diagnostic};
use tua_core::{format_diagnostics_for_cli, Diagnostic, LoadedConfig};

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(250);
const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(150);
const POLL_INTERVAL_ENV: &str = "TUA_INTERNAL_WATCH_POLL_MS";
const DEBOUNCE_ENV: &str = "TUA_INTERNAL_WATCH_DEBOUNCE_MS";
const EXIT_AFTER_REBUILDS_ENV: &str = "TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS";

#[derive(Clone, Copy, Debug)]
struct WatchOptions {
    poll_interval: Duration,
    debounce: Duration,
    exit_after_rebuilds: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceFingerprint {
    len: u64,
    modified: SystemTime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceSnapshot {
    files: BTreeMap<PathBuf, SourceFingerprint>,
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

impl WatchOptions {
    fn from_env() -> Self {
        Self {
            poll_interval: env_duration(POLL_INTERVAL_ENV, DEFAULT_POLL_INTERVAL),
            debounce: env_duration(DEBOUNCE_ENV, DEFAULT_DEBOUNCE),
            exit_after_rebuilds: env_usize(EXIT_AFTER_REBUILDS_ENV),
        }
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
    loaded: LoadedConfig,
    options: WatchOptions,
    stdout: &mut W,
    stderr: &mut E,
) -> Result<(), CliError> {
    let source_dir = cwd.join(&loaded.config.source);

    write_stdout(
        stdout,
        &format!(
            "watching {}",
            display_path(
                cwd,
                &source_dir.join(format!("**/*.{}", loaded.config.extension))
            )
        ),
    )?;

    let initial = build_project(cwd, loaded.clone(), false)?;
    write_outcome(stdout, stderr, "initial build", "built", &initial)?;

    let mut snapshot = snapshot_source_tree(&source_dir, &loaded.config.extension)
        .map_err(|diagnostic| watch_error("failed to read source directory", diagnostic))?;
    let mut rebuilds = 0usize;

    loop {
        thread::sleep(options.poll_interval);

        let next = snapshot_source_tree(&source_dir, &loaded.config.extension)
            .map_err(|diagnostic| watch_error("failed to read source directory", diagnostic))?;

        if snapshot == next {
            continue;
        }

        let (stable_snapshot, changes) =
            wait_for_debounced_changes(&source_dir, &loaded.config.extension, &snapshot, options)
                .map_err(|diagnostic| watch_error("failed to read source directory", diagnostic))?;

        snapshot = stable_snapshot;

        if changes.is_empty() {
            continue;
        }

        write_stdout(stdout, &format!("changed {} file(s)", changes.len()))?;

        let removed = remove_deleted_outputs(cwd, &loaded, &changes.deleted);
        let mut outcome = build_changed_files(cwd, loaded.clone(), changes.changed);
        outcome.diagnostics.extend(removed.diagnostics);

        if removed.removed > 0 {
            write_stdout(
                stdout,
                &format!("removed {} output file(s)", removed.removed),
            )?;
        }

        write_outcome(stdout, stderr, "rebuild", "rebuilt", &outcome)?;

        rebuilds += 1;
        if options
            .exit_after_rebuilds
            .is_some_and(|limit| rebuilds >= limit)
        {
            return Ok(());
        }
    }
}

fn wait_for_debounced_changes(
    source_dir: &Path,
    extension: &str,
    previous: &SourceSnapshot,
    options: WatchOptions,
) -> Result<(SourceSnapshot, ChangeSet), Diagnostic> {
    let mut latest = snapshot_source_tree(source_dir, extension)?;
    let mut stable_since = Instant::now();

    loop {
        thread::sleep(options.poll_interval);

        let current = snapshot_source_tree(source_dir, extension)?;

        if current == latest {
            if stable_since.elapsed() >= options.debounce {
                let changes = previous.diff(&current);
                return Ok((current, changes));
            }
        } else {
            latest = current;
            stable_since = Instant::now();
        }
    }
}

fn snapshot_source_tree(source_dir: &Path, extension: &str) -> Result<SourceSnapshot, Diagnostic> {
    let mut files = BTreeMap::new();

    for path in discover_source_files(source_dir, extension)? {
        let metadata = fs::metadata(&path).map_err(|source| io_diagnostic(&path, source))?;
        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);

        files.insert(
            path,
            SourceFingerprint {
                len: metadata.len(),
                modified,
            },
        );
    }

    Ok(SourceSnapshot { files })
}

fn remove_deleted_outputs(
    cwd: &Path,
    loaded: &LoadedConfig,
    deleted: &[PathBuf],
) -> RemovalOutcome {
    let source_dir = cwd.join(&loaded.config.source);
    let out_dir = cwd.join(&loaded.config.out);
    let mut outcome = RemovalOutcome::default();

    for source_path in deleted {
        let output_path = output_path_for(&source_dir, &out_dir, source_path);

        match fs::remove_file(&output_path) {
            Ok(()) => outcome.removed += 1,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => outcome
                .diagnostics
                .push(io_diagnostic(&output_path, source)),
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
    } else {
        let diagnostics = sorted_diagnostics(outcome.diagnostics.clone());

        write_stderr(stderr, &format_diagnostics_for_cli(&diagnostics))?;
        write_stdout(
            stdout,
            &format!("{label}: {} file(s) emitted with errors", outcome.emitted),
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

fn display_path(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests;
