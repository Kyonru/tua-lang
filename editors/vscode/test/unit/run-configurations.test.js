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
