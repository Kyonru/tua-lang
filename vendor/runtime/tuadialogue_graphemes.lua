-- Unicode 15.1 extended grapheme clusters (UAX #29 revision 43).
-- Byte offsets are one-based and inclusive; these are not shaped glyphs.
local data = require("tuadialogue_unicode_data")
local Graphemes = { version = data.version }

local function property(rows, code)
  local low, high = 1, #rows
  while low <= high do
    local middle = math.floor((low + high) / 2)
    local row = rows[middle]
    if code < row[1] then high = middle - 1
    elseif code > row[2] then low = middle + 1
    else return row[3] end
  end
  return "Other"
end

local function decode(text, at)
  local first = text:byte(at)
  if first < 128 then return first, at + 1 end
  local count, code, minimum
  if first >= 194 and first <= 223 then count, code, minimum = 2, first - 192, 128
  elseif first >= 224 and first <= 239 then count, code, minimum = 3, first - 224, 2048
  elseif first >= 240 and first <= 244 then count, code, minimum = 4, first - 240, 65536
  else error("invalid UTF-8 at byte " .. at, 0) end
  for index = at + 1, at + count - 1 do
    local byte = text:byte(index)
    assert(byte and byte >= 128 and byte <= 191, "invalid UTF-8 continuation at byte " .. index)
    code = code * 64 + byte - 128
  end
  assert(
    code >= minimum and code <= 1114111 and not (code >= 55296 and code <= 57343),
    "invalid UTF-8 scalar"
  )
  return code, at + count
end

local function kind(code)
  if code >= 44032 and code <= 55203 then
    return (code - 44032) % 28 == 0 and "LV" or "LVT"
  end
  return property(data.gcb, code)
end

local function control(value)
  return value == "Control" or value == "CR" or value == "LF"
end

local function joins(points, index, regionalCount)
  local before, after = points[index - 1], points[index]
  local left, right = before.kind, after.kind
  if left == "CR" and right == "LF" then return true end -- GB3
  if control(left) or control(right) then return false end -- GB4/GB5
  if left == "L" and (right == "L" or right == "V" or right == "LV" or right == "LVT") then
    return true
  end
  if (left == "LV" or left == "V") and (right == "V" or right == "T") then return true end
  if (left == "LVT" or left == "T") and right == "T" then return true end
  if right == "Extend" or right == "ZWJ" or right == "SpacingMark" then return true end
  if left == "Prepend" then return true end
  -- GB9c: a conjunct links back to the nearest preceding Indic consonant.
  if after.indic == "Consonant" then
    local cursor, linker = index - 1, false
    while cursor > 0 and (points[cursor].indic == "Extend" or points[cursor].indic == "Linker") do
      if points[cursor].indic == "Linker" then linker = true end
      cursor = cursor - 1
    end
    if linker and cursor > 0 and points[cursor].indic == "Consonant" then return true end
  end
  -- GB11: Extended_Pictographic Extend* ZWJ × Extended_Pictographic.
  if after.pictographic and left == "ZWJ" then
    local cursor = index - 2
    while cursor > 0 and points[cursor].kind == "Extend" do cursor = cursor - 1 end
    if cursor > 0 and points[cursor].pictographic then return true end
  end
  -- GB12/GB13: pair regional indicators without quadratic backward scans.
  if left == "Regional_Indicator" and right == "Regional_Indicator" then
    return regionalCount % 2 == 1
  end
  return false
end

function Graphemes.segment(text)
  assert(
    type(text) == "string" and #text <= 65536,
    "reveal text must be a string of at most 65536 UTF-8 bytes"
  )
  local points, cursor = {}, 1
  while cursor <= #text do
    local code, following = decode(text, cursor)
    points[#points + 1] = {
      first = cursor,
      last = following - 1,
      kind = kind(code),
      indic = property(data.indic, code),
      pictographic = property(data.pictographic, code) == "yes",
      speakable = property(data.speech, code) == "yes",
    }
    cursor = following
  end
  local result, regionalCount = {}, 0
  for index, point in ipairs(points) do
    if index == 1 or not joins(points, index, regionalCount) then
      result[#result + 1] = {
        startByte = point.first,
        endByte = point.last,
        speakable = point.speakable,
      }
    else
      local cluster = result[#result]
      cluster.endByte, cluster.speakable = point.last, cluster.speakable or point.speakable
    end
    regionalCount = point.kind == "Regional_Indicator" and regionalCount + 1 or 0
  end
  for index, cluster in ipairs(result) do
    cluster.index = index
    cluster.text = text:sub(cluster.startByte, cluster.endByte)
  end
  return result
end

return Graphemes
