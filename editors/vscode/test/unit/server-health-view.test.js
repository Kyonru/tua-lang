"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const { serverHealthViewModel } = require("../../src/health/view-model");
const { serverHealthHtml } = require("../../src/health/template");

const webview = { cspSource: "vscode-resource:" };

test("server health distinguishes unmeasured rates and latency from zero", () => {
  const empty = serverHealthHtml(webview, serverHealthViewModel({ revision: 1 }));
  assert.match(empty, /Warm coverage<\/span><strong>No files/);
  assert.match(empty, /Cache hit rate<\/span><strong>No samples/);
  assert.match(empty, /Recent p95<\/span><strong>No samples/);
  assert.match(empty, /Snapshot<\/span><strong>not enabled/);
  assert.doesNotMatch(empty, /Recent p95<\/span><strong>0ms/);

  const measured = serverHealthHtml(webview, serverHealthViewModel({
    revision: 2,
    workspaceWarm: { enabled: true, discovered: 4, warmed: 2, coveragePercent: 50 },
    cache: { hits: 3, misses: 1, hitRate: 75 },
    latency: { recentCount: 5, p50Ms: 2, p95Ms: 9, p99Ms: 10, maxMs: 12 },
  }));
  assert.match(measured, /Warm coverage<\/span><strong>50%/);
  assert.match(measured, /Cache hit rate<\/span><strong>75%/);
  assert.match(measured, /Recent p95<\/span><strong>9ms/);
  assert.match(measured, /Snapshot<\/span><strong>pending/);
});
