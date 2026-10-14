"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const path = require("path");
const { pathToFileURL } = require("url");
const { resolveCommand } = require("./lsp-profile-common");

const command = resolveCommand(process.argv[2] || "target/debug/tua");
const root = path.resolve(process.argv[3] || "examples/features");
const src = path.join(root, "src");
const child = childProcess.spawn(command, ["lsp"], {
  cwd: root,
  env: {
    TUA_COMPLETION_STATS: "0",
    ...process.env,
  },
  stdio: ["pipe", "pipe", "pipe"],
  windowsHide: true,
});

let buffer = Buffer.alloc(0);
let nextId = 1;
let completed = false;
let stderr = "";
const pending = new Map();
const pendingMethods = new Map();
const requestedTimeoutMs = Number(process.env.TUA_LSP_FEATURES_SMOKE_TIMEOUT_MS || 90000);
const timeoutMs = Number.isFinite(requestedTimeoutMs) && requestedTimeoutMs > 0
  ? requestedTimeoutMs
  : 90000;

const sources = fs
  .readdirSync(src)
  .filter((file) => file.endsWith(".tua"))
  .sort()
  .map((file) => {
    const filename = path.join(src, file);

    return {
      file,
      path: filename,
      uri: pathToFileURL(filename).href,
      text: readSource(filename),
    };
  });

const inference = sources.find((source) => source.file === "inference.tua");
if (!inference) {
  fail("examples/features/src/inference.tua is missing");
}
const modules = sources.find((source) => source.file === "modules.tua");
if (!modules) {
  fail("examples/features/src/modules.tua is missing");
}
const functions = sources.find((source) => source.file === "functions.tua");
if (!functions) {
  fail("examples/features/src/functions.tua is missing");
}
const imports = sources.find((source) => source.file === "imports.tua");
if (!imports) {
  fail("examples/features/src/imports.tua is missing");
}
const cheatsheet = sources.find((source) => source.file === "lua_cheatsheet.tua");
if (!cheatsheet) {
  fail("examples/features/src/lua_cheatsheet.tua is missing");
}
const sprites = sources.find((source) => source.file === "sprites.tua");
if (!sprites) {
  fail("examples/features/src/sprites.tua is missing");
}
const legacy = {
  path: path.join(src, "legacy.lua"),
  uri: pathToFileURL(path.join(src, "legacy.lua")).href,
  text: readSource(path.join(src, "legacy.lua")),
};

const timeout = setTimeout(() => {
  child.kill();
  fail(
    `timed out after ${timeoutMs}ms waiting for features LSP smoke responses; pending=${[
      ...pendingMethods.values(),
    ].join(",")}`,
  );
}, timeoutMs);

child.stderr.on("data", (chunk) => {
  stderr += chunk.toString("utf8");
});

child.stdout.on("data", (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);
  readMessages();
});

child.on("error", (error) => {
  fail(`failed to start ${command}: ${error.message}`);
});

child.on("exit", (code) => {
  clearTimeout(timeout);

  if (!completed || code !== 0) {
    fail(`features LSP smoke exited with status ${code}${stderr ? `\nstderr:\n${stderr}` : ""}`);
  }
});

request("initialize", {
  processId: process.pid,
  rootUri: pathToFileURL(root).href,
  capabilities: {},
  workspaceFolders: [
    {
      uri: pathToFileURL(root).href,
      name: "features",
    },
  ],
}, () => {
  send({ jsonrpc: "2.0", method: "initialized", params: {} });

  for (const source of [inference, imports, modules, cheatsheet, sprites]) {
    send({
      jsonrpc: "2.0",
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          uri: source.uri,
          languageId: "tua",
          version: 1,
          text: source.text,
        },
      },
    });
  }

  const expected = [];

  expected.push(
    request("textDocument/documentSymbol", { textDocument: { uri: inference.uri } }, (message) => {
      const names = (message.result || []).map((item) => item.name);
      if (!names.includes("Player") || !names.includes("makePlayer")) {
        fail(`document symbols missing expected names: ${names.join(", ")}`);
      }
    }),
  );

  expected.push(
    request("textDocument/semanticTokens/full", { textDocument: { uri: inference.uri } }, (message) => {
      if (!Array.isArray(message.result?.data) || message.result.data.length === 0) {
        fail("semantic tokens returned no data");
      }
      if (typeof message.result?.resultId !== "string") {
        fail("semantic tokens returned no resultId");
      }
    }).then((message) =>
      request(
        "textDocument/semanticTokens/full/delta",
        {
          textDocument: { uri: inference.uri },
          previousResultId: message.result.resultId,
        },
        (delta) => {
          if (!Array.isArray(delta.result?.edits)) {
            fail("semantic token delta returned no edits array");
          }
        },
      ),
    ),
  );

  expected.push(
    request(
      "textDocument/inlayHint",
      {
        textDocument: { uri: inference.uri },
        range: {
          start: { line: 0, character: 0 },
          end: { line: inference.text.split("\n").length + 1, character: 0 },
        },
      },
      (message) => {
        const labels = (message.result || []).map((item) => item.label);
        if (!labels.includes(": number") || !labels.includes("player:")) {
          fail(`inlay hints missing expected labels: ${labels.join(", ")}`);
        }
      },
    ),
  );

  expected.push(
    request(
      "textDocument/hover",
      {
        textDocument: { uri: inference.uri },
        position: positionAt(inference.text, "statusText(typedPlayer"),
      },
      (message) => {
        const value = message.result?.contents?.value || "";
        if (!value.includes("function statusText(alive: boolean): string")) {
          fail(`hover did not include inferred return type: ${value}`);
        }
      },
    ),
  );

  expected.push(
    request(
      "textDocument/completion",
      {
        textDocument: { uri: inference.uri },
        position: positionAfter(inference.text, "typedPlayer."),
      },
      (message) => {
        const labels = (message.result || []).map((item) => item.label).sort();
        if (!labels.includes("hp") || !labels.includes("pos")) {
          fail(`completion missing typedPlayer fields: ${labels.join(", ")}`);
        }
      },
    ),
  );

  expected.push(
    request(
      "textDocument/signatureHelp",
      {
        textDocument: { uri: inference.uri },
        position: positionAfter(inference.text, "move(typedPlayer, "),
      },
      (message) => {
        const signature = message.result?.signatures?.[0]?.label || "";
        const activeParameter = message.result?.activeParameter;

        if (!signature.includes("function move(player: Player, delta: Vec2): Player")) {
          fail(`signature help missing move signature: ${signature}`);
        }

        if (activeParameter !== 1) {
          fail(`signature help expected active parameter 1, got ${activeParameter}`);
        }
      },
    ),
  );

  expected.push(
    request(
      "textDocument/documentHighlight",
      {
        textDocument: { uri: inference.uri },
        position: positionAt(inference.text, "typedPlayer.hp"),
      },
      (message) => {
        const labels = (message.result || []).map((item) =>
          sourceTextFromRange(inference.text, item.range),
        );

        if (labels.filter((label) => label === "typedPlayer").length < 4) {
          fail(`document highlights missing typedPlayer usages: ${labels.join(", ")}`);
        }
      },
    ),
  );

  expected.push(
    request("workspace/symbol", { query: "inference" }, (message) => {
      const names = (message.result || []).map((item) => item.name);
      if (!names.includes("inference")) {
        fail(`workspace symbols missing inference module: ${names.join(", ")}`);
      }
    }),
  );

  expected.push(
    request("textDocument/documentLink", { textDocument: { uri: modules.uri } }, (message) => {
      const links = message.result || [];
      const legacyLink = links.find((link) => sourceTextFromRange(modules.text, link.range) === "legacy");

      if (!legacyLink) {
        fail(`document links did not include require("legacy"): ${JSON.stringify(links)}`);
      }

      if (legacyLink.target !== legacy.uri) {
        fail(`document link expected ${legacy.uri}, got ${legacyLink.target}`);
      }
    }),
  );

  expected.push(
    request(
      "textDocument/definition",
      {
        textDocument: { uri: modules.uri },
        position: positionAt(modules.text, "legacy\")"),
      },
      (message) => {
        if (message.result?.uri !== legacy.uri) {
          fail(`require definition expected ${legacy.uri}, got ${message.result?.uri}`);
        }
      },
    ),
  );

  expected.push(
    request(
      "textDocument/definition",
      {
        textDocument: { uri: inference.uri },
        position: positionAfter(inference.text, "Legacy."),
      },
      (message) => {
        if (message.result?.uri !== legacy.uri) {
          fail(`definition expected ${legacy.uri}, got ${message.result?.uri}`);
        }

        const target = sourceTextFromRange(legacy.text, message.result?.range);
        if (target !== "describe") {
          fail(`definition expected legacy export "describe", got ${JSON.stringify(target)}`);
        }
      },
    ),
  );

  expected.push(
    request(
      "textDocument/definition",
      {
        textDocument: { uri: modules.uri },
        position: positionAt(modules.text, "makePlayer"),
      },
      (message) => {
        if (message.result?.uri !== functions.uri) {
          fail(`unopened module definition expected ${functions.uri}, got ${message.result?.uri}`);
        }

        const target = sourceTextFromRange(functions.text, message.result?.range);
        if (target !== "makePlayer") {
          fail(`unopened module definition expected "makePlayer", got ${JSON.stringify(target)}`);
        }
      },
    ),
  );

  expected.push(
    request(
      "textDocument/completion",
      {
        textDocument: { uri: cheatsheet.uri },
        position: positionAfter(cheatsheet.text, "outside_function."),
      },
      (message) => {
        const labels = (message.result || []).map((item) => item.label).sort();
        if (!labels.includes("length") || !labels.includes("x") || !labels.includes("y")) {
          fail(`cheatsheet completion missing MetaVector fields: ${labels.join(", ")}`);
        }
      },
    ),
  );

  expected.push(
    request(
      "textDocument/definition",
      {
        textDocument: { uri: cheatsheet.uri },
        position: positionAfter(cheatsheet.text, "outside_function."),
      },
      (message) => {
        if (message.result?.uri !== cheatsheet.uri) {
          fail(`cheatsheet field definition expected ${cheatsheet.uri}, got ${message.result?.uri}`);
        }

        const target = sourceTextFromRange(cheatsheet.text, message.result?.range);
        if (target !== "x") {
          fail(`cheatsheet field definition expected "x", got ${JSON.stringify(target)}`);
        }
      },
    ),
  );

  expected.push(
    request(
      "tua/inspect",
      {
        textDocument: { uri: sprites.uri },
        position: positionAt(sprites.text, "Player = spriteSheet"),
      },
      (message) => {
        const sprite = message.result?.items?.[0];

        if (message.result?.documentVersion !== 1 || sprite?.kind !== "spriteSheet") {
          fail(`sprite inspection returned invalid snapshot metadata: ${JSON.stringify(message.result)}`);
        }
        if (sprite.name !== "Player" || sprite.image?.status !== "valid") {
          fail(`sprite inspection returned invalid declaration metadata: ${JSON.stringify(sprite)}`);
        }
        const animations = Object.fromEntries(
          (sprite.animations || []).map((animation) => [animation.name, animation]),
        );
        const frame = sprite.frame;
        const orange = animations.orange;
        const blue = animations.blue;
        const green = animations.green;

        if (
          frame?.width !== 128 ||
          frame.height !== 128 ||
          frame.left !== 0 ||
          frame.top !== 0 ||
          frame.border !== 0 ||
          sprite.animations?.length !== 3 ||
          orange?.column !== 1 ||
          orange.durations !== "1" ||
          blue?.row !== 1 ||
          blue.onLoop !== "pauseAtEnd" ||
          blue.loop !== false ||
          green?.frame?.width !== 64 ||
          green.frame.height !== 64 ||
          !green.durations?.includes("0.5") ||
          !green.onLoop?.startsWith("function(")
        ) {
          fail(`sprite inspection returned invalid animation metadata: ${JSON.stringify(sprite)}`);
        }
      },
    ),
  );

  Promise.all(expected)
    .then(testClosedModuleAndFreshInlayHints)
    .then(() => {
      request("shutdown", null, () => {
        completed = true;
        console.log("features LSP smoke passed");
        send({ jsonrpc: "2.0", method: "exit", params: null });
      });
    });
});

async function testClosedModuleAndFreshInlayHints() {
  send({
    jsonrpc: "2.0",
    method: "textDocument/didOpen",
    params: {
      textDocument: {
        uri: functions.uri,
        languageId: "tua",
        version: 1,
        text: functions.text,
      },
    },
  });
  send({
    jsonrpc: "2.0",
    method: "textDocument/didClose",
    params: { textDocument: { uri: functions.uri } },
  });

  await request(
    "textDocument/definition",
    {
      textDocument: { uri: modules.uri },
      position: positionAt(modules.text, "makePlayer"),
    },
    (message) => {
      if (message.result?.uri !== functions.uri) {
        fail(`closed module definition expected ${functions.uri}, got ${message.result?.uri}`);
      }
    },
  );

  const changedText = inference.text.replace(
    "\nreturn {\n",
    "\nlocal smokeInlay = 42\n\nreturn {\n",
  );
  const changedLine = positionAt(changedText, "smokeInlay").line;

  send({
    jsonrpc: "2.0",
    method: "textDocument/didChange",
    params: {
      textDocument: { uri: inference.uri, version: 2 },
      contentChanges: [{ text: changedText }],
    },
  });

  await request(
    "textDocument/inlayHint",
    {
      textDocument: { uri: inference.uri },
      range: {
        start: { line: changedLine, character: 0 },
        end: { line: changedLine + 1, character: 0 },
      },
    },
    (message) => {
      const hint = (message.result || []).find(
        (item) => item.label === ": number" && item.position?.line === changedLine,
      );

      if (!hint) {
        fail(`fresh edited inlay hint missing on line ${changedLine}: ${JSON.stringify(message.result)}`);
      }
    },
  );

  let validImportHintLabels = [];

  await request(
    "textDocument/inlayHint",
    {
      textDocument: { uri: imports.uri },
      range: {
        start: { line: 0, character: 0 },
        end: { line: imports.text.split("\n").length + 1, character: 0 },
      },
    },
    (message) => {
      validImportHintLabels = (message.result || []).map((item) => item.label);

      if (validImportHintLabels.length === 0) {
        fail("valid imports document returned no inlay hints");
      }
    },
  );

  const incompleteImports = imports.text.replace("\n\nreturn {", "\nplayer\nreturn {");

  send({
    jsonrpc: "2.0",
    method: "textDocument/didChange",
    params: {
      textDocument: { uri: imports.uri, version: 2 },
      contentChanges: [{ text: incompleteImports }],
    },
  });

  await request(
    "textDocument/inlayHint",
    {
      textDocument: { uri: imports.uri },
      range: {
        start: { line: 0, character: 0 },
        end: { line: incompleteImports.split("\n").length + 1, character: 0 },
      },
    },
    (message) => {
      const labels = (message.result || []).map((item) => item.label);

      if (JSON.stringify(labels) !== JSON.stringify(validImportHintLabels)) {
        fail(
          `bare-identifier edit changed imports inlay hints: before=${validImportHintLabels.join(", ")} after=${labels.join(", ")}`,
        );
      }
    },
  );

  const completionText = `${changedText}\nlocal smokeTable = table.\n`;

  send({
    jsonrpc: "2.0",
    method: "textDocument/didChange",
    params: {
      textDocument: { uri: inference.uri, version: 3 },
      contentChanges: [{ text: completionText }],
    },
  });

  const completionStarted = Date.now();

  await request(
    "textDocument/completion",
    {
      textDocument: { uri: inference.uri },
      position: positionAfter(completionText, "smokeTable = table."),
      context: {
        triggerKind: 2,
        triggerCharacter: ".",
      },
    },
    (message) => {
      const labels = (message.result || []).map((item) => item.label);

      for (const label of ["concat", "insert", "remove", "sort"]) {
        if (!labels.includes(label)) {
          fail(`post-edit table completion missing ${label}: ${labels.join(", ")}`);
        }
      }
    },
  );

  const completionElapsedMs = Date.now() - completionStarted;
  if (completionElapsedMs > 1000) {
    fail(`post-edit table completion exceeded 1000ms: ${completionElapsedMs}ms`);
  }
}

function request(method, params, validate) {
  const id = nextId++;

  send({
    jsonrpc: "2.0",
    id,
    method,
    params,
  });

  return new Promise((resolve) => {
    pendingMethods.set(id, method);
    pending.set(id, (message) => {
      pendingMethods.delete(id);
      if (message.error) {
        fail(`${method} failed: ${JSON.stringify(message.error)}`);
      }

      validate(message);
      resolve(message);
    });
  });
}

function readMessages() {
  while (true) {
    const headerEnd = buffer.indexOf("\r\n\r\n");
    if (headerEnd === -1) {
      return;
    }

    const header = buffer.subarray(0, headerEnd).toString("ascii");
    const lengthMatch = /^Content-Length: (\d+)$/im.exec(header);
    if (!lengthMatch) {
      fail(`missing Content-Length header:\n${header}`);
    }

    const bodyLength = Number(lengthMatch[1]);
    const bodyStart = headerEnd + 4;
    const bodyEnd = bodyStart + bodyLength;
    if (buffer.length < bodyEnd) {
      return;
    }

    const body = buffer.subarray(bodyStart, bodyEnd).toString("utf8");
    buffer = buffer.subarray(bodyEnd);
    handleMessage(JSON.parse(body));
  }
}

function handleMessage(message) {
  if (message.method === "textDocument/publishDiagnostics") {
    return;
  }

  const handler = pending.get(message.id);
  if (!handler) {
    return;
  }

  pending.delete(message.id);
  handler(message);
}

function positionAt(source, needle) {
  const offset = source.indexOf(needle);
  if (offset === -1) {
    fail(`needle not found in source: ${needle}`);
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

function sourceTextFromRange(source, range) {
  if (!range) {
    return undefined;
  }

  const line = source.split("\n")[range.start.line];

  return line.slice(range.start.character, range.end.character);
}

function readSource(filename) {
  return fs.readFileSync(filename, "utf8").replace(/\r\n/g, "\n");
}

function send(message) {
  const body = JSON.stringify(message);
  const length = Buffer.byteLength(body, "utf8");
  child.stdin.write(`Content-Length: ${length}\r\n\r\n${body}`);
}

function fail(message) {
  clearTimeout(timeout);
  if (child.exitCode === null) {
    child.kill();
  }

  console.error(message);
  process.exit(1);
}
