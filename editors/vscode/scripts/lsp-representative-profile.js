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
const completionP95BudgetMs = numberEnv(
  "TUA_REPRESENTATIVE_COMPLETION_P95_BUDGET_MS",
  10000,
);

(async () => {
  const manifest = generateRepresentativeFixtures(fixtureRoot, fixtureNames);
  const fixtures = [];

  console.log("# Tua representative LSP profile");
  console.log(`command: ${command}`);
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
    schemaVersion: 1,
    command,
    configuration: {
      coldRepeat,
      warmRepeat,
      contentionRepeat,
      backgroundBurst,
      contentionDelayMs,
      requestTimeoutMs,
    },
    thresholds: {
      completionP95BudgetMs,
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

  for (let round = 0; round < coldRepeat; round += 1) {
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

  try {
    const plan = representativeRequestPlan(session.main);

    for (const request of plan) {
      const sample = await timedRequest(session.client, request);

      validationFailures[request.label] += Number(!validResult(request.label, sample.result));
    }

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
      background: statsForValues(contention.backgroundRequestAges),
    },
    cache: {
      hitRates: cacheHitRates(cacheStats),
      raw: cacheStats,
    },
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
    },
    validationFailures: {
      byRequest: validationFailures,
      total: Object.values(validationFailures).reduce((sum, count) => sum + count, 0),
    },
  };
}

async function openSession(root) {
  const client = new LspClient(command, root, {
    env: process.env,
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
  let validationFailures = 0;

  for (let round = 0; round < contentionRepeat; round += 1) {
    const background = [];
    const sourceLines = currentText.split("\n").length;

    for (let index = 0; index < backgroundBurst; index += 1) {
      const startLine = index % Math.max(1, Math.min(sourceLines - 1, backgroundBurst));
      const handle = client.startRequest("textDocument/inlayHint", {
        textDocument: { uri: main.uri },
        range: {
          start: { line: startLine, character: 0 },
          end: { line: sourceLines + 1, character: 0 },
        },
      });

      background.push(observeRequest(handle));
    }
    backgroundRequests += background.length;

    await delay(contentionDelayMs);

    version += 1;
    const probe = `profile_contention_probe_${String(round + 1).padStart(2, "0")}`;
    currentText = `${main.text}\nlocal ${probe} = profile_primary.label\n`;
    client.notify("textDocument/didChange", {
      textDocument: { uri: main.uri, version },
      contentChanges: [{ text: currentText }],
    });

    const completion = observeRequest(
      client.startRequest("textDocument/completion", {
        textDocument: { uri: main.uri },
        position: positionAfter(currentText, `${probe} = profile_primary.la`),
        context: { triggerKind: 1 },
      }),
    );
    const fresh = await completion;
    const items = completionItems(fresh.message.result);
    const freshCompletion = !fresh.message.error && items.some((item) => item.label === "label");

    completionRequestAges.push(fresh.ageMs);
    freshCompletionCount += Number(freshCompletion);
    validationFailures += Number(!freshCompletion);

    // A response received before the new-version completion may have already
    // left the server before it processed didChange. The validated completion
    // is the protocol barrier that proves the change has been observed.
    const responses = await Promise.all(background);
    for (const response of responses) {
      backgroundRequestAges.push(response.ageMs);

      const code = response.message.error?.code;
      if (code === -32801) {
        contentModifiedResponses += 1;
        cancelledRequests += 1;
      } else if (code === -32800) {
        requestCancelledResponses += 1;
        cancelledRequests += 1;
      }

      if (response.message.result !== undefined && response.receivedAt > fresh.receivedAt) {
        resultsAfterChange += 1;
        staleResultCount += 1;
      }
    }
  }

  return {
    completionRequestAges,
    backgroundRequestAges,
    backgroundRequests,
    cancelledRequests,
    contentModifiedResponses,
    requestCancelledResponses,
    resultsAfterChange,
    staleResultCount,
    freshCompletionCount,
    validationFailures,
  };
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
  return Object.fromEntries(
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
  );
}

function regressionFailures(report) {
  const failures = [];

  for (const fixture of report.fixtures) {
    const warmCompletionP95 = fixture.latency.completion.warm.p95Ms;
    const contentionCompletionP95 = fixture.requestAge.completionAfterChange.p95Ms;

    if (warmCompletionP95 > report.thresholds.completionP95BudgetMs) {
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
    if (fixture.contention.staleResultCount > report.thresholds.maximumStaleResults) {
      failures.push(
        `${fixture.name}: observed ${fixture.contention.staleResultCount} stale background results`,
      );
    }
    if (fixture.validationFailures.total > report.thresholds.maximumValidationFailures) {
      failures.push(
        `${fixture.name}: ${fixture.validationFailures.total} semantic result validations failed ` +
          `(${JSON.stringify(fixture.validationFailures.byRequest)})`,
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

function stringListEnv(name, fallback) {
  const value = process.env[name];

  return value ? value.split(",").map((item) => item.trim()).filter(Boolean) : fallback;
}
