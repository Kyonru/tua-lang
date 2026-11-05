//! Busted orchestration; compilation and traceback mapping use the normal pipeline.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use tua_core::{DiagnosticSeverity, LoadedConfig, ProjectWorkspace};

mod discovery;
mod report;

use crate::build::{build_project_excluding, OutputGuard};
use crate::error::CliError;
use crate::source::{discover_build_files_with_exclusions, paths_same_location};
use crate::source_map::load_traceback_maps;
use crate::traceback::remap_traceback_text;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TestOptions {
    /// Optional file or directory relative to the configured test discovery root.
    pub path: Option<PathBuf>,
    pub busted: Option<PathBuf>,
    pub filter: Option<String>,
    pub tags: Option<String>,
    pub lua: Option<PathBuf>,
    pub list: bool,
    pub report: Option<PathBuf>,
}

pub(crate) fn run_tests(
    cwd: &Path,
    mut loaded: LoadedConfig,
    options: &TestOptions,
) -> Result<String, CliError> {
    let cwd = fs::canonicalize(cwd)
        .map_err(|e| CliError::usage(format!("cannot open test project: {e}")))?;
    if options.list && options.report.is_some() {
        return Err(CliError::usage(
            "--list and --report cannot be used together",
        ));
    }
    if let Some(destination) = &options.report {
        report::initialize(&cwd, destination)?;
    }
    let workspace = ProjectWorkspace::from_loaded(&cwd, loaded.clone());
    let test_root = workspace.test_source_root();
    let source = test_root
        .as_deref()
        .unwrap_or(workspace.primary_source_root());
    let production_output = workspace.primary_output_root();
    let state_dir = cwd.join(".tua");
    let mut excluded = vec![state_dir.as_path()];
    if !paths_same_location(source, production_output) {
        excluded.push(production_output);
    }
    let selector = options.path.as_deref().unwrap_or(Path::new(""));
    if selector
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(CliError::usage(
            "test paths must be relative to the source root without `..`",
        ));
    }
    let selected = tua_core::normalize_path(source.join(selector));
    let files = discover_build_files_with_exclusions(source, &loaded.config.extension, &excluded)
        .map_err(|d| CliError::from_diagnostics("cannot discover tests", vec![d]))?;
    let selected_tests = files
        .iter()
        .filter(|f| {
            (f.path == selected || f.path.starts_with(&selected))
                && !workspace.is_type_library_path(&f.path)
                && workspace.is_spec_path(&f.path)
        })
        .map(|f| f.path.as_path())
        .collect::<Vec<_>>();
    if options.list {
        return Ok(discovery::list(&workspace, &selected_tests).to_string());
    }
    let tests = selected_tests
        .iter()
        .map(|path| {
            workspace
                .mirrored_output_path_for_source(path)
                .expect("discovered test output")
                .strip_prefix(production_output)
                .expect("relative output")
                .with_extension("lua")
        })
        .collect::<BTreeSet<_>>();
    if tests.is_empty() {
        return Err(CliError::usage(format!(
            "no unit tests found under {}; name tests *_spec.{} or *_spec.lua",
            selected.display(),
            loaded.config.extension
        )));
    }

    if test_root.is_some()
        && workspace
            .primary_source_root()
            .join("__tua_tests__")
            .exists()
    {
        return Err(CliError::usage(
            "__tua_tests__ is reserved for dedicated test output; rename that source directory",
        ));
    }
    let output = TestOutput::new(&cwd)?;
    loaded.config.out = output.0.clone();
    loaded.config.emit_maps = true;
    // Busted supplies these globals only in the test process. Project/editor
    // configuration remains unchanged; authors can declare them there too.
    for name in [
        "describe",
        "context",
        "it",
        "before_each",
        "after_each",
        "setup",
        "teardown",
        "pending",
        "insulate",
        "expose",
        "spy",
        "stub",
        "mock",
        "finally",
    ] {
        if !loaded.config.diagnostics.globals.iter().any(|g| g == name) {
            loaded.config.diagnostics.globals.push(name.to_owned());
        }
    }
    let built = build_project_excluding(&cwd, loaded, false, &excluded, true)?;
    if built
        .diagnostics
        .iter()
        .any(|d| d.severity == DiagnosticSeverity::Error)
    {
        return Err(CliError::from_diagnostics(
            "test build failed",
            built.diagnostics,
        ));
    }
    let mut diagnostics = tua_core::format_diagnostics_for_cli(&built.diagnostics);
    if !diagnostics.is_empty() {
        diagnostics.push('\n');
    }
    let executable = options
        .busted
        .clone()
        .or_else(|| std::env::var_os("TUA_BUSTED").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("busted"));
    let report_output = options
        .report
        .as_ref()
        .map(|_| TestOutput::new(&cwd))
        .transpose()?;
    let reporter_path = report_output.as_ref().map(|dir| dir.0.join("reporter.lua"));
    let raw_report = report_output.as_ref().map(|dir| dir.0.join("results.json"));
    if let Some(path) = &reporter_path {
        fs::write(path, include_str!("testing/reporter.lua"))
            .map_err(|error| CliError::usage(format!("cannot prepare test reporter: {error}")))?;
    }
    let mut command = Command::new(resolve_executable(&cwd, &executable));
    command
        .current_dir(&output.0)
        .arg("--output")
        .arg(
            reporter_path
                .as_deref()
                .unwrap_or(Path::new("plainTerminal")),
        )
        .args([
            "--verbose",
            "--lpath",
            "./?.lua;./?/init.lua;./__tua_tests__/?.lua;./__tua_tests__/?/init.lua",
        ]);
    if let Some(path) = &raw_report {
        command.env("TUA_TEST_REPORT", path);
    }
    for (flag, value) in [("--filter", &options.filter), ("--tags", &options.tags)] {
        if let Some(value) = value {
            command.arg(format!("{flag}={value}"));
        }
    }
    if let Some(lua) = &options.lua {
        command.arg("--lua").arg(resolve_executable(&cwd, lua));
    }
    command.arg("--").args(&tests);
    let result = command.output().map_err(|e| CliError::usage(format!(
        "could not run Busted ({}): {e}. Install Busted for your Lua interpreter with `luarocks install busted`, or use --busted PATH / TUA_BUSTED", executable.display()
    )))?;
    let maps = load_traceback_maps(&output.0)?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    // Busted's human-readable summary uses `file.lua @ line`; its verbose
    // stack already uses the normal Lua traceback form.
    let normalized = text
        .lines()
        .map(|line| line.replace(".lua @ ", ".lua:"))
        .collect::<Vec<_>>()
        .join("\n");
    let report = format!("{diagnostics}{}", remap_traceback_text(&normalized, &maps));
    if let (Some(destination), Some(raw_report)) = (&options.report, &raw_report) {
        report::save(
            &workspace,
            &output.0,
            raw_report,
            destination,
            result.status.success(),
            &report,
        )?;
    }
    if result.status.success() {
        Ok(report)
    } else {
        Err(CliError::usage(format!(
            "unit tests failed ({})\n{report}",
            result.status
        )))
    }
}

fn resolve_executable(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_relative() && path.components().count() > 1 {
        cwd.join(path)
    } else {
        path.to_owned()
    }
}

static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);
struct TestOutput(PathBuf);
impl TestOutput {
    fn new(root: &Path) -> Result<Self, CliError> {
        let parent = root.join(".tua");
        let guard = OutputGuard::for_root(root);
        guard
            .check(&parent)
            .map_err(|d| CliError::from_diagnostics("unsafe test output", vec![d]))?;
        fs::create_dir_all(&parent)
            .map_err(|e| CliError::usage(format!("cannot create test output: {e}")))?;
        for _ in 0..100 {
            let path = parent.join(format!(
                "test-{}-{}",
                std::process::id(),
                NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(CliError::usage(format!("cannot create test output: {e}"))),
            }
        }
        Err(CliError::usage("cannot allocate test output directory"))
    }
}
impl Drop for TestOutput {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
