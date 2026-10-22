#!/usr/bin/env node
"use strict";

const assert = require("assert/strict");
const crypto = require("crypto");
const fs = require("fs");
const net = require("net");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");
const { runTests } = require("@vscode/test-electron");
const { localVSCodeExecutable } = require("../test/vscode-executable");

const REPO_ROOT = path.resolve(__dirname, "..", "..", "..");
const EXTENSION_ROOT = path.resolve(__dirname, "..");
const SUITE_PATH = path.join(
  EXTENSION_ROOT,
  "test",
  "inline-editor-replay",
  "suite",
  "index.js",
);
const MOCK_SERVER = path.join(EXTENSION_ROOT, "test", "fixtures", "mock-lsp-server.js");
const MANIFEST_PATH = path.join(REPO_ROOT, "benchmarks", "inline-eval", "manifest.toml");
const OUTPUT_ROOT = path.join(REPO_ROOT, "target", "tua-speed");
const CAPTURE_PATH = path.join(OUTPUT_ROOT, "inline-editor-replay.jsonl");
const EVALUATOR_PATH = path.join(OUTPUT_ROOT, "inline-editor-replay-eval.json");
const REPORT_PATH = path.join(OUTPUT_ROOT, "inline-editor-replay.json");
const SANDBOX_EXEC = "/usr/bin/sandbox-exec";
const NETWORK_POLICY =
  '(version 1) (allow default) (deny network-outbound (require-not (remote ip "localhost:*")))';
const LOCAL_TIMEOUT_MS = productionLocalTimeoutMs();

const SCENARIOS = Object.freeze([
  {
    id: "tua-identifier-menu-open",
    datasetPath: "examples/love-full/src/player.tua",
    copyRoot: "examples/love-full",
    sourceSha256: "686892ac538244a4b2f9c7f1ceea5938f95937b68c45aafae217c28002129a20",
    editedSha256: "c8a434a53c93a0f5565fe37abb301002303b891b124c031cc5858cead585fa1e",
    line: 119,
    originalLine: "    sound.play_move()",
    finalPrefix: "    sound.play_mov",
    typedText: "mov",
    expectedContext: "identifier",
    menu: "open",
    expectedSelection: "play_move",
    expectedGhostText: "e()",
    expectedCounter: "currentFileReturned",
  },
  {
    id: "tua-comma-menu-closed",
    datasetPath: "examples/love-full/src/obstacles.tua",
    copyRoot: "examples/love-full",
    sourceSha256: "284f8592d3f5ae89b58d8951961c9adedebe92894759248ea9d0674426220e7d",
    editedSha256: "b8d5e68bb5d98800c495968ebbf005435db9acbd74d97332706e64c83a4eae5b",
    line: 75,
    originalLine: "      table.remove(obstacles.list, i)",
    finalPrefix: "      table.remove(obstacles.list,",
    typedText: ",",
    expectedContext: "comma",
    menu: "closed",
    expectedGhostText: " i",
    expectedCounter: "currentFileReturned",
  },
  {
    id: "tua-comma-menu-closed-second",
    datasetPath: "examples/features/src/classes.tua",
    copyRoot: "examples/features",
    sourceSha256: "d71488e29972a3e4e4f90f66b9dc53d82ae2f1bb7e52f6d7f37b22a126afd2a7",
    editedSha256: "87ac4930a2f4aa348218669d0d94ca764cc1bd0db2f142d43faa917fccc129eb",
    line: 7,
    originalLine: "  init(x: number, y: number) {",
    finalPrefix: "  init(x: number,",
    typedText: ",",
    expectedContext: "comma",
    menu: "closed",
    expectedGhostText: " y",
    expectedCounter: "currentFileReturned",
  },
  {
    id: "tua-comma-menu-closed-third",
    datasetPath: "examples/love-full/src/friends.tua",
    copyRoot: "examples/love-full",
    sourceSha256: "8608b177e06c467f0b621ef5593bd66a4771ec5e193d838cefc40869b9704f4c",
    editedSha256: "e6fe9725dda5245fef93eede5172b8ca29168878fece6bf76be6cd6f2f89d507",
    line: 76,
    originalLine: "      table.remove(friends.list, i)",
    finalPrefix: "      table.remove(friends.list,",
    typedText: ",",
    expectedContext: "comma",
    menu: "closed",
    expectedGhostText: " i",
    expectedCounter: "currentFileReturned",
  },
]);

const MAC_NATIVE_KEYS = Object.freeze({
  a: 0,
  b: 11,
  c: 8,
  d: 2,
  e: 14,
  f: 3,
  g: 5,
  h: 4,
  i: 34,
  j: 38,
  k: 40,
  l: 37,
  m: 46,
  n: 45,
  o: 31,
  p: 35,
  q: 12,
  r: 15,
  s: 1,
  t: 17,
  u: 32,
  v: 9,
  w: 13,
  x: 7,
  y: 16,
  z: 6,
  ".": 47,
  ",": 43,
  " ": 49,
  Enter: 36,
  Escape: 53,
  ArrowRight: 124,
});

async function main() {
  const startedAt = new Date().toISOString();
  const runId = crypto.randomUUID();
  fs.mkdirSync(OUTPUT_ROOT, { recursive: true });
  invalidateOutputs([CAPTURE_PATH, EVALUATOR_PATH, REPORT_PATH]);
  const serverPath = path.resolve(process.argv[2] || path.join(REPO_ROOT, "target", "debug", "tua"));
  assertPrerequisites(serverPath);
  validateScenarios(REPO_ROOT, SCENARIOS, MANIFEST_PATH);
  const environment = environmentEvidence(serverPath);

  const evidence = [];
  const networkProofs = [];
  const captureLines = [];
  for (const scenario of SCENARIOS) {
    const result = await runReplayScenario(serverPath, scenario);
    evidence.push(result.evidence);
    networkProofs.push(result.networkProof);
    captureLines.push(JSON.stringify(result.record));
  }
  writePrivateAtomic(CAPTURE_PATH, `${captureLines.join("\n")}\n`);

  const evaluator = evaluateCapture(CAPTURE_PATH, EVALUATOR_PATH, SCENARIOS.length);
  const cancellation = await runCancellationScenario();
  networkProofs.push(cancellation.networkProof);
  const report = {
    schemaVersion: 1,
    runId,
    startedAt,
    finishedAt: new Date().toISOString(),
    dataset: "tune",
    environment,
    artifacts: {
      capture: { path: path.relative(REPO_ROOT, CAPTURE_PATH), sha256: sha256File(CAPTURE_PATH) },
      evaluator: { path: path.relative(REPO_ROOT, EVALUATOR_PATH), sha256: sha256File(EVALUATOR_PATH) },
    },
    networkIsolation: {
      platform: "darwin",
      mechanism: "sandbox-exec",
      policy: NETWORK_POLICY,
      extensionHostProofs: networkProofs,
      externalOutboundDenied: networkProofs.every((proof) => proof.outbound.denied),
      loopbackAllowed: networkProofs.every((proof) => proof.loopback.connected),
    },
    editorConfiguration: {
      language: "tua",
      suggestPreview: true,
    },
    scenarios: evidence,
    evaluator,
    cancellation,
    passed: true,
  };
  writePrivateAtomic(REPORT_PATH, `${JSON.stringify(report, null, 2)}\n`);
  console.log(
    `inline editor replay: ${SCENARIOS.length}/${SCENARIOS.length} exact tune scenarios matched; ` +
      `replay-open ${evaluator.replayOpen}, replay-closed ${evaluator.replayClosed}; ` +
      `cancel ${cancellation.cancelAfterMs.toFixed(1)} ms`,
  );
  console.log(`inline editor replay: wrote ${path.relative(REPO_ROOT, REPORT_PATH)}`);
}

function assertPrerequisites(serverPath) {
  assert.equal(
    process.platform,
    "darwin",
    "real editor replay currently requires macOS sandbox-exec network isolation",
  );
  assert.equal(typeof WebSocket, "function", "real editor replay requires Node with WebSocket");
  assertExecutable(SANDBOX_EXEC, "sandbox-exec");
  assertExecutable(serverPath, "Tua server");
  const vscode = localVSCodeExecutable();
  assert(vscode, "Visual Studio Code was not found in /Applications");
  assertExecutable(vscode, "Visual Studio Code");
}

function environmentEvidence(serverPath) {
  const executable = localVSCodeExecutable();
  const productPath = path.resolve(
    path.dirname(executable),
    "..",
    "Resources",
    "app",
    "product.json",
  );
  const product = readJson(productPath);
  assert(product?.version && product?.commit, "Visual Studio Code has incomplete product metadata");

  const revision = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: REPO_ROOT,
    encoding: "utf8",
  });
  assert.equal(revision.status, 0, `could not identify repository revision: ${revision.stderr}`);
  const status = spawnSync("git", ["status", "--porcelain=v1", "--untracked-files=all"], {
    cwd: REPO_ROOT,
    encoding: "utf8",
  });
  assert.equal(status.status, 0, `could not identify repository status: ${status.stderr}`);

  return {
    platform: process.platform,
    architecture: process.arch,
    node: process.version,
    vscode: {
      productVersion: product.version,
      buildCommit: product.commit,
      architecture: product.darwinUniversalAssetId || process.arch,
    },
    repository: {
      head: revision.stdout.trim(),
      dirty: status.stdout.length > 0,
      harnessSha256: sha256File(__filename),
      extensionPackageSha256: sha256File(path.join(EXTENSION_ROOT, "package.json")),
      extensionBundleSha256: sha256File(path.join(EXTENSION_ROOT, "dist", "extension.js")),
      serverSha256: sha256File(serverPath),
      mockServerSha256: sha256File(MOCK_SERVER),
    },
  };
}

function validateScenarios(repoRoot, scenarios, manifestPath = MANIFEST_PATH) {
  assert(scenarios.length >= 4, "real replay needs at least four curated scenarios");
  assert(scenarios.every((item) => item.datasetPath.endsWith(".tua")));
  assert(scenarios.some((item) => item.menu === "open"));
  assert(scenarios.some((item) => item.menu === "closed"));
  assert(new Set(scenarios.map((item) => item.expectedContext)).size >= 2);
  const tuneRoots = tuningProjectRoots(repoRoot, manifestPath);

  for (const scenario of scenarios) {
    assertSafeRelativePath(scenario.datasetPath, `${scenario.id} datasetPath`);
    assertSafeRelativePath(scenario.copyRoot, `${scenario.id} copyRoot`);
    const copyRoot = path.resolve(repoRoot, scenario.copyRoot);
    assert(tuneRoots.has(copyRoot), `${scenario.id} copyRoot is not a tune project from the manifest`);
    const sourcePath = path.resolve(repoRoot, scenario.datasetPath);
    assert(
      sourcePath.startsWith(`${copyRoot}${path.sep}`),
      `${scenario.id} source is outside its copyRoot`,
    );
    assert(!/(?:^|\/)(?:holdout|external|love-tua|audio-system)(?:\/|$)/u.test(scenario.datasetPath));
    assert(scenario.finalPrefix.endsWith(scenario.typedText));
    assert(scenario.line > 0);
    const source = normalizeSource(fs.readFileSync(sourcePath, "utf8"));
    assert.equal(sha256(source), scenario.sourceSha256, `${scenario.id} source hash drifted`);
    const lines = source.split("\n");
    assert.equal(lines[scenario.line - 1], scenario.originalLine, `${scenario.id} line drifted`);
    lines[scenario.line - 1] = scenario.finalPrefix;
    assert.equal(sha256(lines.join("\n")), scenario.editedSha256, `${scenario.id} edit hash drifted`);
  }
}

function tuningProjectRoots(repoRoot, manifestPath) {
  const source = fs.readFileSync(manifestPath, "utf8");
  const rootMatch = /^root\s*=\s*"([^"]+)"/mu.exec(source);
  assert(rootMatch, "inline-eval manifest omitted root");
  const tuneMatch = /^\[sets\.tune\]\s*$([\s\S]*?)(?=^\[)/mu.exec(source);
  assert(tuneMatch, "inline-eval manifest omitted [sets.tune]");
  const projectsMatch = /^projects\s*=\s*\[([\s\S]*?)\]/mu.exec(tuneMatch[1]);
  assert(projectsMatch, "inline-eval tune set omitted projects");
  const manifestRoot = fs.realpathSync(path.resolve(path.dirname(manifestPath), rootMatch[1]));
  assert.equal(manifestRoot, fs.realpathSync(repoRoot), "manifest root does not match repository root");
  const roots = new Set();
  for (const match of projectsMatch[1].matchAll(/"([^"]+)"/gu)) {
    assertSafeRelativePath(match[1], "tune project");
    roots.add(fs.realpathSync(path.resolve(manifestRoot, match[1])));
  }
  assert(roots.size > 0, "inline-eval tune set has no projects");
  return roots;
}

function assertSafeRelativePath(value, label) {
  assert.equal(typeof value, "string", `${label} must be a string`);
  assert(value.length > 0 && value !== ".", `${label} must name a scoped path`);
  assert(!path.isAbsolute(value), `${label} must be relative`);
  assert(!value.split(/[\\/]/u).includes(".."), `${label} must not contain ..`);
  assert.equal(value, normalizePath(path.normalize(value)), `${label} must be canonical`);
}

async function runReplayScenario(serverPath, scenario) {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tua-ir-"));
  try {
    const workspace = prepareScenarioWorkspace(tempRoot, scenario);
    const capturePath = path.join(tempRoot, "capture.jsonl");
    const session = await startSession({
      tempRoot,
      workspace: workspace.root,
      serverPath,
      capturePath,
      captureRoot: tempRoot,
    });
    let cdp;
    try {
      cdp = await session.connect();
      await session.prepareEditor(workspace.datasetPath, scenario.line - 1);
      await focusEditor(cdp);
      await session.typeText(scenario.typedText);
      await waitForEditorLine(cdp, scenario.finalPrefix);

      if (scenario.menu === "open") {
        await pressKey(cdp, "Escape", "Escape", 0, 27);
        await waitFor(async () => !(await suggestionWidget(cdp)).visible, "suggestion menu reset");
        await delay(120);
        fs.writeFileSync(capturePath, "");
        await session.triggerSuggest();
        await waitForSelectedSuggestion(cdp, scenario.expectedSelection, () =>
          session.triggerSuggest(),
        );
      } else {
        await pressKey(cdp, "Escape", "Escape", 0, 27);
        await waitFor(async () => !(await suggestionWidget(cdp)).visible, "suggestion menu to close");
      }

      await delay(120);
      const preTriggerGhostText = await ghostText(cdp, scenario.finalPrefix);
      const preTriggerGhostOverlap = ghostTextMatches(
        preTriggerGhostText,
        scenario.expectedGhostText,
      );
      fs.writeFileSync(capturePath, "");
      const countersBefore = await session.debugInlineCounters();
      fs.writeFileSync(capturePath, "");
      await session.triggerInline();

      const records = await waitFor(
        () =>
          readJsonLines(capturePath).filter(
            (record) =>
              record.sourceHash === scenario.editedSha256 &&
              record.menuOpen === (scenario.menu === "open") &&
              (scenario.menu !== "open" ||
                record.selectedCompletion?.text === scenario.expectedSelection),
          ),
        `${scenario.id} provider capture`,
        5_000,
        (records) => records.length > 0,
      );
      assert(records.length > 0, `${scenario.id} should capture a matching provider request`);
      const record = records.at(-1);
      const allRecords = readJsonLines(capturePath);
      assert(allRecords.length > 0, `${scenario.id} capture unexpectedly became empty`);
      validateCaptureRecord(record, scenario);
      const countersAfter = await waitFor(
        () => session.debugInlineCounters(),
        `${scenario.id} returned counter`,
        5_000,
        (counters) => counters[scenario.expectedCounter] > countersBefore[scenario.expectedCounter],
      );
      const counterDelta = validateCounterDelta(
        countersBefore,
        countersAfter,
        scenario.expectedCounter,
        scenario.id,
      );
      const renderedGhostText =
        scenario.menu === "closed" ? await waitForRenderedGhost(cdp, scenario) : null;

      const widget = await suggestionWidget(cdp);
      assert.equal(widget.visible, scenario.menu === "open", `${scenario.id} widget state changed`);
      const editorResult = await session.finish();
      assert.equal(normalizePath(editorResult.path).endsWith(scenario.datasetPath), true);
      assert.equal(editorResult.line, scenario.line - 1);
      assert.equal(editorResult.character, utf16Length(scenario.finalPrefix));
      assert.equal(editorResult.lineText, scenario.finalPrefix);

      return {
        record,
        networkProof: session.networkProof,
        evidence: {
          id: scenario.id,
          language: scenario.datasetPath.endsWith(".tua") ? "tua" : "lua",
          context: scenario.expectedContext,
          menu: scenario.menu,
          selectedCompletion: record.selectedCompletion?.text || null,
          elapsedSinceEditMs: record.elapsedSinceEditMs,
          providerInvocations: allRecords.length,
          transitionalProviderInvocations: allRecords.length - records.length,
          renderedGhostText,
          preTriggerGhostText,
          preTriggerGhostOverlap,
          renderingEvidence:
            scenario.menu === "closed"
              ? preTriggerGhostOverlap
                ? "provider-counter-evaluator-and-preexisting-preview"
                : "provider-counter-evaluator-and-rendered-match"
              : "provider-counter-capture-and-evaluator-only",
          localCountersBefore: countersBefore,
          localCountersAfter: countersAfter,
          localCounterDelta: counterDelta,
          editorInlineEnabled: true,
          sourceHash: record.sourceHash,
        },
      };
    } finally {
      cdp?.close();
      await session.stop();
      await session.wait();
    }
  } finally {
    keepOrRemove(tempRoot);
  }
}

function prepareScenarioWorkspace(tempRoot, scenario) {
  const source = path.join(REPO_ROOT, scenario.copyRoot);
  const destination = path.join(tempRoot, scenario.copyRoot);
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.cpSync(source, destination, { recursive: true });

  const filePath = path.join(tempRoot, scenario.datasetPath);
  const text = normalizeSource(fs.readFileSync(filePath, "utf8"));
  const lines = text.split("\n");
  lines[scenario.line - 1] = scenario.finalPrefix.slice(0, -scenario.typedText.length);
  fs.writeFileSync(filePath, lines.join("\n"));
  writeReplayWorkspaceSettings(destination);
  return {
    root: destination,
    datasetPath: normalizePath(path.relative(scenario.copyRoot, scenario.datasetPath)),
  };
}

function writeReplayWorkspaceSettings(workspace) {
  const directory = path.join(workspace, ".vscode");
  fs.mkdirSync(directory, { recursive: true });
  fs.writeFileSync(
    path.join(directory, "settings.json"),
    `${JSON.stringify(
      {
        "editor.inlineSuggest.enabled": true,
        "editor.quickSuggestionsDelay": 0,
        "editor.suggest.preview": true,
        "tua.completion.inline.enabled": true,
        "tua.completion.multiToken.allowModelIntegration": false,
      },
      null,
      2,
    )}\n`,
  );
}

function validateCaptureRecord(record, scenario) {
  assert.equal(record.schemaVersion, 1);
  assert.equal(record.datasetPath, scenario.datasetPath);
  assert.equal(record.sourceHash, scenario.editedSha256);
  assert.deepEqual(record.position, {
    line: scenario.line - 1,
    character: utf16Length(scenario.finalPrefix),
  });
  assert.equal(record.menuOpen, scenario.menu === "open");
  assert(Number.isFinite(record.elapsedSinceEditMs));
  if (scenario.menu === "open") {
    assert.equal(record.selectedCompletion?.text, scenario.expectedSelection);
    assert.equal(record.selectedCompletion.range.end.line, scenario.line - 1);
    assert.equal(record.selectedCompletion.range.end.character, utf16Length(scenario.finalPrefix));
  } else {
    assert.equal(record.selectedCompletion, undefined);
  }
}

function evaluateCapture(capturePath, reportPath, expectedRecords) {
  const temporaryReport = `${reportPath}.tmp-${process.pid}`;
  fs.rmSync(temporaryReport, { force: true });
  const result = spawnSync(
    "cargo",
    [
      "run",
      "-q",
      "-p",
      "tua-core",
      "--features",
      "test-fixture",
      "--example",
      "inline_eval",
      "--",
      "--manifest",
      MANIFEST_PATH,
      "--set",
      "tune",
      "--max-positions",
      "1",
      "--no-warmup",
      "--records",
      "--menu-replay",
      capturePath,
      "--out",
      temporaryReport,
    ],
    { cwd: REPO_ROOT, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
  );
  if (result.status !== 0) {
    fs.rmSync(temporaryReport, { force: true });
    throw new Error(`inline evaluator rejected editor replay:\n${result.stdout}\n${result.stderr}`);
  }
  fs.chmodSync(temporaryReport, 0o600);
  try {
    const report = JSON.parse(fs.readFileSync(temporaryReport, "utf8"));
    const replay = report.menu_replay;
    assert.equal(replay.records, expectedRecords);
    assert.equal(replay.accepted, expectedRecords);
    assert.equal(replay.matched, expectedRecords);
    for (const key of [
      "unmatched",
      "superseded",
      "rejected_schema",
      "rejected_path",
      "rejected_position",
      "rejected_ineligible",
    ]) {
      assert.equal(replay[key], 0, `menu replay ${key} must be zero`);
    }
    const tune = report.sets.find((set) => set.name === "tune");
    assert(tune, "inline evaluator omitted the tune set");
    const phase1 = tune.backends.phase1.by_menu;
    const replayOpen = phase1["replay-open"]?.eligible || 0;
    const replayClosed = phase1["replay-closed"]?.eligible || 0;
    assert.equal(replayOpen, SCENARIOS.filter((item) => item.menu === "open").length);
    assert.equal(replayClosed, SCENARIOS.filter((item) => item.menu === "closed").length);
    assert.equal(phase1["replay-open"]?.suggested, replayOpen);
    assert.equal(phase1["replay-open"]?.correct_prefix, replayOpen);
    assert.equal(phase1["replay-closed"]?.suggested, replayClosed);
    assert.equal(phase1["replay-closed"]?.correct_prefix, replayClosed);
    const phase1Records = report.sets
      .flatMap((set) => set.records)
      .filter((record) => record.backend === "phase1" && record.scenario === "edit");
    const expectedContexts = new Set();
    for (const scenario of SCENARIOS) {
      const column = utf16Length(scenario.finalPrefix) + 1;
      const matches = phase1Records.filter(
        (record) =>
          record.file === scenario.datasetPath &&
          record.line === scenario.line &&
          record.column === column &&
          record.menu === scenario.menu,
      );
      assert(matches.length > 0, `${scenario.id} has no exact Phase 1 evaluator record`);
      assert(
        matches.some(
          (record) =>
            record.context === scenario.expectedContext &&
            record.candidate?.text === scenario.expectedGhostText &&
            record.outcome?.correct_prefix === true,
        ),
        `${scenario.id} evaluator context, candidate, or correctness drifted`,
      );
      expectedContexts.add(scenario.expectedContext);
    }
    const contexts = {};
    for (const context of expectedContexts) {
      const eligible = tune.backends.phase1.by_context[context]?.eligible || 0;
      assert(eligible > 0, `evaluator did not classify any replay position as ${context}`);
      contexts[context] = eligible;
    }
    fs.renameSync(temporaryReport, reportPath);
    return {
      records: replay.records,
      matched: replay.matched,
      replayOpen,
      replayClosed,
      contexts,
      report: path.relative(REPO_ROOT, reportPath),
    };
  } finally {
    fs.rmSync(temporaryReport, { force: true });
  }
}

async function runCancellationScenario() {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tua-it-"));
  try {
    const workspace = path.join(tempRoot, "workspace");
    const sourcePath = path.join(workspace, "src", "main.tua");
    fs.mkdirSync(path.dirname(sourcePath), { recursive: true });
    fs.writeFileSync(path.join(workspace, "tua.toml"), 'source = "src"\nout = "build"\n');
    fs.writeFileSync(sourcePath, "local sample = 1\nlocal repeated = \n");
    writeReplayWorkspaceSettings(workspace);
    const logPath = path.join(tempRoot, "lsp.jsonl");
    const mockLauncher = createMockLauncher(tempRoot);
    const session = await startSession({
      tempRoot,
      workspace,
      serverPath: mockLauncher,
      extraEnv: {
        TUA_EDITOR_REPLAY_SKIP_WARMUP: "1",
        TUA_TEST_INLINE_DELAY_MS: "900",
        TUA_TEST_LSP_LOG: logPath,
      },
    });
    let cdp;
    try {
      cdp = await session.connect();
      await session.prepareEditor("src/main.tua", 1);
      await focusEditor(cdp);
      await delay(150);
      fs.writeFileSync(logPath, "");
      await session.triggerInline();
      const request = await waitFor(
        () => readJsonLines(logPath).find((event) => event.type === "inline-completion.request"),
        "delayed inline request",
      );
      const cancellation = await waitFor(
        () =>
          readJsonLines(logPath).find(
            (event) => event.type === "request.cancel" && event.id === request.id,
          ),
        "linked $/cancelRequest",
      );
      assert(Number.isFinite(request.monotonicMs), "mock request omitted monotonic timing");
      assert(Number.isFinite(cancellation.monotonicMs), "mock cancellation omitted monotonic timing");
      const cancelAfterMs = cancellation.monotonicMs - request.monotonicMs;
      const minimumCancellationMs = Math.max(0, LOCAL_TIMEOUT_MS - 100);
      const maximumCancellationMs = LOCAL_TIMEOUT_MS + 200;
      assert(
        cancelAfterMs >= minimumCancellationMs,
        `inline request cancelled too early (${cancelAfterMs} ms)`,
      );
      assert(
        cancelAfterMs <= maximumCancellationMs,
        `inline request cancelled too late (${cancelAfterMs} ms)`,
      );
      await delay(900 - cancelAfterMs + 150);
      const response = readJsonLines(logPath).find(
        (event) => event.type === "inline-completion.response" && event.id === request.id,
      );
      assert(response, "delayed mock completion never produced its late response");
      assert.equal(response.cancelled, true, "mock response did not observe linked cancellation");
      const inlineText = await ghostText(cdp, "local repeated = ");
      assert.equal(inlineText, "", "timed-out local request must not render ghost text");
      await session.finish();
      return {
        configuredTimeoutMs: LOCAL_TIMEOUT_MS,
        acceptedTimingRangeMs: [minimumCancellationMs, maximumCancellationMs],
        timingClock: "process.hrtime.bigint",
        delayedResponseMs: 900,
        requestId: request.id,
        cancelAfterMs,
        linkedCancellation: true,
        lateNonemptyResponseObserved: true,
        ghostTextAfterCancellation: false,
        networkProof: session.networkProof,
      };
    } finally {
      cdp?.close();
      await session.stop();
      await session.wait();
    }
  } finally {
    keepOrRemove(tempRoot);
  }
}

async function startSession(options) {
  const files = {
    ready: path.join(options.tempRoot, "ready"),
    action: path.join(options.tempRoot, "action.json"),
    ack: path.join(options.tempRoot, "ack.json"),
    done: path.join(options.tempRoot, "done"),
    result: path.join(options.tempRoot, "result.json"),
    networkProof: path.join(options.tempRoot, "network-proof.json"),
  };
  const port = await freePort();
  const launcher = createSandboxedCodeLauncher(options.tempRoot);
  const testPromise = runTests({
    extensionDevelopmentPath: EXTENSION_ROOT,
    extensionTestsPath: SUITE_PATH,
    extensionTestsEnv: {
      TUA_EDITOR_REPLAY_READY: files.ready,
      TUA_EDITOR_REPLAY_ACTION: files.action,
      TUA_EDITOR_REPLAY_ACK: files.ack,
      TUA_EDITOR_REPLAY_DONE: files.done,
      TUA_EDITOR_REPLAY_RESULT: files.result,
      TUA_EDITOR_REPLAY_NETWORK_PROOF: files.networkProof,
      TUA_SERVER_PATH: options.serverPath,
      ...(options.capturePath
        ? {
            TUA_INLINE_REPLAY_ROOT: options.captureRoot,
            TUA_INLINE_REPLAY_CAPTURE: options.capturePath,
          }
        : {}),
      ...(options.extraEnv || {}),
    },
    launchArgs: [
      options.workspace,
      "--disable-extensions",
      "--disable-workspace-trust",
      "--disable-extension=github.copilot",
      "--disable-extension=github.copilot-chat",
      `--remote-debugging-port=${port}`,
      `--user-data-dir=${path.join(options.tempRoot, "user-data")}`,
      `--extensions-dir=${path.join(options.tempRoot, "extensions")}`,
    ],
    reuseMachineInstall: true,
    vscodeExecutablePath: launcher,
  });
  let testError;
  testPromise.catch((error) => {
    testError = error;
  });
  try {
    await waitFor(() => {
      if (testError) throw testError;
      return fs.existsSync(files.ready);
    }, "extension host readiness", 25_000);
    const networkProof = readJson(files.networkProof);
    validateNetworkProof(networkProof);
    return createSession(networkProof);
  } catch (error) {
    fs.writeFileSync(files.done, "abort\n");
    await Promise.race([testPromise.catch(() => {}), delay(5_000)]);
    throw error;
  }

  function createSession(networkProof) {
    let actionId = 0;
    let stopped = false;
    return {
      networkProof,
      async connect() {
        const target = await waitFor(async () => {
          const response = await fetch(`http://127.0.0.1:${port}/json`);
          const targets = await response.json();
          return targets.find((item) => item.type === "page");
        }, "VS Code CDP page", 15_000);
        const cdp = new CdpClient(target.webSocketDebuggerUrl);
        await cdp.connect();
        await cdp.call("Page.bringToFront");
        return cdp;
      },
      async triggerInline() {
        return sendAction({ type: "trigger-inline" });
      },
      async triggerSuggest() {
        return sendAction({ type: "trigger-suggest" });
      },
      async prepareEditor(relativePath, line) {
        return sendAction({ type: "prepare-editor", path: relativePath, line });
      },
      async typeText(value) {
        return sendAction({ type: "type-text", value });
      },
      async debugInlineCounters() {
        const ack = await sendAction({ type: "debug-inline-counters" });
        assert(ack.inlineCounters && typeof ack.inlineCounters === "object");
        return ack.inlineCounters;
      },
      async finish() {
        fs.writeFileSync(files.done, "done\n");
        stopped = true;
        return waitFor(() => readJson(files.result), "extension-host result");
      },
      async stop() {
        if (!stopped) {
          fs.writeFileSync(files.done, "done\n");
          stopped = true;
        }
      },
      wait: () =>
        Promise.race([
          testPromise,
          delay(10_000).then(() => {
            throw new Error("timed out waiting for VS Code test-host shutdown");
          }),
        ]),
    };

    async function sendAction(action) {
      const id = ++actionId;
      fs.writeFileSync(files.action, `${JSON.stringify({ id, ...action })}\n`);
      return waitFor(
        () => {
          const ack = readJson(files.ack);
          return ack?.id === id ? ack : undefined;
        },
        `extension-host ${action.type} acknowledgement`,
      );
    }
  }
}

function validateNetworkProof(proof) {
  assert(proof && typeof proof === "object", "extension host omitted network-isolation proof");
  assert.equal(proof.loopback?.connected, true, "extension host could not reach loopback");
  assert.equal(proof.loopback?.address, "127.0.0.1");
  assert.equal(proof.outbound?.denied, true, "extension host outbound access was not denied");
  assert(
    proof.outbound.errorCode === "EPERM" || proof.outbound.errorCode === "EACCES",
    `extension host outbound denial was inconclusive: ${proof.outbound?.errorCode}`,
  );
}

function validateCounterDelta(before, after, expectedCounter, label = "replay") {
  assert(before && after, `${label} omitted deterministic counters`);
  const counterDelta = Object.fromEntries(
    Object.keys(after).map((key) => [key, after[key] - (before[key] || 0)]),
  );
  assert(
    counterDelta[expectedCounter] >= 1,
    `${label} must return at least one ${expectedCounter} candidate`,
  );
  assert.equal(
    Object.entries(counterDelta)
      .filter(([key]) => key.endsWith("Returned") && key !== expectedCounter)
      .reduce((total, [, value]) => total + value, 0),
    0,
    `${label} returned an unexpected local source`,
  );
  return counterDelta;
}

class CdpClient {
  constructor(url) {
    this.url = url;
    this.sequence = 0;
    this.pending = new Map();
  }

  async connect() {
    this.socket = new WebSocket(this.url);
    await new Promise((resolve, reject) => {
      this.socket.addEventListener("open", resolve, { once: true });
      this.socket.addEventListener("error", reject, { once: true });
    });
    this.socket.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      const pending = this.pending.get(message.id);
      if (!pending) return;
      this.pending.delete(message.id);
      if (message.error) pending.reject(new Error(JSON.stringify(message.error)));
      else pending.resolve(message.result);
    });
  }

  call(method, params = {}) {
    const id = ++this.sequence;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  async evaluate(expression) {
    const response = await this.call("Runtime.evaluate", { expression, returnByValue: true });
    if (response.exceptionDetails) {
      throw new Error(`CDP evaluation failed: ${response.exceptionDetails.text}`);
    }
    return response.result.value;
  }

  close() {
    this.socket?.close();
  }
}

async function focusEditor(cdp) {
  await waitFor(
    () =>
      cdp.evaluate(
        `Boolean(document.querySelector('.monaco-editor.focused .native-edit-context'))`,
      ),
    "Monaco edit context",
  );
  const focused = await cdp.evaluate(`(() => {
    const editor = document.querySelector('.monaco-editor.focused .native-edit-context');
    editor?.focus();
    return document.activeElement === editor;
  })()`);
  assert.equal(focused, true, "Monaco edit context did not take focus");
}

async function pressKey(cdp, key, code, modifiers, windowsVirtualKeyCode) {
  const nativeVirtualKeyCode = MAC_NATIVE_KEYS[key] ?? MAC_NATIVE_KEYS[key.toLowerCase()];
  const base = { key, code, modifiers, windowsVirtualKeyCode, nativeVirtualKeyCode };
  await cdp.call("Input.dispatchKeyEvent", {
    ...base,
    type: "keyDown",
    ...(key.length === 1 ? { text: key, unmodifiedText: key } : {}),
  });
  await cdp.call("Input.dispatchKeyEvent", { ...base, type: "keyUp" });
}

async function waitForSelectedSuggestion(cdp, expected, retrigger) {
  let last;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      return await waitFor(
        async () => {
          const widget = await suggestionWidget(cdp);
          last = widget;
          return widget.visible && widget.selected === expected ? widget.selected : undefined;
        },
        `real VS Code suggestion selection ${JSON.stringify(expected)}`,
        3_000,
      );
    } catch {
      await retrigger();
    }
  }
  const html = await cdp.evaluate(
    `document.querySelector('.suggest-widget')?.outerHTML || ''`,
  );
  throw new Error(
    `expected suggestion ${JSON.stringify(expected)}, last widget ${JSON.stringify(last)}; ` +
      `widget HTML: ${html.slice(0, 4000)}`,
  );
}

async function suggestionWidget(cdp) {
  return cdp.evaluate(`(() => {
    const widget = document.querySelector('.suggest-widget');
    const selected = widget?.querySelector('.monaco-list-row.focused');
    const label = selected?.querySelector('.label-name');
    return {
      visible: Boolean(widget && widget.classList.contains('visible') && getComputedStyle(widget).display !== 'none'),
      selected: label?.textContent?.trim() || selected?.textContent?.trim() || null,
    };
  })()`);
}

async function ghostText(cdp, linePrefix) {
  return cdp.evaluate(`(() => {
    const prefix = ${JSON.stringify(linePrefix)};
    const lines = [...document.querySelectorAll('.monaco-editor.focused .view-line')]
      .map((node) => node.textContent.replace(/\\u00a0/gu, ' '));
    const line = lines.find((value) => value.startsWith(prefix));
    if (line) return line.slice(prefix.length);
    return [...document.querySelectorAll('.view-line span')]
      .filter((node) => /ghost-text|inline-completion/u.test(String(node.className)))
      .map((node) => node.textContent).join('');
  })()`);
}

async function waitForRenderedGhost(cdp, scenario) {
  try {
    return await waitFor(
      async () => {
        const value = await ghostText(cdp, scenario.finalPrefix);
        return ghostTextMatches(value, scenario.expectedGhostText) ? value : undefined;
      },
      `${scenario.id} rendered ghost text ${JSON.stringify(scenario.expectedGhostText)}`,
    );
  } catch (error) {
    const editorHtml = await cdp.evaluate(
      `document.querySelector('.monaco-editor.focused .view-lines')?.innerHTML || ''`,
    );
    throw new Error(`${error.message}; focused editor HTML: ${editorHtml.slice(0, 4000)}`);
  }
}

function ghostTextMatches(rendered, expected) {
  return String(rendered || "").replace(/\u00a0/gu, " ").includes(expected);
}

async function waitForEditorLine(cdp, expected) {
  await waitFor(async () => {
    const lines = await cdp.evaluate(
      `[...document.querySelectorAll('.monaco-editor.focused .view-lines .view-line')].map((node) => node.textContent.replace(/\\u00a0/gu, ' '))`,
    );
    return lines.includes(expected);
  }, `editor line ${JSON.stringify(expected)}`);
}

function createSandboxedCodeLauncher(tempRoot) {
  const vscode = localVSCodeExecutable();
  const launcher = path.join(tempRoot, "sandboxed-code");
  fs.writeFileSync(
    launcher,
    `#!/bin/sh\nexec "${SANDBOX_EXEC}" -p '${NETWORK_POLICY}' "${vscode}" "$@"\n`,
    { mode: 0o700 },
  );
  return launcher;
}

function createMockLauncher(tempRoot) {
  const launcher = path.join(tempRoot, "mock-tua");
  fs.writeFileSync(
    launcher,
    `#!/bin/sh\nexec "${process.execPath}" "${MOCK_SERVER}" "$@"\n`,
    { mode: 0o700 },
  );
  return launcher;
}

async function freePort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return port;
}

async function waitFor(operation, description, timeoutMs = 10_000, predicate = Boolean) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const value = await operation();
      if (predicate(value)) return value;
    } catch (error) {
      lastError = error;
    }
    await delay(50);
  }
  throw new Error(
    `timed out waiting for ${description}${lastError ? `: ${lastError.message}` : ""}`,
  );
}

function readJsonLines(filePath) {
  try {
    return fs
      .readFileSync(filePath, "utf8")
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line));
  } catch (error) {
    if (error.code === "ENOENT") return [];
    throw error;
  }
}

function readJson(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch (error) {
    if (error.code === "ENOENT" || error instanceof SyntaxError) return undefined;
    throw error;
  }
}

function keepOrRemove(tempRoot) {
  if (process.env.TUA_EDITOR_REPLAY_KEEP_TEMP === "1") {
    console.log(`inline editor replay: kept ${tempRoot}`);
  } else {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

function invalidateOutputs(paths) {
  for (const outputPath of paths) fs.rmSync(outputPath, { force: true });
}

function writePrivateAtomic(outputPath, contents) {
  const temporaryPath = `${outputPath}.tmp-${process.pid}-${crypto.randomUUID()}`;
  try {
    fs.writeFileSync(temporaryPath, contents, { mode: 0o600 });
    fs.chmodSync(temporaryPath, 0o600);
    fs.renameSync(temporaryPath, outputPath);
  } finally {
    fs.rmSync(temporaryPath, { force: true });
  }
}

function sha256File(filePath) {
  return crypto.createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
}

function productionLocalTimeoutMs() {
  const sourcePath = path.join(EXTENSION_ROOT, "src", "editor", "inline-completion.js");
  const source = fs.readFileSync(sourcePath, "utf8");
  const match = /const LOCAL_REQUEST_TIMEOUT_MS = (\d+);/u.exec(source);
  assert(match, "production inline-completion timeout constant was not found");
  const value = Number(match[1]);
  assert(Number.isInteger(value) && value > 0, "production inline timeout is invalid");
  return value;
}

function assertExecutable(filePath, label) {
  const stat = fs.statSync(filePath, { throwIfNoEntry: false });
  assert(stat?.isFile(), `${label} not found at ${filePath}`);
  fs.accessSync(filePath, fs.constants.X_OK);
}

function normalizeSource(source) {
  return source.replace(/\r\n/gu, "\n");
}

function normalizePath(filePath) {
  return String(filePath || "").split(path.sep).join("/");
}

function sha256(value) {
  return crypto.createHash("sha256").update(value, "utf8").digest("hex");
}

function utf16Length(value) {
  return value.length;
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error.stack || error);
    process.exitCode = 1;
  });
}

module.exports = {
  NETWORK_POLICY,
  SCENARIOS,
  evaluateCapture,
  ghostTextMatches,
  normalizeSource,
  sha256,
  utf16Length,
  validateCounterDelta,
  validateNetworkProof,
  validateCaptureRecord,
  validateScenarios,
  writePrivateAtomic,
};
