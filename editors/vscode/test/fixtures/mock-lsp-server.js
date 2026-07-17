#!/usr/bin/env node
"use strict";

const fs = require("fs");
const path = require("path");
const { fileURLToPath, pathToFileURL } = require("url");

if (process.argv.includes("--version")) {
  process.stdout.write("tua 0.0.0-vscode-test\n");
  process.exit(0);
}

const documents = new Map();
const cancelledRequests = new Set();
let input = Buffer.alloc(0);
let eventSequence = 0;
let continuedCompletionCount = 0;

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
      log({
        type: "initialize",
        rootUri: message.params.rootUri,
        workspaceFolders: message.params.workspaceFolders || [],
      });
      respond(message.id, initializeResult());
      return;
    case "shutdown":
      respond(message.id, null);
      return;
    case "exit":
      process.exit(0);
      return;
    case "textDocument/didOpen":
      openDocument(message.params.textDocument);
      return;
    case "textDocument/didChange":
      changeDocument(message.params);
      return;
    case "textDocument/didClose":
      documents.delete(message.params.textDocument.uri);
      log({ type: "document.close", uri: message.params.textDocument.uri });
      return;
    case "textDocument/completion":
      complete(message);
      return;
    case "textDocument/hover":
      hover(message);
      return;
    case "textDocument/definition":
      definition(message);
      return;
    case "textDocument/signatureHelp":
      signatureHelp(message);
      return;
    case "textDocument/inlayHint":
      inlayHints(message);
      return;
    case "tua/inspect":
      inspectDocument(message);
      return;
    case "workspace/willRenameFiles":
      willRenameFiles(message);
      return;
    case "workspace/didRenameFiles":
      log({ type: "workspace.didRenameFiles", files: message.params.files });
      return;
    case "workspace/didChangeWorkspaceFolders":
      log({ type: "workspace.folders.change", event: message.params.event });
      return;
    case "$/cancelRequest":
      cancelledRequests.add(message.params.id);
      log({ type: "request.cancel", id: message.params.id });
      return;
    default:
      if (message.id !== undefined) {
        respond(message.id, null);
      }
  }
}

function initializeResult() {
  return {
    capabilities: {
      completionProvider: {},
      definitionProvider: true,
      hoverProvider: true,
      inlayHintProvider: true,
      signatureHelpProvider: {
        retriggerCharacters: [")"],
        triggerCharacters: ["(", ","],
      },
      textDocumentSync: {
        change: 1,
        openClose: true,
      },
      workspace: {
        fileOperations: {
          didRename: {
            filters: fileOperationFilters(),
          },
          willRename: {
            filters: fileOperationFilters(),
          },
        },
        workspaceFolders: {
          changeNotifications: true,
          supported: true,
        },
      },
    },
    serverInfo: {
      name: "tua-vscode-test",
      version: "0.0.0",
    },
  };
}

function fileOperationFilters() {
  return [
    {
      scheme: "file",
      pattern: {
        glob: "**/*",
        matches: "file",
      },
    },
  ];
}

function openDocument(document) {
  documents.set(document.uri, {
    text: document.text,
    version: document.version,
  });
  log({
    type: "document.open",
    languageId: document.languageId,
    uri: document.uri,
    version: document.version,
  });
}

function changeDocument(params) {
  const document = documents.get(params.textDocument.uri);
  const change = params.contentChanges.at(-1);
  const next = {
    text: change?.text ?? document?.text ?? "",
    version: params.textDocument.version,
  };

  documents.set(params.textDocument.uri, next);
  log({
    type: "document.change",
    text: next.text,
    uri: params.textDocument.uri,
    version: next.version,
  });
}

function complete(message) {
  const context = requestContext(message);
  const initial = context.prefix === "o";
  const label = initial ? "outdated_result" : "outside_from_lsp";
  const delayMs = initial ? 600 : continuedCompletionCount++ === 0 ? 20 : 200;

  log({
    type: "completion.request",
    ...context,
    id: message.id,
  });

  setTimeout(() => {
    log({
      type: "completion.response",
      cancelled: cancelledRequests.has(message.id),
      docVersion: context.docVersion,
      id: message.id,
      label,
      prefix: context.prefix,
    });
    respond(message.id, {
      isIncomplete: false,
      items: [
        {
          filterText: context.prefix,
          kind: 6,
          label,
          sortText: "0000",
        },
      ],
    });
  }, delayMs);
}

function hover(message) {
  const context = requestContext(message);

  log({
    type: "hover.request",
    ...context,
    id: message.id,
  });
  respond(message.id, {
    contents: {
      kind: "markdown",
      value: `\`${context.word}\`: number\n\nDocument version ${context.docVersion}.`,
    },
    range: wordRange(context.line, context.position),
  });
}

function definition(message) {
  const context = requestContext(message);
  const declaration = declarationLocation(context.uri, context.word);

  log({
    type: "definition.request",
    ...context,
    id: message.id,
  });
  respond(message.id, declaration);
}

function signatureHelp(message) {
  const context = requestContext(message);

  log({
    type: "signature.request",
    ...context,
    id: message.id,
  });
  respond(message.id, {
    activeParameter: 0,
    activeSignature: 0,
    signatures: [
      {
        label: "greet(name: string): string",
        parameters: [{ label: "name: string" }],
      },
    ],
  });
}

function inlayHints(message) {
  const context = requestContext(message);
  const line = Math.min(message.params.range.end.line, Math.max(0, context.lines.length - 1));

  log({
    type: "inlay.request",
    ...context,
    id: message.id,
    range: message.params.range,
  });
  respond(message.id, [
    {
      kind: 2,
      label: "value:",
      paddingRight: true,
      position: {
        character: 0,
        line,
      },
    },
    {
      kind: 1,
      label: ": number",
      paddingLeft: true,
      position: {
        character: (context.lines[line] || "").length,
        line,
      },
    },
  ]);
}

function inspectDocument(message) {
  const uri = message.params?.textDocument?.uri;
  const document = documents.get(uri);
  const source = document?.text || "";
  const name = /\blocal\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*spriteSheet\s*\(/.exec(source)?.[1];

  log({
    type: "inspect.request",
    uri,
    version: document?.version,
    id: message.id,
  });

  if (!name) {
    respond(message.id, {
      analysisRevision: document?.version || 0,
      documentVersion: document?.version,
      items: [],
    });
    return;
  }

  const imagePath = /\bimage\s*=\s*["']([^"']+)["']/.exec(source)?.[1] || "";
  const frame = /\bframe\s*=\s*\{[^}]*\bwidth\s*=\s*(\d+)[^}]*\bheight\s*=\s*(\d+)/s.exec(
    source,
  );
  const animations = [...source.matchAll(
    /^[ \t]*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*\{[^\n}]*\bfrom\s*=\s*(\d+)[^\n}]*\bto\s*=\s*(\d+)[^\n}]*\bfps\s*=\s*(\d+(?:\.\d+)?)([^\n}]*)\}/gm,
  )].map((match) => ({
    name: match[1],
    from: Number(match[2]),
    to: Number(match[3]),
    fps: Number(match[4]),
    loop: !/\bloop\s*=\s*false\b/.test(match[5]),
  }));
  const documentPath = fileURLToPath(uri);
  const resolvedImagePath = path.resolve(path.dirname(documentPath), imagePath);
  const imageExists = fs.existsSync(resolvedImagePath);

  respond(message.id, {
    analysisRevision: document.version,
    documentVersion: document.version,
    items: [
      {
        kind: "spriteSheet",
        name,
        range: {
          start: { line: 0, character: 0 },
          end: { line: Math.max(0, source.split(/\r?\n/).length - 1), character: 0 },
        },
        selectionRange: {
          start: { line: 0, character: 6 },
          end: { line: 0, character: 6 + name.length },
        },
        image: {
          path: imagePath,
          uri: imageExists ? pathToFileURL(resolvedImagePath).toString() : null,
          status: imageExists ? "valid" : "missing",
        },
        frame: frame
          ? {
              width: Number(frame[1]),
              height: Number(frame[2]),
            }
          : null,
        animations,
        diagnostics: imageExists
          ? []
          : [
              {
                code: "TL5004",
                message: `missing sprite image asset \`${imagePath}\``,
                severity: 1,
              },
            ],
      },
    ],
  });
}

function willRenameFiles(message) {
  const changes = {};

  for (const file of message.params.files) {
    const oldName = path.basename(uriPath(file.oldUri), path.extname(uriPath(file.oldUri)));
    const newName = path.basename(uriPath(file.newUri), path.extname(uriPath(file.newUri)));

    for (const [uri, document] of documents) {
      const index = document.text.indexOf(oldName);
      if (index < 0) {
        continue;
      }

      changes[uri] = [
        {
          newText: newName,
          range: offsetRange(document.text, index, index + oldName.length),
        },
      ];
    }
  }

  log({
    type: "workspace.willRenameFiles",
    files: message.params.files,
    editedUris: Object.keys(changes),
  });
  respond(message.id, { changes });
}

function requestContext(message) {
  const uri = message.params.textDocument.uri;
  const document = documents.get(uri);
  const position = message.params.position || { line: 0, character: 0 };
  const lines = document?.text.split(/\r?\n/) || [];
  const line = lines[position.line] || "";
  const before = line.slice(0, position.character);

  return {
    before,
    docVersion: document?.version,
    line,
    lines,
    position,
    prefix: before.match(/[A-Za-z_][A-Za-z0-9_]*$/)?.[0] || "",
    uri,
    word: wordAtPosition(line, position),
  };
}

function wordAtPosition(line, position) {
  let start = Math.min(position.character, line.length);
  let end = start;

  while (start > 0 && /[A-Za-z0-9_]/.test(line[start - 1])) {
    start -= 1;
  }
  while (end < line.length && /[A-Za-z0-9_]/.test(line[end])) {
    end += 1;
  }

  return line.slice(start, end);
}

function wordRange(line, position) {
  const word = wordAtPosition(line, position);
  const end = Math.min(position.character, line.length);
  const start = Math.max(0, line.lastIndexOf(word, end));

  return {
    start: { line: position.line, character: start },
    end: { line: position.line, character: start + word.length },
  };
}

function declarationLocation(uri, word) {
  const document = documents.get(uri);
  const lines = document?.text.split(/\r?\n/) || [];

  for (let line = 0; line < lines.length; line += 1) {
    const match = new RegExp(`\\blocal\\s+${escapeRegExp(word)}\\b`).exec(lines[line]);
    if (!match) {
      continue;
    }

    const character = lines[line].indexOf(word, match.index);
    return {
      uri,
      range: {
        start: { line, character },
        end: { line, character: character + word.length },
      },
    };
  }

  return null;
}

function offsetRange(text, start, end) {
  return {
    start: offsetPosition(text, start),
    end: offsetPosition(text, end),
  };
}

function offsetPosition(text, offset) {
  const before = text.slice(0, offset);
  const lines = before.split(/\r?\n/);

  return {
    line: lines.length - 1,
    character: lines.at(-1).length,
  };
}

function uriPath(uri) {
  return decodeURIComponent(new URL(uri).pathname);
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function respond(id, result) {
  const body = JSON.stringify({ jsonrpc: "2.0", id, result });
  process.stdout.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
}

function log(event) {
  const logPath = process.env.TUA_TEST_LSP_LOG;
  if (!logPath) {
    return;
  }

  eventSequence += 1;
  fs.appendFileSync(
    logPath,
    `${JSON.stringify({
      ...event,
      sequence: eventSequence,
      timestamp: new Date().toISOString(),
    })}\n`,
  );
}
