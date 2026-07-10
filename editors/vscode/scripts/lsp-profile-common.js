"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const path = require("path");
const { pathToFileURL } = require("url");

class LspClient {
  constructor(command, root, options = {}) {
    this.command = command;
    this.root = root;
    this.env = options.env || process.env;
    this.buffer = Buffer.alloc(0);
    this.nextId = 1;
    this.pending = new Map();
    this.diagnostics = [];
    this.diagnosticWaiters = [];
    this.stderr = "";
  }

  start() {
    this.child = childProcess.spawn(this.command, ["lsp"], {
      cwd: this.root,
      env: { ...this.env },
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
    });

    this.child.stdout.on("data", (chunk) => {
      this.buffer = Buffer.concat([this.buffer, chunk]);
      this.readMessages();
    });
    this.child.stderr.on("data", (chunk) => {
      const text = chunk.toString("utf8");

      this.stderr += text;
      if (process.env.TUA_PROFILE_STREAM) {
        process.stderr.write(text);
      }
    });

    return new Promise((resolve, reject) => {
      this.child.once("spawn", resolve);
      this.child.once("error", reject);
    });
  }

  done() {
    return new Promise((resolve, reject) => {
      this.child.once("exit", (code) => {
        if (code === 0) {
          resolve();
        } else {
          reject(new Error(`LSP exited with ${code}${this.stderr ? `\n${this.stderr}` : ""}`));
        }
      });
    });
  }

  request(method, params) {
    const id = this.nextId++;

    this.send({ jsonrpc: "2.0", id, method, params });

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`${method} timed out`));
      }, 10000);

      this.pending.set(id, (message) => {
        clearTimeout(timeout);

        if (message.error) {
          reject(new Error(`${method} failed: ${JSON.stringify(message.error)}`));
        } else {
          resolve(message.result);
        }
      });
    });
  }

  notify(method, params) {
    this.send({ jsonrpc: "2.0", method, params });
  }

  clearDiagnostics() {
    this.diagnostics = [];
  }

  waitForDiagnostics(uri, options = {}) {
    const timeoutMs = options.timeoutMs || 2000;
    const allowTimeout = Boolean(options.allowTimeout);
    const existingIndex = this.diagnostics.findIndex((message) => diagnosticUri(message) === uri);

    if (existingIndex !== -1) {
      const [message] = this.diagnostics.splice(existingIndex, 1);

      return Promise.resolve(message);
    }

    return new Promise((resolve, reject) => {
      const waiter = {
        uri,
        resolve,
        reject,
        allowTimeout,
        timer: null,
      };

      waiter.timer = setTimeout(() => {
        this.diagnosticWaiters = this.diagnosticWaiters.filter((item) => item !== waiter);
        if (allowTimeout) {
          resolve(null);
        } else {
          reject(new Error(`publishDiagnostics for ${uri} timed out`));
        }
      }, timeoutMs);

      this.diagnosticWaiters.push(waiter);
    });
  }

  send(message) {
    const body = JSON.stringify(message);
    const length = Buffer.byteLength(body, "utf8");

    this.child.stdin.write(`Content-Length: ${length}\r\n\r\n${body}`);
  }

  readMessages() {
    while (true) {
      const headerEnd = this.buffer.indexOf("\r\n\r\n");
      if (headerEnd === -1) {
        return;
      }

      const header = this.buffer.subarray(0, headerEnd).toString("ascii");
      const lengthMatch = /^Content-Length: (\d+)$/im.exec(header);
      if (!lengthMatch) {
        throw new Error(`missing Content-Length header:\n${header}`);
      }

      const bodyLength = Number(lengthMatch[1]);
      const bodyStart = headerEnd + 4;
      const bodyEnd = bodyStart + bodyLength;
      if (this.buffer.length < bodyEnd) {
        return;
      }

      const body = this.buffer.subarray(bodyStart, bodyEnd).toString("utf8");
      this.buffer = this.buffer.subarray(bodyEnd);
      this.handleMessage(JSON.parse(body));
    }
  }

  handleMessage(message) {
    if (message.method === "textDocument/publishDiagnostics") {
      this.handleDiagnostics(message);
      return;
    }

    const handler = this.pending.get(message.id);
    if (!handler) {
      return;
    }

    this.pending.delete(message.id);
    handler(message);
  }

  handleDiagnostics(message) {
    const uri = diagnosticUri(message);
    const waiter = this.diagnosticWaiters.find((item) => item.uri === uri);

    if (!waiter) {
      this.diagnostics.push(message);
      return;
    }

    clearTimeout(waiter.timer);
    this.diagnosticWaiters = this.diagnosticWaiters.filter((item) => item !== waiter);
    waiter.resolve(message);
  }
}

async function profileLspProject(options) {
  const command = options.command;
  const root = options.root;
  const sourceRoot = options.sourceRoot || path.join(root, "src");
  const repeat = options.repeat || 5;
  const warmup = options.warmup || 0;
  const perFile = Boolean(options.perFile);
  const includeDiagnostics = Boolean(options.includeDiagnostics);
  const client = new LspClient(command, root, { env: options.env || process.env });
  const files = sourceFiles(sourceRoot);
  const main = files.find((file) => file.path.endsWith(`${path.sep}main.tua`)) || files[0];

  if (!main) {
    throw new Error(`no .tua or .lua files found under ${sourceRoot}`);
  }

  await client.start();
  await client.request("initialize", {
    processId: process.pid,
    rootUri: pathToFileURL(root).href,
    capabilities: {},
    workspaceFolders: [{ uri: pathToFileURL(root).href, name: path.basename(root) }],
  });
  client.notify("initialized", {});

  for (const file of files) {
    client.notify("textDocument/didOpen", {
      textDocument: {
        uri: file.uri,
        languageId: file.path.endsWith(".lua") ? "lua" : "tua",
        version: 1,
        text: file.text,
      },
    });
  }

  await delay(20);
  client.clearDiagnostics();

  const requests = requestPlan(main, files, { perFile, sourceRoot });
  const timings = [];

  for (let round = 0; round < warmup; round += 1) {
    await profileRequestRound(client, requests, null);
  }

  for (let round = 0; round < repeat; round += 1) {
    await profileRequestRound(client, requests, timings);
  }

  if (includeDiagnostics && main.path.endsWith(".tua")) {
    for (let round = 0; round < warmup; round += 1) {
      await profileTypingDiagnostics(client, main, round + 2, null);
    }
    for (let round = 0; round < repeat; round += 1) {
      await profileTypingDiagnostics(client, main, warmup + round + 2, timings);
    }
  }

  const stats = await client.request("tua/cacheStats", null);

  await client.request("shutdown", null);
  client.notify("exit", null);
  await client.done();

  return {
    root,
    files,
    timings,
    stats,
    stderr: client.stderr,
  };
}

async function profileRequestRound(client, requests, timings) {
  for (const item of requests) {
    const params = item.fromCompletion
      ? await completionResolveParams(client, item.fromCompletion)
      : item.params;

    if (!params) {
      continue;
    }

    const started = process.hrtime.bigint();
    await client.request(item.method, params);
    const elapsedMs = Number(process.hrtime.bigint() - started) / 1_000_000;

    if (timings) {
      timings.push({ label: item.label, elapsedMs });
    }
  }
}

async function profileTypingDiagnostics(client, file, version, timings) {
  const edited = `${file.text}\n-- tua speed profile ${version}\n`;

  client.clearDiagnostics();
  const started = process.hrtime.bigint();
  client.notify("textDocument/didChange", {
    textDocument: {
      uri: file.uri,
      version,
    },
    contentChanges: [{ text: edited }],
  });

  await client.waitForDiagnostics(file.uri, { timeoutMs: 2000 });
  const firstMs = Number(process.hrtime.bigint() - started) / 1_000_000;

  if (timings) {
    timings.push({ label: "diagnostics:firstPublish", elapsedMs: firstMs });
  }

  const settled = await client.waitForDiagnostics(file.uri, {
    timeoutMs: 800,
    allowTimeout: true,
  });

  if (settled && timings) {
    const settledMs = Number(process.hrtime.bigint() - started) / 1_000_000;

    timings.push({ label: "diagnostics:settledPublish", elapsedMs: settledMs });
  }
}

async function completionResolveParams(client, source) {
  const response = await client.request("textDocument/completion", source.params);
  const items = completionItems(response);

  for (const label of source.preferredLabels || []) {
    const item = items.find((candidate) => candidate.label === label && candidate.data);

    if (item) {
      return item;
    }
  }

  return (
    items.find((item) => item.data) ||
    items[0] ||
    syntheticCompletionItem(source)
  );
}

function syntheticCompletionItem(source) {
  const label = (source.preferredLabels || [])[0];

  if (!label) {
    return null;
  }

  return {
    label,
    data: {
      uri: source.params.textDocument.uri,
      label,
      kind: "field",
      position: source.params.position,
    },
  };
}

function completionItems(response) {
  if (Array.isArray(response)) {
    return response;
  }

  if (response && Array.isArray(response.items)) {
    return response.items;
  }

  return [];
}

function requestPlan(main, files, options = {}) {
  const workspaceQuery = path.basename(main.path, path.extname(main.path));
  const hoverNeedle = firstNeedle(main.text, ["makePlayer", "createWorld", "player", "Player"]);
  const completionNeedle = firstNeedle(main.text, ["player.", "Player.", "World.", "world."]);
  const signatureNeedle = firstNeedle(main.text, [
    "setFilter(",
    "makePlayer(",
    "createWorld(",
    "spawnEnemy(",
  ]);
  const definitionNeedle = firstNeedle(main.text, ["require(\"", "Player.", "World."]);
  const referencesNeedle = firstNeedle(main.text, ["makePlayer", "createWorld", "player", "Player"]);
  const rangeEnd = main.text.split("\n").length + 1;
  const plan = [
    {
      label: "documentSymbol",
      method: "textDocument/documentSymbol",
      params: { textDocument: { uri: main.uri } },
    },
    {
      label: "semanticTokens",
      method: "textDocument/semanticTokens/full",
      params: { textDocument: { uri: main.uri } },
    },
    {
      label: "workspaceSymbol",
      method: "workspace/symbol",
      params: { query: workspaceQuery },
    },
    {
      label: "documentLink",
      method: "textDocument/documentLink",
      params: { textDocument: { uri: main.uri } },
    },
    {
      label: "inlayHint",
      method: "textDocument/inlayHint",
      params: {
        textDocument: { uri: main.uri },
        range: {
          start: { line: 0, character: 0 },
          end: { line: rangeEnd, character: 0 },
        },
      },
    },
  ];

  if (hoverNeedle) {
    plan.push({
      label: "hover",
      method: "textDocument/hover",
      params: {
        textDocument: { uri: main.uri },
        position: positionAt(main.text, hoverNeedle),
      },
    });
  }

  if (completionNeedle) {
    const completionParams = {
      textDocument: { uri: main.uri },
      position: positionAfter(main.text, completionNeedle),
    };

    plan.push({
      label: "completion",
      method: "textDocument/completion",
      params: completionParams,
    });
    plan.push({
      label: "completionResolve",
      method: "completionItem/resolve",
      fromCompletion: {
        params: completionParams,
        preferredLabels: ["setFilter", "makePlayer", "createWorld", "spawnEnemy", "hp"],
      },
    });
  }

  if (signatureNeedle) {
    plan.push({
      label: "signatureHelp",
      method: "textDocument/signatureHelp",
      params: {
        textDocument: { uri: main.uri },
        position: positionAfter(main.text, signatureNeedle),
      },
    });
  }

  if (definitionNeedle) {
    const definitionParams = {
      textDocument: { uri: main.uri },
      position: positionAfter(main.text, definitionNeedle),
    };

    plan.push({
      label: "definition",
      method: "textDocument/definition",
      params: definitionParams,
    });
    plan.push({
      label: "definitionAgain",
      method: "textDocument/definition",
      params: definitionParams,
    });
  }

  if (referencesNeedle) {
    const renamePosition = positionAt(main.text, referencesNeedle);

    plan.push({
      label: "references",
      method: "textDocument/references",
      params: {
        textDocument: { uri: main.uri },
        position: renamePosition,
        context: { includeDeclaration: true },
      },
    });
    plan.push({
      label: "prepareRename",
      method: "textDocument/prepareRename",
      params: {
        textDocument: { uri: main.uri },
        position: renamePosition,
      },
    });
    plan.push({
      label: "rename",
      method: "textDocument/rename",
      params: {
        textDocument: { uri: main.uri },
        position: renamePosition,
        newName: "__tuaProfileRename",
      },
    });
  }

  if (options.perFile) {
    for (const file of files.filter((candidate) => candidate.path.endsWith(".tua"))) {
      pushPerFileRequests(plan, file, options.sourceRoot);
    }
  } else {
    for (const file of files.filter((candidate) => candidate.path.endsWith(".tua")).slice(0, 2)) {
      plan.push({
        label: `symbols:${path.basename(file.path)}`,
        method: "textDocument/documentSymbol",
        params: { textDocument: { uri: file.uri } },
      });
    }
  }

  return plan;
}

function pushPerFileRequests(plan, file, sourceRoot) {
  const label = relativeLabel(sourceRoot, file.path);
  const rangeEnd = file.text.split("\n").length + 1;

  plan.push({
    label: `documentSymbol:${label}`,
    method: "textDocument/documentSymbol",
    params: { textDocument: { uri: file.uri } },
  });
  plan.push({
    label: `semanticTokens:${label}`,
    method: "textDocument/semanticTokens/full",
    params: { textDocument: { uri: file.uri } },
  });
  plan.push({
    label: `inlayHint:${label}`,
    method: "textDocument/inlayHint",
    params: {
      textDocument: { uri: file.uri },
      range: {
        start: { line: 0, character: 0 },
        end: { line: rangeEnd, character: 0 },
      },
    },
  });
}

function sourceFiles(src) {
  const files = [];

  walk(src, (filename) => {
    if (!filename.endsWith(".tua") && !filename.endsWith(".lua")) {
      return;
    }

    files.push({
      path: filename,
      uri: pathToFileURL(filename).href,
      text: fs.readFileSync(filename, "utf8"),
    });
  });

  files.sort((left, right) => left.path.localeCompare(right.path));
  return files;
}

function walk(dir, visit) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const filename = path.join(dir, entry.name);

    if (entry.isDirectory()) {
      walk(filename, visit);
    } else {
      visit(filename);
    }
  }
}

function timingsByLabel(timings) {
  const byLabel = new Map();

  for (const timing of timings) {
    const list = byLabel.get(timing.label) || [];

    list.push(timing.elapsedMs);
    byLabel.set(timing.label, list);
  }

  return byLabel;
}

function statsForValues(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const avg = sorted.reduce((sum, value) => sum + value, 0) / sorted.length;

  return {
    avgMs: avg,
    p50Ms: percentile(sorted, 0.5),
    p95Ms: percentile(sorted, 0.95),
    minMs: sorted[0],
    maxMs: sorted[sorted.length - 1],
    runs: sorted.length,
  };
}

function printRequestReport(root, timings, stats) {
  console.log(`\n# ${path.basename(root)} (${root})`);
  for (const [label, values] of timingsByLabel(timings)) {
    const metric = statsForValues(values);

    console.log(
      `${label.padEnd(32)} avg ${metric.avgMs.toFixed(2)}ms  ` +
        `p95 ${metric.p95Ms.toFixed(2)}ms  min ${metric.minMs.toFixed(2)}ms  ` +
        `max ${metric.maxMs.toFixed(2)}ms`,
    );
  }

  console.log(formatCacheStats(stats));
}

function formatCacheStats(stats) {
  return (
    `cache parse h/m ${stats.parse.hits}/${stats.parse.misses}, ` +
    `hir h/m ${stats.hir.hits}/${stats.hir.misses}, ` +
    `scope h/m ${stats.scope.hits}/${stats.scope.misses}, ` +
    `types h/m ${stats.typeEnvironment.hits}/${stats.typeEnvironment.misses}, ` +
    `diagnostics h/m ${stats.diagnostics.hits}/${stats.diagnostics.misses}, ` +
    `symbols h/m ${stats.symbolIndex.hits}/${stats.symbolIndex.misses}, ` +
    `invalidations ${stats.invalidations}`
  );
}

function percentile(sortedValues, percentileValue) {
  if (sortedValues.length === 0) {
    return 0;
  }

  const index = Math.ceil(sortedValues.length * percentileValue) - 1;

  return sortedValues[Math.max(0, Math.min(index, sortedValues.length - 1))];
}

function firstNeedle(source, needles) {
  return needles.find((needle) => source.includes(needle));
}

function positionAt(source, needle) {
  const offset = source.indexOf(needle);
  if (offset === -1) {
    throw new Error(`needle not found in source: ${needle}`);
  }

  const before = source.slice(0, offset);
  const lines = before.split("\n");

  return {
    line: lines.length - 1,
    character: lines[lines.length - 1].length,
  };
}

function positionAfter(source, needle) {
  const position = positionAt(source, needle);

  position.character += needle.length;
  return position;
}

function relativeLabel(root, filename) {
  if (!root) {
    return path.basename(filename);
  }

  return path.relative(root, filename).replaceAll(path.sep, "/");
}

function diagnosticUri(message) {
  return message && message.params && message.params.uri;
}

function delay(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function resolveCommand(command) {
  if (!command.includes(path.sep) || path.isAbsolute(command)) {
    return command;
  }

  return path.resolve(command);
}

module.exports = {
  LspClient,
  completionResolveParams,
  delay,
  formatCacheStats,
  pathToFileURL,
  printRequestReport,
  profileLspProject,
  requestPlan,
  resolveCommand,
  sourceFiles,
  statsForValues,
  timingsByLabel,
};
