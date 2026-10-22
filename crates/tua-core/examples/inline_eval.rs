//! Offline evaluator for local inline completion (Phase 0 of
//! `better-autocompletion.md`).
//!
//! ```sh
//! cargo run -p tua-core --features test-fixture --example inline_eval -- \
//!   --manifest benchmarks/inline-eval/manifest.toml --set tune --out report.json
//! cargo run -p tua-core --features test-fixture --example inline_eval -- <paths> --json
//! ```
//!
//! Positional paths are evaluated as an ad-hoc set: directories as projects,
//! files on their own.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicUsize, Ordering};

use tua_core::inline_completion::eval::backends::RecallVariant;
use tua_core::inline_completion::eval::dataset::{self, Manifest, ManifestSet, SetRole};
use tua_core::inline_completion::eval::{
    self as eval, DeterministicTotals, EvalOptions, Thresholds,
};

/// Counts live heap bytes so the report can show memory for project loading.
struct CountingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        ALLOCATED.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let resized = unsafe { System.realloc(pointer, layout, new_size) };
        if !resized.is_null() {
            ALLOCATED.fetch_add(new_size, Ordering::Relaxed);
            ALLOCATED.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        resized
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn allocated_bytes() -> usize {
    ALLOCATED.load(Ordering::Relaxed)
}

const USAGE: &str =
    "usage: inline_eval [paths...] [--manifest FILE] [--set NAME]... [--json] [--out FILE]
  [--records] [--max-positions N] [--no-menu] [--no-warmup] [--thresholds FILE]
  [--menu-replay FILE]
  [--phase2-retrieval] [--phase3-ngram]
  [--require-correct-backend NAME]...
  [--check-phase-gate FILE]
  [--check-expected FILE] [--write-expected FILE] [--min-prefix N] [--min-support N]
  [--min-full-prefix-support N] [--dominance R]
  [--recall-variant unique|common-prefix] [--contexts LABEL,LABEL,...]

--min-prefix, --min-support, --min-full-prefix-support, --dominance,
--recall-variant, and --contexts configure the candidate `phase1` backend; the
recall options also apply to standalone recall backends. A thresholds file with
a [phase1] table pins that configuration for gate runs instead.";

struct Arguments {
    paths: Vec<PathBuf>,
    manifest: Option<PathBuf>,
    sets: Vec<String>,
    json: bool,
    out: Option<PathBuf>,
    thresholds: Option<PathBuf>,
    check_expected: Option<PathBuf>,
    write_expected: Option<PathBuf>,
    menu_replay: Option<PathBuf>,
    phase_gate: Option<PathBuf>,
    required_correct_backends: Vec<String>,
    options: EvalOptions,
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut arguments = Arguments {
        paths: Vec::new(),
        manifest: None,
        sets: Vec::new(),
        json: false,
        out: None,
        thresholds: None,
        check_expected: None,
        write_expected: None,
        menu_replay: None,
        phase_gate: None,
        required_correct_backends: Vec::new(),
        options: EvalOptions {
            allocated_bytes: Some(allocated_bytes),
            ..EvalOptions::default()
        },
    };
    let mut raw = std::env::args().skip(1);
    while let Some(argument) = raw.next() {
        let mut value = |name: &str| raw.next().ok_or_else(|| format!("{name} needs a value"));
        match argument.as_str() {
            "--manifest" => arguments.manifest = Some(value("--manifest")?.into()),
            "--set" => arguments.sets.push(value("--set")?),
            "--json" => arguments.json = true,
            "--out" => arguments.out = Some(value("--out")?.into()),
            "--records" => arguments.options.keep_records = true,
            "--thresholds" => arguments.thresholds = Some(value("--thresholds")?.into()),
            "--check-expected" => {
                arguments.check_expected = Some(value("--check-expected")?.into())
            }
            "--write-expected" => {
                arguments.write_expected = Some(value("--write-expected")?.into())
            }
            "--menu-replay" => arguments.menu_replay = Some(value("--menu-replay")?.into()),
            "--phase2-retrieval" => arguments.options.phase2_retrieval = true,
            "--phase3-ngram" => {
                arguments.options.phase2_retrieval = true;
                arguments.options.phase3_ngram = true;
            }
            "--require-correct-backend" => arguments
                .required_correct_backends
                .push(value("--require-correct-backend")?),
            "--check-phase-gate" => {
                arguments.phase_gate = Some(value("--check-phase-gate")?.into())
            }
            "--no-menu" => arguments.options.simulate_menu = false,
            "--no-warmup" => arguments.options.measure_warmup = false,
            "--max-positions" => {
                arguments.options.max_positions_per_file = value("--max-positions")?
                    .parse()
                    .map_err(|_| "--max-positions needs a number".to_owned())?;
            }
            "--min-prefix" => {
                arguments.options.recall.min_prefix_chars = value("--min-prefix")?
                    .parse()
                    .map_err(|_| "--min-prefix needs a number".to_owned())?;
            }
            "--dominance" => {
                arguments.options.recall.dominance_ratio = value("--dominance")?
                    .parse()
                    .map_err(|_| "--dominance needs a number".to_owned())?;
            }
            "--min-support" => {
                arguments.options.recall.min_support = value("--min-support")?
                    .parse()
                    .map_err(|_| "--min-support needs a number".to_owned())?;
            }
            "--min-full-prefix-support" => {
                arguments.options.recall.min_full_prefix_support =
                    value("--min-full-prefix-support")?
                        .parse()
                        .map_err(|_| "--min-full-prefix-support needs a number".to_owned())?;
            }
            "--recall-variant" => {
                arguments.options.phase1.recall_variant = match value("--recall-variant")?.as_str()
                {
                    "unique" => RecallVariant::Unique,
                    "common-prefix" => RecallVariant::CommonPrefix,
                    other => return Err(format!("unknown recall variant `{other}`")),
                };
            }
            "--contexts" => {
                let labels = value("--contexts")?;
                arguments.options.phase1.contexts = Some(eval::parse_contexts(labels.split(','))?);
            }
            "-h" | "--help" => return Err(USAGE.to_owned()),
            flag if flag.starts_with("--") => return Err(format!("unknown flag {flag}\n{USAGE}")),
            path => arguments.paths.push(path.into()),
        }
    }
    if arguments.manifest.is_none() && arguments.paths.is_empty() && arguments.phase_gate.is_none()
    {
        return Err(USAGE.to_owned());
    }
    arguments.options.phase1.recall = arguments.options.recall;
    Ok(arguments)
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("inline_eval: {message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let mut arguments = parse_arguments()?;
    if arguments.manifest.is_none() && arguments.paths.is_empty() {
        return check_phase_gate_file(
            arguments
                .phase_gate
                .as_deref()
                .expect("argument parsing requires a phase gate in standalone mode"),
        );
    }
    let mut selected = Vec::<(String, ManifestSet)>::new();

    if let Some(path) = &arguments.manifest {
        let manifest = Manifest::load(path)?;
        let names = if arguments.sets.is_empty() {
            manifest.sets.keys().cloned().collect()
        } else {
            arguments.sets.clone()
        };
        for name in names {
            let set = manifest
                .sets
                .get(&name)
                .ok_or_else(|| format!("{} has no set named `{name}`", path.display()))?;
            selected.push((name, set.clone()));
        }
    }
    if !arguments.paths.is_empty() {
        let (projects, files) = arguments
            .paths
            .iter()
            .cloned()
            .partition(|path| path.is_dir());
        let mut adhoc = ManifestSet {
            role: SetRole::Adhoc,
            description: "paths from the command line".to_owned(),
            projects,
            files,
            exclude: Vec::new(),
        };
        let directory = std::env::current_dir().map_err(|error| error.to_string())?;
        adhoc.resolve(&directory);
        selected.push(("adhoc".to_owned(), adhoc));
    }

    // Held-out and external sets are quality gates: their thresholds must be
    // committed before anyone looks at their results.
    let gated = selected.iter().any(|(_, set)| set.role.is_gate());
    ensure_phase3_sets_are_tuning_only(
        arguments.options.phase3_ngram,
        selected.iter().map(|(_, set)| set.role),
    )?;
    ensure_phase2_sets_are_tuning_only(
        arguments.options.phase2_retrieval,
        selected.iter().map(|(_, set)| set.role),
    )?;
    let thresholds = match (&arguments.thresholds, gated) {
        (Some(path), _) => Some(load_thresholds(path)?),
        (None, true) => return Err(
            "held-out and external sets need --thresholds with the committed acceptance criteria"
                .to_owned(),
        ),
        (None, false) => None,
    };

    if thresholds
        .as_ref()
        .is_some_and(|thresholds| thresholds.backend == "phase1" && thresholds.phase1.is_none())
    {
        return Err(
            "phase1 gate thresholds must include a [phase1] table that pins the evaluated configuration"
                .to_owned(),
        );
    }

    if let Some(pinned) = thresholds
        .as_ref()
        .and_then(|thresholds| thresholds.phase1.as_ref())
    {
        let pinned = pinned.config()?;
        if pinned != eval::backends::Phase1Config::production() {
            return Err(
                "the thresholds [phase1] table does not match the shipping Phase 1 policy"
                    .to_owned(),
            );
        }
        arguments.options.phase1 = pinned;
        eprintln!(
            "inline_eval: evaluating the shipping phase1 policy pinned in the thresholds file"
        );
    }
    if arguments.options.phase2_retrieval {
        let pinned = thresholds
            .as_ref()
            .and_then(|thresholds| thresholds.phase2_tuning.as_ref())
            .ok_or_else(|| {
                "--phase2-retrieval needs --thresholds with a [phase2_tuning] table".to_owned()
            })?
            .config()?;
        if pinned != eval::backends::Phase2TuningConfig::pinned() {
            return Err(
                "the thresholds [phase2_tuning] table does not match the pinned tuning policy"
                    .to_owned(),
            );
        }
        arguments.options.phase2_tuning = pinned;
        eprintln!("inline_eval: evaluating the pinned Phase 2 tuning policy");
    }
    if arguments.options.phase3_ngram {
        let pinned = thresholds
            .as_ref()
            .and_then(|thresholds| thresholds.phase3.as_ref())
            .ok_or_else(|| "--phase3-ngram needs --thresholds with a [phase3] table".to_owned())?
            .config()?;
        if pinned != eval::backends::Phase3TuningConfig::pinned() {
            return Err(
                "the thresholds [phase3] table does not match the pinned tuning policy".to_owned(),
            );
        }
        arguments.options.phase3_tuning = pinned;
        eprintln!("inline_eval: evaluating the pinned Phase 3 tuning policy");
    }

    let (sets, duplicates) = dataset::load_sets(selected)?;
    if let Some(path) = &arguments.menu_replay {
        arguments.options.menu_replay = Some(eval::replay::MenuReplay::load(path, &sets)?);
    }
    let report = eval::evaluate(&sets, duplicates, &arguments.options);

    if arguments.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
        );
    } else {
        print_summary(&report);
    }
    if let Some(path) = &arguments.out {
        let mut stamped = serde_json::to_value(&report).map_err(|error| error.to_string())?;
        stamped["repository"] = repository_state();
        stamped["checks"] = serde_json::json!({
            "validationParseDistribution": eval::validation_parse_check(&report),
        });
        write_json(path, &stamped)?;
        eprintln!("inline_eval: wrote {}", path.display());
    }

    let mut failed = false;
    let deterministic = report.deterministic();
    require_correct_backends(&deterministic, &arguments.required_correct_backends)?;
    if let Some(path) = &arguments.write_expected {
        write_json(path, &deterministic)?;
        eprintln!("inline_eval: wrote expectations to {}", path.display());
    }
    if let Some(path) = &arguments.check_expected {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let expected: BTreeMap<String, BTreeMap<String, DeterministicTotals>> =
            serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        if expected != deterministic {
            failed = true;
            eprintln!(
                "inline_eval: results differ from {}; if the change is intended, rerun with --write-expected",
                path.display()
            );
            eprintln!(
                "{}",
                serde_json::to_string_pretty(&deterministic).map_err(|error| error.to_string())?
            );
        } else {
            eprintln!("inline_eval: results match {}", path.display());
        }
    }
    if gated {
        let thresholds = thresholds
            .as_ref()
            .expect("gate sets require thresholds before evaluation");
        for gate in eval::check_gates(&report, thresholds) {
            let verdict = if gate.passed { "pass" } else { "FAIL" };
            failed |= !gate.passed;
            let scope = [gate.set.as_str(), gate.language.as_str()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            // Limit checks are named like `min_...`; contract failures are
            // sentences with nothing to compare.
            if gate.check.contains(' ') {
                eprintln!("gate {verdict}: {scope}: {}", gate.check);
            } else {
                eprintln!(
                    "gate {verdict}: {scope} {} = {:.3} (limit {:.3})",
                    gate.check, gate.actual, gate.limit
                );
            }
        }
        if arguments.options.phase3_ngram {
            for gate in eval::check_phase3_gates(&report, thresholds) {
                let verdict = if gate.passed { "pass" } else { "FAIL" };
                failed |= !gate.passed;
                let scope = [gate.set.as_str(), gate.language.as_str()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                if gate.check.contains(' ') {
                    eprintln!("phase3 gate {verdict}: {scope}: {}", gate.check);
                } else {
                    eprintln!(
                        "phase3 gate {verdict}: {scope} {} = {:.3} (limit {:.3})",
                        gate.check, gate.actual, gate.limit
                    );
                }
            }
        }
    }

    if let Some(path) = &arguments.phase_gate {
        failed |= check_phase_gate_file(path)? != ExitCode::SUCCESS;
    }

    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn ensure_phase2_sets_are_tuning_only(
    enabled: bool,
    roles: impl IntoIterator<Item = SetRole>,
) -> Result<(), String> {
    if enabled
        && roles
            .into_iter()
            .any(|role| !matches!(role, SetRole::Tune | SetRole::Smoke))
    {
        Err("--phase2-retrieval accepts only manifest tune or smoke sets".to_owned())
    } else {
        Ok(())
    }
}

fn ensure_phase3_sets_are_tuning_only(
    enabled: bool,
    roles: impl IntoIterator<Item = SetRole>,
) -> Result<(), String> {
    if enabled
        && roles
            .into_iter()
            .any(|role| !matches!(role, SetRole::Tune | SetRole::Smoke))
    {
        Err("--phase3-ngram accepts only manifest tune or smoke sets".to_owned())
    } else {
        Ok(())
    }
}

fn require_correct_backends(
    results: &BTreeMap<String, BTreeMap<String, DeterministicTotals>>,
    required: &[String],
) -> Result<(), String> {
    if !required.is_empty() && results.is_empty() {
        return Err("cannot require a correct backend without an evaluated set".to_owned());
    }
    for name in required {
        for (set, backends) in results {
            let totals = backends
                .get(name)
                .ok_or_else(|| format!("set `{set}` has no backend named `{name}`"))?;
            if totals.overall.suggested == 0 {
                return Err(format!(
                    "backend `{name}` produced no suggestions for set `{set}`"
                ));
            }
            if totals.overall.correct_prefix != totals.overall.suggested {
                return Err(format!(
                    "backend `{name}` produced only {}/{} correct-prefix suggestions for set `{set}`",
                    totals.overall.correct_prefix, totals.overall.suggested
                ));
            }
            if totals.overall.exact != totals.overall.suggested {
                return Err(format!(
                    "backend `{name}` produced only {}/{} exact suggestions for set `{set}`",
                    totals.overall.exact, totals.overall.suggested
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase2_retrieval_refuses_gate_roles_even_with_thresholds() {
        assert!(ensure_phase2_sets_are_tuning_only(true, [SetRole::Tune]).is_ok());
        assert!(ensure_phase2_sets_are_tuning_only(false, [SetRole::Holdout]).is_ok());
        assert!(ensure_phase2_sets_are_tuning_only(true, [SetRole::Smoke]).is_ok());
        assert!(ensure_phase2_sets_are_tuning_only(true, [SetRole::Adhoc]).is_err());
        assert!(ensure_phase2_sets_are_tuning_only(true, [SetRole::Holdout]).is_err());
        assert!(ensure_phase2_sets_are_tuning_only(true, [SetRole::External]).is_err());
    }

    #[test]
    fn phase3_ngram_refuses_gate_roles_even_with_thresholds() {
        assert!(ensure_phase3_sets_are_tuning_only(true, [SetRole::Tune]).is_ok());
        assert!(ensure_phase3_sets_are_tuning_only(true, [SetRole::Smoke]).is_ok());
        assert!(ensure_phase3_sets_are_tuning_only(false, [SetRole::Holdout]).is_ok());
        assert!(ensure_phase3_sets_are_tuning_only(true, [SetRole::Adhoc]).is_err());
        assert!(ensure_phase3_sets_are_tuning_only(true, [SetRole::Holdout]).is_err());
        assert!(ensure_phase3_sets_are_tuning_only(true, [SetRole::External]).is_err());
    }

    #[test]
    fn required_correct_backend_fails_closed() {
        let required = ["phase3-ngram".to_owned()];
        assert!(require_correct_backends(&BTreeMap::new(), &required)
            .unwrap_err()
            .contains("without an evaluated set"));
        let mut backends = BTreeMap::new();
        backends.insert("phase3-ngram".to_owned(), DeterministicTotals::default());
        let mut results = BTreeMap::new();
        results.insert("phase3-smoke".to_owned(), backends);

        assert!(require_correct_backends(&results, &required)
            .unwrap_err()
            .contains("produced no suggestions"));

        let totals = results
            .get_mut("phase3-smoke")
            .unwrap()
            .get_mut("phase3-ngram")
            .unwrap();
        totals.overall.suggested = 1;
        assert!(require_correct_backends(&results, &required)
            .unwrap_err()
            .contains("produced only 0/1 correct-prefix suggestions"));

        results
            .get_mut("phase3-smoke")
            .unwrap()
            .get_mut("phase3-ngram")
            .unwrap()
            .overall
            .correct_prefix = 1;
        assert!(require_correct_backends(&results, &required)
            .unwrap_err()
            .contains("produced only 0/1 exact suggestions"));
        results
            .get_mut("phase3-smoke")
            .unwrap()
            .get_mut("phase3-ngram")
            .unwrap()
            .overall
            .exact = 1;
        assert!(require_correct_backends(&results, &required).is_ok());
        assert!(require_correct_backends(&results, &["missing".to_owned()])
            .unwrap_err()
            .contains("has no backend"));
    }
}

fn check_phase_gate_file(path: &Path) -> Result<ExitCode, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let artifact: eval::PhaseGateArtifact =
        toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let root = PathBuf::from(git(None, &["rev-parse", "--show-toplevel"])?.trim());
    let failures = eval::check_phase_gate(&artifact, &|item| verify_evidence(&root, item));
    if failures.is_empty() {
        eprintln!("inline_eval: aggregate Phase 1 gate passes");
        Ok(ExitCode::SUCCESS)
    } else {
        for failure in failures {
            eprintln!("phase gate FAIL: {failure}");
        }
        Ok(ExitCode::from(1))
    }
}

/// Paths whose changes invalidate recorded performance and replay evidence:
/// everything that ships, plus the harnesses and fixtures that measured it.
/// Documentation and the gate file itself are excluded.
const EVIDENCE_PATHSPEC: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "crates",
    "editors/vscode/package.json",
    "editors/vscode/src",
    "editors/vscode/scripts",
    "scripts/generate-lsp-benchmarks.js",
    "benchmarks",
    ":(exclude)benchmarks/inline-eval/phase1-gate.toml",
    ":(exclude,glob)**/*.md",
];

/// Accepts a passed prerequisite only when its artifact records a clean tree
/// at the recorded revision and nothing it measured has changed since.
fn verify_evidence(root: &Path, item: &eval::PhaseGatePrerequisite) -> Result<(), String> {
    let (Some(artifact), Some(revision)) = (&item.artifact, &item.revision) else {
        return Err("no artifact or revision recorded".to_owned());
    };
    let path = root.join(artifact);
    let text =
        fs::read_to_string(&path).map_err(|error| format!("cannot read {artifact}: {error}"))?;
    let report: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| format!("{artifact}: {error}"))?;
    let repository = &report["repository"];
    let Some(head) = repository["head"].as_str() else {
        return Err(format!("{artifact} records no repository revision"));
    };
    if revision.len() < 7 || !head.starts_with(revision.as_str()) {
        return Err(format!(
            "{artifact} was produced from {head}, not {revision}"
        ));
    }
    if repository["dirty"].as_bool() != Some(false) {
        return Err(format!("{artifact} was not produced from a clean tree"));
    }
    let Some(check) = &item.check else {
        return Err("no check recorded".to_owned());
    };
    let outcome = &report["checks"][check.as_str()];
    if outcome["passed"].as_bool() != Some(true) {
        return Err(format!(
            "{artifact} check `{check}` did not pass: {}",
            outcome["failures"]
        ));
    }
    let mut diff = vec!["diff", "--quiet", revision.as_str(), "--"];
    diff.extend(EVIDENCE_PATHSPEC);
    if Command::new("git")
        .current_dir(root)
        .args(&diff)
        .status()
        .map_err(|error| format!("cannot run git diff: {error}"))?
        .code()
        != Some(0)
    {
        return Err(format!(
            "shipped code or its harness changed since {revision}"
        ));
    }
    let mut untracked = vec!["ls-files", "--others", "--exclude-standard", "--"];
    untracked.extend(EVIDENCE_PATHSPEC);
    if !git(Some(root), &untracked)?.trim().is_empty() {
        return Err(format!(
            "untracked files would change what {revision} measured"
        ));
    }
    Ok(())
}

fn git(directory: Option<&Path>, arguments: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    let output = command
        .args(arguments)
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

/// The commit and dirty state a report was produced from, so the Phase 1 gate
/// can reject evidence that no longer describes the code under review.
fn repository_state() -> serde_json::Value {
    let head = git(None, &["rev-parse", "HEAD"]);
    let status = git(None, &["status", "--porcelain=v1", "--untracked-files=all"]);
    match (head, status) {
        (Ok(head), Ok(status)) => serde_json::json!({
            "head": head.trim(),
            "dirty": !status.is_empty(),
        }),
        _ => serde_json::Value::Null,
    }
}

fn load_thresholds(path: &Path) -> Result<Thresholds, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, text + "\n")
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn print_summary(report: &eval::EvalReport) {
    if !report.duplicates.dropped.is_empty() {
        println!(
            "dropped {} duplicate file(s)",
            report.duplicates.dropped.len()
        );
    }
    if let Some(replay) = &report.menu_replay {
        println!(
            "menu replay: {}/{} accepted, {} matched, {} unmatched, {} superseded (schema {}, path {}, position {}, ineligible {} rejected)",
            replay.accepted,
            replay.records,
            replay.matched,
            replay.unmatched,
            replay.superseded,
            replay.rejected_schema,
            replay.rejected_path,
            replay.rejected_position,
            replay.rejected_ineligible,
        );
    }
    for set in &report.sets {
        let files = set
            .files_by_language
            .iter()
            .map(|(language, count)| format!("{count} {language}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "\n== {} ({} project(s), {files} file(s); {} positions, {} line ends, {} ineligible)",
            set.name, set.projects, set.positions, set.abstain_positions, set.ineligible_positions
        );
        println!(
            "{:<20} {:>8} {:>9} {:>7} {:>7} {:>7} {:>9} {:>9} {:>9} {:>9} {:>8}",
            "backend",
            "eligible",
            "sugg/elig",
            "exact",
            "c-prefix",
            "1-tok",
            "saved/el",
            "wrong/el",
            "wrong:sav",
            "eol-sugg",
            "p95 us"
        );
        for (name, backend) in &set.backends {
            let summary = &backend.overall;
            println!(
                "{:<20} {:>8} {:>9.3} {:>7.3} {:>7.3} {:>7.3} {:>9.3} {:>9.3} {:>9.2} {:>9.3} {:>8.1}",
                name,
                summary.eligible,
                summary.suggestions_per_eligible,
                summary.exact_precision,
                summary.correct_prefix_precision,
                summary.first_token_precision,
                summary.chars_saved_per_eligible,
                summary.incorrect_chars_per_eligible,
                summary.wrong_to_saved_ratio,
                backend.abstain.suggestions_per_eligible,
                summary.backend_latency_us.p95,
            );
        }
        if let Some(slate) = &set.candidate_slate {
            println!(
                "candidate slate: {}/{} requests have candidates ({} independent positions); {} disagreements ({} independent: {} mixed, {} all-correct, {} all-wrong)",
                slate.requests_with_candidates,
                slate.eligible_requests,
                slate.independent_candidate_positions,
                slate.disagreement_requests,
                slate.independent_disagreement_positions,
                slate.disagreement_mixed_quality,
                slate.disagreement_all_correct,
                slate.disagreement_all_wrong,
            );
            println!(
                "candidate slate utility: production {:.3} saved/eligible, {:.3} precision, {:.2} wrong/saved; first {:.3}, {:.3}, {:.2}; oracle {:.3}, {:.3}, {:.2}",
                slate.production_baseline.chars_saved_per_eligible,
                slate.production_baseline.correct_prefix_precision,
                slate.production_baseline.wrong_to_saved_ratio,
                slate.slate_first.chars_saved_per_eligible,
                slate.slate_first.correct_prefix_precision,
                slate.slate_first.wrong_to_saved_ratio,
                slate.oracle_best.chars_saved_per_eligible,
                slate.oracle_best.correct_prefix_precision,
                slate.oracle_best.wrong_to_saved_ratio,
            );
            println!(
                "candidate identities: {} rankable candidates, {} distinct structural keys, {} seen in multiple projects, {} failures",
                slate.rankable_candidates,
                slate.distinct_structural_identities,
                slate.identities_seen_in_multiple_projects,
                slate.structural_identity_failures,
            );
        }
        for (language, warmup) in &set.warmup_by_language {
            let rates = warmup
                .hit_rate_by_delay_ms
                .iter()
                .map(|(delay, rate)| format!("{delay}ms {:.0}%", rate * 100.0))
                .collect::<Vec<_>>()
                .join(", ");
            println!(
                "warm-up {language}: {} samples, p50 {:.1} ms, p95 {:.1} ms; cached by {rates}; parsed facts {:.0}%",
                warmup.samples,
                warmup.warm_ms.p50,
                warmup.warm_ms.p95,
                warmup.parsed_facts_rate * 100.0
            );
        }
        for (language, parse) in &set.parse_by_language {
            println!(
                "parse {language}: {} files ({} failed), p50 {:.2} ms, p95 {:.2} ms, {:.0} B/ms median, {:.0} B/ms p5, cap {} B",
                parse.files,
                parse.failed,
                parse.latency_ms.p50,
                parse.latency_ms.p95,
                parse.throughput_bytes_per_ms_p50,
                parse.throughput_bytes_per_ms_p5,
                parse.recommended_max_bytes
            );
        }
        let validation = &set.inline_validation;
        println!(
            "phase1 production ({}): {} requests ({} skipped as oversized), {:.3} parses/request; {} candidate attempts: {} zero-, {} one-, {} two-parse ({:.3} parses/attempt); edited parse sufficient {:.0}%, cache peek {}/{} hits, {} candidates returned ({} after zero, {} after one, {} after two parses)",
            validation.cache_state,
            validation.requests,
            validation.skipped_oversized,
            validation.parses_per_request,
            validation.candidate_attempts,
            validation.zero_parse_candidates,
            validation.one_parse_candidates,
            validation.two_parse_candidates,
            validation.parses_per_candidate_attempt,
            validation.edited_parse_sufficiency_rate * 100.0,
            validation.cache_peek_hits,
            validation.cache_peek_hits + validation.cache_peek_misses,
            validation.candidates_returned,
            validation.returned_after_zero_parses,
            validation.returned_after_one_parse,
            validation.returned_after_two_parses,
        );
        let largest = set
            .memory
            .iter()
            .max_by_key(|(_, memory)| memory.files_delta_bytes + memory.snapshot_delta_bytes);
        if let Some((name, memory)) = largest {
            println!(
                "largest project load: {name}, +{:.1} MiB requested heap for a full snapshot (counted by the allocator; compare peak RSS)",
                (memory.files_delta_bytes + memory.snapshot_delta_bytes) as f64 / 1_048_576.0
            );
        }
        for (project, model) in &set.ngram_indexes {
            println!(
                "phase3 n-gram {project}: {}/{} files, {} contexts, {:.1} KiB max-query estimate, {:.1} KiB evaluator holdout estimate, {:.2} ms build",
                model.files_indexed,
                model.files_considered,
                model.contexts,
                model.approximate_bytes as f64 / 1024.0,
                model.evaluation_holdout_bytes as f64 / 1024.0,
                model.build_ms,
            );
        }
    }
}
