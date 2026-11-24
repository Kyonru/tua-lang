-- Official Unicode 15.1 GraphemeBreakTest data is passed by the host harness.
return function(corpus)
  local Graphemes = require("tuadialogue_graphemes")
  local function utf8(code)
    if code < 128 then return string.char(code) end
    if code < 2048 then return string.char(192 + math.floor(code / 64), 128 + code % 64) end
    if code < 65536 then
      return string.char(
        224 + math.floor(code / 4096),
        128 + math.floor(code / 64) % 64,
        128 + code % 64
      )
    end
    return string.char(240 + math.floor(code / 262144), 128 + math.floor(code / 4096) % 64,
      128 + math.floor(code / 64) % 64, 128 + code % 64)
  end
  local count = 0
  for line in corpus:gmatch("[^\r\n]+") do
    local body = line:match("^[^#]*")
    if body:match("%S") then
      local parts, expected, length = {}, {}, 0
      for value in body:gmatch("%S+") do
        if value == "÷" then expected[#expected + 1] = length + 1
        elseif value ~= "×" then
          local encoded = utf8(assert(tonumber(value, 16)))
          parts[#parts + 1], length = encoded, length + #encoded
        end
      end
      local text = table.concat(parts)
      local actual = Graphemes.segment(text)
      count = count + 1
      assert(#actual + 1 == #expected, "boundary count case " .. count .. ": " .. line)
      for index, cluster in ipairs(actual) do
        assert(cluster.startByte == expected[index], "start byte case " .. count)
        assert(cluster.endByte == expected[index + 1] - 1, "end byte case " .. count)
        assert(cluster.text == text:sub(cluster.startByte, cluster.endByte))
      end
    end
  end
  assert(count == 1187, "unexpected conformance case count: " .. count)
  for _, invalid in ipairs({
    string.char(128),
    string.char(192, 128),
    string.char(237, 160, 128),
    string.char(244, 144, 128, 128),
    string.char(226, 130),
    string.rep("a", 65537),
  }) do
    assert(not pcall(Graphemes.segment, invalid))
  end
  assert(#Graphemes.segment("") == 0)
  print("Unicode 15.1 grapheme conformance passed: " .. count)
end
