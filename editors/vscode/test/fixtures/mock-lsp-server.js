#!/usr/bin/env node
"use strict";

const fs = require("fs");

if (process.argv.includes("--version")) {
  process.stdout.write("tua 0.0.0-vscode-test\n");
  process.exit(0);
}

const documents = new Map();
let input = Buffer.alloc(0);

process.stdin.on("data", (chunk) => {
  input = Buffer.concat([input, chunk]);
  readMessages();
});

function readMessages() {
  while (true) {
    const headerEnd = input.indexOf("\r\n\r\n");
    if (headerEnd < 0) {
      return;
    }

    const header = input.subarray(0, headerEnd).toString("ascii");
    const lengthMatch = /^Content-Length:\s*(\d+)$/im.exec(header);
    if (!lengthMatch) {
      throw new Error(`Missing Content-Length header: ${header}`);
    }

    const bodyLength = Number(lengthMatch[1]);
    const messageEnd = headerEnd + 4 + bodyLength;
    if (input.length < messageEnd) {
      return;
    }

    const body = input.subarray(headerEnd + 4, messageEnd).toString("utf8");
    input = input.subarray(messageEnd);
    handleMessage(JSON.parse(body));
  }
}

function handleMessage(message) {
  switch (message.method) {
    case "initialize":
      respond(message.id, {
        capabilities: {
          completionProvider: {},
          textDocumentSync: {
            change: 1,
            openClose: true,
          },
        },
        serverInfo: {
          name: "tua-vscode-test",
          version: "0.0.0",
        },
      });
      return;
    case "shutdown":
      respond(message.id, null);
      return;
    case "exit":
      process.exit(0);
      return;
    case "textDocument/didOpen": {
      const document = message.params.textDocument;
      documents.set(document.uri, {
        text: document.text,
        version: document.version,
      });
      log({
        type: "document.open",
        uri: document.uri,
        version: document.version,
      });
      return;
    }
    case "textDocument/didChange": {
      const document = documents.get(message.params.textDocument.uri);
      const change = message.params.contentChanges.at(-1);
      documents.set(message.params.textDocument.uri, {
        text: change?.text ?? document?.text ?? "",
        version: message.params.textDocument.version,
      });
      log({
        type: "document.change",
        uri: message.params.textDocument.uri,
        version: message.params.textDocument.version,
      });
      return;
    }
    case "textDocument/completion":
      complete(message);
      return;
    case "$/cancelRequest":
      log({ type: "request.cancel", id: message.params.id });
      return;
    default:
      if (message.id !== undefined) {
        respond(message.id, null);
      }
  }
}

function complete(message) {
  const uri = message.params.textDocument.uri;
  const document = documents.get(uri);
  const position = message.params.position;
  const line = document?.text.split(/\r?\n/)[position.line] ?? "";
  const before = line.slice(0, position.character);
  const prefix = before.match(/[A-Za-z_][A-Za-z0-9_]*$/)?.[0] ?? "";
  const request = {
    type: "completion.request",
    id: message.id,
    uri,
    docVersion: document?.version,
    position,
    line,
    prefix,
  };

  log(request);

  const initial = prefix === "o";
  const label = initial ? "outdated_result" : "outside_from_lsp";
  setTimeout(() => {
    log({
      type: "completion.response",
      id: message.id,
      docVersion: request.docVersion,
      prefix,
      label,
    });
    respond(message.id, {
      isIncomplete: true,
      items: [
        {
          filterText: prefix,
          kind: 6,
          label,
          sortText: "0000",
        },
      ],
    });
  }, 10);
}

function respond(id, result) {
  const body = JSON.stringify({ jsonrpc: "2.0", id, result });
  process.stdout.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
}

function log(event) {
  const logPath = process.env.TUA_TEST_LSP_LOG;
  if (logPath) {
    fs.appendFileSync(logPath, `${JSON.stringify(event)}\n`);
  }
}
