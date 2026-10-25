"use strict";
// Runs the shipped inspector against real LÖVE processes; never edits a game.
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { createHash } = require("node:crypto");
const { spawn, execFileSync } = require("node:child_process");
const { ActivityStore } = require("../src/workspace/activity-store");
const { InspectorSessions } = require("../src/inspector/sessions");
const { RuntimeOrigins } = require("../src/inspector/origins");
const { presentObject } = require("../src/inspector/presentation");
const allowedModes = ["no-shim", "disabled-after-launch", "paused", "active"];
const assetsDirectory = process.env.TUA_INSPECTOR_ASSETS || path.resolve(__dirname, "../assets/inspector");
const modes = (process.env.TUA_BENCH_MODES || allowedModes.join(",")).split(",");
if (modes.some(mode => !allowedModes.includes(mode))) throw new Error("Unknown benchmark mode");
const seconds = Number(process.env.TUA_BENCH_SECONDS || 8);
const warmup = Number(process.env.TUA_BENCH_WARMUP || 3);
const repetitions = Number(process.env.TUA_BENCH_REPETITIONS || 3);
const profiles = (process.env.TUA_BENCH_PROFILES || "light,heavy").split(",");
if (profiles.some(profile => !["light", "heavy"].includes(profile))) throw new Error("Unknown benchmark profile");
const maxLoad = Number(process.env.TUA_BENCH_MAX_LOAD || os.cpus().length / 2);
const outputPath = path.resolve(process.env.TUA_BENCH_OUTPUT || "inspector-benchmark.json");
const love = process.env.TUA_LOVE || "/Applications/love.app/Contents/MacOS/love";
if (![seconds, warmup, repetitions].every(n => Number.isFinite(n) && n > 0) || !Number.isInteger(repetitions)) throw new Error("Benchmark durations/repetitions must be positive");
const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-inspector-benchmark-"));
const output = path.join(root, "build");
fs.mkdirSync(output);
for (const name of ["tuaecs", "tuastore", "tuaobjectpool", "tuasignal", "tuastatemachine"]) fs.copyFileSync(path.resolve(__dirname, "../../../vendor/runtime", name + ".lua"), path.join(output, name + ".lua"));
fs.copyFileSync(path.join(__dirname, "fixtures/inspector-benchmark.lua"), path.join(output, "main.lua"));
fs.writeFileSync(path.join(output, "conf.lua"), 'io.stdout:setvbuf("no")\nfunction love.conf(t) t.window.visible=false; t.window.width=64; t.window.height=64; t.window.vsync=0; t.modules.audio=false; t.modules.joystick=false end\n');
const percentile = (values, q) => { const sorted = [...values].sort((a,b) => a-b); return sorted[Math.min(sorted.length-1, Math.ceil(sorted.length*q)-1)]; };
const mean = values => values.reduce((sum, value) => sum + value, 0) / Math.max(1, values.length);
async function run(profile, mode, repetition) {
  const hostLoadBefore = os.loadavg();
  if (hostLoadBefore[0] > maxLoad && process.env.TUA_BENCH_ALLOW_BUSY !== "1") {
    throw new Error(`Host is busy (load ${hostLoadBefore[0].toFixed(1)}, limit ${maxLoad}, ${os.cpus().length} logical CPUs). Rerun after other workloads finish, or set TUA_BENCH_ALLOW_BUSY=1 to collect explicitly noisy measurements.`);
  }
  const activity = new ActivityStore(), origins = new RuntimeOrigins();
  const inspector = new InspectorSessions({ activity, enabled: () => mode !== "no-shim", assetsDirectory, describeObject: (entry, object) => origins.describe(entry, object), reportError: message => { throw new Error(message); } });
  const session = activity.begin({ title: mode, kind: "run", cwd: root });
  const launch = await inspector.launch(session, output);
  if (mode === "no-shim" && launch !== undefined) throw new Error("Disabled inspection unexpectedly created a launcher");
  let controlled = false, measuring = false, snapshots = 0, truncated = 0, snapshotBytes = 0, sequence;
  let nodeCpuStart, child, timeout, sampler, text = "", result, stderr = "";
  const rss = [];
  const subscription = inspector.subscribe(() => {
    const entry = inspector.entries.get(session.id);
    if (!entry) return;
    if (!controlled && entry.status === "connected") {
      controlled = true;
      // Delay until launch has published its connection object.
      if (mode === "paused") inspector.pause(session.id, true);
      if (mode === "disabled-after-launch") inspector.disconnectAll();
    }
    if (entry.snapshot && entry.snapshot.sequence !== sequence) {
      sequence = entry.snapshot.sequence;
      if (measuring) {
        snapshots++; truncated += Number(entry.snapshot.truncated);
        snapshotBytes += Buffer.byteLength(JSON.stringify({ ...entry.snapshot, objects: entry.snapshot.objects.map(({ origin, ...object }) => object) }));
        // Exercise editor-side data presentation, excluding VS Code rendering.
        for (const object of entry.snapshot.objects) presentObject(object, String(object.id));
      }
    }
  });
  try {
    result = await new Promise((resolve, reject) => {
      child = spawn(love, [launch || output], { cwd: root, env: { ...process.env, TUA_BENCH_PROFILE: profile, TUA_BENCH_SECONDS: String(seconds), TUA_BENCH_WARMUP: String(warmup) }, stdio: ["ignore", "pipe", "pipe"] });
      timeout = setTimeout(() => { child.kill("SIGKILL"); reject(new Error("Benchmark timed out: " + stderr)); }, (seconds + warmup + 20) * 1000);
      child.on("error", reject);
      child.stderr.on("data", data => { stderr += data; });
      child.stdout.on("data", data => {
        text += data;
        let index;
        while ((index = text.indexOf("\n")) >= 0) {
          const line = text.slice(0, index).trim(); text = text.slice(index + 1);
          if (line === "BENCH_START") {
            measuring = true; nodeCpuStart = process.cpuUsage();
            sampler = setInterval(() => {
              try { rss.push(Number(execFileSync("ps", ["-o", "rss=", "-p", String(child.pid)], { encoding: "utf8" }).trim())); } catch { /* process may have exited */ }
            }, 1000);
          }
          if (line.startsWith("BENCH_RESULT ")) {
            measuring = false;
            try { result = JSON.parse(line.slice(13)); } catch (error) { reject(error); }
          }
        }
      });
      child.on("exit", code => {
        if (code !== 0 || !result) reject(new Error(`LÖVE exit ${code}: ${stderr}\n${text}`));
        else resolve(result);
      });
    });
    const nodeCpu = process.cpuUsage(nodeCpuStart);
    if (mode !== "no-shim" && !controlled) throw new Error("Inspector never connected");
    const warnings = [];
    if (mode === "active" && snapshots < seconds) warnings.push("Active inspection fell below one received snapshot per second");
    if (hostLoadBefore[0] > maxLoad || os.loadavg()[0] > maxLoad) warnings.push("Host load exceeded the benchmark threshold; timing is confounded");
    if (mode !== "active" && snapshots) throw new Error("Inactive inspection still emitted measured snapshots");
    return { profile, mode, repetition, hostLoadBefore, hostLoadAfter: os.loadavg(), warnings, ...result, frameMeanMs: mean(result.frames), frameP95Ms: percentile(result.frames,.95), frameP99Ms: percentile(result.frames,.99), frameMaxMs: Math.max(...result.frames), gameCpuPercent: result.cpuSeconds / result.elapsed * 100, gameRssMeanKiB: mean(rss), gameRssPeakKiB: Math.max(0,...rss), snapshots, truncated, snapshotBytes, nodeCpuMs: (nodeCpu.user + nodeCpu.system) / 1000 };
  } finally {
    clearTimeout(timeout); clearInterval(sampler); child?.kill(); subscription.dispose(); activity.finish(session, 0); inspector.dispose(); activity.dispose();
  }
}
(async () => {
  const report = { createdAt: new Date().toISOString(), host: { platform: process.platform, release: os.release(), arch: process.arch, cpu: os.cpus()[0]?.model, logicalCpus: os.cpus().length, loadAtStart: os.loadavg() }, maxLoad, seconds, warmup, repetitions, methodology: "60 Hz deterministic runtime fixture; measured work excludes sleep, includes the real post-frame shim tick; no rendering. Node CPU includes listener, parsing, source lookup, presentation, launcher reconciliation and benchmark sampling; excludes VS Code UI. RSS sampled once per second. Fresh process per run; mode order rotates per repetition.", runs: [] };
  report.completed = false;
  report.sources = Object.fromEntries(["../assets/inspector/shim.lua", "../assets/inspector/snapshot.lua", "../assets/inspector/transport.lua", "fixtures/inspector-benchmark.lua"].map(file => [file, createHash("sha256").update(fs.readFileSync(file.startsWith("../assets/inspector/") ? path.join(assetsDirectory, path.basename(file)) : path.resolve(__dirname, file))).digest("hex")]));
  const save = () => fs.writeFileSync(outputPath, JSON.stringify(report, null, 2) + "\n");
  save();
  try {
    for (let repetition = 1; repetition <= repetitions; repetition++) for (const profile of profiles) {
      const order = modes.map((_, i) => modes[(i + repetition - 1) % modes.length]);
      for (const mode of order) {
        const result = await run(profile, mode, repetition); report.runs.push(result);
        save();
        console.log(`${profile} ${mode} ${repetition}/${repetitions}: mean ${result.frameMeanMs.toFixed(3)} ms, p99 ${result.frameP99Ms.toFixed(3)} ms, heap ${(result.heapRetainedKiB/1024).toFixed(2)} MiB, ${result.snapshots} snapshots${result.warnings.length ? ` — ${result.warnings.join("; ")}` : ""}`);
      }
    }
  } catch (error) { report.error = error.message; throw error; }
  finally { report.completed = report.runs.length === repetitions * profiles.length * modes.length; save(); fs.rmSync(root, { recursive: true, force: true }); }
})().catch(error => { console.error(error); process.exitCode = 1; });
