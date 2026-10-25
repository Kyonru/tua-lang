"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const net = require("node:net");
const { once } = require("node:events");
const { listen, MAX_FRAME, validSnapshot } = require("../../src/inspector/connection");
const { createLauncher } = require("../../src/inspector/launcher");
const { InspectorSessions } = require("../../src/inspector/sessions");
const { ActivityStore } = require("../../src/workspace/activity-store");
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(predicate) { for (let i = 0; i < 250; i++) { if (predicate()) return; await delay(20); } assert.fail("Timed out"); }
const snapshot = { version: 1, sequence: 1, objects: [{ id: 1, kind: "states", label: "Player", data: { current: "idle" } }] };

test("inspector authenticates loopback connections, handles partial frames and rejects oversized input", async t => {
  const received = [], statuses = [];
  const endpoint = await listen({ onSnapshot: value => received.push(value), onStatus: value => statuses.push(value) });
  t.after(() => endpoint.dispose());
  const bad = net.connect(endpoint.port, "127.0.0.1");
  bad.on("error", () => {});
  const rejected = once(bad, "close");
  bad.write(JSON.stringify({ type: "hello", version: 1, token: "wrong" }) + "\n");
  await rejected;
  assert.equal(received.length, 0);
  const client = net.connect(endpoint.port, "127.0.0.1");
  t.after(() => client.destroy());
  let commands = ""; client.on("data", chunk => { commands += chunk; });
  client.on("error", () => {});
  const hello = JSON.stringify({ type: "hello", version: 1, token: endpoint.token }) + "\n";
  client.write(hello.slice(0, 10)); client.write(hello.slice(10));
  client.write(JSON.stringify(snapshot) + "\n");
  await until(() => received.length === 1);
  assert.deepEqual(received[0], snapshot);
  endpoint.setInterval(1000);
  await until(() => commands.includes("interval 1000\n"));
  endpoint.pause(true); endpoint.refresh();
  await until(() => commands.includes("pause\n") && commands.includes("snapshot\n"));
  const closed = once(client, "close");
  client.write("x".repeat(MAX_FRAME + 1));
  await closed;
  assert(statuses.includes("connected"));
});

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-inspector-test-"));
  const output = path.join(root, "build"); fs.mkdirSync(output);
  fs.writeFileSync(path.join(output, "main.lua"), "-- original game");
  fs.writeFileSync(path.join(output, "conf.lua"), "-- original config");
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  return output;
}

test("development launcher leaves build output untouched and follows new hot-reload files", async t => {
  const output = fixture(t);
  const before = fs.readdirSync(output).map(name => [name, fs.readFileSync(path.join(output, name), "utf8")]);
  const launcher = createLauncher(output, "/extension/assets/inspector", { port: 1234, token: "a".repeat(64) });
  t.after(() => launcher.dispose());
  assert.notEqual(launcher.directory, output);
  assert.equal(path.basename(launcher.directory), path.basename(output), "preserve default save identity");
  assert(fs.lstatSync(path.join(launcher.directory, "main.lua")).isFile());
  fs.writeFileSync(path.join(output, "new.lua"), "return 42");
  await until(() => fs.existsSync(path.join(launcher.directory, "new.lua")));
  for (const [name, data] of before) assert.equal(fs.readFileSync(path.join(output, name), "utf8"), data);
  launcher.dispose();
  assert(fs.existsSync(path.join(output, "main.lua")));
});

test("inspection is opt-in, player launchers are independent, disabling keeps running game files", async t => {
  const output = fixture(t); const activity = new ActivityStore(); let enabled = false;
  const inspector = new InspectorSessions({ activity, enabled: () => enabled, assetsDirectory: "/extension/assets/inspector" });
  t.after(() => { inspector.dispose(); activity.dispose(); });
  const a = activity.begin({ title: "Host" }); const b = activity.begin({ title: "Client" });
  assert.equal(await inspector.launch(a, output), undefined);
  assert.equal(inspector.entries.size, 0);
  enabled = true;
  const first = await inspector.launch(a, output); const second = await inspector.launch(b, output);
  assert.notEqual(first, second);
  assert.notEqual(inspector.entries.get(a.id).connection.token, inspector.entries.get(b.id).connection.token);
  inspector.disconnectAll();
  assert(fs.existsSync(path.join(first, "main.lua")), "disabling inspection must not break running games");
  activity.finish(a, 0);
  assert(!fs.existsSync(first)); assert(fs.existsSync(second));
  activity.finish(b, 0); assert(!fs.existsSync(second));
});

test("Lua snapshots bound large/cyclic data without retaining dead objects or invoking metamethods", t => {
  const { spawnSync } = require("node:child_process");
  const lua = ["lua", "luajit"].find(command => spawnSync(command, ["-v"]).status === 0);
  if (!lua) return t.skip("Lua interpreter unavailable");
  const script = `local inspect = assert(loadfile(arg[1]))()
local live = {}
local value = { text = string.rep("x", 10000) }; value.self = value
setmetatable(value, { __pairs = function() error("metamethod invoked") end })
inspect.register("ecs", live, function() return value end)
do local gone = {}; inspect.register("states", gone, function() return {} end) end
collectgarbage("collect")
io.write(inspect.snapshot(), "\\n")
local keep = {}
for i = 1, 128 do
  local object = {}; keep[i] = object
  inspect.register("pools", object, function()
    local data = {}
    for j = 1, 128 do data[string.rep("k", 240) .. j] = string.rep("v", 1000) end
    return data
  end)
end
io.write(inspect.snapshot(), "\\n")`;
  const result = spawnSync(lua, ["-", path.resolve(__dirname, "../../assets/inspector/snapshot.lua")], { input: script, encoding: "utf8", maxBuffer: 2 * 1024 * 1024 });
  assert.equal(result.status, 0, result.stderr);
  const [small, large] = result.stdout.trim().split("\n").map(line => JSON.parse(line));
  assert.equal(small.objects.length, 1);
  assert.equal(small.objects[0].data.self, "<cycle>");
  assert(small.objects[0].data.text.length < 250);
  assert.equal(large.truncated, true);
  assert.equal(validSnapshot(large), true, "truncation must preserve each object envelope so the connection stays valid");
  assert(Buffer.byteLength(JSON.stringify(large)) < MAX_FRAME);
});

test("store shim preserves returns, errors, notifications and collection without adding subscriptions", t => {
  const { spawnSync } = require("node:child_process");
  const lua = ["lua", "luajit"].find(command => spawnSync(command, ["-v"]).status === 0);
  if (!lua) return t.skip("Lua interpreter unavailable");
  const script = `local assets, runtime = arg[1], arg[2]
package.path = runtime .. "/?.lua;" .. package.path
local originalLoadfile = loadfile
loadfile = function(filename)
  if filename == assets .. "/transport.lua" then return function() return function() return function() end end end end
  return originalLoadfile(filename)
end
local now = 10
love = { timer = { getTime = function() return now end } }
local inspect = assert(loadfile(assets .. "/shim.lua"))(assets, 1, "token")
local create = require("tuastore")
assert(require("tuastore") == create, "requiring twice must not wrap twice")
local store = create({ score = 0, nested = { health = 10 } })
assert(#store._listeners == 0 and next(store._watchers) == nil)
local notifications, watches = 0, 0
local unsubscribe = store:subscribe(function() notifications = notifications + 1 end)
local unwatch = store:watch("score", function() watches = watches + 1 end)
local nextState = { score = 1, nested = { health = 10 } }
assert(store:set(nextState) == nextState)
assert(store:update(function(state) state.score = 2 end) == nextState)
local ok, err = pcall(function() store:update(function() error("original error") end) end)
assert(not ok and err:match("original error"))
assert(notifications == 2 and watches == 2)
assert(#store._listeners == 1 and #store._watchers.score == 1)
do local dead = create({}); dead:update(function() end) end
collectgarbage("collect")
now = 12
io.write(inspect.snapshot(), "\\n")
unsubscribe(); unwatch()
store:get().nested.health = 5
io.write(inspect.snapshot(), "\\n")`;
  const result = spawnSync(lua, ["-", path.resolve(__dirname, "../../assets/inspector"), path.resolve(__dirname, "../../../../vendor/runtime")], { input: script, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  const [first, second] = result.stdout.trim().split("\n").map(line => JSON.parse(line));
  assert.equal(first.objects.length, 1, "collected stores must disappear");
  assert.equal(first.objects[0].kind, "stores");
  assert.deepEqual(first.objects[0].data.state, { score: 2, nested: { health: 10 } });
  assert.equal(first.objects[0].data.updates, 2);
  assert.equal(first.objects[0].data.secondsSinceUpdate, 2);
  assert.equal(first.objects[0].data.subscribers, 1);
  assert.equal(first.objects[0].data.watchers, 1);
  assert.equal(second.objects[0].data.subscribers, 0);
  assert.equal(second.objects[0].data.watchers, 0);
  assert.equal(second.objects[0].data.state.nested.health, 5);
  assert.equal(second.objects[0].data.updates, 2, "direct mutations are not successful API calls");
});

test("collection options exclude disabled wrappers and preserve enabled store inspection", t => {
  const { spawnSync } = require("node:child_process");
  const lua = ["lua", "luajit"].find(command => spawnSync(command, ["-v"]).status === 0);
  if (!lua) return t.skip("Lua interpreter unavailable");
  const script = `local assets, runtime = arg[1], arg[2]
package.path = runtime .. '/?.lua;' .. package.path
local store = require('tuastore')
local pool = require('tuaobjectpool')
local machine = require('tuastatemachine').create
local original = loadfile
loadfile = function(file)
 if file:match('transport.lua$') then return function() return function(_, _, _, interval) assert(interval == 1000); return function() end end end end
 return original(file)
end
local inspect = assert(loadfile(assets .. '/shim.lua'))(assets, 1, 'test', { categories = { stores = true }, updateInterval = 1000 })
assert(require('tuaobjectpool') == pool)
assert(require('tuastatemachine').create == machine)
assert(require('tuastore') ~= store)
local keep = require('tuastore')({ health = 100 })
keep:update(function(value) value.health = 80 end)
io.write(inspect.snapshot())`;
  const result = spawnSync(lua, ["-", path.resolve(__dirname, "../../assets/inspector"), path.resolve(__dirname, "../../../../vendor/runtime")], { input: script, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  const value = JSON.parse(result.stdout);
  assert.equal(value.objects.length, 1); assert.equal(value.objects[0].kind, "stores");
  assert.equal(value.objects[0].data.state.health, 80);
});

test("empty categories skip launchers and intervals are clamped", async t => {
  const { normalizeOptions } = require("../../src/inspector/options");
  assert.deepEqual(normalizeOptions({ categories: ["stores", "unknown", "stores"], updateInterval: 1 }), { categories: ["stores"], updateInterval: 100 });
  assert.equal(normalizeOptions({ updateInterval: 999999 }).updateInterval, 10000);
  const activity = new ActivityStore();
  const inspector = new InspectorSessions({ activity, enabled: () => true, options: () => ({ categories: [] }) });
  t.after(() => { inspector.dispose(); activity.dispose(); });
  assert.equal(await inspector.launch(activity.begin({ title: "No categories" }), "/unused"), undefined);
  assert.equal(inspector.entries.size, 0);
});
