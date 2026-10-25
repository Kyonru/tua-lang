"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const { DEFAULT_CONFIGURATIONS, normalizeConfigurations } = require("../../src/workspace/run-options");

test("named run configurations accept supported modes and strip unrelated fields", () => {
  assert.deepEqual(normalizeConfigurations(DEFAULT_CONFIGURATIONS), DEFAULT_CONFIGURATIONS);
  assert.deepEqual(normalizeConfigurations([{ name: " Release ", mode: "package", packageTarget: "macos", command: "unexpected" }]), [{ name: "Release", mode: "package", packageTarget: "macos" }]);
});

test("ambiguous names and unsupported actions cannot be selected for execution", () => {
  assert.throws(() => normalizeConfigurations([{ name: "Game", mode: "run" }, { name: "Game ", mode: "debug" }]), /unique/);
  assert.throws(() => normalizeConfigurations([{ name: "Game", mode: "shell" }]), /Unknown run mode/);
  assert.throws(() => normalizeConfigurations([{ name: "Game", mode: "package", packageTarget: "unknown" }]), /Unknown package target/);
  assert.throws(() => normalizeConfigurations([]), /at least one/);
});

test("normal Run preserves named players and literal arguments", () => {
  assert.deepEqual(normalizeConfigurations([{ name: "Local multiplayer", mode: "run", instances: [
    { name: " Host ", args: ["--host", "two words", "$(literal)"] }, { name: "Client" },
  ] }])[0].instances, [
    { name: "Host", args: ["--host", "two words", "$(literal)"] }, { name: "Client", args: [] },
  ]);
});

test("invalid instance groups fail before launching anything", () => {
  const normalize = instances => normalizeConfigurations([{ name: "Players", mode: "run", instances }]);
  for (const instances of [[], null, "two", [{ name: "" }], [{ name: "A" }, { name: " A " }],
    [{ name: "A", args: "--host" }], [{ name: "A", args: [42] }], [{ name: "A", args: ["\0"] }],
    Array.from({ length: 17 }, (_, i) => ({ name: String(i) }))]) assert.throws(() => normalize(instances));
  for (const mode of ["package"]) {
    assert.throws(() => normalizeConfigurations([{ name: "Players", mode, instances: [{ name: "A" }] }]), /not supported in Package mode/);
  }
});


test("debug and hot reload preserve named instances", () => {
  for (const mode of ["debug", "hotReload"]) {
    const result = normalizeConfigurations([{ name: "Players", mode, instances: [{ name: "Host", args: ["--host"] }, { name: "Client" }] }]);
    assert.deepEqual(result[0].instances, [{ name: "Host", args: ["--host"] }, { name: "Client", args: [] }]);
  }
});
