"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const Module = require("node:module");

let receive;
const executed = [];
const saved = [];
const vscode = {
  ViewColumn: { Beside: 2 },
  Uri: { file: fsPath => ({ fsPath }) },
  extensions: { getExtension: id => id === "ms-vscode.vscode-js-profile-flame" ? {} : undefined },
  commands: { executeCommand: async (...args) => { executed.push(args); } },
  window: {
    createWebviewPanel: () => ({ webview: { onDidReceiveMessage: callback => { receive = callback; } } }),
    showSaveDialog: async () => ({ fsPath: saved.at(-1) }),
    showInformationMessage: async () => {},
    showErrorMessage: async message => assert.fail(message),
  },
};
const original = Module._load;
Module._load = function(id, ...args) { return id === "vscode" ? vscode : original.call(this, id, ...args); };
const { showProfile } = require("../../src/profiler/view");
Module._load = original;

test("profile view exports and opens the same CPU capture in the flame custom editor", async t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-profile-actions-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  saved.push(path.join(root, "keep.cpuprofile"));
  const cpuProfile = { nodes: [], samples: [], timeDeltas: [], startTime: 0, endTime: 1 };
  showProfile({ cpuProfile, reason: "duration", durationMs: 1, p50: 1, p95: 1, worst: 1,
    sampleCount: 1, missedBudget: 0, retainedFrames: 0, budgetMs: 16.67,
    slowFrames: [], stacks: [], traces: [], recentTraces: [] });
  await receive({ type: "exportCpu" });
  assert.deepEqual(JSON.parse(fs.readFileSync(saved[0], "utf8")), cpuProfile);
  await receive({ type: "openFlame" });
  const [command, uri, viewType] = executed.at(-1);
  assert.equal(command, "vscode.openWith");
  assert.equal(viewType, "jsProfileVisualizer.cpuprofile.flame");
  assert.deepEqual(JSON.parse(fs.readFileSync(uri.fsPath, "utf8")), cpuProfile);
  fs.rmSync(path.dirname(uri.fsPath), { recursive: true, force: true });
});
