#!/usr/bin/env node
"use strict";

const fs = require("fs");
const path = require("path");
const { fileURLToPath, pathToFileURL } = require("url");

if (process.argv.includes("--version")) {
  process.stdout.write("tua 0.0.0-vscode-test\n");
  process.exit(0);
}
if (process.argv[2] === "build") {
  mockBuildProject(process.cwd());
  process.stdout.write("built mock Tua project\n");
  process.exit(0);
}

const documents = new Map();
const cancelledRequests = new Set();
let workspaceFolders = [];
let input = Buffer.alloc(0);
let eventSequence = 0;
let continuedCompletionCount = 0;

function mockBuildProject(root) {
  const config = readFile(path.join(root, "tua.toml"));
  const sourceDirectory = path.join(root, topLevelConfigString(config, "source") || "src");
  const outputDirectory = path.join(root, topLevelConfigString(config, "out") || "build");

  for (const sourcePath of tuaFiles(sourceDirectory)) {
    const relative = path.relative(sourceDirectory, sourcePath);
    const parsed = path.parse(relative);
    const outputPath = path.join(outputDirectory, parsed.dir, `${parsed.name}.lua`);

    fs.mkdirSync(path.dirname(outputPath), { recursive: true });
    fs.copyFileSync(sourcePath, outputPath);
  }
}

function topLevelConfigString(source, key) {
  const escaped = key.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
  const match = new RegExp(`^\\s*${escaped}\\s*=\\s*["']([^"']+)["']`, "mu").exec(source);

  return match?.[1];
}

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
      workspaceFolders = message.params.workspaceFolders || [];
      log({
        type: "initialize",
        rootUri: message.params.rootUri,
        workspaceFolders,
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
    case "tua/localInlineCompletion":
      localInlineCompletion(message);
      return;
    case "textDocument/hover":
      hover(message);
      return;
    case "textDocument/definition":
      definition(message);
      return;
    case "textDocument/references":
      references(message);
      return;
    case "textDocument/implementation":
      implementation(message);
      return;
    case "textDocument/documentSymbol":
      documentSymbols(message);
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
    case "tua/projectLayout":
      projectLayout(message);
      return;
    case "tua/assetInventory":
      assetInventory(message);
      return;
    case "tua/moduleGraph":
      moduleGraph(message);
      return;
    case "tua/ecsGraph":
      ecsGraph(message);
      return;
    case "workspace/willRenameFiles":
      willRenameFiles(message);
      return;
    case "workspace/symbol":
      workspaceSymbols(message);
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
      documentSymbolProvider: true,
      hoverProvider: true,
      implementationProvider: true,
      inlayHintProvider: true,
      referencesProvider: true,
      signatureHelpProvider: {
        retriggerCharacters: [")"],
        triggerCharacters: ["(", ","],
      },
      textDocumentSync: {
        change: 1,
        openClose: true,
      },
      workspaceSymbolProvider: true,
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

function localInlineCompletion(message) {
  const context = requestContext(message);
  log({
    type: "inline-completion.request",
    ...context,
    documentVersion: message.params.documentVersion,
    id: message.id,
    selectedCompletion: message.params.selectedCompletion,
  });
  respond(message.id, {
    documentVersion: context.docVersion,
    items: [],
  });
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

function references(message) {
  const context = requestContext(message);
  const locations = referenceLocations(context);

  log({
    type: "references.request",
    ...context,
    count: locations.length,
    id: message.id,
  });
  respond(message.id, locations);
}

function implementation(message) {
  const context = requestContext(message);
  const declaration = declarationLocation(context.uri, context.word);
  const locations = declaration ? [declaration] : [];

  log({
    type: "implementation.request",
    ...context,
    count: locations.length,
    id: message.id,
  });
  respond(message.id, locations);
}

function documentSymbols(message) {
  const uri = message.params.textDocument.uri;
  const document = documents.get(uri);
  const source = document?.text || readUri(uri);

  log({
    type: "documentSymbol.request",
    id: message.id,
    uri,
    version: document?.version,
  });
  respond(message.id, documentSymbolsForSource(source));
}

function workspaceSymbols(message) {
  const query = message.params?.query || "";
  const symbols = [];

  for (const folder of workspaceFolders) {
    const root = fileURLToPath(folder.uri);
    const sourceDirectory = path.join(root, "src");
    for (const filePath of tuaFiles(sourceDirectory)) {
      const uri = pathToFileURL(filePath).toString();
      const source = readFile(filePath);
      const moduleName = moduleNameForPath(sourceDirectory, filePath);
      const moduleRange = {
        start: { line: 0, character: 0 },
        end: { line: 0, character: 0 },
      };

      symbols.push({
        name: moduleName,
        kind: 2,
        location: { uri, range: moduleRange },
      });
      for (const symbol of documentSymbolsForSource(source)) {
        symbols.push({
          name: symbol.name,
          kind: symbol.kind,
          containerName: moduleName,
          location: { uri, range: symbol.selectionRange },
        });
      }
    }
  }

  log({
    type: "workspaceSymbol.request",
    id: message.id,
    query,
    count: symbols.length,
  });
  respond(message.id, symbols.filter((symbol) => workspaceSymbolMatches(symbol, query)));
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

function assetInventory(message) {
  const root = workspaceFolders[0]?.uri
    ? fileURLToPath(workspaceFolders[0].uri)
    : process.cwd();
  const paths = assetPathsInSources();
  if (paths.length === 0) {
    paths.push("assets/player.png");
  }
  const assets = paths.map((assetPath) => {
    const resolvedPath = path.join(root, "src", ...assetPath.split("/"));
    const references = assetReferences(assetPath);

    return {
      uri: pathToFileURL(resolvedPath).toString(),
      path: assetPath,
      category: "image",
      categoryLabel: "Images",
      extension: path.extname(assetPath).slice(1),
      dimensions: { width: 16, height: 16 },
      usageCount: references.length,
      references,
    };
  });

  log({
    type: "assetInventory.request",
    count: assets.reduce((total, asset) => total + asset.references.length, 0),
    id: message.id,
  });
  respond(message.id, {
    analysisRevision: eventSequence,
    inventory: {
      assets,
      diagnostics: [],
    },
  });
}

function projectLayout(message) {
  const sourcePath = fileURLToPath(message.params.uri);
  const folder = workspaceFolders
    .map((entry) => ({ ...entry, path: fileURLToPath(entry.uri) }))
    .filter((entry) => sourcePath.startsWith(entry.path))
    .sort((left, right) => right.path.length - left.path.length)[0];
  if (!folder) {
    respond(message.id, null);
    return;
  }

  const configPath = path.join(folder.path, "tua.toml");
  const config = readFile(configPath);
  const sourceDirectory = path.join(folder.path, topLevelConfigString(config, "source") || "src");
  const outputDirectory = path.join(folder.path, topLevelConfigString(config, "out") || "build");
  const mainSource = topLevelConfigString(config, "main") || "main.tua";
  const relative = path.relative(sourceDirectory, sourcePath);
  const insideSource = relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
  const isMain = path.normalize(relative) === path.normalize(mainSource);
  const generatedPath = insideSource
    ? path.join(outputDirectory, isMain ? "main.lua" : relative.replace(/\.[^.]+$/u, ".lua"))
    : undefined;
  const moduleName = insideSource
    ? relative.replace(/\.[^.]+$/u, "").split(path.sep).filter(Boolean).join(".")
    : undefined;

  respond(message.id, {
    workspaceUri: folder.uri,
    configUri: fs.existsSync(configPath) ? pathToFileURL(configPath).toString() : null,
    projectUri: folder.uri,
    sourceUri: pathToFileURL(sourceDirectory).toString(),
    outputUri: pathToFileURL(outputDirectory).toString(),
    mainSource,
    generatedUri: generatedPath ? pathToFileURL(generatedPath).toString() : null,
    moduleName: moduleName || null,
  });
}

function moduleGraph(message) {
  const rootUri = [...documents.keys()].find((uri) => uri.endsWith("/module-graph-main.tua"));
  const rootPath = rootUri
    ? fileURLToPath(rootUri)
    : path.resolve(process.cwd(), "src", "module-graph-main.tua");
  const sourceDirectory = path.dirname(rootPath);
  const mainUri = rootUri || pathToFileURL(rootPath).toString();
  const playerUri = pathToFileURL(path.join(sourceDirectory, "module_graph_player.tua")).toString();
  const sharedUri = pathToFileURL(path.join(sourceDirectory, "module_graph_shared.lua")).toString();
  const importRange = {
    start: { line: 0, character: 23 },
    end: { line: 0, character: 42 },
  };

  log({
    type: "moduleGraph.request",
    id: message.id,
    rootUri: mainUri,
  });
  respond(message.id, {
    analysisRevision: 11,
    graph: {
      modules: [
        moduleGraphModule(mainUri, {
          root: true,
          dependencyCount: 2,
          unresolvedDependencyCount: 1,
          exports: [{ name: "start", type: "function(): Player", origin: "tua", range: importRange }],
          inclusionPath: { rootUri: mainUri, steps: [] },
        }),
        moduleGraphModule(playerUri, {
          inCycle: true,
          dependencyCount: 1,
          reverseDependencyCount: 2,
          exports: [{ name: "Player", type: "Player", origin: "tua", range: importRange }],
          inclusionPath: {
            rootUri: mainUri,
            steps: [moduleGraphStep(mainUri, playerUri, "module_graph_player", "require", importRange)],
          },
        }),
        moduleGraphModule(sharedUri, {
          sourceKind: "lua",
          inCycle: true,
          dependencyCount: 1,
          reverseDependencyCount: 1,
          exports: [{ name: "version", type: "number", origin: "lua", range: importRange }],
          inclusionPath: {
            rootUri: mainUri,
            steps: [
              moduleGraphStep(mainUri, playerUri, "module_graph_player", "require", importRange),
              moduleGraphStep(playerUri, sharedUri, "module_graph_shared", "import", importRange),
            ],
          },
        }),
      ],
      edges: [
        moduleGraphEdge(mainUri, "module_graph_player", "require", importRange, {
          status: "resolved",
          targetUri: playerUri,
        }),
        moduleGraphEdge(mainUri, "missing_module", "require", importRange, {
          status: "missing",
          searchRootUris: [pathToFileURL(sourceDirectory).toString()],
        }),
        moduleGraphEdge(playerUri, "module_graph_shared", "import", importRange, {
          status: "resolved",
          targetUri: sharedUri,
        }, true),
        moduleGraphEdge(sharedUri, "module_graph_player", "require", importRange, {
          status: "resolved",
          targetUri: playerUri,
        }, true),
      ],
      cycles: [{ id: "cycle:0", moduleUris: [playerUri, sharedUri] }],
    },
  });
}

function ecsGraph(message) {
  const uri = [...documents.keys()].find((candidate) => candidate.endsWith("/ecs.tua"))
    || pathToFileURL(path.resolve(process.cwd(), "src", "ecs.tua")).toString();
  const componentRange = {
    start: { line: 0, character: 6 },
    end: { line: 0, character: 14 },
  };
  const systemRange = {
    start: { line: 2, character: 6 },
    end: { line: 4, character: 4 },
  };
  const positionId = "component:position";
  const velocityId = "component:velocity";
  const systemId = "system:movement";

  log({ type: "ecsGraph.request", id: message.id, rootUri: uri });
  respond(message.id, {
    analysisRevision: 12,
    graph: {
      components: [
        ecsComponent(positionId, uri, "Position", componentRange),
        ecsComponent(velocityId, uri, "Velocity", {
          start: { line: 1, character: 6 },
          end: { line: 1, character: 14 },
        }),
      ],
      systems: [
        {
          id: systemId,
          uri,
          name: "movement",
          range: systemRange,
          nameRange: { start: systemRange.start, end: { line: 2, character: 14 } },
          callbackRange: systemRange,
          components: [
            ecsComponentUse("Position", positionId, "readWrite", componentRange),
            ecsComponentUse("Velocity", velocityId, "read", componentRange),
          ],
        },
      ],
      queries: [
        {
          id: `${systemId}:query`,
          uri,
          name: "movement",
          kind: "system",
          ownerSystemId: systemId,
          range: systemRange,
          nameRange: { start: systemRange.start, end: { line: 2, character: 14 } },
          components: [
            ecsComponentUse("Position", positionId, "readWrite", componentRange),
            ecsComponentUse("Velocity", velocityId, "read", componentRange),
          ],
        },
      ],
      insights: [
        {
          id: `${systemId}:allocation`,
          kind: "callbackAllocation",
          title: "Per-entity callback allocations",
          message: "Consider reusing callback data when profiling shows allocation pressure.",
          uri,
          range: systemRange,
          subjectId: systemId,
        },
      ],
    },
  });
}

function ecsComponent(id, uri, name, range) {
  return {
    id,
    uri,
    localName: name,
    runtimeName: name,
    range,
    nameRange: range,
    fields: [{ name: "x", type: "number", range }],
  };
}

function ecsComponentUse(reference, componentId, access, range) {
  return { reference, componentId, access, range };
}

function moduleGraphModule(uri, overrides) {
  return {
    uri,
    sourceKind: "tua",
    root: false,
    inCycle: false,
    dependencyCount: 0,
    unresolvedDependencyCount: 0,
    reverseDependencyCount: 0,
    exports: [],
    inclusionPath: null,
    ...overrides,
  };
}

function moduleGraphStep(sourceUri, targetUri, specifier, kind, range) {
  return { sourceUri, targetUri, specifier, kind, range };
}

function moduleGraphEdge(sourceUri, specifier, kind, range, resolution, inCycle = false) {
  return { sourceUri, specifier, kind, range, inCycle, resolution };
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

function documentSymbolsForSource(source) {
  return source
    .split(/\r?\n/)
    .flatMap((line, lineNumber) => {
      const symbols = [];
      const typeMatch = /^\s*type\s+([A-Za-z_][A-Za-z0-9_]*)/.exec(line);
      const functionMatch = /^\s*(?:local\s+)?function\s+([A-Za-z_][A-Za-z0-9_]*)/.exec(line);
      const localMatch = /^\s*local\s+([A-Za-z_][A-Za-z0-9_]*)\s*=/.exec(line);

      if (typeMatch) {
        symbols.push(symbolForLine(line, lineNumber, typeMatch[1], 23));
      }
      if (functionMatch) {
        symbols.push(symbolForLine(line, lineNumber, functionMatch[1], 12));
      }
      if (localMatch) {
        symbols.push(symbolForLine(line, lineNumber, localMatch[1], 13));
      }

      return symbols;
    });
}

function symbolForLine(line, lineNumber, name, kind) {
  const character = line.indexOf(name);
  const selectionRange = {
    start: { line: lineNumber, character },
    end: { line: lineNumber, character: character + name.length },
  };

  return {
    name,
    kind,
    range: {
      start: { line: lineNumber, character: 0 },
      end: { line: lineNumber, character: line.length },
    },
    selectionRange,
    children: [],
  };
}

function tuaFiles(directory) {
  if (!fs.existsSync(directory)) {
    return [];
  }

  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const fullPath = path.join(directory, entry.name);

    if (entry.isDirectory()) {
      return tuaFiles(fullPath);
    }

    return entry.isFile() && entry.name.endsWith(".tua") ? [fullPath] : [];
  });
}

function moduleNameForPath(sourceDirectory, filePath) {
  const relative = path.relative(sourceDirectory, filePath);
  const withoutExtension = relative.slice(0, relative.length - path.extname(relative).length);

  return withoutExtension.split(path.sep).join(".");
}

function workspaceSymbolMatches(symbol, query) {
  const trimmed = query.trim().toLowerCase();
  if (!trimmed) {
    return true;
  }

  const qualifiedName = symbol.containerName
    ? `${symbol.containerName}.${symbol.name}`
    : symbol.name;

  return (
    symbol.name.toLowerCase().includes(trimmed) ||
    qualifiedName.toLowerCase().includes(trimmed)
  );
}

function readUri(uri) {
  return readFile(fileURLToPath(uri));
}

function readFile(filePath) {
  try {
    return fs.readFileSync(filePath, "utf8");
  } catch {
    return "";
  }
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

function referenceLocations(context) {
  if (!context.word) {
    return [];
  }

  const document = documents.get(context.uri);
  const source = document?.text || readUri(context.uri);
  const pattern = new RegExp(`\\b${escapeRegExp(context.word)}\\b`, "g");
  const locations = [];
  let match;

  while ((match = pattern.exec(source))) {
    locations.push({
      uri: context.uri,
      range: offsetRange(source, match.index, match.index + context.word.length),
    });
  }

  return locations;
}

function assetReferences(assetPath) {
  const references = [];

  for (const [uri, source] of sourceTexts()) {
    let index = source.indexOf(assetPath);
    while (index >= 0) {
      references.push({
        uri,
        range: offsetRange(source, index, index + assetPath.length),
        loaderKind: "image",
      });
      index = source.indexOf(assetPath, index + assetPath.length);
    }
  }

  return references;
}

function assetPathsInSources() {
  const paths = new Set();
  const pattern = /assets\/[A-Za-z0-9_./-]+\.(?:bmp|exr|hdr|jpe?g|pic|png|tga|flac|mp3|ogg|wav|otf|ttf|frag|fs|glsl|vert|vs|ogm|ogv)/g;

  for (const source of sourceTexts().values()) {
    for (const match of source.matchAll(pattern)) {
      paths.add(match[0]);
    }
  }

  return [...paths].sort();
}

function sourceTexts() {
  const sources = new Map();

  for (const [uri, document] of documents) {
    sources.set(uri, document.text);
  }

  for (const folder of workspaceFolders) {
    const sourceDirectory = path.join(fileURLToPath(folder.uri), "src");
    for (const filePath of tuaFiles(sourceDirectory)) {
      const uri = pathToFileURL(filePath).toString();
      if (!sources.has(uri)) {
        sources.set(uri, readFile(filePath));
      }
    }
  }

  return sources;
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
