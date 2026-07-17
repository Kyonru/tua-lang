"use strict";

const COMMANDS = {
  restartServer: "tua.restartServer",
  focusOutput: "tua.output.focus",
  openSettings: "tua.openSettings",
  selectServer: "tua.selectServer",
  copyDebugInfo: "tua.copyDebugInfo",
  copyCacheStats: "tua.copyCacheStats",
  saveCacheStats: "tua.saveCacheStats",
  resetCompletionStats: "tua.resetCompletionStats",
  saveCpuProfile: "tua.saveCpuProfile",
  saveHeapSummary: "tua.saveHeapSummary",
  openGeneratedLua: "tua.openGeneratedLua",
  jumpBetweenSourceAndGenerated: "tua.jumpBetweenSourceAndGenerated",
  copyQualifiedName: "tua.copyQualifiedName",
  showWorkspaceSymbolsByModule: "tua.showWorkspaceSymbolsByModule",
  showHoverReferences: "tua.hover.references",
  showHoverImplementations: "tua.hover.implementations",
  explainType: "tua.explainType",
  selectInlayHintMode: "tua.inlayHints.selectMode",
  previewSprites: "tua.previewSprites",
  reloadSpritePreview: "tua.previewSprites.reload",
  refreshAssets: "tua.assets.refresh",
  previewAsset: "tua.assets.preview",
  openAsset: "tua.assets.open",
  openAssetReference: "tua.assets.openReference",
  findAssetReferences: "tua.assets.findReferences",
  renameAsset: "tua.assets.rename",
  revealAsset: "tua.assets.reveal",
  showSignalGraph: "tua.showSignalGraph",
  showModuleGraph: "tua.showModuleGraph",
  showEcsExplorer: "tua.showEcsExplorer",
  showObjectPoolExplorer: "tua.showObjectPoolExplorer",
  whyModuleIncluded: "tua.whyModuleIncluded",
  organizeImports: "tua.source.organizeImports",
  sortImports: "tua.source.sortImports",
  removeUnusedImports: "tua.source.removeUnusedImports",
  fixAll: "tua.source.fixAll",
  reportIssue: "tua.reportIssue",
};

const STATUS_SELECTOR = [
  { scheme: "file", language: "tua" },
  { scheme: "file", pattern: "**/*.tua" },
  { scheme: "untitled", language: "tua" },
];

module.exports = {
  COMMANDS,
  STATUS_SELECTOR,
};
