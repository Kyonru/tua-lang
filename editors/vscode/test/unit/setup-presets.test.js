"use strict";

const assert = require("assert/strict");
const test = require("node:test");
const { BACKUP_PREFIX, createPresetManager, PRESETS } = require("../../src/setup/presets");

function fixture(initial = {}, defaults = {}) {
  const values = { ...initial };
  const overrides = {};
  const saved = new Map();
  const writes = [];
  let failure;
  const state = {
    get: (key, fallback) => structuredClone(saved.get(key) ?? fallback),
    update: async (key, value) => {
      if (failure === "backup") throw new Error("storage unavailable");
      if (value === undefined) saved.delete(key);
      else saved.set(key, structuredClone(value));
    },
  };
  const workspace = {
    getConfiguration: () => ({
      inspect: (key) => ({ globalValue: values[key] }),
      get: (key) => Object.hasOwn(overrides, key) ? overrides[key] : values[key] ?? defaults[key],
      update: async (key, value, target) => {
        assert.equal(target, 1);
        if (failure === key) throw new Error(`cannot write ${key}`);
        writes.push([key, value]);
        if (value === undefined) delete values[key];
        else values[key] = value;
      },
    }),
  };
  const create = () => createPresetManager({ workspace, state, target: 1 });
  return { create, manager: create(), values, overrides, saved, writes, fail: (key) => { failure = key; } };
}

test("registration-free construction does not change any settings", () => {
  const f = fixture();
  assert.deepEqual(f.values, {});
  assert.equal(f.writes.length, 0);
});

test("layout restores explicit false values and removes previously unset settings", async () => {
  const initial = { "window.commandCenter": false, "tua.editor.toolbarMode": "full" };
  const f = fixture(initial);
  await f.manager.apply("layout");
  assert.deepEqual(f.values, PRESETS.layout);
  await f.manager.restore("layout");
  assert.deepEqual(f.values, initial);
  assert.equal(f.saved.size, 0);
});

test("repeated apply keeps the original backup across extension reloads", async () => {
  const f = fixture({ "workbench.colorTheme": "My light theme" });
  await f.manager.apply("theme");
  await f.create().apply("theme");
  await f.create().restore("theme");
  assert.equal(f.values["workbench.colorTheme"], "My light theme");
});

test("restore keeps later edits, including removing a setting", async () => {
  const f = fixture();
  await f.manager.apply("layout");
  f.values["workbench.editor.editorActionsLocation"] = "hidden";
  delete f.values["window.commandCenter"];
  const result = await f.manager.restore("layout");
  assert.equal(result.preserved, 2);
  assert.deepEqual(f.values, { "workbench.editor.editorActionsLocation": "hidden" });
});

test("reapplying after a manual change backs up the latest user choice", async () => {
  const f = fixture({ "workbench.colorTheme": "First" });
  await f.manager.apply("theme");
  f.values["workbench.colorTheme"] = "Second";
  await f.manager.apply("theme");
  await f.manager.restore("theme");
  assert.equal(f.values["workbench.colorTheme"], "Second");
});

test("theme, layout, and icon backups are independent", async () => {
  const f = fixture({ "workbench.iconTheme": null });
  await f.manager.apply("theme");
  await f.manager.apply("layout");
  await f.manager.apply("icons");
  await f.manager.restore("theme");
  assert.equal(f.values["workbench.colorTheme"], undefined);
  assert.equal(f.values["workbench.iconTheme"], "material-icon-theme");
  assert.equal(f.values["workbench.editor.editorActionsLocation"], "titleBar");
  await f.manager.restore("icons");
  assert.equal(f.values["workbench.iconTheme"], null);
});

test("workspace overrides are reported and never changed", async () => {
  const f = fixture();
  f.overrides["workbench.editor.editorActionsLocation"] = "default";
  const result = await f.manager.apply("layout");
  assert.deepEqual(result.overridden, ["workbench.editor.editorActionsLocation"]);
  assert.equal(f.overrides["workbench.editor.editorActionsLocation"], "default");
  await f.manager.restore("layout");
  assert.equal(f.overrides["workbench.editor.editorActionsLocation"], "default");
});

test("already matching inherited values stay inherited", async () => {
  const f = fixture({}, PRESETS.layout);
  assert.equal((await f.manager.apply("layout")).changed, 0);
  assert.deepEqual(f.values, {});
  assert.equal((await f.manager.restore("layout")).changed, 0);
});

test("failed backup writes cannot change settings", async () => {
  const f = fixture();
  f.fail("backup");
  await assert.rejects(f.manager.apply("theme"), /storage unavailable/);
  assert.equal(f.writes.length, 0);
});

test("a partial apply can be undone after a write failure", async () => {
  const f = fixture();
  f.fail("workbench.layoutControl.enabled");
  await assert.rejects(f.manager.apply("layout"), /cannot write/);
  assert.equal(f.values["window.commandCenter"], true);
  f.fail(undefined);
  await f.create().restore("layout");
  assert.deepEqual(f.values, {});
});

test("failed restore retains its backup for retry", async () => {
  const f = fixture({ "workbench.colorTheme": "Before" });
  await f.manager.apply("theme");
  f.fail("workbench.colorTheme");
  await assert.rejects(f.manager.restore("theme"), /cannot write/);
  assert(f.saved.has(BACKUP_PREFIX + "theme"));
  f.fail(undefined);
  await f.manager.restore("theme");
  assert.equal(f.values["workbench.colorTheme"], "Before");
});

test("rapid apply and restore execute in order", async () => {
  const f = fixture({ "workbench.colorTheme": "Before" });
  await Promise.all([f.manager.apply("theme"), f.manager.apply("theme"), f.manager.restore("theme")]);
  assert.equal(f.values["workbench.colorTheme"], "Before");
  assert.equal(f.writes.length, 2);
});

test("unknown preset names are rejected without mutation", async () => {
  const f = fixture();
  await assert.rejects(f.manager.apply("__proto__"), /Unknown/);
  await assert.rejects(f.manager.restore("missing"), /Unknown/);
  assert.equal(f.writes.length, 0);
});
