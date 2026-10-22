"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const path = require("path");

const repoRoot = path.resolve(__dirname, "../../..");
const profiler = path.join(__dirname, "lsp-representative-profile.js");

if (require.main === module) {
  main().catch((error) => {
    console.error(error.stack || error.message);
    process.exit(1);
  });
}

async function main() {
  const baselineCommand = requiredCommand(process.argv[2], "baseline Tua binary");
  const candidateCommand = requiredCommand(process.argv[3], "candidate Tua binary");
  const rounds = positiveIntegerEnv("TUA_REPRESENTATIVE_DIAGNOSTICS_ROUNDS", 5);
  const samplesPerRound = positiveIntegerEnv(
    "TUA_REPRESENTATIVE_DIAGNOSTICS_SAMPLES_PER_ROUND",
    20,
  );
  if (samplesPerRound < 20) {
    throw new Error("TUA_REPRESENTATIVE_DIAGNOSTICS_SAMPLES_PER_ROUND must be at least 20");
  }
  if (rounds * samplesPerRound < 100) {
    throw new Error("the diagnostics comparison requires at least 100 samples per fixture");
  }
  const tolerance = ratioEnv("TUA_REPRESENTATIVE_DIAGNOSTICS_TOLERANCE", 0.05);
  const minimumSlackMs = nonnegativeNumberEnv(
    "TUA_REPRESENTATIVE_DIAGNOSTICS_MIN_SLACK_MS",
    10,
  );
  const outputPath = path.resolve(
    process.env.TUA_REPRESENTATIVE_DIAGNOSTICS_COMPARE_OUT ||
      path.join(repoRoot, "target", "tua-speed", "representative-diagnostics-compare.json"),
  );
  const runRoot = path.join(path.dirname(outputPath), "representative-diagnostics-runs");
  const reports = { baseline: [], candidate: [] };

  fs.mkdirSync(runRoot, { recursive: true });
  console.log("# Tua representative diagnostics baseline comparison");
  console.log(`baseline: ${baselineCommand}`);
  console.log(`candidate: ${candidateCommand}`);
  console.log(
    `rounds: ${rounds}, tolerance: ${(tolerance * 100).toFixed(1)}%, ` +
      `samples per round: ${samplesPerRound}, minimum slack: ${minimumSlackMs}ms`,
  );

  for (let round = 0; round < rounds; round += 1) {
    const order = round % 2 === 0 ? ["baseline", "candidate"] : ["candidate", "baseline"];

    for (const role of order) {
      const reportPath = path.join(runRoot, `${role}-${round + 1}.json`);
      const command = role === "baseline" ? baselineCommand : candidateCommand;

      console.log(`\n## round ${round + 1}/${rounds}: ${role}`);
      await runProfiler(command, reportPath, role === "baseline", samplesPerRound);
      reports[role].push(JSON.parse(fs.readFileSync(reportPath, "utf8")));
    }
  }

  const comparison = buildComparison(reports.baseline, reports.candidate, {
    baselineCommand,
    candidateCommand,
    tolerance,
    minimumSlackMs,
    samplesPerRound,
  });

  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(comparison, null, 2)}\n`, "utf8");
  printComparison(comparison);
  console.log(`\nwrote ${outputPath}`);

  if (!comparison.passed) {
    throw new Error(
      `diagnostics freshness regression:\n- ${comparison.failures.join("\n- ")}`,
    );
  }
}

function runProfiler(command, outputPath, baselineCompatibility, samplesPerRound) {
  return new Promise((resolve, reject) => {
    const child = childProcess.spawn(process.execPath, [profiler, command], {
      cwd: repoRoot,
      env: {
        ...process.env,
        TUA_REPRESENTATIVE_BASELINE_ONLY: baselineCompatibility ? "1" : "0",
        TUA_REPRESENTATIVE_DIAGNOSTICS_ONLY: "1",
        TUA_REPRESENTATIVE_CONTENTION_REPEAT: String(samplesPerRound),
        TUA_REPRESENTATIVE_PROFILE_OUT: outputPath,
      },
      stdio: "inherit",
    });

    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) {
        resolve();
      } else {
        reject(
          new Error(
            `representative profiler exited with ${code ?? `signal ${signal}`}`,
          ),
        );
      }
    });
  });
}

function buildComparison(baselineReports, candidateReports, options) {
  if (baselineReports.length === 0 || baselineReports.length !== candidateReports.length) {
    throw new Error("baseline and candidate need the same non-zero number of reports");
  }

  const baselineFixtures = fixturesByName(baselineReports[0]);
  const candidateFixtures = fixturesByName(candidateReports[0]);
  const baselineConfiguration = workloadConfiguration(baselineReports[0]);
  const candidateConfiguration = workloadConfiguration(candidateReports[0]);
  const failures = [];
  const fixtures = [];

  if (JSON.stringify(baselineConfiguration) !== JSON.stringify(candidateConfiguration)) {
    throw new Error("baseline and candidate workload configurations differ");
  }
  if (baselineConfiguration.contentionRepeat !== options.samplesPerRound) {
    throw new Error("recorded sample count does not match the comparison configuration");
  }
  validateReports(
    baselineReports,
    "baseline",
    true,
    baselineFixtures,
    baselineConfiguration,
  );
  validateReports(
    candidateReports,
    "candidate",
    false,
    candidateFixtures,
    candidateConfiguration,
  );

  for (const [name, baselineFixture] of baselineFixtures) {
    const candidateFixture = candidateFixtures.get(name);
    if (!candidateFixture) {
      throw new Error(`candidate report is missing fixture ${name}`);
    }
    if (candidateFixture.contentHash !== baselineFixture.contentHash) {
      throw new Error(`${name}: baseline and candidate fixture hashes differ`);
    }

    const baselineRunP95Ms = baselineReports.map(
      (report) => fixtureNamed(report, name).diagnostics.firstPublishAfterChange.p95Ms,
    );
    const candidateRunP95Ms = candidateReports.map(
      (report) => fixtureNamed(report, name).diagnostics.firstPublishAfterChange.p95Ms,
    );
    const baselineSamplesMs = baselineReports.flatMap(
      (report) => fixtureNamed(report, name).diagnostics.samplesMs,
    );
    const candidateSamplesMs = candidateReports.flatMap(
      (report) => fixtureNamed(report, name).diagnostics.samplesMs,
    );
    if (baselineSamplesMs.length < 100 || candidateSamplesMs.length < 100) {
      throw new Error(`${name}: diagnostics comparison requires at least 100 samples per binary`);
    }
    const baselineP95Ms = percentile(baselineSamplesMs, 0.95);
    const candidateP95Ms = percentile(candidateSamplesMs, 0.95);
    const allowedP95Ms = Math.max(
      baselineP95Ms * (1 + options.tolerance),
      baselineP95Ms + options.minimumSlackMs,
    );
    const passed = candidateP95Ms <= allowedP95Ms;

    if (!passed) {
      failures.push(
        `${name}: candidate pooled p95 ${candidateP95Ms.toFixed(2)}ms exceeds ` +
          `${allowedP95Ms.toFixed(2)}ms (baseline ${baselineP95Ms.toFixed(2)}ms)`,
      );
    }
    fixtures.push({
      name,
      contentHash: baselineFixture.contentHash,
      baselineSamples: baselineSamplesMs.length,
      candidateSamples: candidateSamplesMs.length,
      baselineRunP95Ms,
      candidateRunP95Ms,
      baselineMedianRunP95Ms: median(baselineRunP95Ms),
      candidateMedianRunP95Ms: median(candidateRunP95Ms),
      baselineP95Ms,
      candidateP95Ms,
      allowedP95Ms,
      deltaMs: candidateP95Ms - baselineP95Ms,
      ratio: baselineP95Ms === 0 ? null : candidateP95Ms / baselineP95Ms,
      passed,
    });
  }

  for (const name of candidateFixtures.keys()) {
    if (!baselineFixtures.has(name)) {
      throw new Error(`baseline report is missing fixture ${name}`);
    }
  }

  return {
    schemaVersion: 1,
    baselineCommand: options.baselineCommand,
    candidateCommand: options.candidateCommand,
    configuration: {
      rounds: baselineReports.length,
      samplesPerRound: options.samplesPerRound,
      samplesPerFixture: baselineReports.length * options.samplesPerRound,
      tolerance: options.tolerance,
      minimumSlackMs: options.minimumSlackMs,
      workload:
        "baseline uses compatibility mode; candidate additionally issues the enabled local inline request",
    },
    fixtures,
    failures,
    passed: failures.length === 0,
  };
}

function validateReports(
  reports,
  role,
  expectedBaselineCompatibility,
  expectedFixtures,
  expectedConfiguration,
) {
  for (const [index, report] of reports.entries()) {
    if (report.mode !== "diagnostics-only") {
      throw new Error(`${role} report ${index + 1} was not recorded in diagnostics-only mode`);
    }
    if (report.baselineCompatibility !== expectedBaselineCompatibility) {
      throw new Error(`${role} report ${index + 1} has the wrong compatibility mode`);
    }
    if (report.configuration?.profileEnabled !== false) {
      throw new Error(`${role} report ${index + 1} did not disable internal profiling`);
    }
    if (JSON.stringify(workloadConfiguration(report)) !== JSON.stringify(expectedConfiguration)) {
      throw new Error(`${role} report ${index + 1} changed the workload configuration`);
    }
    const actual = fixturesByName(report);
    if (actual.size !== expectedFixtures.size) {
      throw new Error(`${role} report ${index + 1} has a different fixture count`);
    }
    for (const [name, fixture] of expectedFixtures) {
      const current = actual.get(name);
      if (!current || current.contentHash !== fixture.contentHash) {
        throw new Error(`${role} report ${index + 1} changed fixture ${name}`);
      }
      if (current.diagnostics.firstPublishAfterChange.runs === 0) {
        throw new Error(`${role} report ${index + 1} has no diagnostics samples for ${name}`);
      }
      const expectedRuns = Number(report.configuration?.contentionRepeat || 0);
      if (
        expectedRuns < 20 ||
        current.diagnostics.firstPublishAfterChange.runs !== expectedRuns ||
        current.diagnostics.samplesMs?.length !== expectedRuns
      ) {
        throw new Error(
          `${role} report ${index + 1} has incomplete diagnostics samples for ${name}`,
        );
      }
      if (expectedBaselineCompatibility) {
        if (
          current.inlineCompletion !== null ||
          current.requestAge?.inlineAfterChange !== null ||
          current.requestAge?.inlineAllAfterChange !== null
        ) {
          throw new Error(`${role} report ${index + 1} unexpectedly exercised inline work`);
        }
      } else if (
        current.inlineCompletion?.responses !== expectedRuns ||
        current.inlineCompletion?.requestErrors !== 0 ||
        current.inlineCompletion?.counters?.requests !== expectedRuns ||
        current.requestAge?.inlineAfterChange?.runs !== expectedRuns ||
        current.requestAge?.inlineAllAfterChange?.runs !== expectedRuns
      ) {
        throw new Error(`${role} report ${index + 1} has incomplete inline request coverage`);
      }
    }
  }
}

function fixturesByName(report) {
  return new Map((report.fixtures || []).map((fixture) => [fixture.name, fixture]));
}

function fixtureNamed(report, name) {
  const fixture = (report.fixtures || []).find((candidate) => candidate.name === name);
  if (!fixture) {
    throw new Error(`report is missing fixture ${name}`);
  }
  return fixture;
}

function workloadConfiguration(report) {
  const configuration = report.configuration || {};
  return {
    coldRepeat: configuration.coldRepeat,
    warmRepeat: configuration.warmRepeat,
    contentionRepeat: configuration.contentionRepeat,
    backgroundBurst: configuration.backgroundBurst,
    contentionDelayMs: configuration.contentionDelayMs,
    requestTimeoutMs: configuration.requestTimeoutMs,
    workspaceWarmTimeoutMs: configuration.workspaceWarmTimeoutMs,
    profileEnabled: configuration.profileEnabled,
  };
}

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);

  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

function percentile(values, quantile) {
  if (values.length === 0) {
    throw new Error("cannot calculate a percentile without samples");
  }
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(sorted.length * quantile) - 1));
  return sorted[index];
}

function printComparison(comparison) {
  console.log("\n# Comparison");
  for (const fixture of comparison.fixtures) {
    console.log(
      `${fixture.name.padEnd(18)} baseline ${fixture.baselineP95Ms.toFixed(2)}ms  ` +
        `candidate ${fixture.candidateP95Ms.toFixed(2)}ms  ` +
        `allowed ${fixture.allowedP95Ms.toFixed(2)}ms  ` +
        `${fixture.passed ? "PASS" : "FAIL"}`,
    );
  }
}

function requiredCommand(value, label) {
  if (!value) {
    throw new Error(`usage: node ${path.basename(__filename)} <baseline tua> <candidate tua>`);
  }
  const resolved = path.resolve(value);
  if (!fs.existsSync(resolved)) {
    throw new Error(`${label} does not exist: ${resolved}`);
  }
  return resolved;
}

function positiveIntegerEnv(name, fallback) {
  const value = Number(process.env[name] || fallback);
  if (!Number.isInteger(value) || value < 1) {
    throw new Error(`${name} must be a positive integer`);
  }
  return value;
}

function ratioEnv(name, fallback) {
  const value = Number(process.env[name] ?? fallback);
  if (!Number.isFinite(value) || value < 0 || value > 1) {
    throw new Error(`${name} must be between 0 and 1`);
  }
  return value;
}

function nonnegativeNumberEnv(name, fallback) {
  const value = Number(process.env[name] ?? fallback);
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(`${name} must be a non-negative number`);
  }
  return value;
}

module.exports = { buildComparison, median, percentile };
