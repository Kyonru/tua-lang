"use strict";

const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const { COMMANDS, STATUS_SELECTOR } = require("../constants");

function registerSpritePreviewFeature({ configurationTarget, isTuaDocument }) {
  return vscode.Disposable.from(
    vscode.commands.registerCommand(COMMANDS.previewSprites, () =>
      previewSpriteAnimations({ configurationTarget, isTuaDocument }),
    ),
    registerSpritePreviewHover({ isTuaDocument }),
  );
}

function registerSpritePreviewHover({ isTuaDocument }) {
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

async function previewSpriteAnimations({ configurationTarget, isTuaDocument }) {
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

function isPathInside(child, parent) {
  const relative = path.relative(parent, child);

  return relative === "" || Boolean(relative && !relative.startsWith("..") && !path.isAbsolute(relative));
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

module.exports = {
  parseSpriteSheetPreviews,
  registerSpritePreviewFeature,
};
