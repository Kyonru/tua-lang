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
use std::process::ExitCode;
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
    if arguments.manifest.is_none() && arguments.paths.is_empty() {
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
        arguments.options.phase1 = pinned.config()?;
        eprintln!("inline_eval: evaluating the phase1 configuration pinned in the thresholds file");
    }

    let (sets, duplicates) = dataset::load_sets(selected)?;
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
        write_json(path, &report)?;
        eprintln!("inline_eval: wrote {}", path.display());
    }

    let mut failed = false;
    let deterministic = report.deterministic();
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
    }

    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
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
                summary.latency_us.p95,
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
    }
}
