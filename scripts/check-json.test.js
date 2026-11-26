"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const { checkJsonFiles, isVendoredEditorConfig } = require("./check-json");

test("strict JSON validation excludes only upstream editor JSONC", (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-json-check-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const fixtures = {
    "vendor/formatter/.vscode/settings.json": '{ // upstream comment\n "editor.tabSize": 2,\n}',
    "vendor/formatter/package.json": '{ "name": "formatter", }',
    "config/data.json": '{ // invalid strict JSON\n}',
    "config/valid.json": '{ "url": "https://example.test" }',
  };
  for (const [file, contents] of Object.entries(fixtures)) {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), contents);
  }
  const result = checkJsonFiles(root, Object.keys(fixtures));
  assert.equal(result.checked, 3);
  assert.deepEqual(result.jsonc, ["vendor/formatter/.vscode/settings.json"]);
  assert.equal(result.errors.length, 2);
  assert(result.errors.some((error) => error.startsWith("vendor/formatter/package.json:")));
  assert(result.errors.some((error) => error.startsWith("config/data.json:")));
  assert.equal(isVendoredEditorConfig(".vscode/settings.json"), false);
  assert.equal(isVendoredEditorConfig("vendor/formatter/.vscode/data.json"), false);
});
