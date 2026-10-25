-- Extension-owned development shim. Never copied into compiler output.
local M = {}
local objects = setmetatable({}, { __mode = "k" })
local serial, sequence = 0, 0
local LIMIT = 3000

local function quote(value)
  return '"' .. value:gsub('[%z\1-\31\\"]', function(c)
    return string.format("\\u%04x", string.byte(c))
  end) .. '"'
end

-- Repeated field names dominate encoding work. Keep a bounded cache; never
-- cache arbitrary values or retain references to game tables.
local quotedKeys, quotedKeyCount = {}, 0
local function quoteKey(value)
  local key = tostring(value):sub(1, 240)
  local quoted = quotedKeys[key]
  if quoted then return quoted end
  if quotedKeyCount >= 512 then quotedKeys, quotedKeyCount = {}, 0 end
  quoted = quote(key)
  quotedKeys[key], quotedKeyCount = quoted, quotedKeyCount + 1
  return quoted
end

-- Never invoke game metamethods or stringify userdata; cycles and large values
-- become explicit markers. A shared budget bounds the entire serialized frame.
local function encode(value, budget, seen, depth)
  budget.left = budget.left - 1
  if budget.left < 0 then budget.truncated = true; return '"<snapshot limit>"' end
  local kind = type(value)
  if kind == "nil" then return "null" end
  if kind == "boolean" then return value and "true" or "false" end
  if kind == "number" then
    if value ~= value or value == math.huge or value == -math.huge then return '"<non-finite>"' end
    return tostring(value)
  end
  if kind == "string" then
    if #value > 240 then budget.truncated = true; value = value:sub(1, 240) .. "…" end
    return quote(value)
  end
  if kind ~= "table" then return quote("<" .. kind .. ">") end
  if seen[value] then return '"<cycle>"' end
  if depth >= 8 then budget.truncated = true; return '"<depth limit>"' end
  seen[value] = true
  local parts, count = {}, 0
  for key, child in next, value do
    if count >= 128 or budget.left <= 0 then
      budget.truncated = true
      parts[#parts + 1] = '"<remaining>":"<snapshot limit>"'
      break
    end
    if type(key) == "string" or type(key) == "number" then
      count = count + 1
      parts[#parts + 1] = quoteKey(key) .. ":" .. encode(child, budget, seen, depth + 1)
    end
  end
  seen[value] = nil
  return "{" .. table.concat(parts, ",") .. "}"
end

function M.register(kind, object, reader, label)
  serial = serial + 1
  local info = debug and debug.getinfo and debug.getinfo(3, "Sl") or {}
  local call = debug and debug.getinfo and debug.getinfo(2, "n") or {}
  objects[object] = { constructor = call.name, id = serial, kind = kind, reader = reader,
    label = type(label) == "string" and label or kind .. " " .. serial,
    source = info.source, line = info.currentline, emitted = 0, events = {} }
  return object
end

local function clock()
  return love and love.timer and love.timer.getTime and love.timer.getTime() or os.clock()
end

function M.storeUpdated(object)
  local entry = objects[object]
  if not entry then return end
  entry.updates = (entry.updates or 0) + 1
  entry.lastUpdate = clock()
end

function M.signal(object, name)
  local entry = objects[object]
  if not entry then return end
  entry.emitted = entry.emitted + 1
  -- A bounded recent history, without retaining payloads or game objects.
  entry.eventHead = (entry.eventHead or 0) % 20 + 1
  local event = entry.events[entry.eventHead] or {}
  event.name, event.sequence = name, entry.emitted
  entry.events[entry.eventHead] = event
end


function M.snapshot()
  local budget = { left = LIMIT, truncated = false }
  local entries, count = {}, 0
  for object, entry in pairs(objects) do
    if count >= 128 or budget.left < 8 then budget.truncated = true; break end
    count = count + 1
    local success, data = pcall(entry.reader, object)
    if not success then data = { unavailable = "Could not read runtime object" } end
    if entry.kind == "stores" then
      data.updates = entry.updates or 0
      data.secondsSinceUpdate = entry.lastUpdate and math.max(0, clock() - entry.lastUpdate) or "Never"
    end
    if entry.kind == "signals" then
      data.emitted, data.recent = entry.emitted, {}
      for i = 1, #entry.events do
        data.recent[i] = entry.events[((entry.eventHead - i) % 20) + 1]
      end
    end
    -- Reserve the protocol envelope before encoding game data. Truncation must
    -- never remove required identity fields or replace data with a string.
    if not entry.metadata then
      local metadataBudget = { left = 16 }
      entry.metadata = encode({ id = entry.id, kind = entry.kind, label = entry.label,
        source = entry.source, line = entry.line, constructor = entry.constructor }, metadataBudget, {}, 0):sub(1, -2)
      entry.metadataCost = 16 - metadataBudget.left
      entry.metadataTruncated = metadataBudget.truncated
    end
    budget.left = budget.left - entry.metadataCost
    if entry.metadataTruncated then budget.truncated = true end
    local payload = encode(data, budget, {}, 1)
    entries[#entries + 1] = entry.metadata .. ',"data":' .. payload .. '}'
  end
  sequence = sequence + 1
  local bytes = 0
  for _, entry in ipairs(entries) do bytes = bytes + #entry + 1 end
  while bytes > 900000 and #entries > 0 do
    local removed = table.remove(entries)
    bytes = bytes - #removed - 1
    budget.truncated = true
  end
  return '{"version":1,"sequence":' .. sequence .. ',"truncated":' .. tostring(budget.truncated)
    .. ',"objects":[' .. table.concat(entries, ",") .. ']}'
end
return M
