"use strict";

const vscode = require("vscode");
const { COMMANDS, STATUS_SELECTOR } = require("../constants");
const { isStaleDocumentResponse } = require("../shared/documents");
const { formatError } = require("../shared/errors");
const { lspRangeToVscode } = require("../shared/locations");
const { maskSource } = require("../inspector/origins");

const WORD_PATTERN = /[A-Za-z_][A-Za-z0-9_]*/;
const ASSET_PATH_PATTERN =
  /\.(?:bmp|exr|hdr|jpe?g|pic|png|tga|flac|mp3|ogg|wav|otf|ttf|frag|fs|glsl|vert|vs|ogm|ogv)$/i;
const SIGNAL_EVENT_METHODS = new Set([
  "on",
  "once",
  "register",
  "declare",
  "emit",
  "off",
  "remove",
  "clear",
  "has",
  "listeners",
]);
const INSPECTION_PREVIEWS = {
  spriteSheet: {
    command: COMMANDS.previewSprites,
    title: (item) => `Preview ${item.name}`,
  },
  stateMachine: {
    command: COMMANDS.previewStateMachine,
    title: (item) => `Preview ${item.name} State Machine`,
  },
  behaviorTree: {
    command: COMMANDS.previewBehaviorTree,
    title: (item) => `Preview ${item.name} Behavior Tree`,
  },
};
// These are the constructors of the static previews returned by tua/inspect.
const INSPECTION_SOURCE_PATTERN = /\b(?:spriteSheet|stateMachine|BehaviorTree)\b/;
const INSPECTION_CALL_PATTERN = /\b(?:spriteSheet|stateMachine)\s*\(|\bBehaviorTree\s*\.\s*new\s*\(/g;
const INSPECTION_DECLARATION_PATTERN = /\blocal[ \t]+([A-Za-z_]\w*)\s*(?::[^=\n]+)?=\s*(?:spriteSheet|stateMachine|BehaviorTree\s*\.\s*new)\s*\(/g;
const INSPECTION_UNPARENTHESIZED_PATTERN = /\b(?:spriteSheet|stateMachine)\s*\{|\bBehaviorTree\s*\.\s*new\s*\{/;
const inspectionSourceCache = new WeakMap();

function registerHoverActions({ getClient, isTuaDocument }) {
  const registrations = vscode.Disposable.from(
    vscode.commands.registerCommand(COMMANDS.showHoverReferences, (target, locations) =>
      showProviderLocations({
        locations,
        providerCommand: "vscode.executeReferenceProvider",
        title: "references",
        target,
      }),
    ),
    vscode.commands.registerCommand(COMMANDS.showHoverImplementations, (target) =>
      showProviderLocations({
        providerCommand: "vscode.executeImplementationProvider",
        title: "implementations",
        target,
      }),
    ),
    vscode.languages.registerHoverProvider(STATUS_SELECTOR, {
      provideHover(document, position) {
        return hoverActionsAtPosition({ document, getClient, isTuaDocument, position });
      },
    }),
  );
  return {
    middleware: {
      provideHover: (document, position, token, next) =>
        provideMemberHoverActions({ document, isTuaDocument, next, position, token }),
    },
    dispose: () => registrations.dispose(),
  };
}

async function provideMemberHoverActions({ document, isTuaDocument, next, position, token }) {
  const hover = await next(document, position, token);
  if (!hover || token?.isCancellationRequested || !isTuaDocument(document)) {
    return hover;
  }
  const range = document.getWordRangeAtPosition(position, WORD_PATTERN);
  if (!range || !isMemberIdentifier(document, range) || stringLiteralAtPosition(document, position)) {
    return hover;
  }
  const target = commandTarget(document, position);
  const markdown = navigationActionMarkdown(target);
  return new vscode.Hover([...hover.contents, markdown], hover.range || range);
}

async function hoverActionsAtPosition({ document, getClient, isTuaDocument, position }) {
  if (!isTuaDocument(document)) {
    return undefined;
  }

  const wordRange = document.getWordRangeAtPosition(position, WORD_PATTERN);
  const stringLiteral = stringLiteralAtPosition(document, position);
  const identifierRange = stringLiteral ? undefined : wordRange;
  const memberIdentifier = identifierRange && isMemberIdentifier(document, identifierRange);
  const signalEvent = stringLiteral ? isSignalEventLiteral(document, stringLiteral) : false;
  const [asset, inspectionPreview] = await Promise.all([
    assetAtStringLiteral({ document, getClient, position, stringLiteral }),
    inspectionPreviewAtPosition({ document, getClient, position, wordRange }),
  ]);

  if (!identifierRange && !signalEvent && !asset && !inspectionPreview) {
    return undefined;
  }

  const target = commandTarget(document, position);
  const actions = [];
  const enabledCommands = [];

  if (inspectionPreview) {
    actions.push(
      markdownCommandLink(inspectionPreview.command, inspectionPreview.title, [target]),
    );
    enabledCommands.push(inspectionPreview.command);
  }

  if (asset?.uri) {
    actions.push(markdownCommandLink(COMMANDS.previewAsset, "Preview Asset", [{ uri: asset.uri }]));
    enabledCommands.push(COMMANDS.previewAsset);
    actions.push(markdownCommandLink(COMMANDS.revealAsset, "Reveal Asset", [{ asset: { uri: asset.uri } }]));
    enabledCommands.push(COMMANDS.revealAsset);
  }

  if ((identifierRange && !memberIdentifier) || signalEvent || asset) {
    actions.push(markdownCommandLink(COMMANDS.showHoverReferences, "References", [target]));
    enabledCommands.push(COMMANDS.showHoverReferences);
  }

  if (identifierRange && !memberIdentifier) {
    actions.push(
      markdownCommandLink(COMMANDS.showHoverImplementations, "Implementations", [target]),
    );
    enabledCommands.push(COMMANDS.showHoverImplementations);
  }

  if (signalEvent) {
    actions.push(markdownCommandLink(COMMANDS.showSignalGraph, "Event Graph"));
    enabledCommands.push(COMMANDS.showSignalGraph);
  }

  if (actions.length === 0) {
    return undefined;
  }

  const markdown = new vscode.MarkdownString(actions.join(" · "), true);
  markdown.isTrusted = { enabledCommands };

  return new vscode.Hover(
    markdown,
    identifierRange ||
      stringLiteral?.range ||
      lspRangeToVscode(inspectionPreview?.selectionRange),
  );
}

function isMemberIdentifier(document, range) {
  const before = document.lineAt(range.start.line).text.slice(0, range.start.character);
  return /[.:]\s*$/.test(before);
}

function navigationActionMarkdown(target) {
  const markdown = new vscode.MarkdownString([
    markdownCommandLink(COMMANDS.showHoverReferences, "References", [target]),
    markdownCommandLink(COMMANDS.showHoverImplementations, "Implementations", [target]),
  ].join(" · "), true);
  markdown.isTrusted = {
    enabledCommands: [COMMANDS.showHoverReferences, COMMANDS.showHoverImplementations],
  };
  return markdown;
}

async function inspectionPreviewAtPosition({ document, getClient, position, wordRange }) {
  if (!mayContainInspection(document, position, wordRange)) {
    return undefined;
  }
  const client = getClient();
  if (!client) {
    return undefined;
  }

  const params = {
    textDocument: { uri: document.uri.toString() },
    position: {
      line: position.line,
      character: position.character,
    },
  };

  try {
    let response = await client.sendRequest("tua/inspect", params);
    if (isStaleDocumentResponse(response, document)) {
      response = await client.sendRequest("tua/inspect", params);
    }
    if (isStaleDocumentResponse(response, document)) {
      return undefined;
    }

    for (const item of Array.isArray(response?.items) ? response.items : []) {
      const preview = INSPECTION_PREVIEWS[item?.kind];
      if (preview && typeof item.name === "string") {
        return {
          command: preview.command,
          title: preview.title(item),
          selectionRange: item.selectionRange,
        };
      }
    }
  } catch {
    return undefined;
  }

  return undefined;
}

function mayContainInspection(document, position, wordRange) {
  if (typeof document.getText !== "function") {
    return true;
  }
  const cached = inspectionSourceCache.get(document);
  let index = cached?.version === document.version ? cached.index : undefined;
  if (!index) {
    index = buildInspectionSourceIndex(document.getText());
    inspectionSourceCache.set(document, { version: document.version, index });
  }
  if (!index.possible) {
    return false;
  }
  if (typeof document.offsetAt !== "function") {
    return true;
  }
  const word = wordRange ? document.getText(wordRange) : undefined;
  return mayContainInspectionAtOffset(index, document.offsetAt(position), word);
}

function buildInspectionSourceIndex(source) {
  if (!INSPECTION_SOURCE_PATTERN.test(source)) {
    return { possible: false };
  }
  // This is only a conservative request filter. The server remains the owner
  // of declaration identity, including references that resolve to one.
  const masked = maskSource(source);
  const names = new Set();
  const ranges = [];
  const matchedCalls = new Set();
  let uncertain = INSPECTION_UNPARENTHESIZED_PATTERN.test(masked);
  for (const match of masked.matchAll(INSPECTION_DECLARATION_PATTERN)) {
    const callOffset = match[0].search(/\b(?:spriteSheet|stateMachine|BehaviorTree\s*\.\s*new)\s*\($/);
    if (callOffset < 0) {
      uncertain = true;
      continue;
    }
    matchedCalls.add(match.index + callOffset);
    const end = inspectionCallEnd(masked, match.index + match[0].length - 1);
    if (end === undefined) {
      uncertain = true;
      continue;
    }
    // LuaU interpolated strings retain delimiters in the lightweight masker.
    // An apparent closing parenthesis inside one makes this range uncertain.
    if (masked.slice(match.index, end).includes("`")) {
      uncertain = true;
    }
    names.add(match[1]);
    ranges.push({ start: match.index, end });
  }
  for (const call of masked.matchAll(INSPECTION_CALL_PATTERN)) {
    if (!matchedCalls.has(call.index)) {
      uncertain = true;
      break;
    }
  }
  return { possible: true, uncertain, names, ranges };
}

function inspectionCallEnd(masked, open) {
  let depth = 0;
  for (let offset = open; offset < masked.length; offset += 1) {
    if (masked[offset] === "(") {
      depth += 1;
    } else if (masked[offset] === ")" && --depth === 0) {
      return offset + 1;
    }
  }
  return undefined;
}

function mayContainInspectionAtOffset(index, offset, word) {
  if (!index.possible) {
    return false;
  }
  return Boolean(index.uncertain || index.names.has(word) ||
    index.ranges.some(range => range.start <= offset && offset <= range.end));
}

async function showProviderLocations({ locations: providedLocations, providerCommand, target, title }) {
  const normalized = normalizeTarget(target);
  if (!normalized) {
    vscode.window.showWarningMessage(`Open a Tua document to show ${title}.`);
    return undefined;
  }

  try {
    const rawLocations =
      providedLocations ||
      (await vscode.commands.executeCommand(
        providerCommand,
        normalized.uri,
        normalized.position,
      ));
    const locations = normalizeLocations(rawLocations);

    if (locations.length === 0) {
      vscode.window.showInformationMessage(`No Tua ${title} found at this position.`);
      return 0;
    }

    await vscode.commands.executeCommand(
      "editor.action.showReferences",
      normalized.uri,
      normalized.position,
      locations,
    );
    return locations.length;
  } catch (error) {
    vscode.window.showErrorMessage(`Could not show Tua ${title}: ${formatError(error)}`);
    return undefined;
  }
}

function normalizeTarget(target) {
  const candidate = target || activeTarget();
  if (!candidate?.uri || !candidate.position) {
    return undefined;
  }

  const uri = vscodeUri(candidate.uri);
  if (!uri) {
    return undefined;
  }

  return {
    uri,
    position: new vscode.Position(
      Number(candidate.position.line) || 0,
      Number(candidate.position.character) || 0,
    ),
  };
}

function activeTarget() {
  const editor = vscode.window.activeTextEditor;
  if (!editor?.document) {
    return undefined;
  }

  return commandTarget(editor.document, editor.selection?.active || new vscode.Position(0, 0));
}

function normalizeLocations(rawLocations) {
  return (Array.isArray(rawLocations) ? rawLocations : [])
    .map((location) => {
      if (location?.uri && location.range) {
        return vscodeLocation(location.uri, location.range);
      }
      if (location?.targetUri && location.targetRange) {
        return vscodeLocation(
          location.targetUri,
          location.targetSelectionRange || location.targetRange,
        );
      }
      return undefined;
    })
    .filter(Boolean);
}

function vscodeLocation(uriValue, rangeValue) {
  const uri = vscodeUri(uriValue);
  const range = lspRangeToVscode(rangeValue);

  return uri && range ? new vscode.Location(uri, range) : undefined;
}

function vscodeUri(value) {
  if (!value) {
    return undefined;
  }

  try {
    return vscode.Uri.parse(typeof value === "string" ? value : value.toString());
  } catch {
    return undefined;
  }
}

async function assetAtStringLiteral({ document, getClient, position, stringLiteral }) {
  if (!stringLiteral || !looksLikeAssetPath(stringLiteral.value)) {
    return undefined;
  }

  const client = getClient();
  if (!client) {
    return undefined;
  }

  try {
    const response = await client.sendRequest("textDocument/definition", {
      textDocument: { uri: document.uri.toString() },
      position: { line: position.line, character: position.character },
    });
    const location = Array.isArray(response) ? response.length === 1 ? response[0] : undefined : response;
    const uri = vscodeUri(location?.uri || location?.targetUri);
    if (uri?.scheme === "file" && ASSET_PATH_PATTERN.test(uri.fsPath || uri.path || "")) {
      return { uri: uri.toString() };
    }
  } catch {
    return undefined;
  }
}

function looksLikeAssetPath(value) {
  return ASSET_PATH_PATTERN.test(value);
}

function isSignalEventLiteral(document, literal) {
  const before = document
    .lineAt(literal.range.start.line)
    .text.slice(0, literal.range.start.character);
  const match = /(?:[:.])\s*([A-Za-z_][A-Za-z0-9_]*)\s*\(\s*$/.exec(before);

  return Boolean(match && SIGNAL_EVENT_METHODS.has(match[1]));
}

function stringLiteralAtPosition(document, position) {
  const line = document.lineAt(position.line).text;

  for (let index = 0; index < line.length; index += 1) {
    const quote = line[index];
    if (quote !== "\"" && quote !== "'") {
      continue;
    }

    const start = index;
    index += 1;
    let value = "";

    while (index < line.length) {
      const character = line[index];
      if (character === "\\" && index + 1 < line.length) {
        value += line[index + 1];
        index += 2;
        continue;
      }
      if (character === quote) {
        const range = new vscode.Range(
          new vscode.Position(position.line, start),
          new vscode.Position(position.line, index + 1),
        );

        if (rangeContainsPosition(range, position)) {
          return { range, value };
        }
        break;
      }

      value += character;
      index += 1;
    }
  }

  return undefined;
}

function rangeContainsPosition(range, position) {
  return comparePositions(range.start, position) <= 0 && comparePositions(position, range.end) <= 0;
}

function comparePositions(left, right) {
  return left.line - right.line || left.character - right.character;
}

function commandTarget(document, position) {
  return {
    uri: document.uri.toString(),
    position: {
      line: position.line,
      character: position.character,
    },
  };
}

function markdownCommandLink(command, title, args = []) {
  const encodedArgs = args.length > 0 ? `?${encodeURIComponent(JSON.stringify(args))}` : "";

  return `[${title}](command:${command}${encodedArgs})`;
}

module.exports = {
  buildInspectionSourceIndex,
  hoverActionsAtPosition,
  isMemberIdentifier,
  mayContainInspectionAtOffset,
  provideMemberHoverActions,
  registerHoverActions,
};
