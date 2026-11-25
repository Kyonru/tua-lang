"use strict";
// Native LÖVE smoke test: no debugger, game networking or GUI automation required.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");
const { ActivityStore } = require("../src/workspace/activity-store");
const { InspectorSessions } = require("../src/inspector/sessions");
const { causalTraceNodes } = require("../src/inspector/causal-trace");
const { RuntimeOrigins } = require("../src/inspector/origins");
const origins = new RuntimeOrigins();
const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-inspector-native-"));
const output = path.join(root, "build");
const activity = new ActivityStore();
const inspector = new InspectorSessions({ describeObject: (entry, object) => origins.describe(entry, object), activity, enabled: () => true, options: () => ({ dialogueHistory: true }), assetsDirectory: path.resolve(__dirname, "../assets/inspector"), reportError: message => { throw new Error(message); } });
const children = []; let logs = "";
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(predicate) { for (let i = 0; i < 300; i++) { if (predicate()) return; await sleep(50); } throw new Error("Native inspector timed out\n" + logs); }
const kind = (entry, name, binding) => entry.snapshot?.objects.find(object => object.kind === name && (!binding || object.origin?.name === binding))?.data;
async function main() {
  fs.mkdirSync(output);
  for (const name of ["tuaecs", "tuastatemachine", "tuasignal", "tuaobjectpool", "tuastore", "baton", "tuabehavior", "tuaaudio"]) fs.copyFileSync(path.resolve(__dirname, "../../../vendor/runtime", name + ".lua"), path.join(output, name + ".lua"));
  const runtime = path.resolve(__dirname, "../../../vendor/runtime");
  for (const name of fs.readdirSync(runtime).filter(name => name.startsWith("tuadialogue") && name.endsWith(".lua"))) {
    fs.copyFileSync(path.join(runtime, name), path.join(output, name));
  }
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
local causal = makeStore({ inspectionKind = "causal-trace/v1", epoch = 1, enabled = true,
  events = {
    { sequence = 1, attackId = 1, parentId = 0, rootId = 1, phase = "processing", reason = "primary" },
    { sequence = 2, attackId = 2, parentId = 1, rootId = 1, phase = "suppressed", reason = "root_limit" }
  }
})
local __tua_input = require("baton").new
local controls = __tua_input({ controls = { confirm = { "key:space" } } })
local BehaviorTree = require("tuabehavior")
local tree = BehaviorTree.new(BehaviorTree.sequence({ BehaviorTree.wait(10) }))
local runner = tree:instance({})
local __tua_audio_system = require("tuaaudio")
local audio = __tua_audio_system({ buses = { sfx = { parent = "master", volume = 0.5 } } })
store:subscribe(function() end)
store:watch("score", function() end)
store:set({ score = 1 })
store:update(function(state) state.score = 2 end)
local Dialogue = require("tuadialogue")
local dialogue = Dialogue.new({ schemaVersion = 9, documentId = "native", entry = "wait", nodes = {
  wait = { kind = "wait", duration = 100, next = "done" },
  done = { kind = "end" }
} })
dialogue:start()
local translated = Dialogue.new({
  schemaVersion = 8,
  documentId = "translated",
  entry = "s",
  nodes = {
    s = {
      kind = "line",
      textId = "greeting",
      text = "PRIVATE translation",
      richText = "[em]PRIVATE translation[/em]",
      next = "e",
    },
    e = { kind = "end" },
  },
}, {
  locale = "fr-CA",
  resolveTemplate = function(_, source)
    return { template = source, locale = "fr" }
  end,
})
translated:start()
local looping = Dialogue.new({
  schemaVersion = 3,
  documentId = "budget",
  entry = "loop",
  nodes = {
    loop = {
      kind = "branch",
      condition = { { op = "value", value = true } },
      yes = "loop",
      no = "done",
    },
    done = { kind = "end" },
  },
}, { transitionBudget = 2 })
looping:start()
local completedAction = Dialogue.actionHandle()
assert(completedAction:cancel())
assert(not completedAction:resolve(true))
assert(not completedAction:reject("PRIVATE action error"))
local function actionCycle()
  local handle
  handle = Dialogue.actionHandle({ cancel = function() assert(handle) end })
  return setmetatable({ handle }, { __mode = "v" })
end
local weakAction = actionCycle()
collectgarbage("collect")
collectgarbage("collect")
assert(weakAction[1] == nil)

local player = "unknown"
function love.load(args) player = args[1] or "unknown" end
function love.update(dt)
  controls:update()
  dialogue:update(dt, false)
  assert(runner:update(dt) == "running")
  audio:update(dt)
  data.player = player
  local changed = love.filesystem.read("change.txt")
  if changed then data.x = tonumber(changed) or 42 end
  store:get().player = player
  bus:emit("frame")
  -- Keep runtime objects reachable, as a real game does.
  assert(looping and translated and world and machine.current == "walking" and pool:owns(bullet) and causal)
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
  await until(() => players.every(player => kind(player.entry, "signals", "bus")?.emitted > 0));
  for (const player of players) {
    for (const [kindName, name] of Object.entries({ ecs: "world", states: "machine", signals: "bus", pools: "pool", stores: "store", inputs: "controls", audio: "audio" })) {
      const object = player.entry.snapshot.objects.find(item => item.kind === kindName && item.origin?.name === name);
      assert.equal(object.origin?.name, name, JSON.stringify(object));
      assert.equal(object.origin.path, path.join(output, "main.lua"));
      assert(object.origin.line > 0);
    }
    const causal = causalTraceNodes(kind(player.entry, "stores", "causal").state, "native");
    assert.equal(causal.children[0].description, "2 attacks · 1 suppressed");
    assert.equal(causal.children[0].children[0].children[1].children[0].label, "suppressed: root_limit");
    assert.equal(kind(player.entry, "states").current, "walking");
    assert.equal(kind(player.entry, "stores", "store").state.score, 2);
    assert.equal(kind(player.entry, "stores", "store").state.player, player.session.title);
    assert.equal(kind(player.entry, "stores", "store").updates, 2);
    assert.equal(kind(player.entry, "stores", "store").subscribers, 1);
    assert.equal(kind(player.entry, "stores", "store").watchers, 1);
    assert.deepEqual(kind(player.entry, "pools", "pool"), { active: 1, available: 3, capacity: 4, highWater: 1, failedReleases: 0 });
    assert.equal(kind(player.entry, "inputs", "controls").controls.confirm.value, 0);
    assert.equal(kind(player.entry, "inputs", "controls").controls.confirm.sources, "key:space");
    const behavior = kind(player.entry, "behaviors");
    assert.equal(behavior.lastStatus, "running");
    assert(behavior.ticks > 0);
    assert(Object.values(behavior.active).some(value => value.includes("0.0 · wait")));
    assert.equal(Object.values(behavior.history)[0].status, "running");
    assert.equal(kind(player.entry, "audio", "audio").buses.sfx.gain, 0.5);
    assert.equal(kind(player.entry, "audio", "audio").activeVoices, 0);
    const dialogue = player.entry.snapshot.objects.find(object => object.kind === "dialogues" && object.data.documentId === "native")?.data;
    const action = player.entry.snapshot.objects.find(object => object.kind === "dialogues" && object.data.actionHandle)?.data;
    assert.equal(action.status, "cancelled");
    assert.equal(action.ignoredAfterCancel, 2);
    assert.equal(action.ignoredResolve, 1);
    assert.equal(action.ignoredReject, 1);
    assert(!JSON.stringify(player.entry.snapshot).includes("PRIVATE action error"));
    const translated = player.entry.snapshot.objects.find(object => object.data.documentId === "translated")?.data;
    assert.equal(translated.localization.fields["1"].status, "locale_fallback");
    assert.equal(translated.localization.fields["1"].resolvedLocale, "fr");
    assert(!JSON.stringify(player.entry.snapshot).includes("PRIVATE translation"));
    const budget = player.entry.snapshot.objects.find(object => object.data.documentId === "budget")?.data;
    assert.equal(budget.issue.code, "transition_budget_exceeded");
    assert.equal(budget.issue.nodeId, "loop");
    assert.equal(budget.issue.transitionBudget, 2);
    assert.equal(budget.issue.message, undefined);
    assert.equal(dialogue.state, "waiting");
    assert.equal(dialogue.documentId, "native");
    assert.equal(dialogue.sampledChanges, 1, "wait ticks do not fill sampled history");
    assert.equal(dialogue.history["1"].state, "waiting");
    assert(dialogue.wait.elapsed > 0 && dialogue.wait.remaining < 100);
    const component = Object.values(kind(player.entry, "ecs", "world").entities["1"])[0];
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
  assert(!contents.stdout.includes("shim.lua"));
  assert(!contents.stdout.includes("127.0.0.1"));
  console.log("Native runtime inspector passed: two players, ECS/state/signal/pool/store/input/behavior/audio/dialogue snapshots, pause/refresh/resume, live files, independent exit, cleanup, and inspector-free package.");
}
main().catch(error => { console.error([...inspector.entries.values()].map(entry => ({ title: entry.session.title, status: entry.status, sequence: entry.snapshot?.sequence }))); console.error(error); process.exitCode = 1; }).finally(async () => {
  for (const child of children) if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
  await sleep(150); inspector.dispose(); activity.dispose(); fs.rmSync(root, { recursive: true, force: true });
});
