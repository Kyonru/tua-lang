"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");

const {
  formatCacheStats,
  profileLspProject,
  resolveCommand,
  statsForValues,
  timingsByLabel,
} = require("../editors/vscode/scripts/lsp-profile-common");

const repoRoot = path.resolve(__dirname, "..");
const examplesRoot = path.join(repoRoot, "examples");
const defaultCommand = path.join(repoRoot, "target", "release", executableName("tua"));
const command = resolveCommand(process.argv[2] || process.env.TUA_SPEED_TUA || defaultCommand);
const warmup = numberEnv("TUA_SPEED_WARMUP", 1);
const repeat = numberEnv("TUA_SPEED_REPEAT", 5);
const thresholdPercent = numberEnv("TUA_SPEED_THRESHOLD_PERCENT", 25);
const baselinePath = process.env.TUA_SPEED_BASELINE
  ? path.resolve(process.env.TUA_SPEED_BASELINE)
  : null;
const outputPath = path.resolve(
  process.env.TUA_SPEED_OUT || path.join(repoRoot, "target", "tua-speed", "examples-profile.json"),
);

(async () => {
  const projects = discoverExampleProjects();
  const report = {
    generatedAt: new Date().toISOString(),
    command,
    warmup,
    repeat,
    thresholdPercent,
    projects: [],
  };

  console.log(`# Tua examples speed suite`);
  console.log(`command: ${command}`);
  console.log(`projects: ${projects.map((project) => project.name).join(", ")}`);
  console.log(`warmup: ${warmup}, repeat: ${repeat}`);

  for (const project of projects) {
    console.log(`\n## ${project.name}`);

    const projectReport = {
      name: project.name,
      root: project.relativeRoot,
      sourceRoot: path.relative(repoRoot, project.sourceRoot).replaceAll(path.sep, "/"),
      metrics: [],
      cacheStats: null,
    };

    projectReport.metrics.push(...profileCliProject(project));
    const copied = copyProjectToTemp(project);
    let lsp;

    try {
      lsp = await profileLspProject({
        command,
        root: copied.root,
        sourceRoot: copied.sourceRoot,
        warmup,
        repeat,
        perFile: true,
        includeDiagnostics: true,
      });
    } finally {
      copied.cleanup();
    }

    projectReport.cacheStats = lsp.stats;
    projectReport.metrics.push(...metricsFromTimings(project.name, "lsp", lsp.timings));
    report.projects.push(projectReport);

    printProjectReport(projectReport);
    console.log(formatCacheStats(lsp.stats));

    if (process.env.TUA_PROFILE && lsp.stderr.trim()) {
      console.error(`\n# ${project.name} LSP trace`);
      console.error(lsp.stderr.trim());
    }
  }

  const warnings = compareBaseline(report, baselinePath, thresholdPercent);

  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`\nwrote ${path.relative(repoRoot, outputPath)}`);

  if (warnings.length) {
    console.warn(`\n# Soft regression warnings`);
    for (const warning of warnings) {
      console.warn(warning);
    }

    if (truthy(process.env.TUA_SPEED_FAIL_ON_REGRESSION)) {
      process.exitCode = 1;
    }
  }
})().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});

function profileCliProject(project) {
  const metrics = [];
  const commands = [
    {
      label: "check",
      args: ["check"],
      trace: true,
      tempCopy: true,
      acceptedDiagnostics:
        project.name === "audio-system" ? [{ severity: "warning", code: "TL2012" }] : [],
    },
    {
      label: "lint",
      args: ["lint"],
      trace: true,
      tempCopy: true,
      acceptDiagnosticExit: true,
    },
    { label: "fmt --check", args: ["fmt", "--check"], trace: false, tempCopy: true },
    { label: "build", args: ["build"], trace: false, tempCopy: true },
  ];

  for (const item of commands) {
    const values = [];
    const traces = [];

    for (let round = 0; round < warmup; round += 1) {
      runCliCommand(project, item);
    }

    for (let round = 0; round < repeat; round += 1) {
      const result = runCliCommand(project, item);

      values.push(result.elapsedMs);
      if (result.trace) {
        traces.push(result.trace);
      }
    }

    const metric = {
      project: project.name,
      surface: "cli",
      label: item.label,
      stats: statsForValues(values),
    };
    const trace = aggregateDiagnosticTraces(traces);

    if (trace) {
      metric.trace = trace;
    }

    metrics.push(metric);
  }

  return metrics;
}

function runCliCommand(project, item) {
  const copied = item.tempCopy ? copyProjectToTemp(project) : null;
  const cwd = copied ? copied.root : project.root;
  const env = {
    ...process.env,
  };

  if (item.trace) {
    env.TUA_TRACE = "diagnostics";
  }

  const started = process.hrtime.bigint();
  const result = childProcess.spawnSync(command, item.args, {
    cwd,
    env,
    encoding: "utf8",
    windowsHide: true,
  });
  const elapsedMs = Number(process.hrtime.bigint() - started) / 1_000_000;

  if (copied) {
    copied.cleanup();
  }

  if (result.error) {
    throw result.error;
  }

  const stderr = result.stderr || "";
  const reportedDiagnostics = parseDiagnosticHeaders(stderr);
  const acceptedDiagnostics = item.acceptedDiagnostics || [];
  const acceptedDiagnosticExit =
    acceptedDiagnostics.length > 0 &&
    reportedDiagnostics.length > 0 &&
    reportedDiagnostics.every((reported) =>
      acceptedDiagnostics.some(
        (accepted) =>
          accepted.severity === reported.severity && accepted.code === reported.code,
      ),
    );
  const expectedDiagnosticExit =
    result.status === 1 &&
    parseDiagnosticTrace(stderr) !== null &&
    (item.acceptDiagnosticExit || acceptedDiagnosticExit);

  if (result.status !== 0 && !expectedDiagnosticExit) {
    throw new Error(
      `${project.name}: tua ${item.args.join(" ")} failed with ${result.status}\n` +
        `${result.stdout || ""}${result.stderr || ""}`,
    );
  }

  return {
    elapsedMs,
    trace: item.trace ? parseDiagnosticTrace(stderr) : null,
  };
}

function parseDiagnosticHeaders(stderr) {
  return [...stderr.matchAll(/\b(error|warning|info|hint) (TL\d{4}):/g)].map((match) => ({
    severity: match[1],
    code: match[2],
  }));
}

function metricsFromTimings(projectName, surface, timings) {
  const metrics = [];

  for (const [label, values] of timingsByLabel(timings)) {
    metrics.push({
      project: projectName,
      surface,
      label,
      stats: statsForValues(values),
    });
  }

  return metrics;
}

function printProjectReport(projectReport) {
  for (const metric of projectReport.metrics) {
    const stats = metric.stats;
    const label = `${metric.surface}:${metric.label}`;

    console.log(
      `${label.padEnd(44)} avg ${stats.avgMs.toFixed(2)}ms  ` +
        `p50 ${stats.p50Ms.toFixed(2)}ms  p95 ${stats.p95Ms.toFixed(2)}ms  ` +
        `p99 ${stats.p99Ms.toFixed(2)}ms  min ${stats.minMs.toFixed(2)}ms  ` +
        `max ${stats.maxMs.toFixed(2)}ms`,
    );
  }
}

function discoverExampleProjects() {
  return fs
    .readdirSync(examplesRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => {
      const root = path.join(examplesRoot, entry.name);
      const configPath = path.join(root, "tua.toml");

      if (!fs.existsSync(configPath)) {
        return null;
      }

      const config = parseTuaToml(configPath);

      return {
        name: entry.name,
        root,
        relativeRoot: path.relative(repoRoot, root).replaceAll(path.sep, "/"),
        sourceRoot: path.join(root, config.source || "src"),
      };
    })
    .filter(Boolean)
    .sort((left, right) => left.name.localeCompare(right.name));
}

function parseTuaToml(configPath) {
  const config = {};

  for (const line of fs.readFileSync(configPath, "utf8").split(/\r?\n/)) {
    const match = /^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*"([^"]*)"/.exec(line.trim());

    if (match) {
      config[match[1]] = match[2];
    }
  }

  return config;
}

function copyProjectToTemp(project) {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), `tua-speed-${project.name}-`));
  const root = path.join(tempRoot, project.name);
  const files = projectWorktreeFiles(project.relativeRoot);

  fs.mkdirSync(root, { recursive: true });

  for (const relativeFile of files) {
    const source = path.join(repoRoot, relativeFile);
    const projectRelative = path.relative(project.root, source);
    const destination = path.join(root, projectRelative);

    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.copyFileSync(source, destination);
  }

  return {
    root,
    sourceRoot: path.join(root, path.relative(project.root, project.sourceRoot)),
    cleanup() {
      fs.rmSync(tempRoot, { recursive: true, force: true });
    },
  };
}

function projectWorktreeFiles(relativeRoot) {
  const result = childProcess.spawnSync(
    "git",
    ["ls-files", "--cached", "--others", "--exclude-standard", "--", relativeRoot],
    {
      cwd: repoRoot,
      encoding: "utf8",
      windowsHide: true,
    },
  );

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(`git ls-files failed\n${result.stdout || ""}${result.stderr || ""}`);
  }

  return result.stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function parseDiagnosticTrace(stderr) {
  const line = stderr
    .split(/\r?\n/)
    .find((candidate) => candidate.includes("[tua-trace diagnostics]"));

  if (!line) {
    return null;
  }

  const trace = {
    totalMs: null,
    coveredMs: null,
    unaccountedMs: null,
    buckets: {},
  };

  for (const part of line.split(/\s+/)) {
    let match = /^([a-z_]+)=([0-9.]+)ms$/.exec(part);

    if (match) {
      trace[`${camelCase(match[1])}Ms`] = Number(match[2]);
      continue;
    }

    match = /^([a-z_]+)=([0-9.]+)ms\/([0-9]+)$/.exec(part);
    if (match) {
      trace.buckets[match[1]] = {
        ms: Number(match[2]),
        count: Number(match[3]),
      };
    }
  }

  return trace;
}

function aggregateDiagnosticTraces(traces) {
  const present = traces.filter(Boolean);

  if (!present.length) {
    return null;
  }

  const output = {
    totalMs: average(present.map((trace) => trace.totalMs).filter((value) => value !== null)),
    coveredMs: average(present.map((trace) => trace.coveredMs).filter((value) => value !== null)),
    unaccountedMs: average(
      present.map((trace) => trace.unaccountedMs).filter((value) => value !== null),
    ),
    buckets: {},
  };
  const bucketNames = new Set();

  for (const trace of present) {
    for (const name of Object.keys(trace.buckets)) {
      bucketNames.add(name);
    }
  }

  for (const name of [...bucketNames].sort()) {
    const buckets = present.map((trace) => trace.buckets[name]).filter(Boolean);

    output.buckets[name] = {
      avgMs: average(buckets.map((bucket) => bucket.ms)),
      avgCount: average(buckets.map((bucket) => bucket.count)),
    };
  }

  return output;
}

function compareBaseline(report, baselinePath, threshold) {
  if (!baselinePath) {
    return [];
  }

  if (!fs.existsSync(baselinePath)) {
    console.warn(`baseline not found: ${baselinePath}`);
    return [];
  }

  const baseline = JSON.parse(fs.readFileSync(baselinePath, "utf8"));
  const currentMetrics = metricMap(report);
  const baselineMetrics = metricMap(baseline);
  const warnings = [];

  for (const [key, metric] of currentMetrics) {
    const previous = baselineMetrics.get(key);

    if (!previous) {
      continue;
    }

    for (const field of ["avgMs", "p95Ms"]) {
      const current = metric.stats[field];
      const base = previous.stats[field];

      if (!Number.isFinite(current) || !Number.isFinite(base) || base <= 0) {
        continue;
      }

      const increase = ((current - base) / base) * 100;

      if (increase > threshold) {
        warnings.push(
          `${key} ${field} ${current.toFixed(2)}ms exceeded baseline ` +
            `${base.toFixed(2)}ms by ${increase.toFixed(1)}%`,
        );
      }
    }
  }

  return warnings;
}

function metricMap(report) {
  const output = new Map();

  for (const project of report.projects || []) {
    for (const metric of project.metrics || []) {
      output.set(`${project.name}:${metric.surface}:${metric.label}`, metric);
    }
  }

  return output;
}

function average(values) {
  if (!values.length) {
    return 0;
  }

  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function camelCase(value) {
  return value.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
}

function numberEnv(name, fallback) {
  const value = Number(process.env[name]);

  return Number.isFinite(value) ? value : fallback;
}

function truthy(value) {
  return value === "1" || value === "true" || value === "yes";
}

function executableName(name) {
  return process.platform === "win32" ? `${name}.exe` : name;
}
