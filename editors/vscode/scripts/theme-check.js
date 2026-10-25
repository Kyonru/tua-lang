"use strict";

const assert = require("assert/strict");
const fs = require("fs");
const path = require("path");
const root = path.resolve(__dirname, "..");
const manifest = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
const theme = JSON.parse(fs.readFileSync(path.join(root, "themes/indigo-carbon.json"), "utf8"));
const colors = theme.colors;
let checks = 0;
let minimumText = Infinity;
let minimumFocus = Infinity;

function rgb(hex) {
  return [1, 3, 5].map((index) => parseInt(hex.slice(index, index + 2), 16) / 255);
}

function luminance(values) {
  return values.reduce((sum, value, index) =>
    sum + (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4) * [0.2126, 0.7152, 0.0722][index], 0);
}

function check(label, foreground, background, minimum = 4.5) {
  const alpha = foreground.length === 9 ? parseInt(foreground.slice(7), 16) / 255 : 1;
  const bg = rgb(background);
  const fg = rgb(foreground).map((value, index) => value * alpha + bg[index] * (1 - alpha));
  const a = luminance(fg);
  const b = luminance(bg);
  const ratio = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
  assert(ratio >= minimum, `${label}: ${ratio.toFixed(2)}:1 is below ${minimum}:1`);
  if (minimum === 3) minimumFocus = Math.min(minimumFocus, ratio);
  else minimumText = Math.min(minimumText, ratio);
  checks += 1;
}

for (const [key, value] of Object.entries(colors)) {
  assert.match(value, /^#[0-9a-f]{6}([0-9a-f]{2})?$/i, `Invalid theme color: ${key}`);
}
for (const rule of theme.tokenColors) {
  for (const background of [colors["editor.background"], colors["editor.selectionBackground"]]) {
    check(rule.name, rule.settings.foreground, background);
  }
}
for (const [selector, value] of Object.entries(theme.semanticTokenColors)) {
  const foreground = typeof value === "string" ? value : value.foreground;
  if (!foreground) continue;
  check(selector, foreground, colors["editor.background"]);
  check(`${selector} selected`, foreground, colors["editor.selectionBackground"]);
}
for (const prefix of ["tab.active", "tab.unfocusedActive", "editorSuggestWidget.selected", "list.focus"]) {
  check(prefix, colors[`${prefix}Foreground`], colors[`${prefix}Background`]);
}
check("inlay hints", colors["editorInlayHint.foreground"], colors["editor.background"]);
for (const key of ["editor.background", "list.activeSelectionBackground", "editorWidget.background"]) {
  check("keyboard focus", colors.focusBorder, colors[key], 3);
}
for (let index = 1; index <= 4; index += 1) {
  assert(theme.semanticTokenColors[`property.propertyChain${index}:tua`]);
  assert(manifest.contributes.semanticTokenModifiers.some((modifier) => modifier.id === `propertyChain${index}`));
}
const contribution = manifest.contributes.themes.find((item) => item.label === "Indigo Carbon HC");
assert.equal(contribution?.path, "./themes/indigo-carbon.json");
assert.equal(contribution?.uiTheme, "vs-dark");
for (const key of Object.keys(manifest.contributes.configurationDefaults)) {
  assert(!/^(workbench|window)\./.test(key), `Appearance/layout must remain opt-in: ${key}`);
}
assert(!manifest.extensionDependencies?.includes("PKief.material-icon-theme"));
console.log(`Theme: ${checks} contrast checks; text minimum ${minimumText.toFixed(2)}:1, focus minimum ${minimumFocus.toFixed(2)}:1. Appearance is opt-in.`);
