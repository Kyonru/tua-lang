local Dialogue = require("tuadialogue")
local function clone(value)
  if type(value) ~= "table" then return value end
  local result = {}
  for key, child in pairs(value) do result[key] = clone(child) end
  return result
end
local doc = {
  schemaVersion = 9,
  documentId = "checkpoint.test",
  entry = "line",
  nodes = {
    line = {
      kind = "line",
      text = "Hello",
      textId = "greeting",
      next = "choice",
      events = {
        { name = "early", payload = "one", anchor = "shown", delay = 0.1 },
        { name = "late", payload = "two", anchor = "shown", delay = 1 },
        { name = "revealed", payload = "three", anchor = "revealed", delay = 0.5 },
      },
    },
    choice = { kind = "choice", options = { { id = "wait", text = "Wait", target = "wait" } } },
    wait = { kind = "wait", duration = 2, next = "end" },
    ["end"] = { kind = "end" },
  },
}
local options = { checkpointVersion = "v1", locale = "en" }
local function fresh(source, settings) return Dialogue.new(source or doc, settings or options) end
local player = fresh()
assert(not pcall(player.checkpoint, player), "idle is not a checkpoint boundary")
player:start()
assert(#player:update(0.25, false) == 1)
local saved = player:checkpoint()
local first, second = fresh(), fresh()
first:restore(saved)
second:restore(saved)
saved.fired[1] = 3
assert(first:snapshot().node.text == "Hello" and first:snapshot().revealComplete)
assert(first:snapshot().restored)
assert(#first:update(0, true) == 0)
assert(#first:update(0.5, true) == 1)
assert(#first:update(0.25, true) == 1)
assert(#first:update(0, true) == 0)
assert(#second:update(0, true) == 0, "restored players own their event state")
assert(not pcall(first.restore, first, player:checkpoint()), "restore needs a fresh player")

local good = player:checkpoint()
for _, mutate in ipairs({
  function(v) v.version = 2 end,
  function(v) v.extra = true end,
  function(v) v.documentId = "other" end,
  function(v) v.contentVersion = "v2" end,
  function(v) v.graph = v.graph .. "changed" end,
  function(v) v.nodeId = "end" end,
  function(v) v.state = "waiting" end,
  function(v) v.elapsed = -1 end,
  function(v) v.elapsed = math.huge end,
  function(v) v.suspended = "yes" end,
  function(v) v.locale = "" end,
  function(v) v.fired = { 1, 1 } end,
  function(v) v.fired = { 9 } end,
  function(v) v.fired = { 3 } end,
  function(v) v.fired = { [2] = 1 } end,
  function(v) v.revealedAt = 0 end,
  function(v) setmetatable(v, {}) end,
}) do
  local bad = clone(good)
  mutate(bad)
  local target = fresh()
  assert(not pcall(target.restore, target, bad))
  assert(target:snapshot().state == "idle", "validation is transactional")
  target:restore(good)
  target:dispose()
end
local changed = clone(doc)
changed.nodes.line.text = "Revised prose"
changed.nodes.wait.duration = 2.0
local edited = fresh(changed)
edited:restore(good)
assert(edited:snapshot().node.text == "Revised prose")
for _, mutate in ipairs({
  function(d) d.nodes.line.textId = "different" end,
  function(d) d.nodes.line.next = "end" end,
  function(d) d.nodes.line.events[1].payload = "changed" end,
  function(d) d.nodes.wait.duration = 3 end,
  function(d) d.nodes.choice.options[1].id = "different" end,
}) do
  local source = clone(doc)
  mutate(source)
  assert(not pcall(function() fresh(source):restore(good) end))
end
local restoredLocale = fresh(nil, { checkpointVersion = "v1", locale = "fr" })
restoredLocale:restore(good)
assert(restoredLocale:snapshot().locale == "en")
assert(restoredLocale:setLocale("fr"))

local fail = true
local input = clone(good)
local resolving = fresh(nil, {
  checkpointVersion = "v1",
  locale = "en",
  resolve = function(_, text)
    if fail then error("resolver failed") end
    input.nodeId = "end"
    input.elapsed = -1
    return text
  end,
})
assert(not pcall(resolving.restore, resolving, input))
assert(resolving:snapshot().state == "idle")
fail = false
resolving:restore(input)
assert(resolving:snapshot().nodeId == "line", "resolver cannot mutate owned restore input")

local noVersion = fresh(nil, { locale = "en" })
noVersion:start()
assert(not pcall(noVersion.checkpoint, noVersion))
local actionPlayer = require("forward").new(require("scene"), {
  checkpointVersion = "v1",
  handlers = { purchase = function() return true end },
})
actionPlayer:start()
assert(not pcall(actionPlayer.checkpoint, actionPlayer), "action documents are not checkpointable")
assert(actionPlayer:runAction())
assert(actionPlayer:snapshot().state == "awaiting_advance")
assert(
  not pcall(actionPlayer.checkpoint, actionPlayer),
  "completed actions do not opt a document in"
)
actionPlayer:dispose()
for _, p in ipairs({ player, first, second, edited, restoredLocale, resolving, noVersion }) do
  p:dispose()
end
print("dialogue checkpoint validation passed")

-- Use Save's real schema/JSON pipeline with an in-memory test backend.
local Save = require("tuasave")
local files = {}
local backend = {
  mkdir = function() return true end,
  info = function(path) return files[path] and { size = #files[path] } or nil end,
  read = function(path) return files[path] end,
  write = function(path, bytes)
    files[path] = bytes
    return true
  end,
  remove = function(path)
    files[path] = nil
    return true
  end,
  -- Test-only checksum; native Save backends use SHA-256.
  hash = function(bytes)
    local sum = 0
    for index = 1, #bytes do sum = (sum * 31 + bytes:byte(index)) % 2147483647 end
    return string.format("%08x", sum):rep(8)
  end,
}
local schema = Save.record({
  version = Save.number(1),
  documentId = Save.string(""),
  contentVersion = Save.string(""),
  graph = Save.string(""),
  nodeId = Save.string(""),
  state = Save.enum({ "awaiting_advance", "awaiting_choice", "waiting" }, "awaiting_advance"),
  elapsed = Save.number(0),
  revealedElapsed = Save.optional(Save.number()),
  revealedAt = Save.optional(Save.number()),
  suspended = Save.boolean(false),
  locale = Save.optional(Save.string()),
  fired = Save.array(Save.number(), {}),
})
local disk = Save.contract({ slot = "dialogue", version = 1, schema = schema, backend = backend })
assert(disk:save(good))
local reader = Save.contract({ slot = "dialogue", version = 1, schema = schema, backend = backend })
local decoded = assert(reader:load())
local fromJson = fresh()
fromJson:restore(decoded)
assert(#fromJson:update(0, true) == 0)
fromJson:dispose()
print("checkpoint JSON round trip passed")

local large = clone(doc)
large.nodes.line.events[1].payload = string.rep("x", 262145)
local oversized = fresh(large)
oversized:start()
assert(not pcall(oversized.checkpoint, oversized))
assert(oversized:snapshot().state == "awaiting_advance")
oversized:dispose()
local invalidId = clone(doc)
invalidId.documentId = string.char(255)
local invalidText = fresh(invalidId)
invalidText:start()
assert(not pcall(invalidText.checkpoint, invalidText))
invalidText:dispose()
local context = Dialogue.context({
  facts = Save.shape(Save.record({ active = Save.boolean(true) })),
  locals = Save.shape(Save.record({ count = Save.number(0) })),
})
local contextual = context.new({
  schemaVersion = 2,
  documentId = "context",
  context = context,
  bindings = {},
  entry = "s",
  nodes = { s = { kind = "line", text = "Hi", next = "e" }, e = { kind = "end" } },
}, { active = true }, options)
contextual:start()
assert(not pcall(contextual.checkpoint, contextual))
contextual:dispose()
print("checkpoint limits and registry exclusions passed")

local unicode = clone(doc)
unicode.nodes["ä"] = { kind = "end" }
unicode.nodes.z = { kind = "end" }
local ordered = fresh(unicode)
ordered:start()
local graph = ordered:checkpoint().graph
assert(graph:find("s1:z", 1, true) < graph:find("s2:ä", 1, true), "graph keys use byte order")
ordered:dispose()
