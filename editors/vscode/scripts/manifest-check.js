"use strict";

const fs = require("fs");
const path = require("path");

const packagePath = path.resolve(__dirname, "..", "package.json");
const extensionRoot = path.dirname(packagePath);
const manifest = JSON.parse(fs.readFileSync(packagePath, "utf8"));
const commands = new Set((manifest.contributes?.commands || []).map((command) => command.command));
const expected = [
  "tua.initProject",
  "tua.restartServer",
  "tua.output.focus",
  "tua.openSettings",
  "tua.selectServer",
  "tua.copyDebugInfo",
  "tua.copyCacheStats",
  "tua.saveCacheStats",
  "tua.showServerHealth",
  "tua.resetCompletionStats",
  "tua.saveCpuProfile",
  "tua.saveHeapSummary",
  "tua.openGeneratedLua",
  "tua.jumpBetweenSourceAndGenerated",
  "tua.runLoveProject",
  "tua.restartLoveProject",
  "tua.copyQualifiedName",
  "tua.showWorkspaceSymbolsByModule",
  "tua.hover.references",
  "tua.hover.implementations",
  "tua.explainType",
  "tua.inlayHints.selectMode",
  "tua.previewSprites",
  "tua.assets.refresh",
  "tua.assets.preview",
  "tua.assets.open",
  "tua.assets.openReference",
  "tua.assets.findReferences",
  "tua.assets.rename",
  "tua.assets.reveal",
  "tua.showSignalGraph",
  "tua.showModuleGraph",
  "tua.showEcsExplorer",
  "tua.showObjectPoolExplorer",
  "tua.whyModuleIncluded",
  "tua.source.organizeImports",
  "tua.source.sortImports",
  "tua.source.removeUnusedImports",
  "tua.source.fixAll",
  "tua.reportIssue",
];
const missing = expected.filter((command) => !commands.has(command));

if (missing.length > 0) {
  console.error(`Missing contributed commands: ${missing.join(", ")}`);
  process.exit(1);
}

const assetView = (manifest.contributes?.views?.explorer || []).find(
  (view) => view.id === "tua.assets",
);
if (!assetView) {
  console.error("Missing Tua Asset Explorer contribution.");
  process.exit(1);
}

const assetViewCommands = new Set(
  [
    ...(manifest.contributes?.menus?.["view/title"] || []),
    ...(manifest.contributes?.menus?.["view/item/context"] || []),
  ]
    .filter((item) => item.when?.includes("tua.assets"))
    .map((item) => item.command),
);
for (const command of [
  "tua.assets.refresh",
  "tua.assets.findReferences",
  "tua.assets.rename",
  "tua.assets.reveal",
]) {
  if (!assetViewCommands.has(command)) {
    console.error(`Missing Asset Explorer menu command: ${command}`);
    process.exit(1);
  }
}

const commandPalette = manifest.contributes?.menus?.commandPalette || [];
for (const command of [
  "tua.assets.preview",
  "tua.assets.open",
  "tua.assets.openReference",
  "tua.assets.findReferences",
  "tua.assets.rename",
  "tua.assets.reveal",
  "tua.hover.references",
  "tua.hover.implementations",
]) {
  if (!commandPalette.some((item) => item.command === command && item.when === "false")) {
    console.error(`Hover or asset node command should be hidden from the command palette: ${command}`);
    process.exit(1);
  }
}

const tuaDefaults = manifest.contributes?.configurationDefaults?.["[tua]"];
if (!tuaDefaults) {
  console.error("Missing Tua editor configuration defaults.");
  process.exit(1);
}

if (tuaDefaults["editor.quickSuggestionsDelay"] !== 0) {
  console.error("Tua editor defaults should request immediate quick suggestions.");
  process.exit(1);
}

const quickSuggestions = tuaDefaults["editor.quickSuggestions"];
if (
  quickSuggestions?.other !== "on" ||
  quickSuggestions?.strings !== "on" ||
  quickSuggestions?.comments !== "off"
) {
  console.error("Tua editor defaults should enable quick suggestions outside comments.");
  process.exit(1);
}

const languages = manifest.contributes?.languages || [];
const tuaLanguage = languages.find((language) => language.id === "tua");
if (!tuaLanguage) {
  console.error("Missing tua language contribution.");
  process.exit(1);
}

for (const requiredPath of [tuaLanguage.configuration]) {
  assertExistingJson(requiredPath, "language configuration");
}

const grammars = manifest.contributes?.grammars || [];
const tuaGrammar = grammars.find((grammar) => grammar.language === "tua");
if (!tuaGrammar) {
  console.error("Missing tua grammar contribution.");
  process.exit(1);
}
assertExistingJson(tuaGrammar.path, "TextMate grammar");

const snippets = manifest.contributes?.snippets || [];
const tuaSnippets = snippets.find((snippet) => snippet.language === "tua");
if (!tuaSnippets) {
  console.error("Missing tua snippet contribution.");
  process.exit(1);
}
assertExistingJson(tuaSnippets.path, "snippet contribution");

function assertExistingJson(relativePath, label) {
  if (typeof relativePath !== "string" || relativePath.length === 0) {
    console.error(`Missing ${label} path.`);
    process.exit(1);
  }

  const fullPath = path.resolve(extensionRoot, relativePath);
  if (!fs.existsSync(fullPath)) {
    console.error(`Missing ${label} file: ${relativePath}`);
    process.exit(1);
  }

  try {
    JSON.parse(fs.readFileSync(fullPath, "utf8"));
  } catch (error) {
    console.error(`Invalid ${label} JSON: ${relativePath}`);
    console.error(error.message);
    process.exit(1);
  }
}
