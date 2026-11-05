"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const Module = require("node:module");
const fs = require("node:fs");
const path = require("node:path");
let callback;
let opened;
const vscode = {
  Uri: { joinPath: (root, ...parts) => path.join(root, ...parts) },
  commands: {
    registerCommand: (_id, handler) => { callback = handler; return { dispose() {} }; },
    executeCommand: (...args) => { opened = args; },
  },
};
const originalLoad = Module._load;
Module._load = function (id, ...args) {
  return id === "vscode" ? vscode : originalLoad.call(this, id, ...args);
};
const { COMMAND, registerAnnotationGuide } = require("../../src/annotation-guide");
Module._load = originalLoad;

test("annotation guide opens only the fixed bundled resource, ignoring command arguments", () => {
  registerAnnotationGuide({ extensionUri: path.resolve(__dirname, "../..") });
  callback("https://example.com/untrusted");
  assert.equal(opened[0], "markdown.showPreview");
  assert.match(fs.readFileSync(opened[1], "utf8"), /Metadata only/);
  assert.ok(require("../../package.json").contributes.commands.some(item => item.command === COMMAND));
});
