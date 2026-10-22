"use strict";

const assert = require("assert");
const test = require("node:test");
const { buildComparison, median } = require("./representative-diagnostics-compare");

test("median handles odd and even samples", () => {
  assert.strictEqual(median([9, 1, 5]), 5);
  assert.strictEqual(median([9, 1, 5, 3]), 4);
});

test("comparison uses pooled p95 and the larger configured allowance", () => {
  const baseline = reports(true, 20);
  const candidate = reports(false, 30);
  const result = buildComparison(baseline, candidate, {
    baselineCommand: "/tmp/baseline",
    candidateCommand: "/tmp/candidate",
    tolerance: 0.05,
    minimumSlackMs: 10,
    samplesPerRound: 20,
  });

  assert.strictEqual(result.fixtures[0].baselineP95Ms, 20);
  assert.strictEqual(result.fixtures[0].candidateP95Ms, 30);
  assert.strictEqual(result.fixtures[0].baselineSamples, 100);
  assert.strictEqual(result.fixtures[0].allowedP95Ms, 30);
  assert.strictEqual(result.passed, true);
});

test("comparison fails a regression beyond the allowance", () => {
  const result = buildComparison(reports(true, 100), reports(false, 111), {
    baselineCommand: "/tmp/baseline",
    candidateCommand: "/tmp/candidate",
    tolerance: 0.05,
    minimumSlackMs: 10,
    samplesPerRound: 20,
  });

  assert.strictEqual(result.fixtures[0].allowedP95Ms, 110);
  assert.strictEqual(result.passed, false);
  assert.match(result.failures[0], /candidate pooled p95 111\.00ms/);
});

test("comparison rejects fixture drift", () => {
  const candidate = reports(false, 10);
  candidate[0].fixtures[0].contentHash = "changed";

  assert.throws(
    () =>
      buildComparison(reports(true, 10), candidate, {
        baselineCommand: "/tmp/baseline",
        candidateCommand: "/tmp/candidate",
        tolerance: 0.05,
        minimumSlackMs: 10,
        samplesPerRound: 20,
      }),
    /changed fixture|fixture hashes differ/,
  );
});

test("comparison rejects baseline inline work", () => {
  const baseline = reports(true, 10);
  baseline[0].fixtures[0].inlineCompletion = { responses: 20 };

  assert.throws(() => compare(baseline, reports(false, 10)), /unexpectedly exercised inline/);
});

test("comparison rejects incomplete candidate inline coverage", () => {
  const candidate = reports(false, 10);
  candidate[0].fixtures[0].inlineCompletion.responses = 19;

  assert.throws(() => compare(reports(true, 10), candidate), /incomplete inline request/);
});

test("comparison rejects candidate requests that bypass the core backend", () => {
  const candidate = reports(false, 10);
  candidate[0].fixtures[0].inlineCompletion.counters.requests = 19;

  assert.throws(() => compare(reports(true, 10), candidate), /incomplete inline request/);
});

test("comparison rejects workload configuration drift", () => {
  const candidate = reports(false, 10);
  candidate[0].configuration.backgroundBurst = 31;

  assert.throws(() => compare(reports(true, 10), candidate), /configurations differ/);
});

test("comparison rejects a claimed sample count that differs from the reports", () => {
  assert.throws(
    () =>
      buildComparison(reports(true, 10), reports(false, 10), {
        baselineCommand: "/tmp/baseline",
        candidateCommand: "/tmp/candidate",
        tolerance: 0.05,
        minimumSlackMs: 10,
        samplesPerRound: 21,
      }),
    /sample count does not match/,
  );
});

function compare(baseline, candidate) {
  return buildComparison(baseline, candidate, {
    baselineCommand: "/tmp/baseline",
    candidateCommand: "/tmp/candidate",
    tolerance: 0.05,
    minimumSlackMs: 10,
    samplesPerRound: 20,
  });
}

function reports(baselineCompatibility, p95Ms) {
  return Array.from({ length: 5 }, () => report(baselineCompatibility, p95Ms));
}

function report(baselineCompatibility, p95Ms) {
  const inlineStats = { runs: 20 };
  return {
    mode: "diagnostics-only",
    baselineCompatibility,
    configuration: {
      coldRepeat: 2,
      warmRepeat: 5,
      contentionRepeat: 20,
      backgroundBurst: 32,
      contentionDelayMs: 10,
      requestTimeoutMs: 60000,
      workspaceWarmTimeoutMs: 120000,
      profileEnabled: false,
    },
    fixtures: [
      {
        name: "fixture",
        contentHash: "same",
        requestAge: {
          inlineAfterChange: baselineCompatibility ? null : inlineStats,
          inlineAllAfterChange: baselineCompatibility ? null : inlineStats,
        },
        inlineCompletion: baselineCompatibility
          ? null
          : { responses: 20, requestErrors: 0, counters: { requests: 20 } },
        diagnostics: {
          firstPublishAfterChange: { p95Ms, runs: 20 },
          samplesMs: Array(20).fill(p95Ms),
        },
      },
    ],
  };
}
