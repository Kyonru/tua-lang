"use strict";
// Optional paired native-LÖVE overhead probe. It is not a release threshold.
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");
const { createProfileLauncher } = require("../src/profiler/launcher");

function percentile(values, fraction) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1)];
}

function runLove(directory) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.env.TUA_LOVE || "/Applications/LOVE.app/Contents/MacOS/love", [directory],
      { stdio: ["ignore", "pipe", "pipe"] });
    let output = "";
    child.stdout.on("data", bytes => { output += bytes; });
    child.stderr.on("data", bytes => { output += bytes; });
    child.once("error", reject);
    const timeout = setTimeout(() => { child.kill(); reject(new Error(`LÖVE benchmark timed out: ${output}`)); }, 12000);
    child.once("exit", code => {
      clearTimeout(timeout);
      if (code !== 0) return reject(new Error(`LÖVE exited ${code}: ${output}`));
      const cpu = /^BENCHCPU:([^\r\n]+)/mu.exec(output);
      const frame = /^BENCHFRAME:([^\r\n]+)/mu.exec(output);
      if (!cpu || !frame) return reject(new Error(`LÖVE produced no benchmark data: ${output}`));
      resolve({ cpu: cpu[1].split(",").map(Number), frame: frame[1].split(",").map(Number) });
    });
  });
}

async function main() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-profiler-bench-"));
  const build = path.join(root, "build");
  fs.mkdirSync(build);
  fs.writeFileSync(path.join(build, "conf.lua"), `io.stdout:setvbuf("no")
function love.conf(t)
  t.window.visible = false; t.window.width = 64; t.window.height = 64; t.window.vsync = 0
  t.modules.audio = false; t.modules.joystick = false
end
`);
  fs.writeFileSync(path.join(build, "main.lua"), `local started = love.timer.getTime()
local cpu, frames, done = {}, {}, false
function love.update(dt)
  local before = love.timer.getTime()
  local sum = 0
  for i = 1, 300000 do sum = sum + i end
  cpu[#cpu + 1] = (love.timer.getTime() - before) * 1000
  frames[#frames + 1] = dt * 1000
  if not done and love.timer.getTime() - started > 1.5 then
    done = true
    print("BENCHCPU:" .. table.concat(cpu, ","))
    print("BENCHFRAME:" .. table.concat(frames, ","))
    love.event.quit()
  end
end
jit.off(love.update, true)
`);
  const pairs = [];
  try {
    for (let index = 0; index < 3; index++) {
      const baseline = await runLove(build);
      const launcher = createProfileLauncher(build, path.resolve(__dirname, "../assets/profiler/shim.lua"), 0);
      let capture;
      try { capture = await runLove(launcher.directory); }
      finally { launcher.dispose(); }
      pairs.push({ baseline, capture });
    }
    for (const metric of ["cpu", "frame"]) {
      for (const fraction of [0.5, 0.95]) {
        const ordinary = percentile(pairs.map(pair => percentile(pair.baseline[metric].slice(10), fraction)), 0.5);
        const profiled = percentile(pairs.map(pair => percentile(pair.capture[metric].slice(10), fraction)), 0.5);
        process.stdout.write(`${metric} p${Math.round(fraction * 100)}: ordinary ${ordinary.toFixed(3)} ms; capture ${profiled.toFixed(3)} ms; delta ${((profiled / ordinary - 1) * 100).toFixed(1)}%\n`);
      }
    }
    process.stdout.write("Paired hidden-window fixture: 3 runs each, 1.5 s/run, first 10 frames omitted; host-specific, not a game-wide guarantee.\n");
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
}

main().catch(error => { console.error(error); process.exitCode = 1; });
