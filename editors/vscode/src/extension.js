"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");
const vscode = require("vscode");

let client;
let extensionContext;
let languageClientModule;
let outputChannel;
let statusItem;
let currentServerInfo;
let starting = false;
let suppressNextServerPathPrompt = false;

const COMMANDS = {
  restartServer: "tua.restartServer",
  focusOutput: "tua.output.focus",
  openSettings: "tua.openSettings",
  selectServer: "tua.selectServer",
  copyDebugInfo: "tua.copyDebugInfo",
  copyCacheStats: "tua.copyCacheStats",
  saveCacheStats: "tua.saveCacheStats",
  saveCpuProfile: "tua.saveCpuProfile",
  saveHeapSummary: "tua.saveHeapSummary",
  previewSprites: "tua.previewSprites",
  organizeImports: "tua.source.organizeImports",
  sortImports: "tua.source.sortImports",
  removeUnusedImports: "tua.source.removeUnusedImports",
  fixAll: "tua.source.fixAll",
  reportIssue: "tua.reportIssue",
};

const STATUS_SELECTOR = [
  { scheme: "file", language: "tua" },
  { scheme: "file", pattern: "**/*.tua" },
  { scheme: "untitled", language: "tua" },
];

const DEBUG_LSP_ENV = "TUA_DEBUG_LSP";
const DEBUG_LOG_REPEAT_FLUSH_MS = 250;
const SUGGEST_TRIGGER_DELAY_MS = 25;
const SUGGEST_TRIGGER_PATTERN = /^[A-Za-z0-9_.:'"@-]$/;
let pendingDebugRepeat;

async function activate(context) {
  extensionContext = context;
  outputChannel = vscode.window.createOutputChannel("tua");
  context.subscriptions.push(outputChannel);
  context.subscriptions.push(registerBuiltinDocumentProvider(outputChannel));
  context.subscriptions.push(registerLifecycleCommands());
  context.subscriptions.push(registerSuggestTrigger());
  context.subscriptions.push(registerSpritePreviewHover());
  context.subscriptions.push(vscode.workspace.onDidChangeConfiguration(handleConfigurationChange));
  statusItem = createStatusItem();
  context.subscriptions.push(statusItem);
  debugLog(`${DEBUG_LSP_ENV}=1 enabled; verbose extension/LSP typing trace is active.`);

  await startLanguageServer();
}

function registerSuggestTrigger() {
  let pendingTrigger;

  const changeSubscription = vscode.workspace.onDidChangeTextDocument((event) => {
    if (!isTuaDocument(event.document)) {
      return;
    }

    if (!isActiveDocument(event.document)) {
      debugLog(`suggest skip inactive Tua document: ${documentDebugLabel(event.document)}`, {
        repeatKey: "suggest.skipInactive",
        repeatLabel: "suggest skip inactive Tua document",
      });
      return;
    }

    const changes = event.contentChanges.map(contentChangeDebugLabel).join(", ");

    if (!event.contentChanges.some((change) => SUGGEST_TRIGGER_PATTERN.test(change.text))) {
      debugLog(
        `suggest skip non-trigger change: ${documentDebugLabel(event.document)} changes=[${changes}]`,
        {
          repeatKey: "suggest.skipNonTrigger",
          repeatLabel: "suggest skip non-trigger change",
        },
      );
      return;
    }

    if (pendingTrigger) {
      debugLog("suggest debounce: canceled older pending trigger.", {
        repeatKey: "suggest.debounceCanceled",
        repeatLabel: "suggest debounce canceled",
      });
      clearTimeout(pendingTrigger);
    }

    const document = event.document;

    debugLog(
      `suggest scheduled in ${SUGGEST_TRIGGER_DELAY_MS}ms: ${documentDebugLabel(document)} changes=[${changes}] server=${currentServerInfo?.state || "unknown"}`,
    );

    pendingTrigger = setTimeout(() => {
      pendingTrigger = undefined;

      if (currentServerInfo?.state !== "running" || !isActiveDocument(document)) {
        debugLog(
          `suggest canceled before firing: ${documentDebugLabel(document)} server=${currentServerInfo?.state || "unknown"}`,
          {
            repeatKey: "suggest.canceledBeforeFiring",
            repeatLabel: "suggest canceled before firing",
          },
        );
        return;
      }

      debugLog(`suggest trigger command: ${documentDebugLabel(document)}`);
      vscode.commands.executeCommand("editor.action.triggerSuggest");
    }, SUGGEST_TRIGGER_DELAY_MS);
  });

  return vscode.Disposable.from(changeSubscription, {
    dispose() {
      if (pendingTrigger) {
        clearTimeout(pendingTrigger);
        pendingTrigger = undefined;
      }
    },
  });
}

function registerSpritePreviewHover() {
  return vscode.languages.registerHoverProvider(STATUS_SELECTOR, {
    provideHover(document, position) {
      if (!isTuaDocument(document) || typeof document.getText !== "function") {
        return undefined;
      }

      const range = document.getWordRangeAtPosition?.(position, /[A-Za-z_][A-Za-z0-9_]*/);
      const word = range ? document.getText(range) : "";

      if (!word) {
        return undefined;
      }

      const sprites = parseSpriteSheetPreviews(document.getText());
      const matchingSprite = sprites.find((sprite) => sprite.name === word);

      if (word !== "SpriteSheet" && !matchingSprite) {
        return undefined;
      }

      if (word === "SpriteSheet" && sprites.length === 0) {
        return undefined;
      }

      const label = matchingSprite ? `Preview ${matchingSprite.name}` : "Preview sprite animations";
      const markdown = new vscode.MarkdownString(`[${label}](command:${COMMANDS.previewSprites})`, true);
      markdown.isTrusted = { enabledCommands: [COMMANDS.previewSprites] };

      return new vscode.Hover(markdown, range);
    },
  });
}

function isTuaDocument(document) {
  if (document.languageId === "tua") {
    return true;
  }

  const scheme = document.uri?.scheme;
  if (scheme && scheme !== "file" && scheme !== "untitled") {
    return false;
  }

  const filename = document.fileName || document.uri?.fsPath || document.uri?.path || "";

  return filename.endsWith(".tua");
}

function isActiveDocument(document) {
  const active = vscode.window.activeTextEditor?.document;

  if (!active) {
    return false;
  }

  return active === document || active.uri?.toString() === document.uri?.toString();
}

function debugLspLoggingEnabled() {
  return ["1", "true", "yes", "on"].includes(String(process.env[DEBUG_LSP_ENV] || "").toLowerCase());
}

function debugLog(message, options = {}) {
  if (!debugLspLoggingEnabled()) {
    return;
  }

  const repeatKey = options.repeatKey || message;
  const repeatLabel = options.repeatLabel || message;

  if (pendingDebugRepeat?.key === repeatKey) {
    pendingDebugRepeat.count += 1;
    pendingDebugRepeat.latestMessage = message;
    scheduleDebugRepeatFlush();
    return;
  }

  flushDebugRepeat();
  appendDebugLine(message);
  pendingDebugRepeat = {
    key: repeatKey,
    label: repeatLabel,
    firstMessage: message,
    latestMessage: message,
    count: 1,
    timer: undefined,
  };
}

function appendDebugLine(message) {
  outputChannel?.appendLine(`[debug ${new Date().toISOString()}] ${message}`);
}

function scheduleDebugRepeatFlush() {
  if (pendingDebugRepeat?.timer) {
    clearTimeout(pendingDebugRepeat.timer);
  }

  pendingDebugRepeat.timer = setTimeout(flushDebugRepeat, DEBUG_LOG_REPEAT_FLUSH_MS);
}

function flushDebugRepeat() {
  const repeat = pendingDebugRepeat;

  if (!repeat) {
    return;
  }

  if (repeat.timer) {
    clearTimeout(repeat.timer);
  }

  pendingDebugRepeat = undefined;

  if (repeat.count <= 1) {
    return;
  }

  const latest =
    repeat.latestMessage === repeat.firstMessage ? "" : ` latest=${repeat.latestMessage}`;

  appendDebugLine(`debug repeat: ${repeat.label} count=${repeat.count}${latest}`);
}

function documentDebugLabel(document) {
  const uri = document?.uri?.toString?.() || document?.fileName || "<unknown>";
  const language = document?.languageId || "<unknown>";
  const version = document?.version ?? "<unknown>";

  return `uri=${uri} language=${language} version=${version}`;
}

function contentChangeDebugLabel(change) {
  const text = change?.text || "";
  const range = change?.range
    ? `${change.range.start.line}:${change.range.start.character}-${change.range.end.line}:${change.range.end.character}`
    : "full";

  return `range=${range} text=${textPreview(text)} length=${text.length}`;
}

function textPreview(text) {
  if (text.length <= 24) {
    return JSON.stringify(text);
  }

  return `${JSON.stringify(text.slice(0, 24))}...`;
}

function registerLifecycleCommands() {
  return vscode.Disposable.from(
    vscode.commands.registerCommand(COMMANDS.restartServer, restartLanguageServer),
    vscode.commands.registerCommand(COMMANDS.focusOutput, () => outputChannel?.show(true)),
    vscode.commands.registerCommand(COMMANDS.openSettings, () =>
      vscode.commands.executeCommand("workbench.action.openSettings", "tua"),
    ),
    vscode.commands.registerCommand(COMMANDS.selectServer, selectServerVersion),
    vscode.commands.registerCommand(COMMANDS.copyDebugInfo, copyDebugInfo),
    vscode.commands.registerCommand(COMMANDS.copyCacheStats, copyCacheStats),
    vscode.commands.registerCommand(COMMANDS.saveCacheStats, saveCacheStats),
    vscode.commands.registerCommand(COMMANDS.saveCpuProfile, () =>
      saveServerProfileArtifact("cpuProfile", "CPU profile"),
    ),
    vscode.commands.registerCommand(COMMANDS.saveHeapSummary, () =>
      saveServerProfileArtifact("heapSummary", "heap/allocation summary"),
    ),
    vscode.commands.registerCommand(COMMANDS.previewSprites, previewSpriteAnimations),
    vscode.commands.registerCommand(COMMANDS.organizeImports, () =>
      runSourceAction("source.organizeImports"),
    ),
    vscode.commands.registerCommand(COMMANDS.sortImports, () => runSourceAction("source.sortImports")),
    vscode.commands.registerCommand(COMMANDS.removeUnusedImports, () =>
      runSourceAction("source.removeUnusedImports"),
    ),
    vscode.commands.registerCommand(COMMANDS.fixAll, () => runSourceAction("source.fixAll")),
    vscode.commands.registerCommand(COMMANDS.reportIssue, reportIssue),
  );
}

function runSourceAction(kind) {
  return vscode.commands.executeCommand("editor.action.codeAction", {
    kind,
    apply: "first",
  });
}

async function previewSpriteAnimations() {
  const config = vscode.workspace.getConfiguration("tua");

  if (!config.get("preview.enabled", false)) {
    const choice = await vscode.window.showInformationMessage(
      "Tua sprite animation previews are experimental. Enable preview features for this workspace?",
      "Enable",
      "Open Settings",
    );

    if (choice === "Enable") {
      await config.update("preview.enabled", true, configurationTarget());
    } else if (choice === "Open Settings") {
      await vscode.commands.executeCommand("workbench.action.openSettings", "tua.preview.enabled");
      return;
    } else {
      return;
    }
  }

  const editor = vscode.window.activeTextEditor;
  const document = editor?.document;

  if (!document || !isTuaDocument(document)) {
    vscode.window.showWarningMessage("Open a Tua file with spriteSheet definitions to preview animations.");
    return;
  }

  if (document.uri?.scheme !== "file") {
    vscode.window.showWarningMessage("Sprite animation preview currently supports file-backed Tua documents.");
    return;
  }

  const panel = vscode.window.createWebviewPanel(
    "tuaSpritePreview",
    "Tua Sprite Preview",
    vscode.ViewColumn.Beside,
    spritePreviewWebviewOptions([]),
  );

  if (!renderSpritePreviewPanel(panel, document)) {
    panel.dispose?.();
    return;
  }

  panel.webview.onDidReceiveMessage?.((message) => {
    if (message?.command !== "reload") {
      return;
    }

    renderSpritePreviewPanel(panel, document);
  });
}

function renderSpritePreviewPanel(panel, document) {
  const sprites = parseSpriteSheetPreviews(document.getText());

  if (sprites.length === 0) {
    vscode.window.showWarningMessage("No spriteSheet definitions were found in the active Tua file.");
    return false;
  }

  const resolvedSprites = sprites.map((sprite) => resolveSpritePreviewAsset(document, sprite));

  panel.webview.options = spritePreviewWebviewOptions(resolvedSprites);
  panel.webview.html = spritePreviewHtml(panel.webview, resolvedSprites, document.uri.fsPath);

  return true;
}

function spritePreviewWebviewOptions(resolvedSprites) {
  const roots = resolvedSprites
    .map((sprite) => sprite.resolvedImagePath && path.dirname(sprite.resolvedImagePath))
    .filter(Boolean)
    .map((directory) => vscode.Uri.file(directory));

  return {
    enableScripts: true,
    localResourceRoots: uniqueUriRoots(roots),
  };
}

function parseSpriteSheetPreviews(source) {
  const sprites = [];
  let searchIndex = 0;

  while (searchIndex < source.length) {
    const helperIndex = source.indexOf("spriteSheet", searchIndex);

    if (helperIndex < 0) {
      break;
    }

    if (!isWordBoundary(source, helperIndex - 1) || !isWordBoundary(source, helperIndex + "spriteSheet".length)) {
      searchIndex = helperIndex + "spriteSheet".length;
      continue;
    }

    const openParen = skipWhitespace(source, helperIndex + "spriteSheet".length);
    if (source[openParen] !== "(") {
      searchIndex = helperIndex + "spriteSheet".length;
      continue;
    }

    const tableStart = skipWhitespace(source, openParen + 1);
    if (source[tableStart] !== "{") {
      searchIndex = openParen + 1;
      continue;
    }

    const tableEnd = findMatchingDelimiter(source, tableStart, "{", "}");
    const closeParen = findMatchingDelimiter(source, openParen, "(", ")");

    if (tableEnd === -1 || closeParen === -1) {
      searchIndex = openParen + 1;
      continue;
    }

    const name = spriteAssignmentNameBefore(source, helperIndex);
    const config = parseSpriteSheetConfig(source.slice(tableStart + 1, tableEnd));

    if (name && config) {
      sprites.push({
        name,
        imagePath: config.imagePath,
        frame: config.frame,
        animations: config.animations,
      });
    }

    searchIndex = closeParen + 1;
  }

  return sprites;
}

function spriteAssignmentNameBefore(source, helperIndex) {
  const lineStart = source.lastIndexOf("\n", helperIndex - 1) + 1;
  const prefix = source.slice(lineStart, helperIndex);
  const match = prefix.match(/\blocal\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?::[^=]+)?=\s*$/);

  return match?.[1];
}

function parseSpriteSheetConfig(tableBody) {
  const fields = namedFieldsInTable(tableBody);
  const imagePath = parseLuaStringLiteral(fields.get("image"));
  const frameBody = tableBodyFromValue(fields.get("frame"));
  const animationBody = tableBodyFromValue(fields.get("animations"));

  if (!imagePath || !frameBody || !animationBody) {
    return undefined;
  }

  const frameFields = namedFieldsInTable(frameBody);
  const width = parseNumberLiteral(frameFields.get("width"));
  const height = parseNumberLiteral(frameFields.get("height"));

  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
    return undefined;
  }

  const animations = [];

  for (const [name, value] of namedFieldsInTable(animationBody)) {
    const body = tableBodyFromValue(value);

    if (!body) {
      continue;
    }

    const fields = namedFieldsInTable(body);
    const from = parseNumberLiteral(fields.get("from"));
    const to = parseNumberLiteral(fields.get("to"));
    const fps = parseNumberLiteral(fields.get("fps"));
    const loop = parseBooleanLiteral(fields.get("loop"));

    if (!Number.isFinite(from) || !Number.isFinite(to) || !Number.isFinite(fps) || fps <= 0) {
      continue;
    }

    animations.push({
      name,
      from: Math.trunc(from),
      to: Math.trunc(to),
      fps,
      loop: loop ?? true,
    });
  }

  if (animations.length === 0) {
    return undefined;
  }

  return {
    imagePath,
    frame: {
      width: Math.trunc(width),
      height: Math.trunc(height),
    },
    animations,
  };
}

function namedFieldsInTable(tableBody) {
  const fields = new Map();
  let index = 0;

  while (index < tableBody.length) {
    index = skipSeparators(tableBody, index);

    const match = /^[A-Za-z_][A-Za-z0-9_]*/.exec(tableBody.slice(index));
    if (!match) {
      index += 1;
      continue;
    }

    const key = match[0];
    let cursor = skipWhitespaceAndComments(tableBody, index + key.length);

    if (tableBody[cursor] !== "=") {
      index = cursor + 1;
      continue;
    }

    const valueStart = skipWhitespaceAndComments(tableBody, cursor + 1);
    const valueEnd = topLevelValueEnd(tableBody, valueStart);

    fields.set(key, tableBody.slice(valueStart, valueEnd).trim());
    index = valueEnd + 1;
  }

  return fields;
}

function tableBodyFromValue(value) {
  if (!value) {
    return undefined;
  }

  const start = skipWhitespaceAndComments(value, 0);
  if (value[start] !== "{") {
    return undefined;
  }

  const end = findMatchingDelimiter(value, start, "{", "}");
  if (end === -1) {
    return undefined;
  }

  return value.slice(start + 1, end);
}

function parseLuaStringLiteral(value) {
  if (!value) {
    return undefined;
  }

  const start = skipWhitespaceAndComments(value, 0);
  const quote = value[start];

  if (quote !== "\"" && quote !== "'") {
    return undefined;
  }

  let result = "";

  for (let index = start + 1; index < value.length; index += 1) {
    const character = value[index];

    if (character === quote) {
      return result;
    }

    if (character === "\\") {
      index += 1;
      result += luaEscapeValue(value[index]);
      continue;
    }

    result += character;
  }

  return undefined;
}

function luaEscapeValue(character) {
  if (character === "n") {
    return "\n";
  }

  if (character === "r") {
    return "\r";
  }

  if (character === "t") {
    return "\t";
  }

  return character || "";
}

function parseNumberLiteral(value) {
  const match = String(value || "").trim().match(/^[+-]?\d+(?:\.\d+)?/);

  return match ? Number(match[0]) : NaN;
}

function parseBooleanLiteral(value) {
  const trimmed = String(value || "").trim();

  if (trimmed.startsWith("true")) {
    return true;
  }

  if (trimmed.startsWith("false")) {
    return false;
  }

  return undefined;
}

function topLevelValueEnd(text, start) {
  let index = start;
  let curly = 0;
  let paren = 0;
  let bracket = 0;

  while (index < text.length) {
    if (startsLineComment(text, index)) {
      index = skipLineComment(text, index);
      continue;
    }

    const character = text[index];

    if (character === "\"" || character === "'") {
      index = skipQuotedString(text, index) + 1;
      continue;
    }

    if (character === "{") {
      curly += 1;
    } else if (character === "}") {
      if (curly === 0 && paren === 0 && bracket === 0) {
        return index;
      }
      curly -= 1;
    } else if (character === "(") {
      paren += 1;
    } else if (character === ")") {
      if (paren > 0) {
        paren -= 1;
      }
    } else if (character === "[") {
      bracket += 1;
    } else if (character === "]") {
      if (bracket > 0) {
        bracket -= 1;
      }
    } else if (character === "," && curly === 0 && paren === 0 && bracket === 0) {
      return index;
    }

    index += 1;
  }

  return text.length;
}

function findMatchingDelimiter(text, openIndex, open, close) {
  let depth = 0;

  for (let index = openIndex; index < text.length; index += 1) {
    if (startsLineComment(text, index)) {
      index = skipLineComment(text, index);
      continue;
    }

    const character = text[index];

    if (character === "\"" || character === "'") {
      index = skipQuotedString(text, index);
      continue;
    }

    if (character === open) {
      depth += 1;
    } else if (character === close) {
      depth -= 1;

      if (depth === 0) {
        return index;
      }
    }
  }

  return -1;
}

function skipSeparators(text, index) {
  let cursor = index;

  while (cursor < text.length) {
    cursor = skipWhitespaceAndComments(text, cursor);

    if (text[cursor] !== ",") {
      return cursor;
    }

    cursor += 1;
  }

  return cursor;
}

function skipWhitespaceAndComments(text, index) {
  let cursor = index;

  while (cursor < text.length) {
    if (/\s/.test(text[cursor])) {
      cursor += 1;
      continue;
    }

    if (startsLineComment(text, cursor)) {
      cursor = skipLineComment(text, cursor);
      continue;
    }

    return cursor;
  }

  return cursor;
}

function skipWhitespace(text, index) {
  let cursor = index;

  while (cursor < text.length && /\s/.test(text[cursor])) {
    cursor += 1;
  }

  return cursor;
}

function startsLineComment(text, index) {
  return text[index] === "-" && text[index + 1] === "-";
}

function skipLineComment(text, index) {
  const newline = text.indexOf("\n", index + 2);

  return newline === -1 ? text.length : newline;
}

function skipQuotedString(text, index) {
  const quote = text[index];
  let cursor = index + 1;

  while (cursor < text.length) {
    if (text[cursor] === "\\") {
      cursor += 2;
      continue;
    }

    if (text[cursor] === quote) {
      return cursor;
    }

    cursor += 1;
  }

  return text.length - 1;
}

function isWordBoundary(text, index) {
  const character = text[index];

  return !character || !/[A-Za-z0-9_]/.test(character);
}

function resolveSpritePreviewAsset(document, sprite) {
  const candidates = spriteImageCandidates(document.uri.fsPath, sprite.imagePath);
  const resolvedImagePath = candidates.find((candidate) => fs.existsSync(candidate));

  return {
    ...sprite,
    resolvedImagePath,
    imageUri: resolvedImagePath,
  };
}

function spriteImageCandidates(documentPath, imagePath) {
  if (path.isAbsolute(imagePath)) {
    return [imagePath];
  }

  const documentDirectory = path.dirname(documentPath);
  const workspaceFolder = workspaceFolderForPath(documentPath);
  const projectRoot = findProjectRoot(documentDirectory, workspaceFolder);
  const sourceRoot = projectRoot ? readTuaSourceRoot(projectRoot) : undefined;
  const candidates = [
    path.resolve(documentDirectory, imagePath),
    projectRoot && sourceRoot && path.resolve(projectRoot, sourceRoot, imagePath),
    workspaceFolder && sourceRoot && path.resolve(workspaceFolder, sourceRoot, imagePath),
    projectRoot && path.resolve(projectRoot, imagePath),
    workspaceFolder && path.resolve(workspaceFolder, imagePath),
  ].filter(Boolean);

  return [...new Set(candidates)];
}

function workspaceFolderForPath(filename) {
  const resolved = path.resolve(filename);
  const folders = vscode.workspace.workspaceFolders || [];

  return folders
    .filter((folder) => folder.uri.scheme === "file")
    .map((folder) => folder.uri.fsPath)
    .filter((folder) => isPathInside(resolved, folder))
    .sort((left, right) => right.length - left.length)[0];
}

function findProjectRoot(startDirectory, workspaceFolder) {
  let current = path.resolve(startDirectory);
  const stop = workspaceFolder ? path.resolve(workspaceFolder) : path.parse(current).root;

  while (current && isPathInside(current, stop)) {
    if (fs.existsSync(path.join(current, "tua.toml"))) {
      return current;
    }

    const parent = path.dirname(current);
    if (parent === current) {
      break;
    }

    current = parent;
  }

  return workspaceFolder;
}

function readTuaSourceRoot(projectRoot) {
  const configPath = path.join(projectRoot, "tua.toml");

  try {
    const source = fs.readFileSync(configPath, "utf8");
    const match = source.match(/^\s*source\s*=\s*["']([^"']+)["']/m);

    return match?.[1] || "src";
  } catch {
    return "src";
  }
}

function uniqueUriRoots(roots) {
  const seen = new Set();
  const unique = [];

  for (const root of roots) {
    const key = root.toString();

    if (!seen.has(key)) {
      seen.add(key);
      unique.push(root);
    }
  }

  return unique;
}

function spritePreviewHtml(webview, sprites, documentPath) {
  const nonce = webviewNonce();
  const viewModel = sprites.map((sprite) => ({
    ...sprite,
    imageUri: sprite.resolvedImagePath
      ? webview.asWebviewUri(vscode.Uri.file(sprite.resolvedImagePath)).toString()
      : undefined,
  }));

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src ${webview.cspSource} data:; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}';">
  <title>Tua Sprite Preview</title>
  <style>
    :root {
      color-scheme: light dark;
      --canvas: var(--vscode-editor-background);
      --panel: var(--vscode-sideBar-background, var(--vscode-editor-background));
      --panel-raised: var(--vscode-editorWidget-background, var(--vscode-sideBar-background));
      --panel-border: color-mix(in srgb, var(--vscode-panel-border) 82%, transparent);
      --text: var(--vscode-foreground);
      --muted: var(--vscode-descriptionForeground);
      --quiet: color-mix(in srgb, var(--vscode-descriptionForeground) 72%, transparent);
      --accent: var(--vscode-button-background);
      --accent-text: var(--vscode-button-foreground);
      --accent-hover: var(--vscode-button-hoverBackground, var(--vscode-button-background));
      --input: var(--vscode-input-background);
      --input-border: var(--vscode-input-border, var(--vscode-panel-border));
      --focus: var(--vscode-focusBorder, var(--vscode-button-background));
      --danger: var(--vscode-errorForeground);
      --radius: 6px;
    }

    * {
      box-sizing: border-box;
    }

    body {
      margin: 0;
      color: var(--text);
      background: var(--canvas);
      font: 13px/1.45 var(--vscode-font-family);
      -webkit-font-smoothing: antialiased;
    }

    .shell {
      min-height: 100vh;
      display: grid;
      grid-template-rows: auto 1fr;
    }

    header {
      position: sticky;
      top: 0;
      z-index: 2;
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 12px;
      align-items: center;
      padding: 12px 16px;
      background: color-mix(in srgb, var(--canvas) 94%, transparent);
      border-bottom: 1px solid var(--panel-border);
    }

    h1 {
      margin: 0;
      font-size: 16px;
      line-height: 1.2;
      font-weight: 600;
    }

    .source {
      max-width: 70ch;
      margin-top: 2px;
      color: var(--muted);
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-size: 12px;
    }

    .summary {
      display: flex;
      gap: 6px;
      flex-wrap: wrap;
      justify-content: end;
    }

    .header-actions {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      align-items: center;
      justify-content: end;
    }

    .pill {
      min-height: 24px;
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 2px 8px;
      border: 1px solid var(--panel-border);
      border-radius: 999px;
      color: var(--muted);
      background: var(--panel);
      font-size: 12px;
      font-variant-numeric: tabular-nums;
    }

    .sprites {
      display: grid;
      gap: 14px;
      padding: 16px;
    }

    .sprite {
      display: grid;
      gap: 12px;
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      background: var(--panel);
      overflow: visible;
    }

    .sprite-head {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 12px;
      align-items: center;
      padding: 12px 14px 0;
    }

    .name {
      font-size: 15px;
      font-weight: 600;
      line-height: 1.25;
    }

    .path {
      margin-top: 2px;
      color: var(--muted);
      font-size: 12px;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .meta,
    .error {
      color: var(--muted);
      font-size: 12px;
      font-variant-numeric: tabular-nums;
    }

    .error {
      color: var(--danger);
    }

    .workbench {
      display: grid;
      grid-template-columns: 1fr;
      min-height: 360px;
      border-top: 1px solid var(--panel-border);
    }

    .stage {
      position: sticky;
      top: 64px;
      z-index: 1;
      display: grid;
      grid-template-rows: minmax(220px, 42vh) auto;
      gap: 12px;
      align-self: start;
      max-height: calc(100vh - 80px);
      padding: 14px;
      border-bottom: 1px solid var(--panel-border);
      background: var(--panel-raised);
    }

    .stage-frame {
      min-height: 220px;
      max-height: 420px;
      display: grid;
      place-items: center;
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      background:
        linear-gradient(45deg, rgba(127, 127, 127, .14) 25%, transparent 25%),
        linear-gradient(-45deg, rgba(127, 127, 127, .14) 25%, transparent 25%),
        linear-gradient(45deg, transparent 75%, rgba(127, 127, 127, .14) 75%),
        linear-gradient(-45deg, transparent 75%, rgba(127, 127, 127, .14) 75%);
      background-size: 20px 20px;
      background-position: 0 0, 0 10px, 10px -10px, -10px 0;
      overflow: auto;
      padding: 12px;
    }

    canvas {
      image-rendering: pixelated;
      image-rendering: crisp-edges;
    }

    .sprite-canvas {
      display: block;
      box-shadow: 0 0 0 1px rgba(127, 127, 127, .22);
      background: transparent;
    }

    .stage-readout {
      display: grid;
      gap: 6px;
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .readout {
      min-width: 0;
      display: grid;
      gap: 1px;
      padding: 8px 10px;
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      background: var(--panel);
    }

    .readout span:first-child {
      color: var(--muted);
      font-size: 11px;
    }

    .readout span:last-child {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-size: 13px;
      font-weight: 600;
      font-variant-numeric: tabular-nums;
    }

    .inspector {
      display: grid;
      gap: 14px;
      align-content: start;
      padding: 14px;
    }

    .toolbar {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      align-items: center;
      justify-content: space-between;
    }

    .transport,
    .adjustments {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
      align-items: center;
    }

    label {
      color: var(--muted);
      font-size: 12px;
    }

    button,
    input[type="range"] {
      font: inherit;
    }

    button {
      min-height: 30px;
      padding: 4px 10px;
      color: var(--text);
      background: var(--input);
      border: 1px solid var(--input-border);
      border-radius: 4px;
      cursor: pointer;
      transition: background-color 140ms ease-out, border-color 140ms ease-out, color 140ms ease-out, transform 100ms ease-out;
    }

    button:hover {
      border-color: var(--focus);
      background: color-mix(in srgb, var(--input) 72%, var(--accent) 28%);
    }

    button:focus-visible,
    input[type="range"]:focus-visible {
      outline: 1px solid var(--focus);
      outline-offset: 2px;
    }

    button:active {
      transform: scale(.98);
    }

    .primary {
      color: var(--accent-text);
      background: var(--accent);
      border-color: var(--accent);
    }

    .primary:hover {
      background: var(--accent-hover);
    }

    .animation-tabs {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
    }

    .animation-tabs button {
      color: var(--muted);
    }

    .animation-tabs button.active {
      color: var(--accent-text);
      background: var(--accent);
      border-color: var(--accent);
    }

    .slider {
      min-width: 138px;
      display: grid;
      grid-template-columns: auto minmax(80px, 1fr) 34px;
      gap: 7px;
      align-items: center;
      color: var(--muted);
      font-size: 12px;
      font-variant-numeric: tabular-nums;
    }

    input[type="range"] {
      width: 100%;
      accent-color: var(--accent);
    }

    .metrics {
      display: grid;
      grid-template-columns: repeat(4, minmax(0, 1fr));
      gap: 6px;
    }

    .metric {
      min-width: 0;
      display: grid;
      gap: 2px;
      padding: 8px 10px;
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      background: var(--panel-raised);
    }

    .metric span:first-child {
      color: var(--muted);
      font-size: 11px;
    }

    .metric span:last-child {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-weight: 600;
      font-variant-numeric: tabular-nums;
    }

    .strip {
      display: grid;
      gap: 8px;
    }

    .strip-head,
    .sheet-head {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      align-items: center;
      color: var(--muted);
      font-size: 12px;
    }

    .timeline {
      display: flex;
      flex-wrap: wrap;
      gap: 4px;
    }

    .frame-chip {
      min-width: 32px;
      min-height: 28px;
      padding: 3px 7px;
      border: 1px solid var(--panel-border);
      border-radius: 4px;
      text-align: center;
      color: var(--muted);
      background: var(--panel-raised);
      font-variant-numeric: tabular-nums;
    }

    .frame-chip.active {
      color: var(--accent-text);
      background: var(--accent);
      border-color: var(--accent);
    }

    .sheet {
      display: grid;
      gap: 8px;
    }

    .sheet-viewport {
      min-height: 96px;
      max-height: 260px;
      overflow: auto;
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      background: var(--panel-raised);
      padding: 10px;
    }

    .sheet-canvas {
      display: block;
    }

    .empty {
      margin: 0 14px 14px;
      padding: 12px;
      border: 1px solid color-mix(in srgb, var(--danger) 42%, var(--panel-border));
      border-radius: var(--radius);
      background: color-mix(in srgb, var(--danger) 8%, var(--panel));
    }

    @media (max-width: 780px) {
      header,
      .sprite-head,
      .workbench {
        grid-template-columns: 1fr;
      }

      .summary {
        justify-content: start;
      }

      .stage {
        position: sticky;
        top: 70px;
      }

      .metrics {
        grid-template-columns: repeat(2, minmax(0, 1fr));
      }
    }

    @media (prefers-reduced-motion: reduce) {
      button {
        transition: none;
      }

      button:active {
        transform: none;
      }
    }
  </style>
</head>
<body>
  <div class="shell">
    <header>
      <div>
        <h1>Sprite Animation Preview</h1>
        <div class="source">${escapeHtml(documentPath)}</div>
      </div>
      <div class="header-actions">
        <div class="summary" id="summary"></div>
        <button type="button" class="reload-preview" id="reload-preview">Reload</button>
      </div>
    </header>
    <main class="sprites" id="sprites"></main>
  </div>
  <script nonce="${nonce}">
    const sprites = ${JSON.stringify(viewModel)};
    const root = document.getElementById("sprites");
    const summary = document.getElementById("summary");
    const reloadButton = document.getElementById("reload-preview");
    const vscode = typeof acquireVsCodeApi === "function" ? acquireVsCodeApi() : undefined;
    const animationCount = sprites.reduce((total, sprite) => total + sprite.animations.length, 0);

    summary.appendChild(pill(sprites.length + " sprite" + (sprites.length === 1 ? "" : "s")));
    summary.appendChild(pill(animationCount + " animation" + (animationCount === 1 ? "" : "s")));
    reloadButton.addEventListener("click", () => {
      reloadButton.disabled = true;
      reloadButton.textContent = "Reloading";

      if (vscode) {
        vscode.postMessage({ command: "reload" });
      } else {
        window.location.reload();
      }
    });

    for (const sprite of sprites) {
      root.appendChild(createSpriteView(sprite));
    }

    function pill(text) {
      const element = document.createElement("span");
      element.className = "pill";
      element.textContent = text;
      return element;
    }

    function createSpriteView(sprite) {
      const section = document.createElement("section");
      section.className = "sprite";
      section.innerHTML = \`
        <div class="sprite-head">
          <div>
            <div class="name"></div>
            <div class="path"></div>
          </div>
          <div class="pill"></div>
        </div>
      \`;

      section.querySelector(".name").textContent = sprite.name;
      section.querySelector(".path").textContent = sprite.imagePath;
      section.querySelector(".pill").textContent = sprite.frame.width + "x" + sprite.frame.height;

      if (!sprite.imageUri) {
        const message = document.createElement("div");
        message.className = "empty error";
        message.textContent = "Could not find " + sprite.imagePath;
        section.appendChild(message);
        return section;
      }

      const workbench = document.createElement("div");
      workbench.className = "workbench";
      workbench.innerHTML = \`
        <div class="stage">
          <div class="stage-frame">
            <canvas class="sprite-canvas"></canvas>
          </div>
          <div class="stage-readout">
            <div class="readout"><span>Frame</span><span class="frame-value">-</span></div>
            <div class="readout"><span>Source rect</span><span class="rect-value">-</span></div>
          </div>
        </div>
        <div class="inspector">
          <div class="animation-tabs"></div>
          <div class="toolbar">
            <div class="transport">
              <button type="button" class="previous">Prev</button>
              <button type="button" class="primary play">Pause</button>
              <button type="button" class="next">Next</button>
            </div>
            <div class="adjustments">
              <label class="slider">Speed <input class="speed" type="range" min="0.25" max="2" step="0.25" value="1"><span class="speed-value">1x</span></label>
              <label class="slider">Zoom <input class="zoom" type="range" min="1" max="4" step="0.25" value="1"><span class="zoom-value">Fit</span></label>
            </div>
          </div>
          <div class="metrics">
            <div class="metric"><span>Range</span><span class="range-value">-</span></div>
            <div class="metric"><span>FPS</span><span class="fps-value">-</span></div>
            <div class="metric"><span>Mode</span><span class="mode-value">-</span></div>
            <div class="metric"><span>Sheet</span><span class="sheet-value">-</span></div>
          </div>
          <div class="strip">
            <div class="strip-head">
              <span>Frames</span>
              <span class="frame-count">-</span>
            </div>
            <div class="timeline"></div>
          </div>
          <div class="sheet">
            <div class="sheet-head">
              <span>Sprite sheet</span>
              <span class="sheet-meta">-</span>
            </div>
            <div class="sheet-viewport">
              <canvas class="sheet-canvas"></canvas>
            </div>
          </div>
        </div>
      \`;
      section.appendChild(workbench);

      const stageFrame = workbench.querySelector(".stage-frame");
      const canvas = workbench.querySelector(".sprite-canvas");
      const sheetCanvas = workbench.querySelector(".sheet-canvas");
      const tabs = workbench.querySelector(".animation-tabs");
      const previousButton = workbench.querySelector(".previous");
      const playButton = workbench.querySelector(".play");
      const nextButton = workbench.querySelector(".next");
      const speedInput = workbench.querySelector(".speed");
      const speedValue = workbench.querySelector(".speed-value");
      const zoomInput = workbench.querySelector(".zoom");
      const zoomValue = workbench.querySelector(".zoom-value");
      const frameValue = workbench.querySelector(".frame-value");
      const rectValue = workbench.querySelector(".rect-value");
      const rangeValue = workbench.querySelector(".range-value");
      const fpsValue = workbench.querySelector(".fps-value");
      const modeValue = workbench.querySelector(".mode-value");
      const sheetValue = workbench.querySelector(".sheet-value");
      const sheetMeta = workbench.querySelector(".sheet-meta");
      const frameCount = workbench.querySelector(".frame-count");
      const timeline = workbench.querySelector(".timeline");
      const context = canvas.getContext("2d");
      const sheetContext = sheetCanvas.getContext("2d");
      const image = new Image();
      const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      let animation = sprite.animations[0];
      let frameOffset = 0;
      let lastTime = 0;
      let playing = !reducedMotion;
      let speedScale = 1;
      let zoomScale = 1;

      for (const candidate of sprite.animations) {
        const tab = document.createElement("button");
        tab.type = "button";
        tab.textContent = candidate.name;
        tab.addEventListener("click", () => {
          animation = candidate;
          frameOffset = 0;
          playing = !reducedMotion;
          lastTime = 0;
          updateAll();
        });
        tabs.appendChild(tab);
      }

      previousButton.addEventListener("click", () => {
        stepFrame(-1);
      });

      nextButton.addEventListener("click", () => {
        stepFrame(1);
      });

      playButton.addEventListener("click", () => {
        playing = !playing;
        updatePlayButton();
      });

      speedInput.addEventListener("input", () => {
        speedScale = Number(speedInput.value) || 1;
        speedValue.textContent = speedScale + "x";
      });

      zoomInput.addEventListener("input", () => {
        zoomScale = Number(zoomInput.value) || 1;
        updateZoom();
      });

      window.addEventListener("resize", updateZoom);

      image.addEventListener("load", () => {
        canvas.width = sprite.frame.width;
        canvas.height = sprite.frame.height;
        context.imageSmoothingEnabled = false;
        sheetContext.imageSmoothingEnabled = false;
        sheetValue.textContent = image.naturalWidth + "x" + image.naturalHeight;
        sheetMeta.textContent = image.naturalWidth + "x" + image.naturalHeight + " px";
        updateAll();
        requestAnimationFrame(tick);
      });

      image.addEventListener("error", () => {
        sheetMeta.className = "error";
        sheetMeta.textContent = "Could not load " + sprite.imagePath;
      });

      image.src = sprite.imageUri;

      function tick(time) {
        if (playing && time - lastTime >= 1000 / (animation.fps * speedScale)) {
          lastTime = time;
          const frames = animationFrames(animation);
          if (animation.loop || frameOffset < frames.length - 1) {
            frameOffset = (frameOffset + 1) % frames.length;
          } else {
            playing = false;
          }
          updateAll();
        }

        requestAnimationFrame(tick);
      }

      function updateAll() {
        updateTabs();
        updatePlayButton();
        updateZoom();
        updateMetrics();
        updateTimeline();
        drawFrame();
        drawSheet();
      }

      function updateTabs() {
        for (const tab of tabs.querySelectorAll("button")) {
          tab.classList.toggle("active", tab.textContent === animation.name);
        }
      }

      function updatePlayButton() {
        playButton.textContent = playing ? "Pause" : "Play";
      }

      function updateZoom() {
        zoomScale = Number(zoomInput.value) || zoomScale;

        const fitScale = containedFrameScale();
        const displayScale = fitScale * zoomScale;

        zoomValue.textContent = zoomScale === 1 ? "Fit" : Math.round(zoomScale * 100) + "%";
        canvas.style.width = Math.max(1, Math.floor(sprite.frame.width * displayScale)) + "px";
        canvas.style.height = Math.max(1, Math.floor(sprite.frame.height * displayScale)) + "px";
      }

      function containedFrameScale() {
        const availableWidth = Math.max(1, stageFrame.clientWidth - 24);
        const availableHeight = Math.max(1, stageFrame.clientHeight - 24);

        return Math.max(
          1,
          Math.min(availableWidth / sprite.frame.width, availableHeight / sprite.frame.height),
        );
      }

      function updateMetrics() {
        const frames = animationFrames(animation);
        const frame = currentFrame();
        const rect = sourceRect(frame);

        frameValue.textContent = frame + " (" + (frameOffset + 1) + "/" + frames.length + ")";
        rectValue.textContent = rect.x + "," + rect.y + " " + sprite.frame.width + "x" + sprite.frame.height;
        rangeValue.textContent = animation.from + ".." + animation.to;
        fpsValue.textContent = animation.fps + " fps";
        modeValue.textContent = animation.loop ? "Loop" : "Pause at end";
        frameCount.textContent = frames.length + " frame" + (frames.length === 1 ? "" : "s");
      }

      function stepFrame(direction) {
        const frames = animationFrames(animation);
        frameOffset = (frameOffset + direction + frames.length) % frames.length;
        playing = false;
        updateAll();
      }

      function drawFrame() {
        const rect = sourceRect(currentFrame());

        context.clearRect(0, 0, canvas.width, canvas.height);
        context.drawImage(
          image,
          rect.x,
          rect.y,
          sprite.frame.width,
          sprite.frame.height,
          0,
          0,
          canvas.width,
          canvas.height,
        );
      }

      function drawSheet() {
        const scale = Math.max(1, Math.min(8, Math.floor(320 / Math.max(image.naturalWidth, image.naturalHeight))));
        const rect = sourceRect(currentFrame());

        sheetCanvas.width = image.naturalWidth * scale;
        sheetCanvas.height = image.naturalHeight * scale;
        sheetCanvas.style.width = sheetCanvas.width + "px";
        sheetCanvas.style.height = sheetCanvas.height + "px";
        sheetContext.clearRect(0, 0, sheetCanvas.width, sheetCanvas.height);
        sheetContext.drawImage(image, 0, 0, sheetCanvas.width, sheetCanvas.height);
        sheetContext.strokeStyle = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() || "#ffffff";
        sheetContext.lineWidth = Math.max(2, scale);
        sheetContext.strokeRect(
          rect.x * scale + sheetContext.lineWidth / 2,
          rect.y * scale + sheetContext.lineWidth / 2,
          sprite.frame.width * scale - sheetContext.lineWidth,
          sprite.frame.height * scale - sheetContext.lineWidth,
        );
      }

      function updateTimeline() {
        const frames = animationFrames(animation);
        timeline.replaceChildren(...frames.map((frame, index) => {
          const entry = document.createElement("button");
          entry.type = "button";
          entry.className = index === frameOffset ? "frame-chip active" : "frame-chip";
          entry.textContent = String(frame);
          entry.addEventListener("click", () => {
            frameOffset = index;
            playing = false;
            updateAll();
          });
          return entry;
        }));
      }

      function currentFrame() {
        const frames = animationFrames(animation);
        return frames[Math.min(frameOffset, frames.length - 1)] || animation.from;
      }

      function sourceRect(frame) {
        const columns = Math.max(1, Math.floor(image.naturalWidth / sprite.frame.width));

        return {
          x: (frame % columns) * sprite.frame.width,
          y: Math.floor(frame / columns) * sprite.frame.height,
        };
      }

      return section;
    }

    function animationFrames(animation) {
      const step = animation.to >= animation.from ? 1 : -1;
      const frames = [];

      for (let frame = animation.from; step > 0 ? frame <= animation.to : frame >= animation.to; frame += step) {
        frames.push(frame);
      }

      return frames;
    }
  </script>
</body>
</html>`;
}

function webviewNonce() {
  return Array.from({ length: 16 }, () => Math.floor(Math.random() * 36).toString(36)).join("");
}

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function createStatusItem() {
  const item = vscode.languages.createLanguageStatusItem("tua.languageServer", STATUS_SELECTOR);
  item.name = "Tua Language Server";
  item.text = "$(circle-slash) Tua stopped";
  item.detail = "Tua language server is stopped.";
  item.busy = false;
  item.severity = vscode.LanguageStatusSeverity.Warning;
  item.command = {
    command: COMMANDS.restartServer,
    title: "Restart Tua Language Server",
  };

  return item;
}

async function startLanguageServer() {
  if (starting || !extensionContext || !outputChannel) {
    return;
  }

  starting = true;

  const serverCommand = configuredServerPath(extensionContext);
  const cwd = workspaceCwd();
  const baseDetail = serverDetail(serverCommand, cwd);
  currentServerInfo = {
    command: serverCommand,
    cwd,
    version: undefined,
    state: "starting",
  };

  if (!vscode.workspace.isTrusted && isWorkspaceLocalPath(serverCommand)) {
    const error = new Error("Workspace trust is required before running a workspace-local Tua server binary.");
    showWorkspaceTrustWarning(error);
    setServerStatus("failed", `${baseDetail}\n${formatError(error)}`);
    currentServerInfo.state = "failed";
    currentServerInfo.error = formatError(error);
    starting = false;

    return;
  }

  setServerStatus("starting", baseDetail);
  outputChannel.appendLine(`Starting tua language server: ${serverCommand}`);
  if (cwd) {
    outputChannel.appendLine(`Workspace cwd: ${cwd}`);
  }

  const preflight = await checkServerCommand(serverCommand, cwd);

  if (!preflight.ok) {
    showServerStartupWarning(serverCommand, preflight.error);
    setServerStatus("failed", `${baseDetail}\n${formatError(preflight.error)}`);
    currentServerInfo.state = "failed";
    currentServerInfo.error = formatError(preflight.error);
    starting = false;

    return;
  }

  outputChannel.appendLine(`Tua language server version: ${preflight.version || "unknown"}`);
  currentServerInfo.version = preflight.version;

  const languageClient = loadLanguageClient();
  if (!languageClient) {
    currentServerInfo.state = "failed";
    currentServerInfo.error = "Missing vscode-languageclient dependency.";
    starting = false;

    return;
  }

  const { LanguageClient } = languageClient;
  const serverOptions = {
    run: serverExecutable(serverCommand, cwd),
    debug: serverExecutable(serverCommand, cwd),
  };
  const initializationOptions = traceConfiguration();

  debugLog(
    `language client initialization options: trace.server=${initializationOptions.trace.server} logVerbosity=${initializationOptions.logVerbosity}`,
  );

  const clientOptions = {
    documentSelector: STATUS_SELECTOR,
    initializationOptions,
    outputChannel,
    synchronize: {
      configurationSection: "tua",
      fileEvents: [
        vscode.workspace.createFileSystemWatcher("**/*.{tua,lua}"),
        vscode.workspace.createFileSystemWatcher("**/tua.toml"),
      ],
    },
  };

  client = new LanguageClient("tua", "tua language server", serverOptions, clientOptions);

  try {
    await client.start();
    currentServerInfo.state = "running";
    setServerStatus("running", serverDetail(serverCommand, cwd, preflight.version));
  } catch (error) {
    outputChannel.appendLine(`Failed to start tua language server: ${formatError(error)}`);
    showServerStartupWarning(serverCommand, error);
    client = undefined;
    currentServerInfo.state = "failed";
    currentServerInfo.error = formatError(error);
    setServerStatus("failed", `${baseDetail}\n${formatError(error)}`);
  } finally {
    starting = false;
  }
}

function loadLanguageClient() {
  if (languageClientModule) {
    return languageClientModule;
  }

  try {
    languageClientModule = require("vscode-languageclient/node");

    return languageClientModule;
  } catch (error) {
    outputChannel.appendLine(`Failed to load vscode-languageclient: ${formatError(error)}`);
    vscode.window.showErrorMessage(
      "The tua extension is missing its language client dependency. Reinstall the extension or run npm install in editors/vscode.",
    );
    setServerStatus("failed", `Missing language client dependency.\n${formatError(error)}`);

    return undefined;
  }
}

async function restartLanguageServer() {
  if (!extensionContext) {
    return;
  }

  outputChannel?.show(true);
  outputChannel?.appendLine("Restarting tua language server...");
  await stopLanguageServer({ restarting: true });
  await startLanguageServer();
}

async function selectServerVersion() {
  if (!extensionContext) {
    return;
  }

  const picks = serverVersionPicks(extensionContext);
  const selected = await vscode.window.showQuickPick(picks, {
    placeHolder: "Select the Tua server binary to use",
    ignoreFocusOut: true,
  });

  if (!selected) {
    return;
  }

  if (selected.kind === "trust") {
    await vscode.commands.executeCommand("workbench.trust.manage");
    return;
  }

  if (selected.kind === "settings") {
    await vscode.commands.executeCommand(COMMANDS.openSettings);
    return;
  }

  suppressNextServerPathPrompt = true;
  await vscode.workspace
    .getConfiguration("tua")
    .update("server.path", selected.serverPath, configurationTarget());
  await restartLanguageServer();
}

function handleConfigurationChange(event) {
  const serverPathChanged = event.affectsConfiguration("tua.server.path");
  const traceChanged =
    event.affectsConfiguration("tua.trace.server") ||
    event.affectsConfiguration("tua.log.verbosity");

  if (traceChanged) {
    sendTraceConfiguration();
  }

  if (!serverPathChanged) {
    return;
  }

  if (suppressNextServerPathPrompt) {
    suppressNextServerPathPrompt = false;
    return;
  }

  vscode.window
    .showInformationMessage("Tua server path changed. Restart the language server?", "Restart")
    .then((choice) => {
      if (choice === "Restart") {
        restartLanguageServer();
      }
    });
}

function serverVersionPicks(context) {
  const picks = [];
  const bundled = bundledServerPath(context);

  if (bundled) {
    picks.push({
      label: "Bundled Tua server",
      description: bundled,
      detail: "Use the server packaged with this extension.",
      kind: "server",
      serverPath: "tua",
    });
  }

  if (vscode.workspace.isTrusted) {
    for (const candidate of workspaceServerPaths()) {
      picks.push({
        label: `Workspace Tua server (${path.basename(path.dirname(candidate))})`,
        description: candidate,
        detail: "Use a binary from this trusted workspace.",
        kind: "server",
        serverPath: candidate,
      });
    }
  } else if (hasWorkspaceFolders()) {
    picks.push({
      label: "Trust workspace to use workspace binaries",
      description: "Workspace trust required",
      detail: "Tua will not execute workspace-local server binaries until the workspace is trusted.",
      kind: "trust",
    });
  }

  picks.push({
    label: "Configure custom server path...",
    description: "Open settings",
    detail: "Set tua.server.path manually.",
    kind: "settings",
  });

  return picks;
}

function workspaceServerPaths() {
  const executable = platformExecutableName();
  const candidates = [];

  for (const folder of vscode.workspace.workspaceFolders || []) {
    if (folder.uri.scheme !== "file") {
      continue;
    }

    candidates.push(
      path.join(folder.uri.fsPath, "target", "debug", executable),
      path.join(folder.uri.fsPath, "target", "release", executable),
      path.join(folder.uri.fsPath, "editors", "vscode", "bin", executable),
    );
  }

  return [...new Set(candidates)].filter((candidate) => fs.existsSync(candidate));
}

function configurationTarget() {
  return hasWorkspaceFolders() ? vscode.ConfigurationTarget.Workspace : vscode.ConfigurationTarget.Global;
}

async function copyDebugInfo() {
  const info = debugInfoText();
  await vscode.env.clipboard.writeText(info);
  vscode.window.showInformationMessage("Tua debug information copied to clipboard.");
}

async function copyCacheStats() {
  try {
    const stats = await requestCacheStats();
    await vscode.env.clipboard.writeText(JSON.stringify(stats, null, 2));
    vscode.window.showInformationMessage("Tua cache statistics copied to clipboard.");
  } catch (error) {
    if (!client) {
      vscode.window.showWarningMessage("Tua language server is not running.");
      return;
    }

    outputChannel?.appendLine(`Failed to read tua cache stats: ${formatError(error)}`);
    vscode.window.showErrorMessage(`Failed to read Tua cache stats: ${formatError(error)}`);
  }
}

async function saveCacheStats() {
  try {
    const stats = await requestCacheStats();
    const directory = profileOutputDirectory();
    const filename = `cache-stats-${timestampForFilename(new Date())}.json`;
    const outputPath = path.join(directory, filename);

    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(outputPath, `${JSON.stringify(stats, null, 2)}\n`, "utf8");
    outputChannel?.appendLine(`Tua cache statistics saved to: ${outputPath}`);
    vscode.window.showInformationMessage(`Tua cache statistics saved to ${outputPath}`);
  } catch (error) {
    if (!client) {
      vscode.window.showWarningMessage("Tua language server is not running.");
      return;
    }

    outputChannel?.appendLine(`Failed to save tua cache stats: ${formatError(error)}`);
    vscode.window.showErrorMessage(`Failed to save Tua cache stats: ${formatError(error)}`);
  }
}

function requestCacheStats() {
  if (!client) {
    return Promise.reject(new Error("Tua language server is not running."));
  }

  return client.sendRequest("tua/cacheStats");
}

async function saveServerProfileArtifact(kind, label) {
  if (!client) {
    vscode.window.showWarningMessage("Tua language server is not running.");
    return;
  }

  const directory = profileOutputDirectory();

  try {
    fs.mkdirSync(directory, { recursive: true });

    const result = await client.sendRequest("tua/profileArtifact", {
      kind,
      outputDirectory: directory,
    });
    const outputPaths = profileArtifactPaths(result);
    const outputLabel = outputPaths.length > 0 ? outputPaths.join(", ") : directory;

    outputChannel?.appendLine(`Tua ${label} saved to: ${outputLabel}`);
    vscode.window.showInformationMessage(`Tua ${label} saved to ${outputLabel}`);
  } catch (error) {
    outputChannel?.appendLine(`Tua ${label} unavailable: ${formatError(error)}`);
    vscode.window.showWarningMessage(`Tua ${label} is not available from this server: ${formatError(error)}`);
  }
}

function profileArtifactPaths(result) {
  if (typeof result === "string" && result.trim()) {
    return [result.trim()];
  }

  if (!result || typeof result !== "object") {
    return [];
  }

  if (typeof result.path === "string" && result.path.trim()) {
    return [result.path.trim()];
  }

  if (Array.isArray(result.paths)) {
    return result.paths.filter((entry) => typeof entry === "string" && entry.trim());
  }

  return [];
}

function profileOutputDirectory() {
  if (extensionContext?.globalStorageUri?.scheme === "file" && extensionContext.globalStorageUri.fsPath) {
    return path.join(extensionContext.globalStorageUri.fsPath, "profiles");
  }

  return path.join(os.tmpdir(), "tua-vscode-profiles");
}

function timestampForFilename(date) {
  return date.toISOString().replaceAll(":", "-").replace(/\.\d{3}Z$/, "Z");
}

function traceConfiguration() {
  const config = vscode.workspace.getConfiguration("tua");
  const configuredTrace = config.get("trace.server", "off");
  const configuredVerbosity = config.get("log.verbosity", "off");

  if (debugLspLoggingEnabled()) {
    return {
      trace: {
        server: configuredTrace === "verbose" ? "verbose" : "messages",
      },
      logVerbosity: configuredVerbosity === "trace" ? "trace" : "debug",
    };
  }

  return {
    trace: {
      server: configuredTrace,
    },
    logVerbosity: configuredVerbosity,
  };
}

function sendTraceConfiguration() {
  if (!client) {
    return;
  }

  Promise.resolve(client.sendNotification("tua/setTrace", traceConfiguration())).catch((error) => {
    outputChannel?.appendLine(`Failed to update tua trace settings: ${formatError(error)}`);
    vscode.window.showErrorMessage(`Failed to update Tua trace settings: ${formatError(error)}`);
  });
}

function debugInfoText() {
  const packageInfo = extensionContext ? require(path.join(extensionContext.extensionPath, "package.json")) : {};
  const lines = [
    `Tua extension: ${packageInfo.version || "unknown"}`,
    `VS Code: ${vscode.version}`,
    `Platform: ${process.platform} ${process.arch}`,
    `Workspace trusted: ${String(vscode.workspace.isTrusted)}`,
    `Server state: ${currentServerInfo?.state || "unknown"}`,
    `Server path: ${currentServerInfo?.command || configuredServerPath(extensionContext)}`,
    `Server version: ${currentServerInfo?.version || "unknown"}`,
  ];

  if (currentServerInfo?.cwd) {
    lines.push(`Workspace cwd: ${currentServerInfo.cwd}`);
  }

  if (currentServerInfo?.error) {
    lines.push(`Last server error: ${currentServerInfo.error}`);
  }

  return lines.join("\n");
}

async function reportIssue() {
  const body = encodeURIComponent(`<!-- Review before submitting. No source code is included. -->\n\n\`\`\`\n${debugInfoText()}\n\`\`\`\n`);
  const uri = vscode.Uri.parse(`https://github.com/Kyonru/tua-lang/issues/new?body=${body}`);
  await vscode.env.openExternal(uri);
}

async function stopLanguageServer(options = {}) {
  const activeClient = client;
  client = undefined;

  if (!activeClient) {
    if (!options.restarting) {
      setServerStatus("stopped", "Tua language server is stopped.");
    }

    return undefined;
  }

  if (!options.restarting) {
    setServerStatus("stopped", "Stopping Tua language server...");
  }

  try {
    await activeClient.stop();
  } catch (error) {
    outputChannel?.appendLine(`Failed to stop tua language server: ${formatError(error)}`);
  } finally {
    if (!options.restarting) {
      setServerStatus("stopped", "Tua language server is stopped.");
    }
  }

  return undefined;
}

function setServerStatus(state, detail) {
  if (!statusItem) {
    return;
  }

  statusItem.detail = detail;
  statusItem.command = statusCommand(state);

  if (state === "starting") {
    statusItem.text = "$(sync~spin) Tua starting";
    statusItem.busy = true;
    statusItem.severity = vscode.LanguageStatusSeverity.Information;
  } else if (state === "running") {
    statusItem.text = "$(check) Tua";
    statusItem.busy = false;
    statusItem.severity = vscode.LanguageStatusSeverity.Information;
  } else if (state === "failed") {
    statusItem.text = "$(error) Tua failed";
    statusItem.busy = false;
    statusItem.severity = vscode.LanguageStatusSeverity.Error;
  } else {
    statusItem.text = "$(circle-slash) Tua stopped";
    statusItem.busy = false;
    statusItem.severity = vscode.LanguageStatusSeverity.Warning;
  }
}

function statusCommand(state) {
  if (state === "failed") {
    return {
      command: COMMANDS.openSettings,
      title: "Open Tua Settings",
    };
  }

  return {
    command: COMMANDS.restartServer,
    title: "Restart Tua Language Server",
  };
}

function serverDetail(command, cwd, version) {
  const lines = [`Server: ${command}`];

  if (version) {
    lines.push(`Version: ${version}`);
  }

  if (cwd) {
    lines.push(`Workspace: ${cwd}`);
  }

  return lines.join("\n");
}

function registerBuiltinDocumentProvider(outputChannel) {
  return vscode.workspace.registerTextDocumentContentProvider("tua", {
    async provideTextDocumentContent(uri) {
      if (!client) {
        return "";
      }

      try {
        return (await client.sendRequest("tua/builtinDocument", { uri: uri.toString() })) || "";
      } catch (error) {
        outputChannel.appendLine(`Failed to load tua builtin document: ${formatError(error)}`);
        return "";
      }
    },
  });
}

async function deactivate() {
  flushDebugRepeat();

  return stopLanguageServer();
}

function configuredServerPath(context) {
  const envOverride = process.env.TUA_SERVER_PATH;
  if (typeof envOverride === "string" && envOverride.trim()) {
    return resolveServerCommand(context, envOverride.trim());
  }

  const configured = vscode.workspace.getConfiguration("tua").get("server.path");
  if (typeof configured === "string" && configured.trim() && configured.trim() !== "tua") {
    return resolveServerCommand(context, configured.trim());
  }

  const bundled = bundledServerPath(context);
  if (bundled) {
    return bundled;
  }

  const fallback = typeof configured === "string" && configured.trim() ? configured.trim() : "tua";

  return resolveServerCommand(context, fallback);
}

function bundledServerPath(context) {
  const executable = platformExecutableName();
  const candidates = [
    path.join(context.extensionPath, "bin", executable),
    path.resolve(context.extensionPath, "..", "..", "target", "debug", executable),
  ];

  return candidates.find((candidate) => fs.existsSync(candidate));
}

function platformExecutableName() {
  return process.platform === "win32" ? "tua.exe" : "tua";
}

function workspaceCwd() {
  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];

  return workspaceFolder?.uri.scheme === "file" ? workspaceFolder.uri.fsPath : undefined;
}

function hasWorkspaceFolders() {
  return Boolean(vscode.workspace.workspaceFolders?.length);
}

function isWorkspaceLocalPath(command) {
  if (!command || isBareCommand(command)) {
    return false;
  }

  const resolved = path.resolve(command);

  return (vscode.workspace.workspaceFolders || []).some((folder) => {
    if (folder.uri.scheme !== "file") {
      return false;
    }

    return isPathInside(resolved, folder.uri.fsPath);
  });
}

function isPathInside(child, parent) {
  const relative = path.relative(parent, child);

  return relative === "" || Boolean(relative && !relative.startsWith("..") && !path.isAbsolute(relative));
}

function resolveServerCommand(context, command) {
  if (isBareCommand(command) || path.isAbsolute(command)) {
    return command;
  }

  const candidates = candidateRelativeServerPaths(context, command);
  const existing = candidates.find((candidate) => fs.existsSync(candidate));

  return existing || candidates[0] || command;
}

function isBareCommand(command) {
  return !command.includes("/") && !command.includes("\\");
}

function candidateRelativeServerPaths(context, command) {
  const bases = [
    vscode.workspace.isTrusted ? workspaceCwd() : undefined,
    process.cwd(),
    context.extensionPath,
    path.resolve(context.extensionPath, "..", ".."),
  ].filter(Boolean);
  const unique = [...new Set(bases)];

  return unique.map((base) => path.resolve(base, command));
}

function serverExecutable(command, cwd) {
  return {
    command,
    args: ["lsp"],
    options: {
      cwd,
    },
  };
}

function checkServerCommand(command, cwd) {
  return new Promise((resolve) => {
    let settled = false;
    let stderr = "";
    let stdout = "";
    const child = childProcess.spawn(command, ["--version"], {
      cwd,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });

    const timeout = setTimeout(() => {
      child.kill();
      finish({
        ok: false,
        error: new Error(`\`${command} --version\` timed out`),
      });
    }, 5000);

    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8");
    });

    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8");
    });

    child.once("error", (error) => {
      finish({ ok: false, error });
    });

    child.once("exit", (code) => {
      if (code === 0) {
        finish({ ok: true, version: firstLine(stdout) || firstLine(stderr) });
      } else {
        finish({
          ok: false,
          error: new Error(
            `\`${command} --version\` exited with status ${code}${stderr ? `: ${stderr.trim()}` : ""}`,
          ),
        });
      }
    });

    function finish(result) {
      if (settled) {
        return;
      }

      settled = true;
      clearTimeout(timeout);
      resolve(result);
    }
  });
}

function firstLine(text) {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean);
}

function showServerStartupWarning(command, error) {
  const detail = formatError(error);
  const message =
    `Could not start the tua language server with \`${command}\`. ` +
    "Install a VSIX bundled with the tua server, set `tua.server.path`, or launch with `TUA_SERVER_PATH`.";

  vscode.window
    .showWarningMessage(`${message}${detail ? ` ${detail}` : ""}`, "Open Settings")
    .then((choice) => {
      if (choice === "Open Settings") {
        vscode.commands.executeCommand("workbench.action.openSettings", "tua.server.path");
      }
    });
}

function showWorkspaceTrustWarning(error) {
  vscode.window
    .showWarningMessage(formatError(error), "Manage Workspace Trust", "Open Settings")
    .then((choice) => {
      if (choice === "Manage Workspace Trust") {
        vscode.commands.executeCommand("workbench.trust.manage");
      } else if (choice === "Open Settings") {
        vscode.commands.executeCommand(COMMANDS.openSettings);
      }
    });
}

function formatError(error) {
  if (!error) {
    return "";
  }

  if (typeof error.message === "string" && error.message) {
    return error.message;
  }

  return String(error);
}

module.exports = {
  activate,
  deactivate,
};
