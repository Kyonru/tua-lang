"use strict";

const COMMANDS = {
  restartServer: "tua.restartServer",
  focusOutput: "tua.output.focus",
  openSettings: "tua.openSettings",
  selectServer: "tua.selectServer",
  copyDebugInfo: "tua.copyDebugInfo",
  copyCacheStats: "tua.copyCacheStats",
  saveCacheStats: "tua.saveCacheStats",
  saveCpuProfile: "tua.saveCpuProfile",
  saveHeapSummary: "tua.saveHeapSummary",
  previewSprites: "tua.previewSprites",
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
