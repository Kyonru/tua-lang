"use strict";

const fs = require("fs");
const path = require("path");

const packagePath = path.resolve(__dirname, "..", "package.json");
const extensionRoot = path.dirname(packagePath);
const manifest = JSON.parse(fs.readFileSync(packagePath, "utf8"));
const commands = new Set((manifest.contributes?.commands || []).map((command) => command.command));

if (manifest.main !== "./dist/extension.js") {
  console.error("The packaged extension entry point should use the bundled runtime.");
  process.exit(1);
}

if (!manifest.scripts?.build?.includes("esbuild src/extension.js")) {
  console.error("The extension build script should bundle the runtime with esbuild.");
  process.exit(1);
}

const untrustedWorkspaces = manifest.capabilities?.untrustedWorkspaces;
if (untrustedWorkspaces?.supported !== "limited") {
  console.error("Restricted Mode support must be declared as limited.");
  process.exit(1);
}
const restrictedConfigurations = new Set(untrustedWorkspaces.restrictedConfigurations || []);
for (const setting of ["tua.server.path", "tua.love.command"]) {
  if (!restrictedConfigurations.has(setting)) {
    console.error(`Restricted Mode must restrict executable setting: ${setting}`);
    process.exit(1);
  }
}

if (manifest.publisher !== "SolenodonteLabs") {
  console.error("Extension publisher should be SolenodonteLabs.");
  process.exit(1);
}

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
  "tua.completion.toggleExperimental",
  "tua.completion.toggleLocalInline",
  "tua.saveCpuProfile",
  "tua.saveHeapSummary",
  "tua.openGeneratedLua",
  "tua.jumpBetweenSourceAndGenerated",
  "tua.runLoveProject",
  "tua.stopLoveProject",
  "tua.debugLoveProject",
  "tua.restartLoveProject",
  "tua.runLoveProjectWithHotReload",
  "tua.packageGame",
  "tua.copyQualifiedName",
  "tua.showWorkspaceSymbolsByModule",
  "tua.hover.references",
  "tua.hover.implementations",
  "tua.explainType",
  "tua.inlayHints.selectMode",
  "tua.previewSprites",
  "tua.previewStateMachine",
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
  "tua.showAudioExplorer",
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

if (!(manifest.contributes?.breakpoints || []).some((entry) => entry.language === "tua")) {
  console.error("Missing Tua breakpoint contribution.");
  process.exit(1);
}

if (!manifest.activationEvents?.includes("onDebugResolve:lua-local")) {
  console.error("Missing lua-local debug resolution activation event.");
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

const configurationProperties = manifest.contributes?.configuration?.properties || {};
for (const setting of ["tua.completion.inline.sources.statistical"]) {
  const contribution = configurationProperties[setting];
  if (contribution?.type !== "boolean" || contribution.default !== false) {
    console.error(`${setting} must remain an explicit default-off experiment.`);
    process.exit(1);
  }
  if (!/experimental|experimentally/i.test(contribution.description || "")) {
    console.error(`${setting} must be described as experimental.`);
    process.exit(1);
  }
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

const tomlLanguage = languages.find((language) => language.id === "toml");
if (!tomlLanguage?.filenames?.includes("tua.toml")) {
  console.error("tua.toml should be associated with the TOML language.");
  process.exit(1);
}

if (!manifest.activationEvents?.includes("workspaceContains:**/tua.toml")) {
  console.error("A workspace containing tua.toml should activate the extension.");
  process.exit(1);
}

const editorTitle = manifest.contributes?.menus?.["editor/title"] || [];
const editorTitleCommands = editorTitle.flatMap((item) =>
  item.submenu ? manifest.contributes?.menus?.[item.submenu] || [] : [item],
);
for (const command of [
  "tua.runLoveProject",
  "tua.stopLoveProject",
  "tua.debugLoveProject",
  "tua.restartLoveProject",
  "tua.runLoveProjectWithHotReload",
]) {
  if (!editorTitleCommands.some((item) => item.command === command && item.when.includes("editorLangId == tua"))) {
    console.error(`LOVE editor title command should be scoped to Tua editors: ${command}`);
    process.exit(1);
  }
}

const assetsView = (manifest.contributes?.views?.explorer || []).find(
  (view) => view.id === "tua.assets",
);
if (assetsView?.when !== "tua.hasProject") {
  console.error("The Tua Assets view should only appear when a Tua project is present.");
  process.exit(1);
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
