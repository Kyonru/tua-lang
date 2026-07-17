"use strict";

const vscode = require("vscode");
const { registerEcsExplorerFeature } = require("./explorer");

function registerEcsDevex(options) {
  return vscode.Disposable.from(registerEcsExplorerFeature(options));
}

module.exports = {
  registerEcsDevex,
};
