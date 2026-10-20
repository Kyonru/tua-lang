"use strict";

const vscode = require("vscode");
const { COMMANDS, STATUS_SELECTOR } = require("../constants");
const { isStaleDocumentResponse } = require("../shared/documents");
const { formatError } = require("../shared/errors");
const { lspRangeToVscode } = require("../shared/locations");

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
};

function registerHoverActions({ getClient, isTuaDocument }) {
  return vscode.Disposable.from(
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
}

async function hoverActionsAtPosition({ document, getClient, isTuaDocument, position }) {
  if (!isTuaDocument(document)) {
    return undefined;
  }

  const wordRange = document.getWordRangeAtPosition(position, WORD_PATTERN);
  const stringLiteral = stringLiteralAtPosition(document, position);
  const identifierRange = stringLiteral ? undefined : wordRange;
  const signalEvent = stringLiteral ? isSignalEventLiteral(document, stringLiteral) : false;
  const [asset, inspectionPreview] = await Promise.all([
    assetAtStringLiteral({ document, getClient, position, stringLiteral }),
    inspectionPreviewAtPosition({ document, getClient, position }),
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
  }

  if (identifierRange || signalEvent || asset) {
    actions.push(markdownCommandLink(COMMANDS.showHoverReferences, "References", [target]));
    enabledCommands.push(COMMANDS.showHoverReferences);
  }

  if (identifierRange) {
    actions.push(
      markdownCommandLink(COMMANDS.showHoverImplementations, "Implementations", [target]),
    );
    enabledCommands.push(COMMANDS.showHoverImplementations);
  }

  if (signalEvent) {
    actions.push(markdownCommandLink(COMMANDS.showSignalGraph, "Event Graph"));
    enabledCommands.push(COMMANDS.showSignalGraph);
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

async function inspectionPreviewAtPosition({ document, getClient, position }) {
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
    const response = await client.sendRequest("tua/assetInventory", null);
    const assets = Array.isArray(response?.inventory?.assets) ? response.inventory.assets : [];

    return assets.find((asset) => assetMatchesLiteral(asset, document, position, stringLiteral));
  } catch {
    return undefined;
  }
}

function assetMatchesLiteral(asset, document, position, stringLiteral) {
  if (!asset?.uri) {
    return false;
  }

  if (normalizedAssetPath(asset.path) === normalizedAssetPath(stringLiteral.value)) {
    return true;
  }

  return (asset.references || []).some((reference) => {
    if (reference?.uri !== document.uri.toString()) {
      return false;
    }

    const range = lspRangeToVscode(reference.range);
    return range ? rangeContainsPosition(range, position) : false;
  });
}

function normalizedAssetPath(value) {
  return typeof value === "string" ? value.replaceAll("\\", "/") : "";
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
  registerHoverActions,
};
