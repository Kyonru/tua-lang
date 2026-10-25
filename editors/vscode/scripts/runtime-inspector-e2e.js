"use strict";
// Native LÖVE smoke test: no debugger, game networking or GUI automation required.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");
const { ActivityStore } = require("../src/workspace/activity-store");
const { InspectorSessions } = require("../src/inspector/sessions");
const { RuntimeOrigins } = require("../src/inspector/origins");
const origins = new RuntimeOrigins();
const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-inspector-native-"));
const output = path.join(root, "build");
const activity = new ActivityStore();
const inspector = new InspectorSessions({ describeObject: (entry, object) => origins.describe(entry, object), activity, enabled: () => true, assetsDirectory: path.resolve(__dirname, "../assets/inspector"), reportError: message => { throw new Error(message); } });
const children = []; let logs = "";
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(predicate) { for (let i = 0; i < 300; i++) { if (predicate()) return; await sleep(50); } throw new Error("Native inspector timed out\n" + logs); }
const kind = (entry, name) => entry.snapshot?.objects.find(object => object.kind === name)?.data;
async function main() {
  fs.mkdirSync(output);
  for (const name of ["tuaecs", "tuastatemachine", "tuasignal", "tuaobjectpool", "tuastore"]) fs.copyFileSync(path.resolve(__dirname, "../../../vendor/runtime", name + ".lua"), path.join(output, name + ".lua"));
  fs.writeFileSync(path.join(output, "conf.lua"), `io.stdout:setvbuf("no")
function love.conf(t)
  t.window.visible = false; t.window.width = 64; t.window.height = 64
  t.modules.audio = false; t.modules.joystick = false
end
`);
  fs.writeFileSync(path.join(output, "main.lua"), `local ecs = require("tuaecs")
local world = ecs.world()
local position = ecs.component("Position", {})
local data = { x = 42, y = 7, huge = string.rep("x", 10000) }; data.self = data
world:add(world:create(), position, data)
local stateMachine = require("tuastatemachine")
local machine = stateMachine.create({ initial = "idle", events = {{ name = "move", from = "idle", to = "walking" }} })
machine:move()
local signals = require("tuasignal")
local bus = signals.new()
local objectPool = require("tuaobjectpool")
local pool = objectPool({ create = function() return {} end, initial = 4 })
local bullet = pool:acquire()
local makeStore = require("tuastore")
local store = makeStore({ score = 0 })
store:subscribe(function() end)
store:watch("score", function() end)
store:set({ score = 1 })
store:update(function(state) state.score = 2 end)
local player = "unknown"
function love.load(args) player = args[1] or "unknown" end
function love.update(dt)
  data.player = player
  local changed = love.filesystem.read("change.txt")
  if changed then data.x = tonumber(changed) or 42 end
  store:get().player = player
  bus:emit("frame")
  -- Keep runtime objects reachable, as a real game does.
  assert(world and machine.current == "walking" and pool:owns(bullet))
end
`);
  const originals = new Map(fs.readdirSync(output).map(name => [name, fs.readFileSync(path.join(output, name))]));
  const players = [];
  for (const title of ["Host", "Client"]) {
    const session = activity.begin({ title, kind: "run", cwd: root });
    const launch = await inspector.launch(session, output);
    const bootstrap = path.join(launch, "conf.lua");
    fs.writeFileSync(bootstrap, 'io.stdout:setvbuf("no")\n' + fs.readFileSync(bootstrap, "utf8"));
    const child = spawn(process.env.TUA_LOVE || "/Applications/love.app/Contents/MacOS/love", [launch, title], { cwd: root, stdio: ["ignore", "pipe", "pipe"] });
    children.push(child);
    child.stdout.on("data", value => { logs += value; }); child.stderr.on("data", value => { logs += value; });
    child.on("error", error => { logs += error.message; });
    child.on("exit", code => activity.finish(session, code || 0));
    players.push({ session, child, launch, entry: inspector.entries.get(session.id) });
  }
  await until(() => players.every(player => kind(player.entry, "signals")?.emitted > 0));
  for (const player of players) {
    for (const [kindName, name] of Object.entries({ ecs: "world", states: "machine", signals: "bus", pools: "pool", stores: "store" })) {
      const object = player.entry.snapshot.objects.find(item => item.kind === kindName);
      assert.equal(object.origin?.name, name, JSON.stringify(object));
      assert.equal(object.origin.path, path.join(output, "main.lua"));
      assert(object.origin.line > 0);
    }
    assert.equal(kind(player.entry, "states").current, "walking");
    assert.equal(kind(player.entry, "stores").state.score, 2);
    assert.equal(kind(player.entry, "stores").state.player, player.session.title);
    assert.equal(kind(player.entry, "stores").updates, 2);
    assert.equal(kind(player.entry, "stores").subscribers, 1);
    assert.equal(kind(player.entry, "stores").watchers, 1);
    assert.deepEqual(kind(player.entry, "pools"), { active: 1, available: 3, capacity: 4 });
    const component = Object.values(kind(player.entry, "ecs").entities["1"])[0];
    assert.equal(component.player, player.session.title); assert.equal(component.self, "<cycle>");
    assert(component.huge.length < 250);
  }
  const [host, client] = players;
  inspector.pause(host.session.id);
  await sleep(800);
  const pausedSequence = host.entry.snapshot.sequence;
  await sleep(800); assert.equal(host.entry.snapshot.sequence, pausedSequence);
  inspector.refresh(host.session.id);
  await until(() => host.entry.snapshot.sequence > pausedSequence);
  inspector.pause(host.session.id);
  fs.writeFileSync(path.join(output, "change.txt"), "99");
  await until(() => players.every(player => Object.values(kind(player.entry, "ecs").entities["1"])[0].x === 99));
  // Loop restarts reconnect with the same session token.
  const oldSequence = client.entry.snapshot.sequence;
  client.entry.connection.refresh();
  await until(() => client.entry.snapshot.sequence > oldSequence);
  host.child.kill("SIGKILL");
  await until(() => !host.session.active);
  assert(!fs.existsSync(host.launch)); assert(fs.existsSync(client.launch));
  const lastClient = client.entry.snapshot.sequence;
  await until(() => client.entry.snapshot.sequence > lastClient);
  inspector.disconnectAll();
  assert(fs.existsSync(path.join(client.launch, "main.lua")));
  for (const [name, content] of originals) assert.deepEqual(fs.readFileSync(path.join(output, name)), content);
  assert(!fs.readdirSync(output).some(name => /inspect|shim/.test(name)));
  // Packaging the same files after inspection must contain no shim or token.
  fs.mkdirSync(path.join(root, "src"));
  for (const [name, content] of originals) fs.writeFileSync(path.join(root, "src", name), content);
  fs.writeFileSync(path.join(root, "tua.toml"), 'source = "src"\nout = "release"\n');
  const packaged = spawnSync(process.env.TUA_BIN || path.resolve(__dirname, "../bin/tua"), ["package", "--target", "love"], { cwd: root, encoding: "utf8", timeout: 30000 });
  assert.equal(packaged.status, 0, packaged.stdout + packaged.stderr);
  const archives = fs.readdirSync(path.join(root, "dist")).filter(name => name.endsWith(".love"));
  assert.equal(archives.length, 1);
  const contents = spawnSync("unzip", ["-p", path.join(root, "dist", archives[0])], { encoding: "utf8", maxBuffer: 10 * 1024 * 1024 });
  assert.equal(contents.status, 0);
  assert(!contents.stdout.includes("Tua editor-only inspector"));
  assert(!contents.stdout.includes("127.0.0.1"));
  console.log("Native runtime inspector passed: two players, ECS/state/signal/pool/store snapshots, pause/refresh/resume, live files, independent exit, cleanup, and inspector-free package.");
}
main().catch(error => { console.error([...inspector.entries.values()].map(entry => ({ title: entry.session.title, status: entry.status, sequence: entry.snapshot?.sequence }))); console.error(error); process.exitCode = 1; }).finally(async () => {
  for (const child of children) if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
  await sleep(150); inspector.dispose(); activity.dispose(); fs.rmSync(root, { recursive: true, force: true });
});
