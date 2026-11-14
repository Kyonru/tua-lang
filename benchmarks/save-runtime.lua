-- Run: lua benchmarks/save-runtime.lua vendor/runtime
-- Plain-Lua timing of store revisions and binding polling; filesystem flush
-- throughput must be measured separately under LÖVE on the target platform.
package.path = (arg[1] or "vendor/runtime") .. "/?.lua;" .. package.path
local Save = require("tuasave")
local Store = require("tuastore")
local files, writes = {}, 0
local backend = {
  info = function(path) return files[path] and { size = #files[path] } or nil end,
  read = function(path) return files[path] end,
  write = function(path, bytes) files[path] = bytes; writes = writes + 1; return true end,
  remove = function(path) files[path] = nil; return true end,
  mkdir = function() return true end,
  hash = function(bytes)
    local value = 2166136261
    for index = 1, #bytes do value = (value * 16777619 + bytes:byte(index)) % 4294967296 end
    return string.format("%08x", value):rep(8)
  end,
}
local profile = Save.contract({ slot = "benchmark", version = 1, backend = backend,
  schema = Save.record({ score = Save.number(0) }) })
local state = Store({ profile = profile:defaults() })
local binding = profile:bind(state, {
  select = function(value) return value.profile end,
  debounce = 1, max_interval = 10,
})
local function mutate(value) value.profile.score = value.profile.score + 1 end
local function measure(label, count, work)
  collectgarbage("collect")
  local beforeBytes = collectgarbage("count") * 1024
  local before = os.clock()
  for _ = 1, count do work() end
  local elapsed = os.clock() - before
  local bytes = collectgarbage("count") * 1024 - beforeBytes
  print(string.format("%-24s %.3f us/op %.3f bytes/op", label,
    elapsed * 1000000 / count, bytes / count))
end
measure("store updates, bound", 10000, function() state:update(mutate) end)
assert(writes == 0 and binding:isDirty())
assert(binding:flush())
measure("clean binding polls", 10000, function() binding:update(0.001) end)
assert(writes == 1 and not binding:isDirty())
