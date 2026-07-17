"use strict";

const vscode = require("vscode");
const { registerModuleGraphFeature } = require("./module-graph");

function registerModuleDevex(options) {
  return vscode.Disposable.from(registerModuleGraphFeature(options));
}

module.exports = {
  registerModuleDevex,
};
