"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { isStaleDocumentResponse } = require("../shared/documents");
const { formatError } = require("../shared/errors");

const STEP_ICONS = {
  annotation: "symbol-type-parameter",
  inference: "lightbulb",
  widening: "arrow-expand",
  narrowing: "filter",
  module: "package",
  field: "symbol-field",
  dynamic: "warning",
  unresolved: "question",
};

function registerTypeExplanation({ getClient, isTuaDocument, onInspect }) {
  return vscode.commands.registerCommand(COMMANDS.explainType, () =>
    explainTypeAtCursor({ getClient, isTuaDocument, onInspect }),
  );
}

async function explainTypeAtCursor({ getClient, isTuaDocument, onInspect }) {
  const editor = vscode.window.activeTextEditor;
  const document = editor?.document;
  if (!document || !isTuaDocument(document)) {
    vscode.window.showWarningMessage("Open a Tua document to explain a type.");
    return undefined;
  }

  const client = getClient();
  if (!client) {
    vscode.window.showWarningMessage("The Tua language server is not running.");
    return undefined;
  }

  const position = editor.selection?.active || new vscode.Position(0, 0);
  onInspect?.(document, position);
  const params = {
    textDocument: { uri: document.uri.toString() },
    position: { line: position.line, character: position.character },
  };

  try {
    let response = await client.sendRequest("tua/explainType", params);
    if (isStaleDocumentResponse(response, document)) {
      response = await client.sendRequest("tua/explainType", params);
    }
    if (isStaleDocumentResponse(response, document)) {
      throw new Error("The explanation is stale. Try again after document synchronization.");
    }

    const explanation = normalizeExplanation(response?.explanation);
    if (!explanation) {
      vscode.window.showInformationMessage("No explainable Tua type was found at the cursor.");
      return undefined;
    }

    const selected = await vscode.window.showQuickPick(
      explanation.steps.map(stepQuickPickItem),
      {
        title: explanation.isAny
          ? `Why is ${explanation.expression} any?`
          : `Explain Type: ${explanation.expression} -> ${explanation.type}`,
        placeHolder: explanation.steps.some(hasLocation)
          ? "Select an inference step to open its source"
          : "Compiler inference steps",
        ignoreFocusOut: true,
        matchOnDescription: true,
        matchOnDetail: true,
      },
    );

    if (selected && hasLocation(selected)) {
      await openStepLocation(selected);
    }

    return explanation;
  } catch (error) {
    vscode.window.showErrorMessage(`Could not explain Tua type: ${formatError(error)}`);
    return undefined;
  }
}

function normalizeExplanation(value) {
  if (
    !value ||
    typeof value.expression !== "string" ||
    typeof value.type !== "string" ||
    !Array.isArray(value.steps)
  ) {
    return undefined;
  }

  return {
    expression: value.expression,
    type: value.type,
    isAny: value.isAny === true,
    range: value.range,
    steps: value.steps.filter(
      (step) =>
        step &&
        typeof step.kind === "string" &&
        typeof step.title === "string" &&
        typeof step.detail === "string",
    ),
  };
}

function stepQuickPickItem(step) {
  const icon = STEP_ICONS[step.kind] || "circle-outline";

  return {
    label: `$(${icon}) ${step.title}`,
    description: typeof step.type === "string" ? `-> ${step.type}` : undefined,
    detail: step.detail,
    uri: typeof step.uri === "string" ? step.uri : undefined,
    range: validRange(step.range) ? step.range : undefined,
    alwaysShow: true,
  };
}

function hasLocation(item) {
  return typeof item.uri === "string" && validRange(item.range);
}

async function openStepLocation(item) {
  const range = new vscode.Range(
    item.range.start.line,
    item.range.start.character,
    item.range.end.line,
    item.range.end.character,
  );

  await vscode.commands.executeCommand("vscode.open", vscode.Uri.parse(item.uri), {
    preview: true,
    selection: range,
  });
}

function validRange(range) {
  return (
    Number.isInteger(range?.start?.line) &&
    Number.isInteger(range?.start?.character) &&
    Number.isInteger(range?.end?.line) &&
    Number.isInteger(range?.end?.character)
  );
}

module.exports = {
  registerTypeExplanation,
};
