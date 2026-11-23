-- Typed save contracts for LÖVE 11. State validation and JSON never execute save text.
-- Copyright (c) 2026 Kyonru. MIT license.
local Json = require("tuasave_json")
local Save = {}
local Contract = {}
Contract.__index = Contract
local Binding = {}
Binding.__index = Binding
local SCHEMA = {}
local DIRECTORY = "__tua_saves"
local DEFAULT_BYTES = 128 * 1024
local HARD_BYTES = 4 * 1024 * 1024
local MAX_DEPTH = 32
local MAX_ENTRIES = 100000
local MAX_GENERATION = 9007199254740991

local function issue(code, message, slot, phase, recoverable)
  return {
    code = code,
    message = message,
    slot = slot,
    phase = phase,
    recoverable = recoverable == true,
  }
end

local function descriptor(kind, default, extra)
  local value = extra or {}
  value.kind, value.default, value.hasDefault, value._tag =
    kind, default, default ~= nil, SCHEMA
  return value
end

local function schema(value)
  assert(type(value) == "table" and value._tag == SCHEMA,
    "Save schema builder expected")
  return value
end

function Save.number(default) return descriptor("number", default) end
function Save.string(default) return descriptor("string", default) end
function Save.boolean(default) return descriptor("boolean", default) end
function Save.optional(child) return descriptor("optional", nil, { child = schema(child) }) end
function Save.array(child, default)
  child = schema(child)
  assert(child.kind ~= "optional", "optional array elements would create sparse JSON arrays")
  return descriptor("array", default, { child = child })
end
function Save.record(fields)
  assert(type(fields) == "table", "Save.record expects a field table")
  local copy = {}
  for key, child in pairs(fields) do
    assert(type(key) == "string" and #key > 0 and Json.validUtf8(key),
      "Save.record field names must be nonempty UTF-8 strings")
    copy[key] = schema(child)
  end
  return descriptor("record", nil, { fields = copy })
end
function Save.enum(values, default)
  assert(type(values) == "table" and #values > 0, "Save.enum expects nonempty values")
  local allowed, copy = {}, {}
  for index = 1, #values do
    local value = values[index]
    assert(type(value) == "string" and Json.validUtf8(value) and not allowed[value],
      "Save.enum values must be unique UTF-8 strings")
    allowed[value], copy[index] = true, value
  end
  return descriptor("enum", default, { allowed = allowed, values = copy })
end

local function copySchema(input, depth, active, count)
  schema(input)
  assert(depth <= MAX_DEPTH and not active[input], "save schema is too deep or cyclic")
  count.value = count.value + 1
  assert(count.value <= MAX_ENTRIES, "save schema entry limit exceeded")
  active[input] = true
  local result = descriptor(input.kind, input.default)
  if input.kind == "record" then
    result.fields = {}
    for key, child in pairs(input.fields) do
      assert(type(key) == "string" and Json.validUtf8(key), "invalid save field")
      result.fields[key] = copySchema(child, depth + 1, active, count)
    end
  elseif input.kind == "array" or input.kind == "optional" then
    result.child = copySchema(input.child, depth + 1, active, count)
  elseif input.kind == "enum" then
    result.allowed, result.values = {}, {}
    for index, value in ipairs(input.values) do
      result.allowed[value], result.values[index] = true, value
    end
  else
    assert(input.kind == "number" or input.kind == "string" or
      input.kind == "boolean", "invalid save schema kind")
  end
  active[input] = nil
  return result
end

local function pathOf(parent, field)
  if type(field) == "number" then return parent .. "[" .. field .. "]" end
  return parent .. "." .. tostring(field)
end

local function validate(spec, input, path, seen, depth, count, decoded)
  count.value = count.value + 1
  if count.value > MAX_ENTRIES then error(path .. ": entry limit exceeded", 0) end
  if depth > MAX_DEPTH then error(path .. ": nesting limit exceeded", 0) end
  local kind = spec.kind
  if kind == "optional" then
    if input == nil then return nil end
    return validate(spec.child, input, path, seen, depth + 1, count, decoded)
  end
  if kind == "number" then
    if type(input) ~= "number" or input ~= input or input == math.huge or input == -math.huge then
      error(path .. ": expected finite number", 0)
    end
    return input
  end
  if kind == "string" or kind == "enum" then
    if type(input) ~= "string" or not Json.validUtf8(input) then
      error(path .. ": expected UTF-8 string", 0)
    end
    if kind == "enum" and not spec.allowed[input] then
      error(path .. ": unknown enum value", 0)
    end
    return input
  end
  if kind == "boolean" then
    if type(input) ~= "boolean" then error(path .. ": expected boolean", 0) end
    return input
  end
  if type(input) ~= "table" or seen[input] or getmetatable(input) ~= nil then
    error(path .. ": expected plain acyclic " .. kind, 0)
  end
  if decoded and ((kind == "array" and not input[Json.array])
    or (kind == "record" and not input[Json.object])) then
    error(path .. ": JSON container does not match " .. kind, 0)
  end
  seen[input] = true
  local output = {}
  if kind == "array" then
    local length, highest = 0, 0
    for key in pairs(input) do
      if key ~= Json.array then
        if type(key) ~= "number" or key < 1 or key % 1 ~= 0 then
          error(path .. ": array needs positive integer indices", 0)
        end
        length = length + 1
        if key > highest then highest = key end
      end
    end
    if length ~= highest then error(path .. ": sparse array", 0) end
    for index = 1, length do
      output[index] = validate(spec.child, input[index], pathOf(path, index),
        seen, depth + 1, count, decoded)
    end
  elseif kind == "record" then
    for key in pairs(input) do
      if key ~= Json.object then
        if type(key) ~= "string" or not spec.fields[key] then
          error(pathOf(path, key) .. ": unknown field", 0)
        end
      end
    end
    for key, child in pairs(spec.fields) do
      local value = input[key]
      if value == nil and child.kind ~= "optional" then
        error(pathOf(path, key) .. ": missing required field", 0)
      end
      if value ~= nil then
        output[key] = validate(child, value, pathOf(path, key),
          seen, depth + 1, count, decoded)
      end
    end
  else error(path .. ": invalid schema kind", 0) end
  seen[input] = nil
  return output
end

local function defaults(spec, path)
  if spec.kind == "optional" then return nil end
  if spec.kind == "record" then
    local result = {}
    for key, child in pairs(spec.fields) do
      local value = defaults(child, pathOf(path, key))
      if value ~= nil then result[key] = value end
    end
    return result
  end
  if not spec.hasDefault then error(path .. ": missing default", 0) end
  return validate(spec, spec.default, path, {}, 1, { value = 0 }, false)
end

local function encode(spec, value)
  if spec.kind == "optional" then return encode(spec.child, value) end
  if spec.kind == "string" or spec.kind == "enum" then return Json.quote(value) end
  if spec.kind == "number" then
    return (string.format("%.17g", value):gsub(",", "."))
  end
  if spec.kind == "boolean" then return value and "true" or "false" end
  if spec.kind == "array" then
    local pieces = {}
    for index = 1, #value do pieces[index] = encode(spec.child, value[index]) end
    return "[" .. table.concat(pieces, ",") .. "]"
  end
  local keys, pieces = {}, {}
  for key in pairs(spec.fields) do keys[#keys + 1] = key end
  table.sort(keys)
  for _, key in ipairs(keys) do
    if value[key] ~= nil then
      pieces[#pieces + 1] = Json.quote(key) .. ":" .. encode(spec.fields[key], value[key])
    end
  end
  return "{" .. table.concat(pieces, ",") .. "}"
end

local function loveBackend()
  assert(type(love) == "table" and love.filesystem and love.data,
    "Save needs LÖVE 11 filesystem/data APIs or an injected test backend")
  return {
    info = function(path) return love.filesystem.getInfo(path, "file") end,
    read = function(path) return love.filesystem.read(path) end,
    write = function(path, bytes) return love.filesystem.write(path, bytes) end,
    remove = function(path) return love.filesystem.remove(path) end,
    mkdir = function(path) return love.filesystem.createDirectory(path) end,
    hash = function(bytes)
      local digest = love.data.hash("sha256", bytes)
      return love.data.encode("string", "hex", digest)
    end,
  }
end

local function envelope(backend, generation, version, payload)
  local prefix = "TUA-SAVE-1\n" .. generation .. "\n" .. version .. "\n" .. #payload .. "\n"
  local checksum = backend.hash(prefix .. payload)
  assert(type(checksum) == "string" and checksum:match("^[0-9a-fA-F]+$")
    and #checksum == 64, "save backend hash must return 64 hex digits")
  return prefix .. checksum:lower() .. "\n" .. payload
end

local function parseEnvelope(backend, bytes, maxBytes)
  if type(bytes) ~= "string" or #bytes > maxBytes + 160 then
    return nil, "save envelope exceeds byte limit"
  end
  local position, lines = 1, {}
  for index = 1, 5 do
    local boundary = bytes:find("\n", position, true)
    if not boundary or boundary - position > 100 then return nil, "invalid save header" end
    lines[index] = bytes:sub(position, boundary - 1)
    position = boundary + 1
  end
  if lines[1] ~= "TUA-SAVE-1" then return nil, "unknown save format" end
  for index = 2, 4 do
    if not lines[index]:match("^[1-9]%d*$") and
      not (index == 3 and lines[index] == "0") and
      not (index == 4 and lines[index] == "0") then
      return nil, "invalid save metadata"
    end
  end
  local generation, version, length = tonumber(lines[2]), tonumber(lines[3]), tonumber(lines[4])
  if not generation or generation > MAX_GENERATION or not version or not length
    or length > maxBytes or #bytes - position + 1 ~= length then
    return nil, "save length or generation is invalid"
  end
  if not lines[5]:match("^[0-9a-f]+$") or #lines[5] ~= 64 then
    return nil, "invalid save checksum"
  end
  local payload = bytes:sub(position)
  local prefix = table.concat({ lines[1], lines[2], lines[3], lines[4], "" }, "\n")
  if backend.hash(prefix .. payload):lower() ~= lines[5] then
    return nil, "save checksum mismatch"
  end
  return { generation = generation, version = version, payload = payload }
end

local function readSlot(contract, suffix)
  local path = DIRECTORY .. "/" .. contract.slot .. "." .. suffix
  local ok, info, infoError = pcall(contract.backend.info, path)
  if not ok or (not info and infoError) then
    return { error = issue("read_failed", tostring(infoError or info), path, "read", true) }
  end
  if not info then return { missing = true } end
  if type(info.size) ~= "number" or info.size < 0 or info.size ~= info.size
    or info.size > contract.maxBytes + 160 then
    return { error = issue("oversized", "save slot exceeds byte cap", path, "read", true) }
  end
  local readOk, bytes, readError = pcall(contract.backend.read, path)
  if not readOk or type(bytes) ~= "string" then
    return { error = issue("read_failed", tostring(readError or bytes), path, "read", true) }
  end
  local parsed, decoded, problem = pcall(parseEnvelope, contract.backend, bytes, contract.maxBytes)
  if not parsed or not decoded then
    return { error = issue("corrupt_envelope", tostring(problem or decoded), path, "decode", true) }
  end
  decoded.path, decoded.suffix = path, suffix
  return decoded
end

local function inspect(contract)
  if contract.inspected then return end
  contract.inspected = true
  local a, b = readSlot(contract, "a"), readSlot(contract, "b")
  contract.slots = { a, b }
  for _, slot in ipairs(contract.slots) do
    if slot.version and slot.version > contract.version then
      contract.blocked = issue("future_version", "save uses a newer schema version",
        slot.path, "decode", false)
      return
    end
  end
  if a.missing and b.missing then return end
  local valid = {}
  for _, slot in ipairs(contract.slots) do
    if slot.version then valid[#valid + 1] = slot end
  end
  table.sort(valid, function(left, right) return left.generation > right.generation end)
  if #valid == 0 then
    contract.blocked = a.error or b.error or issue("unreadable", "no usable save slot",
      contract.slot, "read", true)
    return
  end
  for _, slot in ipairs(valid) do
    local decoded, problem = Json.decode(slot.payload, MAX_DEPTH, MAX_ENTRIES)
    if not decoded then
      slot.error = issue("invalid_json", problem, slot.path, "decode", true)
    else
      local candidate = decoded
      if slot.version < contract.version then
        local converter = contract.migrations[slot.version]
        if not converter then
          slot.error = issue("missing_migration", "no direct migration from version " .. slot.version,
            slot.path, "migrate", true)
        else
          local migrated, result = pcall(converter, decoded)
          if migrated then candidate = result
          else
            slot.error = issue("migration_failed", tostring(result), slot.path, "migrate", true)
          end
        end
      end
      if not slot.error then
        local accepted, result = pcall(validate, contract.schema, candidate, "$", {}, 1,
          { value = 0 }, slot.version == contract.version)
        if accepted then slot.snapshot = result
        else
          slot.error = issue("invalid_snapshot", tostring(result), slot.path, "validate", true)
        end
      end
    end
    if slot.error then
      -- A valid envelope whose data fails validation is not silently rolled back.
      contract.blocked = slot.error
      return
    end
    contract.active, contract.generation = slot.suffix, slot.generation
    contract.snapshot = slot.snapshot
    contract.pendingUpgrade = slot.version < contract.version
    local damaged = a.error or b.error
    if damaged then
      contract.blocked = issue("damaged_slot_unverified",
        "one save slot is damaged; selected a valid snapshot, but the damaged slot's generation is unknown",
        damaged.slot, damaged.phase, true)
    end
    return
  end
end

function Save.contract(options)
  assert(type(options) == "table", "Save.contract expects options")
  local slot, version = options.slot, options.version
  local spec = copySchema(options.schema, 1, {}, { value = 0 })
  assert(type(slot) == "string" and #slot <= 64
    and slot:match("^[A-Za-z0-9][A-Za-z0-9_-]*$"),
    "save slot needs an ASCII identifier")
  assert(type(version) == "number" and version >= 1 and version < 1000000
    and version % 1 == 0, "save version needs a positive integer")
  assert(spec.kind == "record", "save root schema must be a record")
  local maxBytes = options.max_bytes or DEFAULT_BYTES
  assert(type(maxBytes) == "number" and maxBytes >= 1024 and maxBytes <= HARD_BYTES
    and maxBytes % 1 == 0, "save byte cap must be between 1 KiB and 4 MiB")
  local migrations = options.migrations or {}
  assert(type(migrations) == "table", "save migrations must be a table")
  for oldVersion, converter in pairs(migrations) do
    assert(type(oldVersion) == "number" and oldVersion >= 1 and oldVersion < version
      and oldVersion % 1 == 0 and type(converter) == "function",
      "save migrations need direct old-version function entries")
  end
  local initial = defaults(spec, "$")
  validate(spec, initial, "$", {}, 1, { value = 0 }, false)
  local backend = options.backend or loveBackend()
  for _, operation in ipairs({ "info", "read", "write", "remove", "mkdir", "hash" }) do
    assert(type(backend[operation]) == "function", "save backend needs " .. operation)
  end
  return setmetatable({
    slot = slot,
    version = version,
    schema = spec,
    migrations = migrations,
    backend = backend,
    maxBytes = maxBytes,
    generation = 0,
    initial = initial,
    inspected = false,
    pendingUpgrade = false,
  }, Contract)
end

function Contract:defaults()
  return validate(self.schema, self.initial, "$", {}, 1, { value = 0 }, false)
end

function Contract:load()
  inspect(self)
  if self.blocked then return self.snapshot and self:copySnapshot() or nil, self.blocked end
  return self.snapshot and self:copySnapshot() or nil, nil
end

function Contract:copySnapshot()
  return validate(self.schema, self.snapshot, "$", {}, 1, { value = 0 }, false)
end

function Contract:acceptRecovery()
  if not self.blocked or self.blocked.code ~= "damaged_slot_unverified" then return false end
  self.blocked = nil
  return true
end

function Contract:reset()
  -- Destructive by design: the caller has explicitly chosen to discard both
  -- slots. If removal fails, the contract remains blocked for a later retry.
  for _, suffix in ipairs({ "a", "b" }) do
    local path = DIRECTORY .. "/" .. self.slot .. "." .. suffix
    local found, info, infoError = pcall(self.backend.info, path)
    if not found or (not info and infoError) then
      return nil, issue("reset_failed", tostring(infoError or info), path, "write", true)
    end
    if info then
      local removed, ok, message = pcall(self.backend.remove, path)
      if not removed or not ok then
        return nil, issue("reset_failed", tostring(message or ok), path, "write", true)
      end
    end
  end
  self.inspected, self.blocked, self.snapshot = true, nil, nil
  self.active, self.generation, self.pendingUpgrade = nil, 0, false
  return self:defaults()
end

function Contract:save(snapshot)
  inspect(self)
  if self.blocked then return false, self.blocked end
  if self.generation >= MAX_GENERATION then
    return false, issue("generation_exhausted", "save generation limit reached",
      self.slot, "write", false)
  end
  local ok, normalized = pcall(validate, self.schema, snapshot, "$", {}, 1,
    { value = 0 }, false)
  if not ok then
    return false, issue("invalid_snapshot", tostring(normalized), self.slot, "validate", true)
  end
  local encoded, payload = pcall(encode, self.schema, normalized)
  if not encoded then
    return false, issue("encode_failed", tostring(payload), self.slot, "validate", true)
  end
  if #payload > self.maxBytes then
    return false, issue("oversized", "encoded save exceeds byte cap",
      self.slot, "validate", true)
  end
  local suffix = self.active == "a" and "b" or "a"
  local path = DIRECTORY .. "/" .. self.slot .. "." .. suffix
  local hashed, bytes = pcall(envelope, self.backend, self.generation + 1, self.version, payload)
  if not hashed then
    return false, issue("hash_failed", tostring(bytes), path, "write", true)
  end
  local made, created, createError = pcall(self.backend.mkdir, DIRECTORY)
  if not made or not created then
    return false, issue("write_failed", tostring(createError or created), path, "write", true)
  end
  local wrote, result, writeError = pcall(self.backend.write, path, bytes)
  if not wrote or not result then
    return false, issue("write_failed", tostring(writeError or result), path, "write", true)
  end
  local read, check, readError = pcall(self.backend.read, path)
  if not read or type(check) ~= "string" then
    return false, issue("verify_failed", tostring(readError or check), path, "verify", true)
  end
  local parsed, verified = pcall(parseEnvelope, self.backend, check, self.maxBytes)
  if check ~= bytes or not parsed or not verified then
    return false, issue("verify_failed", "written slot failed read-back verification",
      path, "verify", true)
  end
  self.active, self.generation, self.snapshot, self.pendingUpgrade =
    suffix, self.generation + 1, normalized, false
  return true, nil
end

local function flush(binding)
  if binding.disposed then
    return false, issue("disposed", "persistence binding is disposed",
      binding.contract.slot, "write", false)
  end
  binding:observe()
  if not binding.dirty then return true, nil end
  local revision = binding.store:revision()
  local selected, snapshot = pcall(binding.select, binding.store:get())
  if not selected then
    binding.issue = issue("select_failed", tostring(snapshot), binding.contract.slot,
      "validate", true)
    binding.failures = math.min(binding.failures + 1, 5)
    binding.retry = math.min(2 ^ binding.failures, 30)
    return false, binding.issue
  end
  local ok, problem = binding.contract:save(snapshot)
  binding.issue = problem
  if ok and binding.store:revision() == revision then
    binding.dirty, binding.retry, binding.failures = false, 0, 0
    binding.elapsed, binding.sinceDirty = 0, 0
  elseif not ok then
    binding.failures = math.min(binding.failures + 1, 5)
    binding.retry = math.min(2 ^ binding.failures, 30)
  end
  return ok, problem
end

function Binding:observe()
  local revision = self.store:revision()
  if revision ~= self.observed then
    if not self.dirty then self.sinceDirty = 0 end
    self.observed, self.dirty, self.elapsed = revision, true, 0
  end
end

function Binding:update(dt)
  if self.disposed then return false end
  assert(type(dt) == "number" and dt >= 0 and dt < math.huge,
    "persistence:update expects nonnegative finite dt")
  self:observe()
  if not self.dirty then return false end
  self.elapsed, self.sinceDirty = self.elapsed + dt, self.sinceDirty + dt
  if self.retry > 0 then
    self.retry = math.max(0, self.retry - dt)
    return false
  end
  if self.elapsed >= self.debounce or self.sinceDirty >= self.maxInterval then
    return flush(self)
  end
  return false
end

function Binding:flush() return flush(self) end
function Binding:isDirty()
  self:observe()
  return self.dirty
end
function Binding:lastIssue() return self.issue end
function Binding:markDirty()
  self.dirty = true
  self.elapsed = 0
end
function Binding:dispose() self.disposed = true end

function Contract:bind(store, options)
  assert(type(store) == "table" and type(store.get) == "function"
    and type(store.revision) == "function", "Save.bind expects a store with revisions")
  assert(type(options) == "table" and type(options.select) == "function",
    "Save.bind needs a select(state) projection")
  local debounce = options.debounce or 1
  local maxInterval = options.max_interval or 10
  assert(type(debounce) == "number" and debounce >= 0 and debounce < math.huge,
    "save debounce must be finite and nonnegative")
  assert(type(maxInterval) == "number" and maxInterval > 0 and maxInterval < math.huge,
    "save max_interval must be finite and positive")
  local revision = store:revision()
  return setmetatable({
    contract = self,
    store = store,
    select = options.select,
    observed = revision,
    dirty = self.pendingUpgrade,
    issue = self.blocked,
    debounce = debounce,
    maxInterval = maxInterval,
    elapsed = 0,
    sinceDirty = 0,
    retry = 0,
    failures = 0,
    disposed = false,
  }, Binding)
end

return Save
