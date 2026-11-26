"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const path = require("path");

function isVendoredEditorConfig(file) {
  // These upstream VS Code settings are JSONC, not strict JSON metadata.
  // Preserve their comments without exempting package manifests or data files.
  return /^vendor\/[^/]+\/\.vscode\/(extensions|launch|settings|tasks)\.json$/.test(file);
}

function checkJsonFiles(repoRoot, files) {
  const errors = [];
  const jsonc = files.filter(isVendoredEditorConfig);
  for (const file of files.filter((file) => !isVendoredEditorConfig(file))) {
    try {
      JSON.parse(fs.readFileSync(path.join(repoRoot, file), "utf8"));
    } catch (error) {
      errors.push(`${file}: ${error.message}`);
    }
  }
  return { errors, checked: files.length - jsonc.length, jsonc };
}

function main() {
  const repoRoot = path.resolve(__dirname, "..");
  const files = childProcess
    .execFileSync("git", ["ls-files", "-z", "--", "*.json"], {
      cwd: repoRoot,
      encoding: "utf8",
    })
    .split("\0")
    .filter(Boolean);
  const result = checkJsonFiles(repoRoot, files);
  for (const error of result.errors) console.error(error);
  if (result.jsonc.length) {
    console.log(`Excluded upstream editor JSONC from strict JSON validation: ${result.jsonc.join(", ")}`);
  }
  if (result.errors.length) process.exitCode = 1;
  else console.log(`validated ${result.checked} tracked JSON files`);
}

if (require.main === module) main();
module.exports = { checkJsonFiles, isVendoredEditorConfig };
