"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const Module = require("node:module");

const originalLoad = Module._load;
Module._load = function (id, ...args) { return id === "vscode" ? {} : originalLoad.call(this, id, ...args); };
const { createProfileLauncher } = require("../../src/profiler/launcher");
const { parseReport, framePercentile, stackLocation, buildProfileModel } = require("../../src/profiler/model");
const { ProfileCapture } = require("../../src/profiler/capture");
Module._load = originalLoad;

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-profiler-test-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const build = path.join(root, "build");
  fs.mkdirSync(build);
  fs.writeFileSync(path.join(build, "main.lua"), "function love.update() end\n");
  fs.writeFileSync(path.join(build, "conf.lua"), "function love.conf(t) t.window.visible = false end\n");
  fs.mkdirSync(path.join(build, "assets"));
  fs.writeFileSync(path.join(build, "assets", "sprite.png"), "image");
  return { root, build };
}

test("temporary profile launcher preserves the build, assets, and cleanup", t => {
  const { build } = fixture(t);
  const shim = path.resolve(__dirname, "../../assets/profiler/shim.lua");
  const launch = createProfileLauncher(build, shim, 10);
  assert.notEqual(launch.directory, build);
  assert.match(fs.readFileSync(path.join(launch.directory, "main.lua"), "utf8"), /profiler\.install\(\)/u);
  assert.equal(fs.readFileSync(path.join(build, "main.lua"), "utf8"), "function love.update() end\n");
  assert.equal(fs.readFileSync(path.join(launch.directory, "conf.lua"), "utf8"), fs.readFileSync(path.join(build, "conf.lua"), "utf8"));
  assert.equal(fs.readFileSync(path.join(launch.directory, "assets", "sprite.png"), "utf8"), "image");
  launch.stop();
  assert.equal(fs.readFileSync(launch.stopPath, "utf8"), "stop\n");
  launch.dispose();
  assert.equal(fs.existsSync(launch.directory), false);
  assert.equal(fs.existsSync(path.join(build, "main.lua")), true);
  assert.equal(fs.existsSync(path.join(build, "profiler.lua")), false);
});

test("profile model preserves bounded samples and maps LuaJIT stack paths", () => {
  const build = path.join(os.tmpdir(), "tua-profile-model-build");
  const map = { generated: path.join(build, "main.lua"), sourcePath: "/project/src/main.tua",
    lineMappings: [{ generatedLine: 9, sourceLine: 4 }] };
  const report = parseReport(JSON.stringify({ version: 1, reason: "duration", durationMs: 100,
    frameCount: 3, sampleCount: 4, droppedSamples: 0,
    frames: [{ index: 1, wallMs: 5 }, { index: 2, wallMs: 20 }, { index: 3, wallMs: 10 }],
    stacks: [{ stack: `@${build}/main.lua:9\n[C]:0`, count: 4 }] }));
  assert.equal(framePercentile(report.frames, 0.95), 20);
  assert.equal(stackLocation(report.stacks[0].stack, [map], build).sourceLine, 4);
  const model = buildProfileModel(report, [map], build);
  assert.equal(model.slowFrames[0].index, 2);
  assert.equal(model.stacks[0].location.sourcePath, "/project/src/main.tua");
  assert.equal(model.stacks[0].share, 1);
  const traced = parseReport(JSON.stringify({ ...report, traces: [{ kind: "ecs", name: "system",
    location: `@${build}/main.lua:9`, calls: 3, totalMs: 7, worstMs: 4 }], recentTraces: [] }));
  assert.equal(buildProfileModel(traced, [map], build).traces[0].source.sourceLine, 4);
  assert.throws(() => parseReport(JSON.stringify({ ...report, traces: [{ kind: "ecs", name: "x",
    calls: 1, totalMs: -1, worstMs: 0 }] })), /invalid/u);
  assert.throws(() => parseReport(JSON.stringify({ ...report, frames: Array(2049).fill(report.frames[0]) })), /invalid/u);
  assert.throws(() => parseReport("x".repeat(2 * 1024 * 1024 + 1)), /exceeds 2 MiB/u);
});

test("capture reads a completed report once and does not warn after a failed build", t => {
  const { build } = fixture(t);
  const reports = [], errors = [];
  const capture = new ProfileCapture({ durationSeconds: 1,
    assetsDirectory: path.resolve(__dirname, "../../assets/profiler"),
    onReport: report => reports.push(report), onError: error => errors.push(error) });
  capture.finish();
  assert.equal(errors.length, 0);
  const second = new ProfileCapture({ durationSeconds: 1,
    assetsDirectory: path.resolve(__dirname, "../../assets/profiler"),
    onReport: report => reports.push(report), onError: error => errors.push(error) });
  second.launch(build);
  fs.writeFileSync(second.launcher.reportPath, JSON.stringify({ version: 1, reason: "manual",
    durationMs: 1, frameCount: 0, sampleCount: 0, droppedSamples: 0, frames: [], stacks: [] }));
  second.readReport();
  second.finish();
  assert.equal(reports.length, 1);
  assert.equal(reports[0].reason, "manual");
  assert.equal(errors.length, 0);
});
