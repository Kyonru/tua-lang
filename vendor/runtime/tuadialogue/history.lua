-- Optional transcript storage. Only scalar presentation data crosses this boundary.
local History = {}
local function copy(entry)
  local result = {}
  for key, value in pairs(entry) do result[key] = value end
  return result
end
function History.new(options)
  assert(type(options) == "table" and getmetatable(options) == nil, "invalid history options")
  for key in pairs(options) do
    assert(key == "capacity" or key == "maxBytes", "unknown history option")
  end
  local capacity, maxBytes = options.capacity, options.maxBytes
  if maxBytes == nil then maxBytes = 65536 end
  assert(type(capacity) == "number" and capacity % 1 == 0 and capacity >= 1 and capacity <= 1024,
    "history capacity must be an integer from 1 to 1024")
  assert(type(maxBytes) == "number" and maxBytes % 1 == 0 and maxBytes >= 1 and maxBytes <= 1048576,
    "history maxBytes must be an integer from 1 to 1048576")
  return {
    capacity = capacity,
    maxBytes = maxBytes,
    rows = {},
    head = 1,
    count = 0,
    bytes = 0,
    dropped = 0,
    serial = 0,
  }
end
function History.capture(player, node, choice)
  return {
    kind = choice and "choice" or "line",
    documentId = player.document.documentId,
    nodeId = player.current,
    textId = choice and choice.id or node.textId,
    choiceId = choice and choice.id or nil,
    text = choice and choice.text or node.text,
    speaker = node.speaker,
    speakerKey = node.speakerKey,
    locale = player.locale,
  }
end
local function removeFirst(history)
  local row = history.rows[history.head]
  history.rows[history.head] = nil
  history.head = history.head % history.capacity + 1
  history.count = history.count - 1
  history.bytes = history.bytes - row.bytes
end
function History.push(history, entry)
  local bytes = 0
  for _, value in pairs(entry) do
    if type(value) == "string" then bytes = bytes + #value end
  end
  if bytes > history.maxBytes then
    history.dropped = history.dropped + 1
    return
  end
  while history.count == history.capacity or history.bytes + bytes > history.maxBytes do
    removeFirst(history)
    history.dropped = history.dropped + 1
  end
  history.serial = history.serial + 1
  entry.id = history.serial
  local index = (history.head + history.count - 1) % history.capacity + 1
  history.rows[index] = { entry = entry, bytes = bytes }
  history.count = history.count + 1
  history.bytes = history.bytes + bytes
end
function History.snapshot(history)
  local result = { entries = {}, capacity = 0, maxBytes = 0, bytes = 0, dropped = 0 }
  if not history then return result end
  result.capacity, result.maxBytes = history.capacity, history.maxBytes
  result.bytes, result.dropped = history.bytes, history.dropped
  for offset = 0, history.count - 1 do
    local index = (history.head + offset - 1) % history.capacity + 1
    result.entries[#result.entries + 1] = copy(history.rows[index].entry)
  end
  return result
end
function History.clear(history)
  history.rows, history.head, history.count = {}, 1, 0
  history.bytes, history.dropped = 0, 0
end
function History.redact(history, id)
  local rows, found = {}, false
  for offset = 0, history.count - 1 do
    local index = (history.head + offset - 1) % history.capacity + 1
    local row = history.rows[index]
    if row.entry.id == id then
      history.bytes = history.bytes - row.bytes
      found = true
    else
      rows[#rows + 1] = row
    end
  end
  history.rows, history.head, history.count = rows, 1, #rows
  return found
end
return History
