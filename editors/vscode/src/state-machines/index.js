"use strict";

const vscode = require("vscode");
const { registerStateMachinePreviewFeature } = require("./preview");

function registerStateMachineDevex(options) {
  return vscode.Disposable.from(registerStateMachinePreviewFeature(options));
}

module.exports = {
  registerStateMachineDevex,
};
