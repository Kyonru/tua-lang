"use strict";

const COMMANDS = {
  initProject: "tua.initProject",
  restartServer: "tua.restartServer",
  focusOutput: "tua.output.focus",
  openSettings: "tua.openSettings",
  selectServer: "tua.selectServer",
  copyDebugInfo: "tua.copyDebugInfo",
  copyCacheStats: "tua.copyCacheStats",
  saveCacheStats: "tua.saveCacheStats",
  showServerHealth: "tua.showServerHealth",
  resetCompletionStats: "tua.resetCompletionStats",
  toggleExperimentalCompletions: "tua.completion.toggleExperimental",
  toggleOllamaCompletions: "tua.completion.toggleOllama",
  warmOllamaModel: "tua.completion.warmOllama",
  ollamaCompletionAccepted: "tua.completion.ollamaAccepted",
  saveCpuProfile: "tua.saveCpuProfile",
  saveHeapSummary: "tua.saveHeapSummary",
  openGeneratedLua: "tua.openGeneratedLua",
  jumpBetweenSourceAndGenerated: "tua.jumpBetweenSourceAndGenerated",
  runLoveProject: "tua.runLoveProject",
  debugLoveProject: "tua.debugLoveProject",
  restartLoveProject: "tua.restartLoveProject",
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

const CONFIG_SELECTOR = [{ scheme: "file", pattern: "**/tua.toml" }];
const DOCUMENT_SELECTOR = [...STATUS_SELECTOR, ...CONFIG_SELECTOR];

module.exports = {
  COMMANDS,
  CONFIG_SELECTOR,
  DOCUMENT_SELECTOR,
  STATUS_SELECTOR,
};
