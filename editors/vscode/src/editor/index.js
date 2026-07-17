"use strict";

const vscode = require("vscode");
const { registerInlayHintPresentation } = require("./inlay-hints");

function registerEditorDevex(options) {
  return vscode.Disposable.from(registerInlayHintPresentation(options));
}

module.exports = {
  registerEditorDevex,
};
