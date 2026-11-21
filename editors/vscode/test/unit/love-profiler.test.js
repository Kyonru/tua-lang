"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const Module = require("node:module");
const { spawnSync } = require("node:child_process");

const originalLoad = Module._load;
Module._load = function (id, ...args) { return id === "vscode" ? {} : originalLoad.call(this, id, ...args); };
const { createProfileLauncher, createReplayLauncher } = require("../../src/profiler/launcher");
const { parseReport, framePercentile, stackLocation, buildProfileModel } = require("../../src/profiler/model");
const { ProfileCapture } = require("../../src/profiler/capture");
const { renderProfileHtml, saveCpuProfile } = require("../../src/profiler/view");
const { loadSourceMapsFromDirectory } = require("../../src/editor/source-maps");
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

test("module-load failure writes a profile while preserving the original error", t => {
  const { root, build } = fixture(t);
  fs.writeFileSync(path.join(build, "main.lua"), 'error("startup boom")\n');
  fs.mkdirSync(path.join(root, "src"));
  fs.writeFileSync(path.join(root, "src", "main.tua"), 'error("startup boom")\n');
  fs.writeFileSync(path.join(build, "main.lua.map.json"), JSON.stringify({ version: 1,
    source: "../src/main.tua", generated: "main.lua", lineMappings: [{ generatedLine: 1, sourceLine: 1 }] }));
  const shim = path.resolve(__dirname, "../../assets/profiler/shim.lua");
  const launcher = createProfileLauncher(build, shim, 1);
  t.after(() => launcher.dispose());
  const result = spawnSync("lua", [path.join(launcher.directory, "main.lua")], { encoding: "utf8" });
  if (result.error?.code === "ENOENT") return t.skip("Lua interpreter unavailable");
  assert.equal(result.status, 1);
  assert.match(result.stderr, /startup boom/u);
  const report = parseReport(fs.readFileSync(launcher.reportPath, "utf8"));
  assert.equal(report.reason, "error");
  assert.match(report.errorMessage, /startup boom/u);
  assert.match(report.errorTraceback, /main\.lua:1/u);
  assert.equal(report.sampleCount, 0);
  assert.equal(buildProfileModel(report, loadSourceMapsFromDirectory(build), build).failureLocation?.sourcePath,
    path.join(root, "src", "main.tua"));
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

test("timed LuaJIT callbacks export a source-mapped V8 CPU profile", async t => {
  const { root, build } = fixture(t);
  const map = { generated: path.join(build, "main.lua"), sourcePath: path.join(root, "src", "main.tua"),
    lineMappings: [{ generatedLine: 9, sourceLine: 4 }] };
  const report = parseReport(JSON.stringify({ version: 1, reason: "duration", durationMs: 50,
    frameCount: 0, sampleCount: 3, frames: [], stacks: [
      { stack: `@${build}/main.lua:9\n[C]:0`, count: 2 },
      { stack: `@${build}/main.lua:10\n[C]:0`, count: 1 },
    ], sampleEvents: [{ stackId: 1, atMs: 10, count: 2 }, { stackId: 2, atMs: 20, count: 1 }],
    sampleEventsDropped: 0 }));
  const model = buildProfileModel(report, [map], build);
  const profile = model.cpuProfile;
  assert.deepEqual(profile.timeDeltas, [10000, 10000]);
  assert.equal(profile.samples.length, 2, "one timeline point per callback, even if a callback batches samples");
  assert.equal(profile.endTime, 50000);
  assert(profile.nodes.some(node => node.callFrame.url.endsWith("/src/main.tua") && node.callFrame.lineNumber === 3));
  assert.match(renderProfileHtml(model, "nonce"), /Open flame chart.*Export \.cpuprofile/u);
  const target = path.join(root, "capture.cpuprofile");
  await saveCpuProfile(model, target);
  assert.deepEqual(JSON.parse(fs.readFileSync(target, "utf8")), profile);
  assert.throws(() => parseReport(JSON.stringify({ ...report, sampleEvents: [{ stackId: 3, atMs: 1, count: 1 }] })), /invalid/u);
  assert.throws(() => parseReport(JSON.stringify({ ...report, sampleEvents: [
    { stackId: 1, atMs: 20, count: 1 }, { stackId: 1, atMs: 10, count: 1 }] })), /invalid/u);
  const legacy = buildProfileModel({ ...report, sampleEvents: undefined }, [map], build);
  assert.equal(legacy.cpuProfile, undefined);
  assert.doesNotMatch(renderProfileHtml(legacy, "nonce"), /id="export-cpu"/u);
});

test("temporary Lua shim writes bounded, parseable timed samples", t => {
  const { root } = fixture(t);
  const reportPath = path.join(root, "capture.json");
  const fixturePath = path.resolve(__dirname, "../fixtures/profiler-shim-mock.lua");
  const shimPath = path.resolve(__dirname, "../../assets/profiler/shim.lua");
  const result = spawnSync("lua", [fixturePath, reportPath, shimPath], { encoding: "utf8" });
  if (result.error?.code === "ENOENT") return t.skip("Lua interpreter unavailable");
  assert.equal(result.status, 0, result.stderr);
  const report = parseReport(fs.readFileSync(reportPath, "utf8"));
  assert.equal(report.sampleCount, 1);
  assert.equal(report.sampleEvents.length, 1);
  assert.equal(report.sampleEvents[0].stackId, 1);
  assert(report.sampleEvents[0].atMs >= 0);
  assert.equal(report.sampleEventsDropped, 0);
});

test("long captures export only the retained CPU window and disclose evicted callbacks", t => {
  const { root, build } = fixture(t);
  const reportPath = path.join(root, "long-capture.json");
  const fixturePath = path.resolve(__dirname, "../fixtures/profiler-shim-mock.lua");
  const shimPath = path.resolve(__dirname, "../../assets/profiler/shim.lua");
  const result = spawnSync("lua", [fixturePath, reportPath, shimPath, "10000"], { encoding: "utf8" });
  if (result.error?.code === "ENOENT") return t.skip("Lua interpreter unavailable");
  assert.equal(result.status, 0, result.stderr);
  const report = parseReport(fs.readFileSync(reportPath, "utf8"));
  assert.equal(report.sampleEvents.length, 4096);
  assert.equal(report.sampleEventsDropped, 5904);
  const model = buildProfileModel(report, [], build);
  assert.equal(model.cpuProfile.startTime, Math.round(report.sampleEvents[0].atMs * 1000));
  assert.equal(model.cpuProfile.timeDeltas[0], 0);
  assert(model.cpuProfile.endTime - model.cpuProfile.startTime < 42000000);
  assert.match(renderProfileHtml(model, "nonce"), /5904 earlier CPU callbacks were evicted/u);
});

test("capture reads a completed report once and does not warn after a failed build", t => {
  const { build } = fixture(t);
  const reports = [], errors = [], done = [];
  const capture = new ProfileCapture({ durationSeconds: 1,
    assetsDirectory: path.resolve(__dirname, "../../assets/profiler"),
    onReport: report => reports.push(report), onError: error => errors.push(error), onDone: () => done.push("failed-build") });
  capture.finish();
  assert.equal(errors.length, 0);
  assert.deepEqual(done, ["failed-build"]);
  const second = new ProfileCapture({ durationSeconds: 1,
    assetsDirectory: path.resolve(__dirname, "../../assets/profiler"),
    onReport: report => reports.push(report), onError: error => errors.push(error), onDone: () => done.push("report") });
  second.stop();
  second.launch(build);
  assert.equal(fs.existsSync(second.launcher.stopPath), true, "Stop during build must take effect when LÖVE starts");
  fs.writeFileSync(second.launcher.reportPath, JSON.stringify({ version: 1, reason: "manual",
    durationMs: 1, frameCount: 0, sampleCount: 0, droppedSamples: 0, frames: [], stacks: [] }));
  second.readReport();
  second.finish();
  assert.equal(reports.length, 1);
  assert.equal(reports[0].reason, "manual");
  assert.equal(errors.length, 0);
  assert.deepEqual(done, ["failed-build", "report"]);
});

test("input replay launcher rejects incomplete records and keeps injected data private", t => {
  const { build } = fixture(t);
  const inputPath = path.resolve(__dirname, "../../assets/profiler/input.lua");
  const recorded = { seed: 42, dropped: 0, events: [{ frame: 1, name: "keypressed", args: ["space", "space", false] }] };
  assert.throws(() => createReplayLauncher(build, inputPath, { ...recorded, dropped: 1 }), /incomplete/u);
  assert.throws(() => createReplayLauncher(build, inputPath, { ...recorded,
    events: [{ frame: 1, name: "filedropped", args: ["secret"] }] }), /invalid/u);
  const launcher = createReplayLauncher(build, inputPath, recorded);
  assert.match(fs.readFileSync(path.join(launcher.directory, "main.lua"), "utf8"), /replay\.install/u);
  assert.equal(fs.readFileSync(path.join(build, "main.lua"), "utf8"), "function love.update() end\n");
  const data = fs.readFileSync(path.join(path.dirname(launcher.directory), "input.lua"), "utf8");
  assert.match(data, /frame = 1, name = "\\107\\101/u);
  assert.equal(fs.statSync(path.dirname(launcher.directory)).mode & 0o777, 0o700);
  launcher.dispose();
  assert.equal(fs.existsSync(launcher.directory), false);
});

test("profile metadata preserves disabled JIT, benchmark context, and legacy unknowns", t => {
  const { root, build } = fixture(t);
  const reportPath = path.join(root, "context.json");
  const result = spawnSync("lua", [path.resolve(__dirname, "../fixtures/profiler-shim-mock.lua"), reportPath,
    path.resolve(__dirname, "../../assets/profiler/shim.lua")], { encoding: "utf8" });
  if (result.error?.code === "ENOENT") return t.skip("Lua interpreter unavailable");
  assert.equal(result.status, 0, result.stderr);
  const report = parseReport(fs.readFileSync(reportPath, "utf8"));
  assert.equal(report.metadata.jitEnabledStart, false);
  assert.equal(report.metadata.jitEnabledEnd, false);
  const model = buildProfileModel(report, [], build);
  assert.equal(model.metadata.loveVersion, "11.5.0");
  assert.equal(model.metadata.gcPolicy, "running");
  assert.equal(model.metadata.benchmark.warmupUpdates, 10);
  assert.equal(model.cpuProfile.tua.metadata.benchmark.revision, "test");
  assert.equal(model.cpuProfile.tua.durationMs, report.durationMs);
  assert.match(renderProfileHtml(model, "nonce"), /JIT disabled at start/u);
  const legacy = buildProfileModel({ ...report, metadata: undefined }, [], build);
  assert.equal(legacy.metadata.jitEnabledStart, null);
  assert.equal(legacy.metadata.gcPolicy, "unknown");
  assert.equal(legacy.metadata.benchmark, null);
  assert.match(renderProfileHtml(legacy, "nonce"), /JIT unknown/u);
  const malformed = buildProfileModel({ ...report, metadata: { jitEnabledStart: "false", benchmark: { repetitions: -1 } } }, [], build);
  assert.equal(malformed.metadata.jitEnabledStart, null);
  assert.equal(malformed.metadata.benchmark.repetitions, null);
});

test("profile launcher fingerprints the existing build manifest without inventing revisions", t => {
  const { build } = fixture(t);
  fs.writeFileSync(path.join(build, ".tua-build-manifest.json"), '{"main.lua":123}');
  const launcher = createProfileLauncher(build, path.resolve(__dirname, "../../assets/profiler/shim.lua"), 1, [], undefined,
    { target: "love11-luajit", benchmark: { workload: "fixed", warmupUpdates: 600, repetitions: 5, revision: "abc" } });
  t.after(() => launcher.dispose());
  const bootstrap = fs.readFileSync(path.join(launcher.directory, "main.lua"), "utf8");
  const decoded = bootstrap.replace(/\\(\d{3})/gu, (_, code) => String.fromCharCode(Number(code)));
  assert.match(decoded, /manifest-sha256:[a-f0-9]{64}/u);
  assert.match(bootstrap, /warmupUpdates = 600/u);
  assert.match(decoded, /gcPolicy = "unknown"/u);
});
