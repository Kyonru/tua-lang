"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const { emitMapsSetting, probeVersion } = require("../../src/workspace/check-helpers");

test("source-map fixes target only the root boolean and preserve comments and tables", () => {
  const text = '# emit_maps = false\r\n"emit_maps" = false # debug maps\r\n[other]\r\nemit_maps = true\r\n';
  const setting = emitMapsSetting(text);
  assert.equal(setting.enabled, false);
  assert.equal(text.slice(setting.start, setting.end), "false");
  const updated = text.slice(0, setting.start) + "true" + text.slice(setting.end);
  assert.equal(emitMapsSetting(updated).enabled, true);
  assert.match(updated, /true # debug maps/);
  assert.equal(emitMapsSetting('# emit_maps = false\n[other]\nemit_maps = false').enabled, true);
});

test("version probes report missing executables and successful launches", async () => {
  assert.equal((await probeVersion('/definitely-missing/tua')).ok, false);
  assert.equal((await probeVersion(process.execPath)).ok, true);
});
