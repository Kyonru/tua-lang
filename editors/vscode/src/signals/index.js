"use strict";

const vscode = require("vscode");
const { registerSignalGraphFeature } = require("./signal-graph");

function registerSignalDevex(options) {
  return vscode.Disposable.from(registerSignalGraphFeature(options));
}

module.exports = {
  registerSignalDevex,
};
