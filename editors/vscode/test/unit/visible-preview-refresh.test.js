"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const { createVisiblePreviewRefresh } = require("../../src/shared/visible-preview-refresh");

const wait = ms => new Promise(resolve => setTimeout(resolve, ms));

test("hidden previews coalesce edits and refresh once when revealed", async () => {
  const panel = { visible: false };
  let calls = 0;
  const refresh = createVisiblePreviewRefresh(() => panel, () => { calls++; }, 5);
  refresh.schedule();
  refresh.schedule();
  await wait(15);
  assert.equal(calls, 0);
  panel.visible = true;
  refresh.onVisible();
  refresh.onVisible();
  assert.equal(calls, 1);
  refresh.schedule();
  await wait(15);
  assert.equal(calls, 2);
  refresh.cancel();
});

test("a pending refresh waits if the panel becomes hidden before its timer fires", async () => {
  const panel = { visible: true };
  let calls = 0;
  const refresh = createVisiblePreviewRefresh(() => panel, () => { calls++; }, 5);
  refresh.schedule();
  panel.visible = false;
  await wait(15);
  assert.equal(calls, 0);
  panel.visible = true;
  refresh.onVisible();
  assert.equal(calls, 1);
  refresh.cancel();
});
