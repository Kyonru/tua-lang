"use strict";

const assert = require("assert/strict");
const fs = require("fs");
const path = require("path");
const test = require("node:test");
const vm = require("vm");
const { createPresetManager } = require("../../src/setup/presets");

function delayedConfiguration({ brokenRestore = false, delayAll = true } = {}) {
  let restoringLayout = false;
  const values = new Map();
  const saved = new Map();
  const notifications = [];
  let waits = 0;
  const workspace = {
    getConfiguration: (section) => ({
      get: (key) => values.get(section ? `${section}.${key}` : key),
      inspect: (key) => section === "editor"
        ? { workspaceLanguageValue: values.get(`editor.${key}`) }
        : { globalValue: values.get(key) },
      update: async (key, value) => {
        if (brokenRestore && key === "workbench.editor.editorActionsLocation" && value === undefined) return;
        // Completing a write does not immediately deliver its configuration notification.
        const notify = () => value === undefined ? values.delete(key) : values.set(key, value);
        if (delayAll || restoringLayout) notifications.push(notify);
        else notify();
      },
    }),
  };
  const manager = createPresetManager({
    workspace, target: 1,
    state: {
      get: (key, fallback) => structuredClone(saved.get(key) ?? fallback),
      update: async (key, value) => value === undefined ? saved.delete(key) : saved.set(key, structuredClone(value)),
    },
  });
  const vscode = {
    workspace,
    ConfigurationTarget: { Global: 1 },
    commands: {
      executeCommand: async (command, mode) => {
        if (command === "tua.openSetup") return;
        if (command === "tua.inlayHints.selectMode") {
          await workspace.getConfiguration().update("editor.inlayHints.maximumLength", mode === "compact" ? 1 : undefined);
          return mode;
        }
        const operations = {
          "tua.setup.applyTheme": () => manager.apply("theme"),
          "tua.setup.restoreTheme": () => manager.restore("theme"),
          "tua.setup.applyLayout": () => manager.apply("layout"),
          "tua.setup.restoreLayout": () => manager.restore("layout"),
        };
        assert(operations[command], `unexpected command ${command}`);
        restoringLayout = command === "tua.setup.restoreLayout";
        try { return await operations[command](); }
        finally { restoringLayout = false; }
      },
    },
  };
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, "../suite/setup.js"), "utf8"), {
    module,
    process: { env: {} },
    console: { log() {} },
    require: (id) => {
      if (id === "vscode") return vscode;
      if (id === "./support") return {
        waitUntil: async (predicate, description) => {
          waits += 1;
          // Deterministic delivery of delayed host notifications, without sleeps.
          notifications.splice(0).forEach((notify) => notify());
          if (!await predicate()) throw new Error(`Timed out waiting for ${description}`);
        },
      };
      return require(id);
    },
  });
  return { run: module.exports.runSetupTests, values, saved, waits: () => waits };
}

test("setup integration waits for delayed configuration snapshots", async () => {
  const fixture = delayedConfiguration();
  await fixture.run();
  assert.equal(fixture.values.size, 0, "restored settings must remain unset");
  assert.equal(fixture.saved.size, 0, "restore must consume its backups");
  assert(fixture.waits() > 0);
});

test("setup integration still fails when layout restoration never arrives", async () => {
  const fixture = delayedConfiguration({ brokenRestore: true });
  await assert.rejects(fixture.run(), /Timed out waiting for workbench.editor.editorActionsLocation user setting/);
});

test("setup integration handles a delayed layout restore after synchronous apply", async () => {
  const fixture = delayedConfiguration({ delayAll: false });
  await fixture.run();
  assert.equal(fixture.values.size, 0);
});
