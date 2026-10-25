"use strict";
const fs = require("node:fs"), os = require("node:os"), path = require("node:path"), net = require("node:net"), assert = require("node:assert/strict");
const { spawn } = require("node:child_process");
const { ActivityStore } = require("../src/workspace/activity-store");
const { InspectorSessions } = require("../src/inspector/sessions");
const { createLauncher } = require("../src/inspector/launcher");
const { presentObject } = require("../src/inspector/presentation");
const duration = Number(process.env.TUA_SOAK_SECONDS || 600);
const outputFile = path.resolve(process.env.TUA_SOAK_OUTPUT || path.join(os.tmpdir(), "tua-inspector-soak.json"));
const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-inspector-soak-")), output = path.join(root, "build");
fs.mkdirSync(output);
for (const name of ["tuaecs", "tuastore", "tuasignal"]) fs.copyFileSync(path.resolve(__dirname, "../../../vendor/runtime", name + ".lua"), path.join(output, name + ".lua"));
fs.copyFileSync(path.join(__dirname, "fixtures/inspector-soak.lua"), path.join(output, "main.lua"));
fs.writeFileSync(path.join(output, "conf.lua"), 'io.stdout:setvbuf("no")\nfunction love.conf(t) t.window.visible=false; t.window.width=64; t.window.height=64; t.modules.audio=false; t.modules.joystick=false end\n');
const activity = new ActivityStore();
const inspector = new InspectorSessions({ activity, enabled: () => true, assetsDirectory: path.resolve(__dirname, "../assets/inspector") });
const report = { started: new Date().toISOString(), duration, snapshots: 0, truncated: 0, reconnects: [], samples: [], errors: [], completed: false };
const children = [], sockets = new Set(), timers = [];
let proxy, attempts = 0, entry, lastSequence, lastGeneration, previousSnapshotTime;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const until = async (fn, label) => { for (let i = 0; i < 100; i++) { if (fn()) return; await sleep(50); } throw new Error("Timeout: " + label); };
const save = () => fs.writeFileSync(outputFile, JSON.stringify(report, null, 2) + "\n");
function launch(name, directory) {
  const child = spawn(process.env.TUA_LOVE || "/Applications/love.app/Contents/MacOS/love", [directory], { cwd: root, env: { ...process.env, TUA_SOAK_SECONDS: String(duration) }, stdio: ["ignore", "pipe", "pipe"] });
  children.push(child); let text = "", done = false, errors = "";
  child.stderr.on("data", value => { errors += value; });
  child.stdout.on("data", value => {
    text += value; let newline;
    while ((newline = text.indexOf("\n")) >= 0) {
      const line = text.slice(0, newline).trim(); text = text.slice(newline + 1);
      if (line.startsWith("SOAK ")) {
        if (global.gc) global.gc();
        report.samples.push({ mode: name, ...JSON.parse(line.slice(5)), nodeHeapKiB: process.memoryUsage().heapUsed / 1024, snapshots: report.snapshots, generation: entry?.connectionGeneration });
        save();
      }
      if (line === "DONE") done = true;
    }
  });
  return new Promise((resolve, reject) => { child.once("error", reject); child.once("exit", code => code === 0 && done ? resolve() : reject(new Error(`${name} exited ${code}: ${errors}`))); });
}
(async () => {
  const session = activity.begin({ title: "Soak", kind: "run", cwd: root });
  const cleanup = [];
  try {
    await inspector.launch(session, output); entry = inspector.entries.get(session.id);
    const endpointPort = entry.connection.port;
    proxy = net.createServer(client => {
      attempts++; sockets.add(client); const upstream = net.connect(endpointPort, "127.0.0.1"); sockets.add(upstream);
      client.pipe(upstream); upstream.pipe(client);
      client.on("error", () => {}); upstream.on("error", () => {});
      client.on("close", () => { sockets.delete(client); upstream.destroy(); });
      upstream.on("close", () => { sockets.delete(upstream); client.destroy(); });
    });
    await new Promise(resolve => proxy.listen(0, "127.0.0.1", resolve));
    entry.launcher.dispose(); entry.launcher = createLauncher(output, path.resolve(__dirname, "../assets/inspector"), { ...entry.connection, port: proxy.address().port });
    cleanup.push(inspector.subscribe(() => {
      if (!entry.snapshot || (lastSequence === entry.snapshot.sequence && lastGeneration === entry.connectionGeneration)) return;
      lastSequence = entry.snapshot.sequence; lastGeneration = entry.connectionGeneration;
      report.snapshots++; report.truncated += Number(entry.snapshot.truncated);
      previousSnapshotTime = Date.now();
      for (const object of entry.snapshot.objects) presentObject(object, String(object.id));
    }));
    const processes = Promise.all([launch("no-shim", output), launch("active", entry.launcher.directory)]);
    // Keep rejection observed even while performing checks and reconnects.
    processes.catch(error => report.errors.push(error.message));
    await until(() => report.snapshots > 1, "initial snapshots");
    const started = Date.now();
    let reconnectBusy = false;
    const reconnectTimer = setInterval(async () => {
      if (reconnectBusy || Date.now() - started > (duration - 20) * 1000) return;
      reconnectBusy = true;
      const before = entry.connectionGeneration, began = Date.now();
      for (const socket of [...sockets]) socket.destroy();
      try {
        await until(() => entry.connectionGeneration > before && entry.snapshot, "snapshot after reconnect");
        report.reconnects.push({ seconds: (Date.now() - started) / 1000, recoveryMs: Date.now() - began });
        console.log(`Reconnect ${report.reconnects.length}: ${Date.now() - began} ms`);
      } catch (error) { report.errors.push(error.message); }
      reconnectBusy = false; save();
    }, 30000); timers.push(reconnectTimer);
    // Exercise controls early, before the long steady-state interval.
    inspector.pause(session.id, true); await sleep(1000);
    const paused = report.snapshots; await sleep(1200); assert.equal(report.snapshots, paused);
    inspector.refresh(session.id); await until(() => report.snapshots > paused, "manual paused refresh");
    inspector.pause(session.id, false);
    for (const interval of [1000, 200, 500]) {
      inspector.updateInterval(interval); await sleep(1500); const count = report.snapshots;
      const before = Date.now(); await sleep(3000);
      report[`rate${interval}`] = (report.snapshots - count) / ((Date.now() - before) / 1000);
    }
    const remaining = Math.max(0, duration * 1000 - (Date.now() - started) - 12000);
    await sleep(remaining);
    clearInterval(reconnectTimer);
    assert(Date.now() - previousSnapshotTime < 5000, "snapshots must continue through the soak");
    inspector.disconnectAll(); assert(fs.existsSync(entry.launcher.directory));
    const stoppedAttempts = attempts; await sleep(5000);
    assert.equal(attempts, stoppedAttempts, "explicit disable must not reconnect");
    await processes;
    assert(report.reconnects.length >= Math.floor((duration - 30) / 30) - 1, "expected repeated reconnects");
    assert.equal(report.errors.length, 0, report.errors.join("\n"));
    for (const mode of ["no-shim", "active"]) {
      const samples = report.samples.filter(sample => sample.mode === mode && sample.seconds >= 30);
      const first = samples.slice(0, 3), last = samples.slice(-3);
      const mean = list => list.reduce((sum, sample) => sum + sample.heapKiB, 0) / list.length;
      report[mode + "HeapGrowthKiB"] = mean(last) - mean(first);
      assert(report[mode + "HeapGrowthKiB"] < 256, `${mode} retained Lua heap growth exceeded 256 KiB`);
    }
    const retained = report.samples.filter(sample => sample.seconds >= 30).map(sample => sample.nodeHeapKiB);
    report.nodeHeapRangeKiB = Math.max(...retained) - Math.min(...retained);
    assert(report.nodeHeapRangeKiB < 2048, "retained Node heap drift exceeded 2 MiB");
    assert(report.rate1000 > .5 && report.rate1000 < 1.5); assert(report.rate200 > 3 && report.rate200 < 7);
    report.completed = true; console.log("Soak passed", JSON.stringify({ snapshots: report.snapshots, reconnects: report.reconnects.length, luaGrowthKiB: report.activeHeapGrowthKiB, nodeRangeKiB: report.nodeHeapRangeKiB }));
  } catch (error) { report.errors.push(error.message); throw error; }
  finally {
    for (const timer of timers) clearInterval(timer);
    for (const child of children) child.kill();
    for (const socket of sockets) socket.destroy(); proxy?.close();
    for (const item of cleanup) item.dispose();
    inspector.dispose(); activity.dispose(); save(); fs.rmSync(root, { recursive: true, force: true });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
