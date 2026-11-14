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
const { createProfileLauncher } = require("../src/profiler/launcher");
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
  fs.writeFileSync(path.join(build, "conf.lua"), "function love.conf(t) t.window.visible = false; t.window.width = 64; t.window.height = 64; t.modules.audio = false; t.modules.joystick = false end\n");
  fs.writeFileSync(path.join(build, "main.lua"), `local started = love.timer.getTime()
function love.update(dt)
  local x = 0
  for i = 1, 1000000 do x = x + i end
  if love.timer.getTime() - started > 1.5 then love.event.quit() end
end
jit.off(love.update, true)
`);
  fs.writeFileSync(path.join(build, "main.lua.map.json"), JSON.stringify({ version: 1, source: "../src/main.tua", generated: "main.lua", lineMappings: [{ generatedLine: 4, sourceLine: 3 }] }));
  const original = fs.readFileSync(path.join(build, "main.lua"));
  const launcher = createProfileLauncher(build, path.resolve(__dirname, "../assets/profiler/shim.lua"), 0.5);
  let child, output = "";
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
    assert.deepEqual(fs.readFileSync(path.join(build, "main.lua")), original);
    assert.equal(fs.existsSync(path.join(build, "profiler.lua")), false);
    process.stdout.write(`Native profiler: ${report.frameCount} frames, ${report.sampleCount} samples; Tua source linked.\n`);
  } finally {
    child?.kill();
    launcher.dispose();
    fs.rmSync(root, { recursive: true, force: true });
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
