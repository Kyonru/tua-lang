"use strict";

const vscode = require("vscode");
const { registerAssetExplorerFeature } = require("./assets");
const { registerLoveProjectDebugger } = require("./debug-project");
const { registerGameCommands } = require("./game-commands");
const { registerLoveProjectRunner } = require("./run-project");
const { registerSpritePreviewFeature } = require("./sprite-preview");

function registerLoveDevex(options) {
  const assetExplorer = registerAssetExplorerFeature(options);
  const disposable = vscode.Disposable.from(
    registerLoveProjectDebugger(options),
    registerLoveProjectRunner(options),
    registerGameCommands(options),
    registerSpritePreviewFeature(options),
    assetExplorer,
  );

  return {
    refreshAssets: assetExplorer.refreshAssets,
    dispose: () => disposable.dispose(),
  };
}

module.exports = {
  registerLoveDevex,
};
