local runtime = arg[1] or "vendor/runtime"
package.path = runtime .. "/?.lua;" .. package.path
local Save = require("tuasave")
local function backend()
  local files = {}
  return {
    files = files,
    info = function(path) return files[path] and { size = #files[path] } end,
    read = function(path) return files[path] end,
    write = function(path, bytes)
      files[path] = bytes
      return true
    end,
    remove = function(path)
      files[path] = nil
      return true
    end,
    mkdir = function() return true end,
    hash = function(bytes)
      local hash = 0
      for index = 1, #bytes do hash = (hash * 31 + bytes:byte(index)) % 4294967296 end
      return string.format("%08x", hash):rep(8)
    end,
  }
end
local function oldSchema()
  return Save.record({ points = Save.number(0), tags = Save.array(Save.string(), {}) })
end
local function newOptions(disk, migrate)
  return {
    backend = disk,
    slot = "history",
    version = 3,
    schema = Save.record({ score = Save.number(0) }),
    migration_schemas = { [1] = oldSchema(), [2] = oldSchema() },
    migrations = { [1] = migrate, [2] = migrate },
  }
end
local function writeOld(disk, version)
  local old = Save.contract({
    backend = disk,
    slot = "history",
    version = version,
    schema = oldSchema(),
  })
  assert(old:save({ points = 7, tags = { "first" } }))
end
local function replacePayload(disk, payload)
  local path = "__tua_saves/history.a"
  local generation, version = disk.files[path]:match("^TUA%-SAVE%-1\n(%d+)\n(%d+)\n")
  local prefix = "TUA-SAVE-1\n" .. generation .. "\n" .. version .. "\n" .. #payload .. "\n"
  disk.files[path] = prefix .. disk.hash(prefix .. payload) .. "\n" .. payload
end
for _, version in ipairs({ 1, 2 }) do
  local disk = backend()
  writeOld(disk, version)
  local bytes = disk.files["__tua_saves/history.a"]
  local calls = 0
  local options = newOptions(disk, function(old)
    calls = calls + 1
    for key in pairs(old) do assert(type(key) == "string") end
    for key in pairs(old.tags) do assert(type(key) == "number") end
    old.tags[1] = "changed"
    return { score = old.points + 1 }
  end)
  local current = Save.contract(options)
  options.migration_schemas[version].fields.points = Save.string("wrong")
  local saved, issue = current:load()
  assert(not issue and saved.score == 8 and calls == 1)
  assert(disk.files["__tua_saves/history.a"] == bytes, "loading must not rewrite old bytes")
  saved.score = 999
  assert(current:load().score == 8 and calls == 1, "loaded snapshots remain owned copies")
  assert(current:save(current:load()))
  assert(Save.contract(newOptions(disk, function()
    error("must not remigrate")
  end)):load().score == 8)
end
for _, payload in ipairs({
  '{"points":7,"tags":[],"unknown":true}',
  '{"tags":[]}',
  '{"points":"7","tags":[]}',
  '{"points":7,"tags":{}}',
  '{"points":7,"tags":[1]}',
  '[]',
}) do
  local disk = backend()
  writeOld(disk, 1)
  replacePayload(disk, payload)
  local before = disk.files["__tua_saves/history.a"]
  local calls = 0
  local current = Save.contract(newOptions(disk, function()
    calls = calls + 1
    return { score = 100 }
  end))
  local value, issue = current:load()
  assert(value == nil and issue.code == "invalid_migration_input" and issue.phase == "validate")
  assert(calls == 0 and not current:save({ score = 1 }))
  assert(not current:acceptRecovery(), "invalid historical data is not damaged-slot recovery")
  assert(disk.files["__tua_saves/history.a"] == before)
end
-- Without an input schema, existing callbacks still receive the original decoded payload.
do
  local disk = backend()
  writeOld(disk, 1)
  local options = newOptions(disk, function(old)
    local marker = false
    for key in pairs(old) do if type(key) == "table" then marker = true end end
    assert(marker)
    return { score = old.points }
  end)
  options.migration_schemas = nil
  assert(Save.contract(options):load().score == 7)
end
-- Callback and output failures keep their existing error phases and block writes.
for _, migrate in ipairs({
  function()
    error("broken")
  end,
  function()
    return { score = "bad" }
  end,
}) do
  local disk = backend()
  writeOld(disk, 1)
  local current = Save.contract(newOptions(disk, migrate))
  local value, issue = current:load()
  assert(value == nil and (issue.code == "migration_failed" or issue.code == "invalid_snapshot"))
  assert(not current:save({ score = 1 }))
end
for _, schemas in ipairs({
  false,
  { [0] = oldSchema() },
  { [3] = oldSchema() },
  { [1.5] = oldSchema() },
  { [1] = Save.array(Save.string(), {}) },
  { [1] = "invalid" },
  { unknown = oldSchema() },
}) do
  local options = newOptions(backend(), function() return { score = 0 } end)
  options.migration_schemas = schemas
  assert(not pcall(Save.contract, options))
end
do
  local options = newOptions(backend(), function() return { score = 0 } end)
  options.migrations[2] = nil
  assert(not pcall(Save.contract, options), "a historical schema needs its migration")
end
print("Historical schemas: strict input, marker-free copies, direct versions and failure preservation passed")
