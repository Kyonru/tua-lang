"use strict";

const vscode = require("vscode");
const { registerObjectPoolExplorerFeature } = require("./explorer");

function registerObjectPoolDevex(options) {
  return vscode.Disposable.from(registerObjectPoolExplorerFeature(options));
}

module.exports = {
  registerObjectPoolDevex,
};
