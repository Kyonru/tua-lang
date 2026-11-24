-- Strict, bounded JSON subset for Tua save contracts. Lua 5.1 compatible.
-- Save and dialogue checkpoints share UTF-8 validation; this is not a public JSON API.
local Json = {}
Json.null = {}
Json.array = {}
Json.object = {}

local function utf8Length(text, position)
  local first = text:byte(position)
  if first < 0x80 then return 1 end
  local count, minimum
  if first >= 0xC2 and first <= 0xDF then count, minimum = 2, 0x80
  elseif first >= 0xE0 and first <= 0xEF then count, minimum = 3, 0x800
  elseif first >= 0xF0 and first <= 0xF4 then count, minimum = 4, 0x10000
  else return nil end
  local value = first % (2 ^ (8 - count - 1))
  for index = 1, count - 1 do
    local byte = text:byte(position + index)
    if not byte or byte < 0x80 or byte > 0xBF then return nil end
    value = value * 64 + byte - 0x80
  end
  if value < minimum or value > 0x10FFFF or (value >= 0xD800 and value <= 0xDFFF) then
    return nil
  end
  return count
end

function Json.validUtf8(text)
  local position = 1
  while position <= #text do
    local width = utf8Length(text, position)
    if not width then return false end
    position = position + width
  end
  return true
end

local function utf8Codepoint(value)
  if value <= 0x7F then return string.char(value) end
  if value <= 0x7FF then
    return string.char(0xC0 + math.floor(value / 64), 0x80 + value % 64)
  end
  if value <= 0xFFFF then
    return string.char(0xE0 + math.floor(value / 4096),
      0x80 + math.floor(value / 64) % 64, 0x80 + value % 64)
  end
  return string.char(0xF0 + math.floor(value / 262144),
    0x80 + math.floor(value / 4096) % 64, 0x80 + math.floor(value / 64) % 64,
    0x80 + value % 64)
end

function Json.quote(text)
  assert(type(text) == "string" and Json.validUtf8(text), "save JSON needs valid UTF-8 strings")
  return '"' .. text:gsub('[%z\1-\31\\"]', function(character)
    if character == '"' then return '\\"' end
    if character == '\\' then return '\\\\' end
    return string.format("\\u%04x", character:byte())
  end) .. '"'
end

function Json.decode(text, maxDepth, maxEntries)
  if type(text) ~= "string" then return nil, "payload is not a string" end
  local position, length, entries = 1, #text, 0
  local function fail(message) error(message .. " at byte " .. position, 0) end
  local function whitespace()
    while position <= length do
      local byte = text:byte(position)
      if byte ~= 32 and byte ~= 10 and byte ~= 13 and byte ~= 9 then break end
      position = position + 1
    end
  end
  local function hex4()
    local digits = text:sub(position, position + 3)
    if #digits ~= 4 or not digits:match("^%x%x%x%x$") then fail("invalid Unicode escape") end
    position = position + 4
    return tonumber(digits, 16)
  end
  local function stringValue()
    position = position + 1
    local pieces = {}
    while position <= length do
      local byte = text:byte(position)
      if byte == 34 then
        position = position + 1
        return table.concat(pieces)
      elseif byte == 92 then
        position = position + 1
        local escaped = text:sub(position, position)
        local basic = {
          ['"'] = '"',
          ['\\'] = '\\',
          ['/'] = '/',
          b = '\b',
          f = '\f',
          n = '\n',
          r = '\r',
          t = '\t',
        }
        if basic[escaped] then
          pieces[#pieces + 1] = basic[escaped]
          position = position + 1
        elseif escaped == 'u' then
          position = position + 1
          local value = hex4()
          if value >= 0xD800 and value <= 0xDBFF then
            if text:sub(position, position + 1) ~= '\\u' then fail("missing low surrogate") end
            position = position + 2
            local low = hex4()
            if low < 0xDC00 or low > 0xDFFF then fail("invalid low surrogate") end
            value = 0x10000 + (value - 0xD800) * 1024 + low - 0xDC00
          elseif value >= 0xDC00 and value <= 0xDFFF then
            fail("unexpected low surrogate")
          end
          pieces[#pieces + 1] = utf8Codepoint(value)
        else fail("invalid string escape") end
      elseif byte < 32 then
        fail("unescaped control character")
      elseif byte >= 128 then
        local width = utf8Length(text, position)
        if not width then fail("invalid UTF-8") end
        pieces[#pieces + 1] = text:sub(position, position + width - 1)
        position = position + width
      else
        pieces[#pieces + 1] = string.char(byte)
        position = position + 1
      end
    end
    fail("unterminated string")
  end
  local function numberValue()
    local start = position
    if text:sub(position, position) == '-' then position = position + 1 end
    local first = text:byte(position)
    if first == 48 then
      position = position + 1
      local nextByte = text:byte(position)
      if nextByte and nextByte >= 48 and nextByte <= 57 then fail("leading zero") end
    elseif first and first >= 49 and first <= 57 then
      repeat
        position = position + 1
        first = text:byte(position)
      until not first or first < 48 or first > 57
    else fail("invalid number") end
    if text:sub(position, position) == '.' then
      position = position + 1
      first = text:byte(position)
      if not first or first < 48 or first > 57 then fail("invalid fraction") end
      repeat
        position = position + 1
        first = text:byte(position)
      until not first or first < 48 or first > 57
    end
    local exponent = text:sub(position, position)
    if exponent == 'e' or exponent == 'E' then
      position = position + 1
      local sign = text:sub(position, position)
      if sign == '+' or sign == '-' then position = position + 1 end
      first = text:byte(position)
      if not first or first < 48 or first > 57 then fail("invalid exponent") end
      repeat
        position = position + 1
        first = text:byte(position)
      until not first or first < 48 or first > 57
    end
    local value = tonumber(text:sub(start, position - 1))
    if not value or value ~= value or value == math.huge or value == -math.huge then
      fail("non-finite number")
    end
    return value
  end
  local value
  value = function(depth)
    if depth > maxDepth then fail("JSON nesting limit exceeded") end
    entries = entries + 1
    if entries > maxEntries then fail("JSON entry limit exceeded") end
    whitespace()
    local byte = text:byte(position)
    if byte == 34 then return stringValue() end
    if byte == 45 or (byte and byte >= 48 and byte <= 57) then return numberValue() end
    if byte == 91 then
      position = position + 1
      local result = { [Json.array] = true }
      whitespace()
      if text:byte(position) == 93 then
        position = position + 1
        return result
      end
      while true do
        result[#result + 1] = value(depth + 1)
        whitespace()
        byte = text:byte(position)
        if byte == 93 then
          position = position + 1
          return result
        end
        if byte ~= 44 then fail("expected array comma or end") end
        position = position + 1
      end
    end
    if byte == 123 then
      position = position + 1
      local result = { [Json.object] = true }
      whitespace()
      if text:byte(position) == 125 then
        position = position + 1
        return result
      end
      while true do
        whitespace()
        if text:byte(position) ~= 34 then fail("expected object key") end
        local key = stringValue()
        if result[key] ~= nil then fail("duplicate object key") end
        whitespace()
        if text:byte(position) ~= 58 then fail("expected colon") end
        position = position + 1
        result[key] = value(depth + 1)
        whitespace()
        byte = text:byte(position)
        if byte == 125 then
          position = position + 1
          return result
        end
        if byte ~= 44 then fail("expected object comma or end") end
        position = position + 1
      end
    end
    for literal, result in pairs({ ['true'] = true, ['false'] = false, ['null'] = Json.null }) do
      if text:sub(position, position + #literal - 1) == literal then
        position = position + #literal
        return result
      end
    end
    fail("invalid JSON value")
  end
  local ok, result = pcall(value, 1)
  if not ok then return nil, result end
  whitespace()
  if position <= length then return nil, "trailing JSON data at byte " .. position end
  return result
end

return Json
