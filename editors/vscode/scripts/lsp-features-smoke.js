"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const path = require("path");
const { pathToFileURL } = require("url");

const command = resolveCommand(process.argv[2] || "target/debug/tua");
const root = path.resolve(process.argv[3] || "examples/features");
const src = path.join(root, "src");
const child = childProcess.spawn(command, ["lsp"], {
  cwd: root,
  stdio: ["pipe", "pipe", "pipe"],
  windowsHide: true,
});

let buffer = Buffer.alloc(0);
let nextId = 1;
let completed = false;
let stderr = "";
const pending = new Map();

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
      text: fs.readFileSync(filename, "utf8"),
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
const legacy = {
  path: path.join(src, "legacy.lua"),
  uri: pathToFileURL(path.join(src, "legacy.lua")).href,
  text: fs.readFileSync(path.join(src, "legacy.lua"), "utf8"),
};

const timeout = setTimeout(() => {
  child.kill();
  fail("timed out waiting for features LSP smoke responses");
}, 10000);

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

  for (const source of sources) {
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
    }),
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

  Promise.all(expected).then(() => {
    request("shutdown", null, () => {
      completed = true;
      console.log("features LSP smoke passed");
      send({ jsonrpc: "2.0", method: "exit", params: null });
    });
  });
});

function request(method, params, validate) {
  const id = nextId++;

  send({
    jsonrpc: "2.0",
    id,
    method,
    params,
  });

  return new Promise((resolve) => {
    pending.set(id, (message) => {
      if (message.error) {
        fail(`${method} failed: ${JSON.stringify(message.error)}`);
      }

      validate(message);
      resolve();
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

function send(message) {
  const body = JSON.stringify(message);
  const length = Buffer.byteLength(body, "utf8");
  child.stdin.write(`Content-Length: ${length}\r\n\r\n${body}`);
}

function resolveCommand(command) {
  if (!command.includes(path.sep) || path.isAbsolute(command)) {
    return command;
  }

  return path.resolve(command);
}

function fail(message) {
  clearTimeout(timeout);
  if (child.exitCode === null) {
    child.kill();
  }

  console.error(message);
  process.exit(1);
}
