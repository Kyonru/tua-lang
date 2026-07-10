use std::borrow::Cow;
use std::collections::BTreeMap;
use std::env;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub struct ProfileSpan {
    enabled: bool,
    name: Cow<'static, str>,
    start: Instant,
}

pub struct DiagnosticsTraceSpan {
    enabled: bool,
    category: DiagnosticsTraceCategory,
    start: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DiagnosticsTraceCategory {
    DiscoverFiles,
    ReadVfsUpdate,
    Parse,
    ModuleGraph,
    TypeEnvironment,
    Checker,
    Lints,
    InlineIgnores,
    Publish,
}

#[derive(Debug)]
struct DiagnosticsTraceTotals {
    started: Instant,
    durations: BTreeMap<DiagnosticsTraceCategory, Duration>,
    counts: BTreeMap<DiagnosticsTraceCategory, usize>,
}

impl Drop for ProfileSpan {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }

        eprintln!(
            "[tua-profile] {} {:.3}ms",
            self.name,
            self.start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

impl Drop for DiagnosticsTraceSpan {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }

        record_diagnostics_trace(self.category, self.start.elapsed());
    }
}

impl DiagnosticsTraceCategory {
    const ALL: [Self; 9] = [
        Self::DiscoverFiles,
        Self::ReadVfsUpdate,
        Self::Parse,
        Self::ModuleGraph,
        Self::TypeEnvironment,
        Self::Checker,
        Self::Lints,
        Self::InlineIgnores,
        Self::Publish,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::DiscoverFiles => "discover_files",
            Self::ReadVfsUpdate => "read_vfs_update",
            Self::Parse => "parse",
            Self::ModuleGraph => "module_graph",
            Self::TypeEnvironment => "type_environment",
            Self::Checker => "checker",
            Self::Lints => "lints",
            Self::InlineIgnores => "inline_ignores",
            Self::Publish => "publish",
        }
    }
}

impl DiagnosticsTraceTotals {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            durations: BTreeMap::new(),
            counts: BTreeMap::new(),
        }
    }

    fn reset(&mut self) {
        self.started = Instant::now();
        self.durations.clear();
        self.counts.clear();
    }

    fn is_empty(&self) -> bool {
        self.durations.is_empty()
    }
}

pub fn enabled() -> bool {
    env::var_os("TUA_PROFILE").is_some_and(|value| value != "0")
}

pub fn span(name: impl Into<Cow<'static, str>>) -> ProfileSpan {
    ProfileSpan {
        enabled: enabled(),
        name: name.into(),
        start: Instant::now(),
    }
}

pub fn span_for_path(name: &'static str, path: &std::path::Path) -> ProfileSpan {
    span(format!("{name} {}", path.display()))
}

pub fn diagnostics_trace_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();

    *ENABLED.get_or_init(|| trace_target_enabled("diagnostics"))
}

pub fn diagnostics_span(category: DiagnosticsTraceCategory) -> DiagnosticsTraceSpan {
    DiagnosticsTraceSpan {
        enabled: diagnostics_trace_enabled(),
        category,
        start: Instant::now(),
    }
}

pub fn report_diagnostics_trace(context: impl AsRef<str>) {
    if !diagnostics_trace_enabled() {
        return;
    }

    let mut totals = diagnostics_trace_totals()
        .lock()
        .expect("diagnostics trace lock should not be poisoned");

    if totals.is_empty() {
        return;
    }

    let elapsed = totals.started.elapsed();
    let mut parts = Vec::new();

    parts.push(format!("total={:.3}ms", duration_ms(elapsed)));

    for category in DiagnosticsTraceCategory::ALL {
        let duration = totals.durations.get(&category).copied().unwrap_or_default();
        let count = totals.counts.get(&category).copied().unwrap_or_default();

        parts.push(format!(
            "{}={:.3}ms/{}",
            category.label(),
            duration_ms(duration),
            count
        ));
    }

    eprintln!(
        "[tua-trace diagnostics] {} {}",
        context.as_ref(),
        parts.join(" ")
    );

    totals.reset();
}

fn record_diagnostics_trace(category: DiagnosticsTraceCategory, duration: Duration) {
    let mut totals = diagnostics_trace_totals()
        .lock()
        .expect("diagnostics trace lock should not be poisoned");

    *totals.durations.entry(category).or_default() += duration;
    *totals.counts.entry(category).or_default() += 1;
}

fn diagnostics_trace_totals() -> &'static Mutex<DiagnosticsTraceTotals> {
    static TOTALS: OnceLock<Mutex<DiagnosticsTraceTotals>> = OnceLock::new();

    TOTALS.get_or_init(|| Mutex::new(DiagnosticsTraceTotals::new()))
}

fn trace_target_enabled(target: &str) -> bool {
    let Some(value) = env::var_os("TUA_TRACE") else {
        return false;
    };
    let value = value.to_string_lossy();

    if value == "0" || value.eq_ignore_ascii_case("false") {
        return false;
    }

    value
        .split([',', ';', ' ', ':'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .any(|part| part.eq_ignore_ascii_case("all") || part.eq_ignore_ascii_case(target))
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
