local runtime = arg[1] or "vendor/runtime"
package.path = runtime .. "/?.lua;" .. package.path
local Save = require("tuasave")
local Store = require("tuastore")
local Json = require("tuasave_json")

do
  local rocket = assert(Json.decode('"\\uD83D\\uDE80"', 32, 100))
  assert(rocket == "🚀" and assert(Json.decode(Json.quote(rocket), 32, 100)) == rocket)
  for _, input in ipairs({ '"\\uD83D"', '"\\uDC00"', '"\\x00"',
    '01', '1e999', '[1,]', '{"x":1,"x":2}', '"' .. string.char(0xC0) .. '"' }) do
    assert(Json.decode(input, 32, 100) == nil, "malformed JSON was accepted: " .. input)
  end
  assert(Json.decode(string.rep("[", 40) .. "0" .. string.rep("]", 40), 32, 100) == nil)
  assert(Json.decode("[1,2,3]", 32, 3) == nil)
end

local function hash(bytes)
  local value = 2166136261
  for index = 1, #bytes do value = (value * 16777619 + bytes:byte(index)) % 4294967296 end
  return string.format("%08x", value):rep(8)
end

local function backend()
  local files, writes = {}, 0
  local service = {
    files = files,
    writes = function() return writes end,
    hash = hash,
    mkdir = function() return true end,
    info = function(path) return files[path] and { size = #files[path] } or nil end,
    read = function(path) return files[path] end,
    remove = function(path) files[path] = nil; return true end,
    write = function(path, bytes)
      writes = writes + 1
      files[path] = bytes
      return true
    end,
  }
  return service
end

local function replacePayload(disk, path, payload)
  local generation, version = assert(disk.files[path]:match("^TUA%-SAVE%-1\n(%d+)\n(%d+)\n"))
  local prefix = "TUA-SAVE-1\n" .. generation .. "\n" .. version .. "\n" .. #payload .. "\n"
  disk.files[path] = prefix .. hash(prefix .. payload) .. "\n" .. payload
end

local function contract(service, version, migrations)
  return Save.contract({
    slot = "profile", version = version or 2, backend = service,
    schema = Save.record({
      score = Save.number(0),
      label = Save.string("Ada"),
      enabled = Save.boolean(false),
      unlocked = Save.array(Save.string(), {}),
      settings = Save.record({ volume = Save.number(0.5),
        color = Save.enum({ "red", "blue" }, "red"),
        nickname = Save.optional(Save.string()) }),
    }),
    migrations = migrations,
  })
end

local function expectCode(actual, code)
  assert(actual and actual.code == code, "expected issue " .. code .. ", got " ..
    tostring(actual and actual.code))
end

-- Defaults are deep-copied, typed values round-trip, and invalid snapshots
-- never reach the backend.
do
  local disk = backend()
  local profile = contract(disk)
  local first, second = profile:defaults(), profile:defaults()
  first.unlocked[1] = "sword"
  first.settings.volume = 0.8
  assert(#second.unlocked == 0 and second.settings.volume == 0.5)
  first.score, first.label = 3, "Étoile"
  first.enabled = false
  assert(profile:load() == nil)
  assert(profile:save(first))
  local loaded = assert(profile:load())
  assert(loaded.score == 3 and loaded.label == "Étoile" and loaded.enabled == false)
  assert(loaded.unlocked[1] == "sword" and loaded.settings.volume == 0.8)
  loaded.unlocked[1] = "changed"
  assert(profile:load().unlocked[1] == "sword")
  local written = disk.writes()
  local bad = profile:defaults(); bad.extra = true
  local ok, problem = profile:save(bad)
  assert(not ok and problem.message:find("$.extra", 1, true))
  expectCode(problem, "invalid_snapshot")
  bad = profile:defaults(); bad.unlocked = { [2] = "hole" }
  ok, problem = profile:save(bad); assert(not ok); expectCode(problem, "invalid_snapshot")
  bad = profile:defaults(); bad.score = 0 / 0
  ok, problem = profile:save(bad); assert(not ok); expectCode(problem, "invalid_snapshot")
  bad = profile:defaults(); bad.settings = bad
  ok, problem = profile:save(bad); assert(not ok); expectCode(problem, "invalid_snapshot")
  bad = profile:defaults(); bad.label = string.char(0xC0, 0xAF)
  ok, problem = profile:save(bad); assert(not ok); expectCode(problem, "invalid_snapshot")
  assert(disk.writes() == written)
end

-- Corrupting a newer slot recovers the older generation, but both automatic
-- and manual writes remain blocked until the game accepts that recovery.
do
  local disk = backend()
  local profile = contract(disk)
  local old = profile:defaults(); old.score = 1
  assert(profile:save(old))
  local newer = profile:defaults(); newer.score = 2
  assert(profile:save(newer))
  disk.files["__tua_saves/profile.b"] = disk.files["__tua_saves/profile.b"]:sub(1, -3)
  local reopened = contract(disk)
  local recovered, problem = reopened:load()
  assert(recovered.score == 1)
  expectCode(problem, "recovered_previous_slot")
  local writes = disk.writes()
  local ok, denied = reopened:save(newer)
  assert(not ok and denied.code == "recovered_previous_slot" and disk.writes() == writes)
  assert(reopened:acceptRecovery())
  assert(reopened:save(newer))
  assert(contract(disk):load().score == 2)
end

-- Future versions, bad JSON, and migration failure preserve both slot bytes.
do
  local disk = backend()
  local future = contract(disk, 3)
  assert(future:save(future:defaults()))
  local bytes = disk.files["__tua_saves/profile.a"]
  local older = contract(disk, 2)
  local loaded, problem = older:load()
  assert(loaded == nil); expectCode(problem, "future_version")
  local ok = older:save(older:defaults())
  assert(not ok and disk.files["__tua_saves/profile.a"] == bytes)
  local fresh = assert(older:reset())
  assert(disk.files["__tua_saves/profile.a"] == nil)
  assert(older:save(fresh))
  assert(contract(disk, 2):load().score == 0)

  local previous = contract(backend(), 1)
  local oldState = previous:defaults(); oldState.score = 9
  assert(previous:save(oldState))
  local converted = contract(previous.backend, 2, { [1] = function(old)
    old.score = old.score + 1
    return old
  end })
  assert(converted:load().score == 10)
  assert(converted:save(converted:load()))
  assert(contract(previous.backend, 2):load().score == 10)

  local broken = contract(backend(), 1)
  assert(broken:save(broken:defaults()))
  local noConverter = contract(broken.backend, 2)
  loaded, problem = noConverter:load()
  assert(loaded == nil); expectCode(problem, "missing_migration")
end

do
  local disk = backend()
  disk.files["__tua_saves/profile.a"] = "garbled"
  disk.files["__tua_saves/profile.b"] = "also garbled"
  local profile = contract(disk)
  local value, problem = profile:load()
  assert(value == nil); expectCode(problem, "corrupt_envelope")
  assert(not profile:save(profile:defaults()))
  local fresh = assert(profile:reset())
  assert(profile:save(fresh))
  assert(contract(disk):load().score == 0)
end

do
  local disk = backend()
  local profile = contract(disk)
  assert(profile:save(profile:defaults()))
  replacePayload(disk, "__tua_saves/profile.a", '{"score":1,"score":2}')
  local value, problem = contract(disk):load()
  assert(value == nil); expectCode(problem, "invalid_json")
  assert(disk.files["__tua_saves/profile.a"]:find('"score":1', 1, true))
  replacePayload(disk, "__tua_saves/profile.a", '{"score":1}')
  value, problem = contract(disk):load()
  assert(value == nil); expectCode(problem, "invalid_snapshot")
end

-- A failed write or read-back leaves the old slot and dirty binding intact.
do
  local disk = backend()
  local profile = contract(disk)
  local initial = profile:defaults()
  assert(profile:save(initial))
  local original = disk.files["__tua_saves/profile.a"]
  local state = Store({ profile = initial, transient = function() end })
  local binding = profile:bind(state, { select = function(value) return value.profile end,
    debounce = 1, max_interval = 3 })
  for index = 1, 10000 do
    state:update(function(value) value.profile.score = index end)
  end
  assert(disk.writes() == 1 and binding:isDirty())
  assert(binding:update(0.4) == false)
  assert(binding:update(0.4) == false)
  local oldWrite = disk.write
  disk.write = function() return nil, "disk full" end
  local ok, problem = binding:flush()
  assert(not ok); expectCode(problem, "write_failed")
  assert(binding:isDirty() and disk.files["__tua_saves/profile.a"] == original)
  disk.write = oldWrite
  assert(binding:flush())
  assert(not binding:isDirty() and contract(disk):load().score == 10000)
  state:get().profile.score = 10001
  assert(not binding:isDirty()) -- direct table writes require markDirty.
  binding:markDirty()
  assert(binding:flush())
  assert(contract(disk):load().score == 10001)
  state:update(function(value) value.profile.score = 10002 end)
  local oldRead = disk.read
  disk.read = function() return "truncated" end
  ok, problem = binding:flush()
  assert(not ok); expectCode(problem, "verify_failed")
  assert(binding:isDirty())
  disk.read = oldRead
  assert(binding:flush())
  assert(contract(disk):load().score == 10002)
  binding:dispose()
  assert(not binding:flush())
end

do
  local disk = backend()
  local profile = contract(disk)
  local state = Store({ profile = profile:defaults() })
  local binding = profile:bind(state, { select = function(value) return value.profile end,
    debounce = 1, max_interval = 2 })
  for index = 1, 5 do
    state:update(function(value) value.profile.score = index end)
    binding:update(0.5)
    if index < 4 then assert(disk.writes() == 0) end
  end
  assert(disk.writes() == 1, "max_interval forces a save during sustained mutations")
end
