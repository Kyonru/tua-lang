"use strict";

const vscode = require("vscode");
const COMMAND = "tua.openAnnotationGuide";

function registerAnnotationGuide(context) {
  return vscode.commands.registerCommand(COMMAND, () =>
    vscode.commands.executeCommand(
      "markdown.showPreview",
      vscode.Uri.joinPath(context.extensionUri, "resources", "annotation-guide.md"),
    ),
  );
}

module.exports = { COMMAND, registerAnnotationGuide };
