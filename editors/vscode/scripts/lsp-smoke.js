"use strict";

const childProcess = require("child_process");

const command = process.argv[2] || "target/debug/tua";
const child = childProcess.spawn(command, ["lsp"], {
  stdio: ["pipe", "pipe", "pipe"],
  windowsHide: true,
});

let buffer = Buffer.alloc(0);
let initialized = false;
let completed = false;
let shutdown = false;
let sawInitialize = false;
let stderr = "";

const sampleUri = "file:///tmp/tua-lsp-smoke.tua";
const sampleSource = [
  "type Player = {",
  "  hp: number,",
  "  alive: boolean,",
  "}",
  "",
  "local player: Player = { hp = 10, alive = true }",
  "player.",
].join("\n");

const timeout = setTimeout(() => {
  child.kill();
  fail("timed out waiting for LSP smoke responses");
}, 5000);

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

  if (!shutdown || code !== 0) {
    fail(`LSP smoke exited with status ${code}${stderr ? `\nstderr:\n${stderr}` : ""}`);
  }
});

send({
  jsonrpc: "2.0",
  id: 1,
  method: "initialize",
  params: {
    processId: process.pid,
    rootUri: null,
    capabilities: {},
    workspaceFolders: null,
  },
});

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
  if (message.id === 1) {
    const capabilities = Object.keys(message.result?.capabilities || {}).sort();
    sawInitialize = true;
    console.log(`initialized tua LSP with capabilities: ${capabilities.join(", ")}`);

    send({ jsonrpc: "2.0", method: "initialized", params: {} });
    send({
      jsonrpc: "2.0",
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          uri: sampleUri,
          languageId: "tua",
          version: 1,
          text: sampleSource,
        },
      },
    });
    send({
      jsonrpc: "2.0",
      id: 2,
      method: "textDocument/completion",
      params: {
        textDocument: { uri: sampleUri },
        position: { line: 6, character: 7 },
      },
    });
    return;
  }

  if (message.id === 2) {
    const labels = (message.result || []).map((item) => item.label).sort();
    const missing = ["alive", "hp"].filter((label) => !labels.includes(label));
    if (missing.length > 0) {
      fail(`completion smoke missing labels: ${missing.join(", ")}`);
    }

    completed = true;
    console.log(`completion smoke labels: ${labels.join(", ")}`);
    send({ jsonrpc: "2.0", id: 3, method: "shutdown", params: null });
    return;
  }

  if (message.id === 3) {
    initialized = sawInitialize;
    shutdown = true;
    console.log("shutdown tua LSP cleanly");
    send({ jsonrpc: "2.0", method: "exit", params: null });
  }
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
  process.exit(initialized && completed ? 2 : 1);
}
