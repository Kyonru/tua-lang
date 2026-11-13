"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const { serverHealthViewModel } = require("../../src/health/view-model");
const { serverHealthHtml } = require("../../src/health/template");

const webview = { cspSource: "vscode-resource:" };

test("server health distinguishes unmeasured rates and latency from zero", () => {
  const empty = serverHealthHtml(webview, serverHealthViewModel({ revision: 1 }));
  assert.match(empty, /No request timings captured yet/);
  assert.match(empty, /Warm files<\/dt><dd>0\/0/);
  assert.match(empty, /<td>Parse<\/td><td>0<\/td><td>0<\/td><td>No samples<\/td>/);
  assert.match(empty, /Request p95<\/dt><dd>No samples/);
  assert.match(empty, /Snapshot<\/span><strong>not enabled/);
  assert.doesNotMatch(empty, /Request p95<\/dt><dd>0ms/);

  const measured = serverHealthHtml(webview, serverHealthViewModel({
    revision: 2,
    workspaceWarm: { enabled: true, discovered: 4, warmed: 2, coveragePercent: 50 },
    cache: { parse: { hits: 3, misses: 1, hitRate: 75 } },
    latency: { recentCount: 5, p50Ms: 2, p95Ms: 9, p99Ms: 10, maxMs: 12 },
  }));
  assert.match(measured, /Warm files<\/dt><dd>2\/4/);
  assert.match(measured, /<td>Parse<\/td><td>3<\/td><td>1<\/td><td>75%<\/td>/);
  assert.match(measured, /Request p95<\/dt><dd>9ms/);
  assert.match(measured, /Snapshot<\/span><strong>pending/);
});
