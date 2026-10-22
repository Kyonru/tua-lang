"use strict";

const assert = require("assert/strict");
const fs = require("fs");
const os = require("os");
const path = require("path");
const test = require("node:test");
const {
  SCENARIOS,
  ghostTextMatches,
  utf16Length,
  validateCaptureRecord,
  validateCounterDelta,
  validateNetworkProof,
  validateScenarios,
  writePrivateAtomic,
} = require("./inline-editor-replay");
const { STATUS_SELECTOR } = require("../src/constants");
const extensionManifest = require("../package.json");

const REPO_ROOT = path.resolve(__dirname, "..", "..", "..");

test("curated replay scenarios remain exact and tuning-only", () => {
  assert.doesNotThrow(() => validateScenarios(REPO_ROOT, SCENARIOS));
  assert.equal(new Set(SCENARIOS.map((scenario) => scenario.copyRoot)).size >= 2, true);
});

test("replay matches the production Tua-only selector and suggest-preview default", () => {
  assert.equal(STATUS_SELECTOR.some((selector) => selector.language === "lua"), false);
  assert.equal(STATUS_SELECTOR.every((selector) => selector.language !== "lua"), true);
  assert.equal(
    extensionManifest.contributes.configurationDefaults["[tua]"]["editor.suggest.preview"],
    true,
  );
});

test("scenario validation rejects files outside the tuning allowlist", () => {
  assert.throws(
    () => validateScenarios(REPO_ROOT, [{ ...SCENARIOS[0], datasetPath: "benchmarks/inline-eval/holdout/example.tua" }, ...SCENARIOS.slice(1)]),
    /outside its copyRoot|copyRoot is not a tune project/u,
  );
});

test("scenario validation rejects source drift", () => {
  const scenarios = [{ ...SCENARIOS[0], sourceSha256: "0".repeat(64) }, ...SCENARIOS.slice(1)];
  assert.throws(() => validateScenarios(REPO_ROOT, scenarios), /source hash drifted/u);
});

test("scenario validation rejects absolute, parent, and broad copy roots", () => {
  for (const copyRoot of [REPO_ROOT, "../tlua", "."]) {
    const scenarios = [{ ...SCENARIOS[0], copyRoot }, ...SCENARIOS.slice(1)];
    assert.throws(() => validateScenarios(REPO_ROOT, scenarios), /copyRoot/u);
  }
});

test("capture validation distinguishes real open and closed menu state", () => {
  const open = SCENARIOS.find((scenario) => scenario.menu === "open");
  const closed = SCENARIOS.find((scenario) => scenario.menu === "closed");
  const base = (scenario) => ({
    schemaVersion: 1,
    datasetPath: scenario.datasetPath,
    sourceHash: scenario.editedSha256,
    position: { line: scenario.line - 1, character: utf16Length(scenario.finalPrefix) },
    elapsedSinceEditMs: 100,
    menuOpen: scenario.menu === "open",
  });
  const openRecord = {
    ...base(open),
    selectedCompletion: {
      text: open.expectedSelection,
      range: { end: { line: open.line - 1, character: utf16Length(open.finalPrefix) } },
    },
  };
  assert.doesNotThrow(() => validateCaptureRecord(openRecord, open));
  assert.doesNotThrow(() => validateCaptureRecord(base(closed), closed));
  assert.throws(
    () => validateCaptureRecord({ ...openRecord, menuOpen: false }, open),
    /false !== true/u,
  );
});

test("network proof fails closed unless loopback works and outbound is denied", () => {
  const proof = {
    loopback: { connected: true, address: "127.0.0.1", port: 12345 },
    outbound: { denied: true, errorCode: "EPERM" },
  };
  assert.doesNotThrow(() => validateNetworkProof(proof));
  assert.throws(() => validateNetworkProof({ ...proof, outbound: { denied: false } }));
  assert.throws(() => validateNetworkProof({ ...proof, loopback: { connected: false } }));
});

test("provider attribution requires expected returns and no competing local source", () => {
  const before = { semanticReturned: 2, currentFileReturned: 4 };
  assert.deepEqual(
    validateCounterDelta(
      before,
      { semanticReturned: 2, currentFileReturned: 5 },
      "currentFileReturned",
    ),
    { semanticReturned: 0, currentFileReturned: 1 },
  );
  assert.deepEqual(
    validateCounterDelta(
      before,
      { semanticReturned: 2, currentFileReturned: 6 },
      "currentFileReturned",
    ),
    { semanticReturned: 0, currentFileReturned: 2 },
  );
  assert.throws(
    () =>
      validateCounterDelta(
        before,
        { semanticReturned: 2, currentFileReturned: 4 },
        "currentFileReturned",
      ),
    /at least one/u,
  );
  assert.throws(
    () =>
      validateCounterDelta(
        before,
        { semanticReturned: 3, currentFileReturned: 5 },
        "currentFileReturned",
      ),
    /unexpected local source/u,
  );
});

test("UTF-16 positions count surrogate pairs as two code units", () => {
  assert.equal(utf16Length("a🚀b"), 4);
});

test("ghost-text matching normalizes nbsp and rejects absent candidates", () => {
  assert.equal(ghostTextMatches("\u00a0i)", " i)"), true);
  assert.equal(ghostTextMatches("play_move", "()"), false);
});

test("atomic evidence writes are private", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-replay-output-"));
  try {
    const output = path.join(root, "report.json");
    writePrivateAtomic(output, "{}\n");
    assert.equal(fs.readFileSync(output, "utf8"), "{}\n");
    assert.equal(fs.statSync(output).mode & 0o777, 0o600);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
