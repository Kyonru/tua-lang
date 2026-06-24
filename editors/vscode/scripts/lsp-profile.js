"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const path = require("path");
const { pathToFileURL } = require("url");

const command = resolveCommand(process.argv[2] || "target/debug/tua");
const roots = (process.argv.slice(3).length ? process.argv.slice(3) : ["benchmarks/lsp/small"]).map((root) =>
  path.resolve(root),
);
const repeat = Number(process.env.TUA_PROFILE_REPEAT || "5");

async function profileProject(root) {
  const client = new LspClient(command, root);
  const files = sourceFiles(path.join(root, "src"));
  const main = files.find((file) => file.path.endsWith(`${path.sep}main.tua`)) || files[0];

  if (!main) {
    throw new Error(`no .tua or .lua files found under ${root}`);
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

  const requests = requestPlan(main, files);
  const timings = [];

  for (let round = 0; round < repeat; round += 1) {
    for (const item of requests) {
      const started = process.hrtime.bigint();
      await client.request(item.method, item.params);
      const elapsedMs = Number(process.hrtime.bigint() - started) / 1_000_000;

      timings.push({ label: item.label, elapsedMs });
    }
  }

  const stats = await client.request("tua/cacheStats", null);

  await client.request("shutdown", null);
  client.notify("exit", null);
  await client.done();

  printReport(root, timings, stats);

  if (process.env.TUA_PROFILE && client.stderr.trim()) {
    console.error(`\n# ${path.basename(root)} trace`);
    console.error(client.stderr.trim());
  }
}

function requestPlan(main, files) {
  const workspaceQuery = path.basename(main.path, path.extname(main.path));
  const hoverNeedle = firstNeedle(main.text, ["makePlayer", "createWorld", "player", "Player"]);
  const completionNeedle = firstNeedle(main.text, ["player.", "Player.", "World.", "world."]);
  const definitionNeedle = firstNeedle(main.text, ["require(\"", "Player.", "World."]);
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
    plan.push({
      label: "completion",
      method: "textDocument/completion",
      params: {
        textDocument: { uri: main.uri },
        position: positionAfter(main.text, completionNeedle),
      },
    });
  }

  if (definitionNeedle) {
    plan.push({
      label: "definition",
      method: "textDocument/definition",
      params: {
        textDocument: { uri: main.uri },
        position: positionAfter(main.text, definitionNeedle),
      },
    });
  }

  for (const file of files.filter((file) => file.path.endsWith(".tua")).slice(0, 2)) {
    plan.push({
      label: `symbols:${path.basename(file.path)}`,
      method: "textDocument/documentSymbol",
      params: { textDocument: { uri: file.uri } },
    });
  }

  return plan;
}

function printReport(root, timings, stats) {
  const byLabel = new Map();

  for (const timing of timings) {
    const list = byLabel.get(timing.label) || [];

    list.push(timing.elapsedMs);
    byLabel.set(timing.label, list);
  }

  console.log(`\n# ${path.basename(root)} (${root})`);
  for (const [label, values] of byLabel) {
    values.sort((left, right) => left - right);
    const avg = values.reduce((sum, value) => sum + value, 0) / values.length;
    const min = values[0];
    const max = values[values.length - 1];
    const p95 = percentile(values, 0.95);

    console.log(
      `${label.padEnd(24)} avg ${avg.toFixed(2)}ms  p95 ${p95.toFixed(2)}ms  min ${min.toFixed(2)}ms  max ${max.toFixed(2)}ms`,
    );
  }

  console.log(
    `cache parse h/m ${stats.parse.hits}/${stats.parse.misses}, ` +
      `types h/m ${stats.typeEnvironment.hits}/${stats.typeEnvironment.misses}, ` +
      `diagnostics h/m ${stats.diagnostics.hits}/${stats.diagnostics.misses}, ` +
      `symbols h/m ${stats.symbolIndex.hits}/${stats.symbolIndex.misses}, ` +
      `invalidations ${stats.invalidations}`,
  );
}

function percentile(sortedValues, percentileValue) {
  if (sortedValues.length === 0) {
    return 0;
  }

  const index = Math.ceil(sortedValues.length * percentileValue) - 1;

  return sortedValues[Math.max(0, Math.min(index, sortedValues.length - 1))];
}

class LspClient {
  constructor(command, root) {
    this.command = command;
    this.root = root;
    this.buffer = Buffer.alloc(0);
    this.nextId = 1;
    this.pending = new Map();
    this.stderr = "";
  }

  start() {
    this.child = childProcess.spawn(this.command, ["lsp"], {
      cwd: this.root,
      env: { ...process.env },
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
      return;
    }

    const handler = this.pending.get(message.id);
    if (!handler) {
      return;
    }

    this.pending.delete(message.id);
    handler(message);
  }
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

function resolveCommand(command) {
  if (!command.includes(path.sep) || path.isAbsolute(command)) {
    return command;
  }

  return path.resolve(command);
}

(async () => {
  for (const root of roots) {
    await profileProject(root);
  }
})().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});
