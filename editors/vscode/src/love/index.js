"use strict";

const vscode = require("vscode");
const { registerSpritePreviewFeature } = require("./sprite-preview");

function registerLoveDevex(options) {
  return vscode.Disposable.from(registerSpritePreviewFeature(options));
}

module.exports = {
  registerLoveDevex,
};
