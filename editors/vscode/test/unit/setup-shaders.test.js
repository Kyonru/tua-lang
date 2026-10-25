"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const { createShaderSetup, BACKUP_KEY } = require("../../src/setup/shaders");
function fixture(initial) {
  let value = initial;
  const saved = new Map();
  const state = { get: (key, fallback) => structuredClone(saved.get(key) ?? fallback), update: async (key, data) => saved.set(key, structuredClone(data)) };
  const workspace = { workspaceFolders: [{}], getConfiguration: () => ({
    inspect: () => ({ workspaceValue: value }),
    update: async (_key, data, target) => { assert.equal(target, 2); value = data; },
  }) };
  return { workspace, saved, get: () => value, set: (data) => { value = data; }, create: () => createShaderSetup({ workspace, state, target: 2 }) };
}
test("shader setup restores previous associations across reloads and repeated apply", async () => {
  const f = fixture({ "*.glsl": "glsl", "*.foo": "text" });
  await f.create().apply();
  await f.create().apply();
  assert.equal(f.get()["*.glsl"], "love-glsl");
  await f.create().restore();
  assert.deepEqual(f.get(), { "*.glsl": "glsl", "*.foo": "text" });
  assert.equal(f.saved.get(BACKUP_KEY), undefined);
});
test("shader restore preserves subsequent edits to owned and unrelated keys", async () => {
  const f = fixture();
  await f.create().apply();
  f.set({ ...f.get(), "*.vert": "glsl", "*.custom": "text" });
  delete f.get()["*.fs"];
  await f.create().restore();
  assert.deepEqual(f.get(), { "*.vert": "glsl", "*.custom": "text" });
});
test("shader setup requires a workspace and does not touch settings at construction", async () => {
  const f = fixture();
  f.workspace.workspaceFolders = [];
  const manager = f.create();
  assert.equal(f.saved.size, 0);
  await assert.rejects(manager.apply(), /Open a workspace/);
});
