"use strict";
// Native LÖVE qualification for the temporary profiler launcher and LuaJIT sampler.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");
const Module = require("node:module");
const originalLoad = Module._load;
Module._load = function (id, ...args) { return id === "vscode" ? {} : originalLoad.call(this, id, ...args); };
const { createProfileLauncher, createReplayLauncher } = require("../src/profiler/launcher");
const { parseReport, buildProfileModel } = require("../src/profiler/model");
const { loadSourceMapsFromDirectory } = require("../src/editor/source-maps");
Module._load = originalLoad;

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
async function main() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-profiler-native-"));
  const build = path.join(root, "build");
  const source = path.join(root, "src", "main.tua");
  fs.mkdirSync(build);
  fs.mkdirSync(path.dirname(source));
  fs.writeFileSync(source, "function love.update(dt)\n  local x = 0\n  for i = 1, 1000000 do x = x + i end\nend\n");
  fs.writeFileSync(path.join(build, "conf.lua"), "io.stdout:setvbuf('no')\nfunction love.conf(t) t.window.visible = false; t.window.width = 64; t.window.height = 64; t.modules.audio = false; t.modules.joystick = false end\n");
  for (const name of ["tuaecs", "tuasignal", "tuabehavior", "tuastore", "tuaaudio"]) {
    fs.copyFileSync(path.resolve(__dirname, "../../../vendor/runtime", `${name}.lua`), path.join(build, `${name}.lua`));
  }
  fs.writeFileSync(path.join(build, "main.lua"), `local started = love.timer.getTime()
local ecs = require("tuaecs")
local world = ecs.world()
local marker = ecs.component("Marker", {})
local entity = world:create()
world:add(entity, marker, {})
local system = ecs.system(world, { marker }, function() end)
local bus = require("tuasignal").new()
bus:on("tick", function() end)
local treeApi = require("tuabehavior")
local runner = treeApi.new(treeApi.action(function() return "success" end)):instance({})
local audio = require("tuaaudio")({ buses = { master = { volume = 1 } } })
local tracedRequire, tracedSystem, tracedSignal, tracedRunner, tracedAudio = require, system.update, bus.emit, runner.update, audio.update
local checked = false
function love.load()
  local pixels = love.image.newImageData(1, 1)
  love.graphics.newImage(pixels)
end
function love.update(dt)
  local x = 0
  for i = 1, 1000000 do x = x + i end
  system:update(dt)
  bus:emit("tick")
  runner:update(dt)
  audio:update(dt)
  if not checked and love.timer.getTime() - started > 1.0 then
    assert(require ~= tracedRequire and system.update ~= tracedSystem and bus.emit ~= tracedSignal and runner.update ~= tracedRunner and audio.update ~= tracedAudio)
    print("TRACE_RESTORED")
    checked = true
  end
  if love.timer.getTime() - started > 1.5 then love.event.quit() end
end
jit.off(love.update, true)
`);
  fs.writeFileSync(path.join(build, "main.lua.map.json"), JSON.stringify({ version: 1, source: "../src/main.tua", generated: "main.lua", lineMappings: [
    { generatedLine: 7, sourceLine: 1 }, { generatedLine: 8, sourceLine: 2 },
    { generatedLine: 11, sourceLine: 3 }, { generatedLine: 21, sourceLine: 4 },
  ] }));
  const original = fs.readFileSync(path.join(build, "main.lua"));
  const launcher = createProfileLauncher(build, path.resolve(__dirname, "../assets/profiler/shim.lua"), 0.5,
    ["ecs", "signals", "behaviors", "audio", "assets"]);
  let child, errorChild, errorLauncher, inputChild, replayChild, inputLauncher, replayLauncher, output = "";
  try {
    child = spawn(process.env.TUA_LOVE || "/Applications/LOVE.app/Contents/MacOS/love", [launcher.directory], { cwd: root, stdio: ["ignore", "pipe", "pipe"] });
    child.stdout.on("data", data => { output += data; });
    child.stderr.on("data", data => { output += data; });
    child.on("error", error => { output += error.message; });
    for (let attempt = 0; attempt < 80 && !fs.existsSync(launcher.reportPath); attempt++) await sleep(100);
    assert(fs.existsSync(launcher.reportPath), `LÖVE did not write a profile: ${output}`);
    const report = parseReport(fs.readFileSync(launcher.reportPath, "utf8"));
    assert.equal(report.reason, "duration");
    assert(report.frameCount > 0);
    assert(report.sampleCount > 0, "LuaJIT profiler did not provide CPU samples");
    const model = buildProfileModel(report, loadSourceMapsFromDirectory(build), build);
    assert(model.stacks.some(stack => stack.location?.sourcePath === source),
      `CPU samples did not map to Tua source: ${JSON.stringify(report.stacks.slice(0, 3))}`);
    assert.equal(report.traceError, null);
    for (const kind of ["ecs", "signals", "behaviors", "audio", "assets"]) {
      assert(model.traces.some(trace => trace.kind === kind && trace.calls > 0),
        `Missing ${kind} trace: ${JSON.stringify(report.traces)}`);
    }
    assert(model.traces.some(trace => trace.kind === "ecs" && trace.source?.sourcePath === source),
      `ECS timing did not link to Tua source: ${JSON.stringify(report.traces)}`);
    for (let attempt = 0; attempt < 40 && !output.includes("TRACE_RESTORED"); attempt++) await sleep(100);
    assert.match(output, /TRACE_RESTORED/u, `Trace wrappers were not restored after capture: ${output}`);
    assert.deepEqual(fs.readFileSync(path.join(build, "main.lua")), original);
    assert.equal(fs.existsSync(path.join(build, "profiler.lua")), false);
    const failedBuild = path.join(root, "failure");
    fs.mkdirSync(failedBuild);
    fs.copyFileSync(path.join(build, "conf.lua"), path.join(failedBuild, "conf.lua"));
    fs.copyFileSync(path.join(build, "tuasignal.lua"), path.join(failedBuild, "tuasignal.lua"));
    fs.writeFileSync(path.join(failedBuild, "main.lua"), 'local bus = require("tuasignal").new()\nfunction love.update()\n  bus:emit("before_error")\n  error("profile boom")\nend\n');
    fs.writeFileSync(path.join(failedBuild, "main.lua.map.json"), JSON.stringify({ version: 1,
      source: "../src/main.tua", generated: "main.lua", lineMappings: [{ generatedLine: 4, sourceLine: 4 }] }));
    errorLauncher = createProfileLauncher(failedBuild, path.resolve(__dirname, "../assets/profiler/shim.lua"), 0, ["signals"]);
    errorChild = spawn(process.env.TUA_LOVE || "/Applications/LOVE.app/Contents/MacOS/love", [errorLauncher.directory], { cwd: root, stdio: ["ignore", "pipe", "pipe"] });
    errorChild.on("error", error => { output += error.message; });
    for (let attempt = 0; attempt < 50 && !fs.existsSync(errorLauncher.reportPath); attempt++) await sleep(100);
    assert(fs.existsSync(errorLauncher.reportPath), `LÖVE failure did not save a profile: ${output}`);
    const failed = parseReport(fs.readFileSync(errorLauncher.reportPath, "utf8"));
    const failureModel = buildProfileModel(failed, loadSourceMapsFromDirectory(failedBuild), failedBuild);
    assert.equal(failed.reason, "error");
    assert.match(failed.errorMessage, /profile boom/u);
    assert.equal(failureModel.failureLocation?.sourcePath, source);
    assert(failureModel.recentTraces.some(trace => trace.name === "before_error"));
    const inputBuild = path.join(root, "input-build");
    fs.mkdirSync(inputBuild);
    fs.copyFileSync(path.join(build, "conf.lua"), path.join(inputBuild, "conf.lua"));
    fs.writeFileSync(path.join(inputBuild, "main.lua"), `local started = love.timer.getTime()
local pressed = 0
function love.load() love.event.push("keypressed", "space", "space", false) end
function love.keypressed(key) if key == "space" then pressed = pressed + 1 end end
function love.update()
  if love.timer.getTime() - started > 1.0 then print("PRESSED:" .. pressed); love.event.quit() end
end
`);
    inputLauncher = createProfileLauncher(inputBuild, path.resolve(__dirname, "../assets/profiler/shim.lua"), 0.5, [], 12345);
    inputChild = spawn(process.env.TUA_LOVE || "/Applications/LOVE.app/Contents/MacOS/love", [inputLauncher.directory], { cwd: root, stdio: ["ignore", "pipe", "pipe"] });
    for (let attempt = 0; attempt < 60 && !fs.existsSync(inputLauncher.reportPath); attempt++) await sleep(100);
    assert(fs.existsSync(inputLauncher.reportPath), "Input capture did not write a report");
    const recorded = parseReport(fs.readFileSync(inputLauncher.reportPath, "utf8")).input;
    assert.equal(recorded.seed, 12345);
    assert.equal(recorded.dropped, 0);
    assert(recorded.events.some(event => event.name === "keypressed"));
    replayLauncher = createReplayLauncher(inputBuild, path.resolve(__dirname, "../assets/profiler/input.lua"), recorded);
    let replayOutput = "";
    replayChild = spawn(process.env.TUA_LOVE || "/Applications/LOVE.app/Contents/MacOS/love", [replayLauncher.directory], { cwd: root, stdio: ["ignore", "pipe", "pipe"] });
    replayChild.stdout.on("data", data => { replayOutput += data; });
    replayChild.stderr.on("data", data => { replayOutput += data; });
    for (let attempt = 0; attempt < 60 && !replayOutput.includes("PRESSED:"); attempt++) await sleep(100);
    assert.match(replayOutput, /PRESSED:1/u, `Replay did not reproduce one key press: ${replayOutput}`);
    process.stdout.write(`Native profiler: ${report.frameCount} frames, ${report.sampleCount} samples; Tua source linked.\n`);
  } finally {
    child?.kill();
    errorChild?.kill();
    inputChild?.kill();
    replayChild?.kill();
    errorLauncher?.dispose();
    inputLauncher?.dispose();
    replayLauncher?.dispose();
    launcher.dispose();
    fs.rmSync(root, { recursive: true, force: true });
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
