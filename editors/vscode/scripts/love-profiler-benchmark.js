"use strict";
// Optional paired native-LÖVE overhead probe. It is not a release threshold.
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");
const { createHash } = require("node:crypto");
const REPETITIONS = 3, WARMUP_UPDATES = 10;
const WORKLOAD = "sum-300000-hidden-window-v1";
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
      const metadata = /^BENCHMETA:([^\r\n]+)/mu.exec(output);
      if (!metadata) return reject(new Error("missing benchmark runtime metadata"));
      resolve({ cpu: cpu[1].split(",").map(Number), frame: frame[1].split(",").map(Number), runtime: JSON.parse(metadata[1]) });
    });
  });
}

async function main() {
  const revision = spawnSync("git", ["rev-parse", "HEAD"], { cwd: path.resolve(__dirname, "../../.."), encoding: "utf8" }).stdout.trim() || "unknown";
  const lovePath = process.env.TUA_LOVE || "/Applications/LOVE.app/Contents/MacOS/love";
  const hash = file => createHash("sha256").update(fs.readFileSync(file)).digest("hex");
  const provenance = { revision, workload: WORKLOAD, warmupUpdates: WARMUP_UPDATES, repetitions: REPETITIONS,
    lovePath, loveSha256: hash(lovePath), harnessSha256: hash(__filename), gcPolicy: "running", allocation: "not measured", order: "alternating independent processes" };
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-profiler-bench-"));
  const build = path.join(root, "build");
  fs.mkdirSync(build);
  fs.writeFileSync(path.join(build, "conf.lua"), `io.stdout:setvbuf("no")
function love.conf(t)
  t.window.visible = false; t.window.width = 64; t.window.height = 64; t.window.vsync = 0
  t.modules.audio = false; t.modules.joystick = false
end
`);
  fs.writeFileSync(path.join(build, "main.lua"), `jit.off()
collectgarbage("restart")
local major, minor, patch = love.getVersion()
print(string.format('BENCHMETA:{"loveVersion":"%d.%d.%d","luaJitVersion":"%s","jitEnabled":%s}', major, minor, patch, jit.version, tostring(jit.status())))
local started = love.timer.getTime()
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
    provenance.workloadSha256 = hash(path.join(build, "main.lua"));
    provenance.shimSha256 = hash(path.resolve(__dirname, "../assets/profiler/shim.lua"));
    for (let index = 0; index < REPETITIONS; index++) {
      const launcher = createProfileLauncher(build, path.resolve(__dirname, "../assets/profiler/shim.lua"), 0, [], undefined,
        { gcPolicy: "running", revision, benchmark: { workload: WORKLOAD, revision, warmupUpdates: WARMUP_UPDATES, repetitions: REPETITIONS } });
      let baseline, capture, profile;
      try {
        if (index % 2 === 0) { baseline = await runLove(build); capture = await runLove(launcher.directory); }
        else { capture = await runLove(launcher.directory); baseline = await runLove(build); }
        profile = JSON.parse(fs.readFileSync(launcher.reportPath, "utf8"));
      }
      finally { launcher.dispose(); }
      pairs.push({ baseline, capture, order: index % 2 === 0 ? ["baseline", "capture"] : ["capture", "baseline"],
        captureMetadata: profile.metadata, captureDurationMs: profile.durationMs, sampleCount: profile.sampleCount });
    }
    const summaries = [];
    for (const metric of ["cpu", "frame"]) {
      for (const fraction of [0.5, 0.95]) {
        const ordinary = percentile(pairs.map(pair => percentile(pair.baseline[metric].slice(WARMUP_UPDATES), fraction)), 0.5);
        const profiled = percentile(pairs.map(pair => percentile(pair.capture[metric].slice(WARMUP_UPDATES), fraction)), 0.5);
        summaries.push({ metric, percentile: fraction, ordinary, profiled,
          trials: pairs.map(pair => ({ ordinary: percentile(pair.baseline[metric].slice(WARMUP_UPDATES), fraction),
            profiled: percentile(pair.capture[metric].slice(WARMUP_UPDATES), fraction) })) });
        process.stdout.write(`${metric} p${Math.round(fraction * 100)}: ordinary ${ordinary.toFixed(3)} ms; capture ${profiled.toFixed(3)} ms; delta ${((profiled / ordinary - 1) * 100).toFixed(1)}%\n`);
      }
    }
    if (process.env.TUA_BENCH_OUTPUT) fs.writeFileSync(process.env.TUA_BENCH_OUTPUT, JSON.stringify({ provenance, pairs, summaries }, null, 2) + "\n");
    process.stdout.write(JSON.stringify(provenance) + "\n");
    process.stdout.write("Paired hidden-window fixture: 3 runs each, 1.5 s/run, first 10 frames omitted; alternating order. CPU column is wall duration of the workload block; frame is LÖVE dt (whole loop). GC-running heap/allocation is not measured. Profiled timings include instrumentation; host-specific, not a game-wide guarantee.\n");
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
}

main().catch(error => { console.error(error); process.exitCode = 1; });
