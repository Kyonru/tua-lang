"use strict";

const fs = require("fs");
const path = require("path");
const {
  LspClient,
  cacheHitRate,
  completionItems,
  delay,
  pathToFileURL,
  positionAfter,
  profileSpans,
  resolveCommand,
  statsForValues,
} = require("./lsp-profile-common");
const {
  fixtureDefinitions,
  generateRepresentativeFixtures,
} = require("../../../scripts/generate-lsp-benchmarks");

const repoRoot = path.resolve(__dirname, "../../..");
const command = resolveCommand(process.argv[2] || "target/release/tua");
const fixtureRoot = path.resolve(
  process.env.TUA_REPRESENTATIVE_FIXTURE_ROOT ||
    path.join(repoRoot, "target", "tua-benchmarks", "lsp-representative"),
);
const outputPath = path.resolve(
  process.env.TUA_REPRESENTATIVE_PROFILE_OUT ||
    path.join(repoRoot, "target", "tua-speed", "representative-profile.json"),
);
const fixtureNames = stringListEnv(
  "TUA_REPRESENTATIVE_FIXTURES",
  fixtureDefinitions.map((fixture) => fixture.name),
);
const coldRepeat = numberEnv("TUA_REPRESENTATIVE_COLD_REPEAT", 2);
const warmRepeat = numberEnv("TUA_REPRESENTATIVE_WARM_REPEAT", 5);
const contentionRepeat = numberEnv("TUA_REPRESENTATIVE_CONTENTION_REPEAT", 3);
const backgroundBurst = numberEnv("TUA_REPRESENTATIVE_BACKGROUND_BURST", 32);
const contentionDelayMs = numberEnv("TUA_REPRESENTATIVE_CONTENTION_DELAY_MS", 10);
const requestTimeoutMs = numberEnv("TUA_LSP_REQUEST_TIMEOUT_MS", 60000);
const workspaceWarmTimeoutMs = numberEnv("TUA_REPRESENTATIVE_WARM_TIMEOUT_MS", 120000);
const completionP95BudgetMs = numberEnv(
  "TUA_REPRESENTATIVE_COMPLETION_P95_BUDGET_MS",
  10000,
);
const inlineServerP95BudgetMs = numberEnv(
  "TUA_REPRESENTATIVE_INLINE_SERVER_P95_BUDGET_MS",
  10000,
);
const inlineWorkerP95BudgetMs = numberEnv(
  "TUA_REPRESENTATIVE_INLINE_WORKER_P95_BUDGET_MS",
  20,
);
const inlineBackendP95BudgetMs = numberEnv(
  "TUA_REPRESENTATIVE_INLINE_BACKEND_P95_BUDGET_MS",
  20,
);
const inlineEndToEndP95BudgetMs = numberEnv(
  "TUA_REPRESENTATIVE_INLINE_E2E_P95_BUDGET_MS",
  10000,
);
const diagnosticsP95BudgetMs = numberEnv(
  "TUA_REPRESENTATIVE_DIAGNOSTICS_P95_BUDGET_MS",
  10000,
);
const baselineOnly = booleanEnv("TUA_REPRESENTATIVE_BASELINE_ONLY", false);
const diagnosticsOnly = booleanEnv("TUA_REPRESENTATIVE_DIAGNOSTICS_ONLY", false);
const minimumReturnedAtMostOneParseRate = 0.5;

(async () => {
  const manifest = generateRepresentativeFixtures(fixtureRoot, fixtureNames);
  const fixtures = [];

  console.log("# Tua representative LSP profile");
  console.log(`command: ${command}`);
  console.log(
    `mode: ${diagnosticsOnly ? "diagnostics-only" : "full"}` +
      `${baselineOnly ? " (baseline compatibility)" : ""}`,
  );
  console.log(`fixtures: ${fixtureNames.join(", ")}`);
  console.log(
    `cold: ${coldRepeat}, warm: ${warmRepeat}, contention: ${contentionRepeat}, ` +
      `background burst: ${backgroundBurst}`,
  );

  for (const fixture of manifest.fixtures) {
    const root = path.join(fixtureRoot, fixture.name);
    const result = await profileFixture(root, fixture);

    fixtures.push(result);
    printFixture(result);
  }

  const report = {
    schemaVersion: 2,
    command,
    mode: diagnosticsOnly ? "diagnostics-only" : "full",
    baselineCompatibility: baselineOnly,
    configuration: {
      coldRepeat,
      warmRepeat,
      contentionRepeat,
      backgroundBurst,
      contentionDelayMs,
      requestTimeoutMs,
      workspaceWarmTimeoutMs,
      profileEnabled: !diagnosticsOnly,
    },
    thresholds: {
      completionP95BudgetMs,
      inlineServerP95BudgetMs,
      inlineWorkerP95BudgetMs,
      inlineBackendP95BudgetMs,
      inlineEndToEndP95BudgetMs,
      diagnosticsP95BudgetMs,
      minimumReturnedAtMostOneParseRate,
      maximumStaleResults: 0,
      maximumValidationFailures: 0,
    },
    fixtureManifest: manifest,
    fixtures,
  };
  const failures = regressionFailures(report);

  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  console.log(`\nwrote ${outputPath}`);

  if (failures.length > 0) {
    throw new Error(`representative benchmark regression:\n- ${failures.join("\n- ")}`);
  }
})().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});

async function profileFixture(root, fixture) {
  const coldValues = metricValues();
  const validationFailures = validationFailureCounts();

  for (let round = 0; !diagnosticsOnly && round < coldRepeat; round += 1) {
    for (const label of Object.keys(coldValues)) {
      const session = await openSession(root);

      try {
        const request = representativeRequestPlan(session.main).find(
          (candidate) => candidate.label === label,
        );
        const sample = await timedRequest(session.client, request);

        coldValues[request.label].push(sample.elapsedMs);
        validationFailures[request.label] += Number(!validResult(request.label, sample.result));
      } finally {
        await closeSession(session.client);
      }
    }
  }

  const warmValues = metricValues();
  const session = await openSession(root);
  let contention;
  let cacheStats;
  let health;
  let inlineProfile;
  let workspaceWarm;

  try {
    const plan = diagnosticsOnly ? [] : representativeRequestPlan(session.main);

    for (const request of plan) {
      const sample = await timedRequest(session.client, request);

      validationFailures[request.label] += Number(!validResult(request.label, sample.result));
    }

    workspaceWarm = await waitForWorkspaceWarm(session.client);

    for (let round = 0; round < warmRepeat; round += 1) {
      for (const request of plan) {
        const sample = await timedRequest(session.client, request);

        warmValues[request.label].push(sample.elapsedMs);
        validationFailures[request.label] += Number(!validResult(request.label, sample.result));
      }
    }

    contention = await profileContention(session.client, session.main);
    validationFailures.contentionCompletion += contention.validationFailures;
    cacheStats = await session.client.request("tua/cacheStats", null);
    health = baselineOnly ? null : await session.client.request("tua/health", null);
    inlineProfile = baselineOnly ? null : inlineProfileStats(session.client.stderr);
  } finally {
    await closeSession(session.client);
  }

  return {
    name: fixture.name,
    kind: fixture.kind,
    ...(fixture.graphShape ? { graphShape: fixture.graphShape } : {}),
    sourceFileCount: fixture.sourceFileCount,
    sourceLineCount: fixture.sourceLineCount,
    contentHash: fixture.contentHash,
    latency: Object.fromEntries(
      Object.keys(coldValues).map((label) => [
        label,
        {
          cold: statsForValues(coldValues[label]),
          warm: statsForValues(warmValues[label]),
        },
      ]),
    ),
    requestAge: {
      completionAfterChange: statsForValues(contention.completionRequestAges),
      inlineAfterChange: baselineOnly
        ? null
        : statsForValues(contention.inlineSuccessfulRequestAges),
      inlineAllAfterChange: baselineOnly ? null : statsForValues(contention.inlineRequestAges),
      diagnosticsAfterChange: statsForValues(contention.diagnosticsRequestAges),
      background: statsForValues(contention.backgroundRequestAges),
    },
    cache: {
      hitRates: cacheHitRates(cacheStats),
      raw: cacheStats,
    },
    inlineCompletion: baselineOnly
      ? null
      : {
          responses: contention.inlineResponses,
          requestErrors: contention.inlineRequestErrors,
          nonEmptyResponses: contention.inlineNonEmptyResponses,
          profile: inlineProfile,
          serverRequest: methodLatency(health, "tua/localInlineCompletion"),
          counters: cacheStats.inlineCompletion || null,
        },
    diagnostics: {
      firstPublishAfterChange: statsForValues(contention.diagnosticsRequestAges),
      samplesMs: contention.diagnosticsRequestAges,
    },
    health,
    workspaceWarm,
    contention: {
      rounds: contentionRepeat,
      backgroundRequests: contention.backgroundRequests,
      cancelledRequests: contention.cancelledRequests,
      contentModifiedResponses: contention.contentModifiedResponses,
      requestCancelledResponses: contention.requestCancelledResponses,
      cancellationRate:
        contention.backgroundRequests === 0
          ? 0
          : contention.cancelledRequests / contention.backgroundRequests,
      resultsAfterChange: contention.resultsAfterChange,
      staleResultCount: contention.staleResultCount,
      freshCompletionCount: contention.freshCompletionCount,
      validDocumentCompletionCount: contention.validDocumentCompletionCount,
      invalidDocumentCompletionCount: contention.invalidDocumentCompletionCount,
      invalidDocumentCompletionSuccesses: contention.invalidDocumentCompletionSuccesses,
    },
    validationFailures: {
      byRequest: validationFailures,
      total: Object.values(validationFailures).reduce((sum, count) => sum + count, 0),
    },
  };
}

async function waitForWorkspaceWarm(client) {
  const started = process.hrtime.bigint();
  const deadline = Date.now() + workspaceWarmTimeoutMs;
  let polls = 0;
  let stats;

  do {
    stats = await client.request("tua/cacheStats", null);
    polls += 1;
    const warm = stats.workspaceWarm || {};
    const discovered = Number(warm.discovered || 0);
    const finished = Number(warm.warmed || 0) + Number(warm.failed || 0);
    const queued = Number(warm.queuedHigh || 0) + Number(warm.queuedLow || 0);
    const snapshotFinished =
      warm.snapshotReadyRevision != null || warm.snapshotFailedRevision != null;

    if (finished >= discovered && queued === 0 && snapshotFinished) {
      return {
        completed: warm.snapshotReadyRevision != null && Number(warm.failed || 0) === 0,
        elapsedMs: elapsedMs(started, process.hrtime.bigint()),
        polls,
        state: warm,
      };
    }
    await delay(25);
  } while (Date.now() < deadline);

  return {
    completed: false,
    elapsedMs: elapsedMs(started, process.hrtime.bigint()),
    polls,
    state: stats?.workspaceWarm || null,
  };
}

async function openSession(root) {
  const client = new LspClient(command, root, {
    env: {
      ...process.env,
      // Cross-version diagnostics comparisons must not make the candidate pay
      // for inline-only profile spans that do not exist in the baseline.
      TUA_PROFILE: diagnosticsOnly ? "0" : process.env.TUA_PROFILE || "1",
    },
    requestTimeoutMs,
  });
  const mainPath = path.join(root, "src", "main.tua");

  if (!fs.existsSync(mainPath)) {
    throw new Error(`${root}: generated fixture has no main.tua`);
  }
  const main = {
    path: mainPath,
    uri: pathToFileURL(mainPath).href,
    text: fs.readFileSync(mainPath, "utf8"),
  };

  await client.start();
  await client.request("initialize", {
    processId: process.pid,
    rootUri: pathToFileURL(root).href,
    capabilities: {},
    workspaceFolders: [{ uri: pathToFileURL(root).href, name: path.basename(root) }],
  });
  client.notify("initialized", {});
  if (!baselineOnly) {
    client.notify("workspace/didChangeConfiguration", {
      settings: {
        tua: {
          completion: {
            inline: { enabled: true },
          },
        },
      },
    });
  }

  client.notify("textDocument/didOpen", {
    textDocument: {
      uri: main.uri,
      languageId: "tua",
      version: 1,
      text: main.text,
    },
  });

  const activeMain = {
    ...main,
    text: `${main.text}\n-- Representative benchmark active-document synchronization.\n`,
    version: 2,
  };

  client.notify("textDocument/didChange", {
    textDocument: { uri: main.uri, version: activeMain.version },
    contentChanges: [{ text: activeMain.text }],
  });

  return { client, main: activeMain };
}

async function closeSession(client) {
  await client.request("shutdown", null);
  client.notify("exit", null);
  await client.done();
}

function representativeRequestPlan(main) {
  const textDocument = { uri: main.uri };
  const lineCount = main.text.split("\n").length + 1;

  return [
    {
      label: "completion",
      method: "textDocument/completion",
      params: {
        textDocument,
        position: positionAfter(main.text, "profile_completion_probe = profile_primary.la"),
        context: { triggerKind: 1 },
      },
    },
    {
      label: "hover",
      method: "textDocument/hover",
      params: {
        textDocument,
        position: positionAfter(main.text, "profile_hover_probe = "),
      },
    },
    {
      label: "definition",
      method: "textDocument/definition",
      params: {
        textDocument,
        position: positionAfter(main.text, "profile_definition_probe = "),
      },
    },
    {
      label: "inlayHint",
      method: "textDocument/inlayHint",
      params: {
        textDocument,
        range: {
          start: { line: 0, character: 0 },
          end: { line: lineCount, character: 0 },
        },
      },
    },
  ];
}

async function timedRequest(client, request) {
  const started = process.hrtime.bigint();
  const result = await client.request(request.method, request.params);

  return {
    result,
    elapsedMs: elapsedMs(started, process.hrtime.bigint()),
  };
}

async function profileContention(client, main) {
  const completionRequestAges = [];
  const inlineRequestAges = [];
  const inlineSuccessfulRequestAges = [];
  const diagnosticsRequestAges = [];
  const backgroundRequestAges = [];
  let currentText = main.text;
  let version = main.version;
  let backgroundRequests = 0;
  let cancelledRequests = 0;
  let contentModifiedResponses = 0;
  let requestCancelledResponses = 0;
  let resultsAfterChange = 0;
  let staleResultCount = 0;
  let freshCompletionCount = 0;
  let validDocumentCompletionCount = 0;
  let invalidDocumentCompletionCount = 0;
  let invalidDocumentCompletionSuccesses = 0;
  let validationFailures = 0;
  let inlineResponses = 0;
  let inlineRequestErrors = 0;
  let inlineNonEmptyResponses = 0;

  for (let round = 0; round < contentionRepeat; round += 1) {
    const background = [];
    const sourceLines = currentText.split("\n").length;
    const preEditBackground = Math.min(backgroundBurst, 8);

    const startBackground = (index, lineCount, sentBeforeChange) => {
      const startLine = index % Math.max(1, Math.min(lineCount - 1, backgroundBurst));
      const handle = client.startRequest("textDocument/inlayHint", {
        textDocument: { uri: main.uri },
        range: {
          start: { line: startLine, character: 0 },
          end: { line: lineCount + 1, character: 0 },
        },
      });

      background.push({ response: observeRequest(handle), sentBeforeChange });
    };
    for (let index = 0; index < preEditBackground; index += 1) {
      startBackground(index, sourceLines, true);
    }

    await delay(contentionDelayMs);

    version += 1;
    const probe = `profile_contention_probe_${String(round + 1).padStart(2, "0")}`;
    const diagnosticProbe = `profile_missing_diagnostic_${String(round + 1).padStart(2, "0")}`;
    const invalidProbe = `profile_invalid_syntax_${String(round + 1).padStart(2, "0")}`;
    const temporarilyInvalid = round === 0;
    const inlineComplete = "print(profile_primary, profile_primary)";
    const inlinePartialPrefix = "print(profile_primary, profile_pri";
    const inlinePartial = `${inlinePartialPrefix})`;
    const invalidLine = temporarilyInvalid ? `local ${invalidProbe} = )\n` : "";
    currentText = insertBeforeReturn(
      main.text,
      `local ${probe} = profile_primary.label\n` +
        `print(${diagnosticProbe})\n` +
        invalidLine +
        `${inlineComplete}\n${inlineComplete}\n${inlinePartial}\n`,
    );
    const invalidLineIndex = temporarilyInvalid
      ? lineIndexContaining(currentText, invalidProbe)
      : undefined;
    client.clearDiagnostics();
    const changedAt = process.hrtime.bigint();
    const diagnostics = client
      .waitForDiagnostics(main.uri, {
        timeoutMs: Math.min(requestTimeoutMs, 10000),
        predicate: (message) =>
          diagnosticsContain(message, diagnosticProbe) ||
          (temporarilyInvalid && diagnosticsTouchLine(message, invalidLineIndex)),
      })
      .then(() => elapsedMs(changedAt, process.hrtime.bigint()));
    client.notify("textDocument/didChange", {
      textDocument: { uri: main.uri, version },
      contentChanges: [{ text: currentText }],
    });

    const inline = baselineOnly
      ? null
      : observeRequest(
          client.startRequest("tua/localInlineCompletion", {
            textDocument: { uri: main.uri },
            position: positionAfterLast(currentText, inlinePartialPrefix),
            documentVersion: version,
          }),
        );
    const completion = observeRequest(
      client.startRequest("textDocument/completion", {
        textDocument: { uri: main.uri },
        position: positionAfter(currentText, `${probe} = profile_primary.la`),
        context: { triggerKind: 1 },
      }),
    );
    const changedSourceLines = currentText.split("\n").length;
    for (let index = preEditBackground; index < backgroundBurst; index += 1) {
      startBackground(index, changedSourceLines, false);
    }
    backgroundRequests += background.length;
    const [fresh, inlineResult, diagnosticsAge] = await Promise.all([
      completion,
      inline,
      diagnostics,
    ]);
    diagnosticsRequestAges.push(diagnosticsAge);
    const items = completionItems(fresh.message.result);
    const freshCompletion = !fresh.message.error && items.some((item) => item.label === "label");

    completionRequestAges.push(fresh.ageMs);
    freshCompletionCount += Number(freshCompletion);
    if (temporarilyInvalid) {
      invalidDocumentCompletionCount += 1;
      invalidDocumentCompletionSuccesses += Number(freshCompletion);
    } else {
      validDocumentCompletionCount += 1;
      validationFailures += Number(!freshCompletion);
    }
    if (inlineResult) {
      inlineRequestAges.push(inlineResult.ageMs);
      if (!inlineResult.message.error) {
        inlineSuccessfulRequestAges.push(inlineResult.ageMs);
      }
      inlineResponses += 1;
      inlineRequestErrors += Number(Boolean(inlineResult.message.error));
      inlineNonEmptyResponses += Number(
        Array.isArray(inlineResult.message.result?.items) &&
          inlineResult.message.result.items.length > 0,
      );
    }

    // A response received before the new-version completion may have already
    // left the server before it processed didChange. The validated completion
    // is the protocol barrier that proves the change has been observed.
    const responses = await Promise.all(
      background.map(async ({ response, sentBeforeChange }) => ({
        response: await response,
        sentBeforeChange,
      })),
    );
    for (const { response, sentBeforeChange } of responses) {
      backgroundRequestAges.push(response.ageMs);

      const code = response.message.error?.code;
      if (code === -32801) {
        contentModifiedResponses += 1;
        cancelledRequests += 1;
      } else if (code === -32800) {
        requestCancelledResponses += 1;
        cancelledRequests += 1;
      }

      if (
        sentBeforeChange &&
        response.message.result !== undefined &&
        response.receivedAt > fresh.receivedAt
      ) {
        resultsAfterChange += 1;
        staleResultCount += 1;
      }
    }
  }

  return {
    completionRequestAges,
    inlineRequestAges,
    inlineSuccessfulRequestAges,
    diagnosticsRequestAges,
    backgroundRequestAges,
    backgroundRequests,
    cancelledRequests,
    contentModifiedResponses,
    requestCancelledResponses,
    resultsAfterChange,
    staleResultCount,
    freshCompletionCount,
    validDocumentCompletionCount,
    invalidDocumentCompletionCount,
    invalidDocumentCompletionSuccesses,
    validationFailures,
    inlineResponses,
    inlineRequestErrors,
    inlineNonEmptyResponses,
  };
}

function insertBeforeReturn(source, insertion) {
  const returnStart = source.lastIndexOf("\nreturn {");

  if (returnStart === -1) {
    return `${source}\n${insertion}`;
  }

  return `${source.slice(0, returnStart + 1)}${insertion}${source.slice(returnStart + 1)}`;
}

function positionAfterLast(source, needle) {
  const offset = source.lastIndexOf(needle);
  if (offset === -1) {
    throw new Error(`needle not found in source: ${needle}`);
  }
  const before = source.slice(0, offset + needle.length);
  const lines = before.split("\n");

  return {
    line: lines.length - 1,
    character: lines[lines.length - 1].length,
  };
}

function lineIndexContaining(source, needle) {
  return source.slice(0, source.indexOf(needle)).split("\n").length - 1;
}

function diagnosticsContain(message, text) {
  return (message?.params?.diagnostics || []).some((diagnostic) =>
    JSON.stringify(diagnostic).includes(text),
  );
}

function diagnosticsTouchLine(message, line) {
  return (message?.params?.diagnostics || []).some(
    (diagnostic) => diagnostic?.range?.start?.line === line,
  );
}

function inlineProfileStats(stderr) {
  const exact = (name) =>
    statsForValues(
      profileSpans(stderr, name)
        .filter((span) => span.name === name)
        .map((span) => span.elapsedMs),
    );

  const coordinatorActive = {};
  for (const span of profileSpans(stderr, "inline.coordinator.active")) {
    const values = coordinatorActive[span.name] || [];
    values.push(span.elapsedMs);
    coordinatorActive[span.name] = values;
  }
  const coordinatorFinish = {};
  for (const span of profileSpans(stderr, "inline.coordinator.finish")) {
    const values = coordinatorFinish[span.name] || [];
    values.push(span.elapsedMs);
    coordinatorFinish[span.name] = values;
  }

  return {
    queue: exact("lsp.queue tua/localInlineCompletion"),
    preparation: exact("inline.request.prepare"),
    workerExecution: exact("lsp.request tua/localInlineCompletion"),
    coreBackend: exact("inline.request.core"),
    semantic: exact("inline.semantic"),
    recall: exact("inline.recall"),
    validation: exact("inline.validate"),
    coordinatorActive: Object.fromEntries(
      Object.entries(coordinatorActive).map(([name, values]) => [name, statsForValues(values)]),
    ),
    coordinatorFinish: Object.fromEntries(
      Object.entries(coordinatorFinish).map(([name, values]) => [name, statsForValues(values)]),
    ),
  };
}

function methodLatency(health, method) {
  return (
    health?.latency?.methods?.find((candidate) => candidate.method === method) || {
      count: 0,
      p95Ms: 0,
      maxMs: 0,
    }
  );
}

function observeRequest(handle) {
  return handle.response.then((message) => {
    const receivedAt = process.hrtime.bigint();

    return {
      message,
      receivedAt,
      ageMs: elapsedMs(handle.sentAt, receivedAt),
    };
  });
}

function metricValues() {
  return {
    completion: [],
    hover: [],
    definition: [],
    inlayHint: [],
  };
}

function validationFailureCounts() {
  return {
    completion: 0,
    hover: 0,
    definition: 0,
    inlayHint: 0,
    contentionCompletion: 0,
  };
}

function validResult(label, result) {
  switch (label) {
    case "completion":
      return completionItems(result).some((item) => item.label === "label");
    case "hover":
      return Boolean(result && result.contents);
    case "definition":
      return Array.isArray(result) ? result.length > 0 : Boolean(result && result.uri);
    case "inlayHint":
      return Array.isArray(result) && result.length > 0;
    default:
      return false;
  }
}

function cacheHitRates(stats) {
  return {
    ...Object.fromEntries(
      [
        "parse",
        "hir",
        "scope",
        "typeEnvironment",
        "diagnostics",
        "moduleExports",
        "requireEdges",
        "symbolIndex",
        "usageIndex",
        "completionContext",
      ].map((name) => [name, cacheHitRate(stats[name])]),
    ),
    inlineCurrentFile: cacheHitRate(stats.inlineCompletion?.cachePeek),
  };
}

function regressionFailures(report) {
  const failures = [];

  for (const fixture of report.fixtures) {
    const warmCompletionP95 = fixture.latency.completion.warm.p95Ms;
    const contentionCompletionP95 = fixture.requestAge.completionAfterChange.p95Ms;
    const diagnosticsP95 = fixture.diagnostics.firstPublishAfterChange.p95Ms;

    if (!diagnosticsOnly && warmCompletionP95 > report.thresholds.completionP95BudgetMs) {
      failures.push(
        `${fixture.name}: warm completion p95 ${warmCompletionP95.toFixed(2)}ms exceeds ` +
          `${report.thresholds.completionP95BudgetMs}ms`,
      );
    }
    if (contentionCompletionP95 > report.thresholds.completionP95BudgetMs) {
      failures.push(
        `${fixture.name}: completion-under-contention p95 ` +
          `${contentionCompletionP95.toFixed(2)}ms exceeds ` +
          `${report.thresholds.completionP95BudgetMs}ms`,
      );
    }
    if (!baselineOnly && !diagnosticsOnly) {
      const inlineEndToEndP95 = fixture.requestAge.inlineAfterChange.p95Ms;
      const inlineServerP95 = fixture.inlineCompletion.serverRequest.p95Ms;
      const inlineWorkerP95 = fixture.inlineCompletion.profile.workerExecution.p95Ms;
      const inlineBackendP95 = fixture.inlineCompletion.profile.coreBackend.p95Ms;

      if (inlineEndToEndP95 > report.thresholds.inlineEndToEndP95BudgetMs) {
        failures.push(
          `${fixture.name}: inline end-to-end p95 ${inlineEndToEndP95.toFixed(2)}ms exceeds ` +
            `${report.thresholds.inlineEndToEndP95BudgetMs}ms`,
        );
      }
      if (inlineServerP95 > report.thresholds.inlineServerP95BudgetMs) {
        failures.push(
          `${fixture.name}: inline server/request p95 ${inlineServerP95.toFixed(2)}ms exceeds ` +
            `${report.thresholds.inlineServerP95BudgetMs}ms`,
        );
      }
      if (inlineBackendP95 > report.thresholds.inlineBackendP95BudgetMs) {
        failures.push(
          `${fixture.name}: inline core/backend p95 ${inlineBackendP95.toFixed(2)}ms exceeds ` +
            `${report.thresholds.inlineBackendP95BudgetMs}ms`,
        );
      }
      if (inlineWorkerP95 > report.thresholds.inlineWorkerP95BudgetMs) {
        failures.push(
          `${fixture.name}: inline worker execution p95 ${inlineWorkerP95.toFixed(2)}ms exceeds ` +
            `${report.thresholds.inlineWorkerP95BudgetMs}ms`,
        );
      }
    }
    if (diagnosticsP95 > report.thresholds.diagnosticsP95BudgetMs) {
      failures.push(
        `${fixture.name}: first diagnostics p95 ${diagnosticsP95.toFixed(2)}ms exceeds ` +
          `${report.thresholds.diagnosticsP95BudgetMs}ms`,
      );
    }
    if (!baselineOnly && fixture.inlineCompletion.requestErrors !== 0) {
      failures.push(
        `${fixture.name}: ${fixture.inlineCompletion.requestErrors} inline request(s) failed`,
      );
    }
    if (
      !baselineOnly &&
      !diagnosticsOnly &&
      (fixture.inlineCompletion.serverRequest.count !== fixture.inlineCompletion.responses ||
        fixture.inlineCompletion.profile.queue.runs !== fixture.inlineCompletion.responses ||
        fixture.inlineCompletion.profile.preparation.runs !==
          fixture.inlineCompletion.responses ||
        fixture.inlineCompletion.profile.workerExecution.runs !==
          fixture.inlineCompletion.responses ||
        fixture.inlineCompletion.profile.coreBackend.runs !==
          fixture.inlineCompletion.responses)
    ) {
      failures.push(
        `${fixture.name}: inline profile spans did not cover every request ` +
          `(responses ${fixture.inlineCompletion.responses}, server ` +
          `${fixture.inlineCompletion.serverRequest.count}, queue ` +
          `${fixture.inlineCompletion.profile.queue.runs}, prepare ` +
          `${fixture.inlineCompletion.profile.preparation.runs}, worker ` +
          `${fixture.inlineCompletion.profile.workerExecution.runs}, core ` +
          `${fixture.inlineCompletion.profile.coreBackend.runs})`,
      );
    }
    const inlineCounters = fixture.inlineCompletion?.counters;
    if (!baselineOnly && !diagnosticsOnly) {
      if (!inlineCounters || Number(inlineCounters.requests || 0) === 0) {
        failures.push(`${fixture.name}: no inline completion reached the core backend`);
      } else {
        const distribution = inlineCounters.candidateParseDistribution || {};
        const distributed =
          Number(distribution.zero || 0) +
          Number(distribution.one || 0) +
          Number(distribution.two || 0);
        if (distributed !== Number(inlineCounters.candidateAttempts || 0)) {
          failures.push(
            `${fixture.name}: inline parse distribution does not match candidate attempts`,
          );
        }
        const peek = inlineCounters.cachePeek || {};
        if (Number(peek.hits || 0) + Number(peek.misses || 0) === 0) {
          failures.push(`${fixture.name}: inline cache-peek counters were not observed`);
        }
        const returned = inlineCounters.returnedParseDistribution || {};
        const returnedZero = Number(returned.zero || 0);
        const returnedOne = Number(returned.one || 0);
        const returnedTwo = Number(returned.two || 0);
        const returnedTotal = returnedZero + returnedOne + returnedTwo;
        if (returnedTotal !== Number(inlineCounters.candidatesReturned || 0)) {
          failures.push(
            `${fixture.name}: returned parse distribution does not match returned candidates`,
          );
        }
        if (returnedZero !== 0) {
          failures.push(`${fixture.name}: ${returnedZero} candidate(s) returned without parsing`);
        }
        if (returnedTotal > 0) {
          const atMostOneRate = (returnedZero + returnedOne) / returnedTotal;
          if (atMostOneRate < report.thresholds.minimumReturnedAtMostOneParseRate) {
            failures.push(
              `${fixture.name}: only ${(atMostOneRate * 100).toFixed(1)}% of returned candidates ` +
                `used at most one parse`,
            );
          }
          if (returnedTwo === 0) {
            failures.push(
              `${fixture.name}: temporarily-invalid scenario did not exercise a returned ` +
                `two-parse candidate`,
            );
          }
        }
      }
    }
    if (fixture.contention.staleResultCount > report.thresholds.maximumStaleResults) {
      failures.push(
        `${fixture.name}: observed ${fixture.contention.staleResultCount} stale background results`,
      );
    }
    if (fixture.contention.invalidDocumentCompletionCount === 0) {
      failures.push(`${fixture.name}: temporarily-invalid contention round was not exercised`);
    }
    if (fixture.contention.validDocumentCompletionCount === 0) {
      failures.push(`${fixture.name}: no valid-document contention completion was checked`);
    }
    if (fixture.validationFailures.total > report.thresholds.maximumValidationFailures) {
      failures.push(
        `${fixture.name}: ${fixture.validationFailures.total} semantic result validations failed ` +
          `(${JSON.stringify(fixture.validationFailures.byRequest)})`,
      );
    }
    if (!fixture.workspaceWarm.completed) {
      failures.push(
        `${fixture.name}: workspace did not finish warming within ` +
          `${report.configuration.workspaceWarmTimeoutMs}ms ` +
          `(${JSON.stringify(fixture.workspaceWarm.state)})`,
      );
    }
  }

  return failures;
}

function printFixture(fixture) {
  console.log(
    `\n## ${fixture.name} (${fixture.sourceFileCount} files, ${fixture.sourceLineCount} lines)`,
  );
  for (const [label, phases] of Object.entries(fixture.latency)) {
    if (phases.cold.runs === 0 && phases.warm.runs === 0) {
      continue;
    }
    console.log(
      `${label.padEnd(12)} cold p95 ${phases.cold.p95Ms.toFixed(2)}ms  ` +
        `warm p95 ${phases.warm.p95Ms.toFixed(2)}ms`,
    );
  }
  console.log(
    `contention completion p95 ` +
      `${fixture.requestAge.completionAfterChange.p95Ms.toFixed(2)}ms  ` +
      `cancelled ${fixture.contention.cancelledRequests}/` +
      `${fixture.contention.backgroundRequests}  stale ${fixture.contention.staleResultCount}`,
  );
  if (fixture.inlineCompletion && !diagnosticsOnly) {
    console.log(
      `inline e2e p95 ${fixture.requestAge.inlineAfterChange.p95Ms.toFixed(2)}ms  ` +
        `queue p95 ${fixture.inlineCompletion.profile.queue.p95Ms.toFixed(3)}ms  ` +
        `prepare p95 ${fixture.inlineCompletion.profile.preparation.p95Ms.toFixed(3)}ms  ` +
        `server/request p95 ${fixture.inlineCompletion.serverRequest.p95Ms.toFixed(2)}ms  ` +
        `worker p95 ${fixture.inlineCompletion.profile.workerExecution.p95Ms.toFixed(3)}ms  ` +
        `core p95 ${fixture.inlineCompletion.profile.coreBackend.p95Ms.toFixed(3)}ms`,
    );
  }
  console.log(
    `diagnostics after edit p95 ` +
      `${fixture.diagnostics.firstPublishAfterChange.p95Ms.toFixed(2)}ms  ` +
      (fixture.inlineCompletion
        ? `inline non-empty ${fixture.inlineCompletion.nonEmptyResponses}/` +
          `${fixture.inlineCompletion.responses}`
        : "inline skipped for baseline compatibility"),
  );
  console.log(
    `workspace warm ${fixture.workspaceWarm.completed ? "complete" : "incomplete"} in ` +
      `${fixture.workspaceWarm.elapsedMs.toFixed(2)}ms`,
  );
}

function elapsedMs(started, finished) {
  return Number(finished - started) / 1_000_000;
}

function numberEnv(name, fallback) {
  const value = Number(process.env[name] || fallback);

  if (!Number.isFinite(value) || value < 1) {
    throw new Error(`${name} must be a positive number`);
  }

  return value;
}

function booleanEnv(name, fallback) {
  const value = process.env[name];

  if (value == null || value === "") {
    return fallback;
  }
  if (value === "1" || value === "true") {
    return true;
  }
  if (value === "0" || value === "false") {
    return false;
  }
  throw new Error(`${name} must be 0, 1, false, or true`);
}

function stringListEnv(name, fallback) {
  const value = process.env[name];

  return value ? value.split(",").map((item) => item.trim()).filter(Boolean) : fallback;
}
