-- Bounded template compilation and renderer-neutral resolved presentations.
local Graphemes = require("tuadialogue_graphemes")
local Text = require("tuadialogue_text")
local Rich = {}

local function identifier(value)
  return value:match("^[A-Za-z_][A-Za-z0-9_]*$") ~= nil
end

local function decimal(value)
  if not value:match("^%d+$") and not value:match("^%d+%.%d+$") then return nil end
  local number = tonumber(value)
  if number and number < math.huge then return number end
end

local function copy(value)
  local result = {}
  for key, child in pairs(value) do
    result[key] = type(child) == "table" and copy(child) or child
  end
  return result
end

local escapes = { n = "\n", r = "\r", t = "\t" }
for character in("\\\"{}$()[]"):gmatch(".") do escapes[character] = character end

function Rich.parse(source, timing)
  Graphemes.segment(source) -- Validate UTF-8 and the 65536-byte input limit.
  local tokens, stack, plain = {}, {}, {}
  local cursor, content, pauses, duration, rounding = 1, 0, 0, 0, 0
  local function push(first, kind, value, name)
    assert(#tokens < 4096, "rich template exceeds 4096 tokens")
    tokens[#tokens + 1] = {
      kind = kind,
      value = value,
      name = name,
      first = first,
      last = cursor - 1,
    }
    if kind == "text" then
      content = content + 1
      plain[#plain + 1] = value:gsub("%%", "%%%%")
    elseif kind == "placeholder" then
      content = content + 1
      plain[#plain + 1] = "%{" .. value .. "}"
    end
  end
  while cursor <= #source do
    local first, ch = cursor, source:sub(cursor, cursor)
    if ch == "\\" then
      local value = escapes[source:sub(cursor + 1, cursor + 1)]
      assert(value, "unknown or unfinished rich text escape")
      cursor = cursor + 2
      push(first, "text", value)
    elseif source:sub(cursor, cursor + 1) == "%%" then
      cursor = cursor + 2
      if source:sub(cursor, cursor) == "{" then
        local ending = assert(source:find("}", cursor + 1, true), "unterminated named placeholder")
        local raw = source:sub(cursor + 1, ending - 1)
        assert(
          not raw:find("%c") and identifier(raw:match("^%s*(.-)%s*$")),
          "invalid named placeholder"
        )
        cursor = ending + 1
      end
      push(first, "text", source:sub(first + 1, cursor - 1))
    elseif source:sub(cursor, cursor + 1) == "%{" then
      local ending = assert(source:find("}", cursor + 2, true), "unterminated named placeholder")
      local raw = source:sub(cursor + 2, ending - 1)
      local name = raw:match("^%s*(.-)%s*$")
      assert(not raw:find("%c") and identifier(name), "invalid named placeholder")
      cursor = ending + 1
      push(first, "placeholder", name)
    elseif ch == "[" then
      cursor = cursor + 1
      local quoted, escaped, closed = false, false, false
      while cursor <= #source do
        ch = source:sub(cursor, cursor)
        assert(not ch:find("%c"), "control character in rich mark")
        cursor = cursor + 1
        if escaped then escaped = false
        elseif quoted and ch == "\\" then escaped = true
        elseif ch == '"' then quoted = not quoted
        elseif ch == "]" and not quoted then
          closed = true
          break
        end
      end
      assert(closed, "unterminated rich mark")
      local body = source:sub(first + 1, cursor - 2)
      local closing = body:match("^/(.+)$")
      if closing then
        local open = stack[#stack]
        assert(open and open.name == closing, "rich marks must close in reverse order")
        assert(open.content < content, "empty paired rich mark")
        stack[#stack] = nil
        push(first, "close", nil, closing)
      else
        local name, value = body:match("^([a-z]+)=(.*)$")
        if body == "em" then name, value = "em", true end
        if name == "pause" or name == "speed" then
          value = decimal(value)
          assert(
            value and (name == "pause" and value <= 60 or name == "speed" and value > 0 and value <= 1000),
            "invalid rich timing value"
          )
          assert(timing, "timing marks are only supported in lines")
        elseif name == "style" then
          assert(#value <= 64 and identifier(value), "invalid rich style identifier")
        elseif name == "pronounce" then
          assert(
            value:sub(1, 1) == '"' and value:sub(-1) == '"',
            "pronunciation needs a quoted hint"
          )
          local decoded, index = {}, 2
          while index < #value do
            local letter = value:sub(index, index)
            if letter == "\\" then
              index = index + 1
              letter = value:sub(index, index)
              assert(
                index < #value and (letter == "\\" or letter == '"'),
                "invalid pronunciation escape"
              )
            else assert(letter ~= '"', "unexpected pronunciation quote") end
            decoded[#decoded + 1] = letter
            index = index + 1
          end
          value = table.concat(decoded)
          assert(
            #value > 0 and #value <= 256 and not value:find("%{", 1, true),
            "invalid pronunciation hint"
          )
          for _, open in ipairs(stack) do
            assert(open.name ~= "pronounce", "pronunciation marks cannot nest")
          end
        else assert(name == "em", "unknown rich mark") end
        if name == "pause" then
          pauses = pauses + 1
          local corrected = value - rounding
          local total = duration + corrected
          rounding, duration = (total - duration) - corrected, total
          assert(pauses <= 256 and duration <= 120, "rich pause budget exceeded")
          push(first, "pause", value)
        else
          assert(#stack < 16, "rich mark nesting exceeds 16")
          stack[#stack + 1] = { name = name, content = content }
          push(first, "open", value, name)
        end
      end
    else
      assert(
        ch ~= "]" and ch ~= "{" and ch ~= "}" and (not ch:find("%c") or ch == "\t"),
        "unescaped rich text delimiter or control"
      )
      cursor = cursor + 1
      while cursor <= #source do
        local next = source:sub(cursor, cursor)
        if next == "["
          or next == "]"
          or next == "{"
          or next == "}"
          or next == "\\"
          or next == "%"
          or next:find("%c") then
          break
        end
        cursor = cursor + 1
      end
      push(first, "text", source:sub(first, cursor - 1))
    end
  end
  assert(#stack == 0, "unclosed rich mark")
  assert(content > 0, "rich text needs text or a placeholder")
  return { source = source, tokens = tokens, plain = table.concat(plain), timing = timing }
end

local function metadata(stack)
  local result = { emphasis = false, styles = {} }
  for _, mark in ipairs(stack) do
    if mark.name == "em" then result.emphasis = true
    elseif mark.name == "style" then result.styles[#result.styles + 1] = mark.value
    elseif mark.name == "speed" then result.rate = mark.value
    elseif mark.name == "pronounce" then result.pronunciation = mark.value end
  end
  return result
end

function Rich.resolve(compiled, args, selected)
  local locale, variant
  if selected then
    assert(
      type(selected) == "table" and type(selected.template) == "string",
      "template resolver must return a template record"
    )
    assert(selected.locale == nil or type(selected.locale) == "string", "invalid template locale")
    assert(
      selected.variant == nil or type(selected.variant) == "string",
      "invalid template variant"
    )
    local expected = Text.placeholders(compiled.source)
    local actual = Text.placeholders(selected.template)
    for name in pairs(expected) do
      assert(actual[name], "missing dialogue placeholder: " .. name)
    end
    for name in pairs(actual) do assert(expected[name], "extra dialogue placeholder: " .. name) end
    compiled = Rich.parse(selected.template, compiled.timing)
    locale, variant = selected.locale, selected.variant
  end
  local output, origins, chunks, pauses, stack, boundaries = {}, {}, {}, {}, {}, {}
  local size = 0
  for _, token in ipairs(compiled.tokens) do
    if token.kind == "text" or token.kind == "placeholder" then
      local value = token.value
      if token.kind == "placeholder" then
        value = args[value]
        assert(
          type(value) == "string" or type(value) == "number" or type(value) == "boolean",
          "missing or invalid rich argument: " .. token.value
        )
        assert(
          type(value) ~= "number" or value == value and math.abs(value) < math.huge,
          "nonfinite rich argument"
        )
        value = tostring(value)
      end
      assert(#value <= 65536 - size, "resolved rich text exceeds 65536 bytes")
      origins[#origins + 1] = {
        startByte = size + 1,
        endByte = size + #value,
        templateStartByte = token.first,
        templateEndByte = token.last,
        argument = token.kind == "placeholder" and token.value or nil,
      }
      if #value > 0 then
        chunks[#chunks + 1] = { first = size + 1, last = size + #value, metadata = metadata(stack) }
      end
      output[#output + 1], size = value, size + #value
    elseif token.kind == "open" then
      boundaries[#boundaries + 1] = size
      stack[#stack + 1] = { name = token.name, value = token.value, first = size }
    elseif token.kind == "close" then
      boundaries[#boundaries + 1] = size
      assert(stack[#stack].first < size, "resolved paired mark is empty")
      stack[#stack] = nil
    else
      pauses[#pauses + 1] = { afterByte = size, seconds = token.value }
      boundaries[#boundaries + 1] = size
    end
  end
  local text = table.concat(output)
  local units = Graphemes.segment(text)
  local boundary, clusterAt = { [0] = 0 }, {}
  for index, unit in ipairs(units) do
    boundary[unit.endByte] = index
    for byte = unit.startByte, unit.endByte do clusterAt[byte] = index end
  end
  for _, byte in ipairs(boundaries) do
    assert(boundary[byte] ~= nil, "rich mark splits a grapheme cluster")
  end
  local runs = {}
  for _, chunk in ipairs(chunks) do
    local first, last = clusterAt[chunk.first], clusterAt[chunk.last]
    -- Adjacent literal/placeholder tokens can share one grapheme. Marks cannot.
    if #runs > 0 and first <= runs[#runs].lastCluster then first = runs[#runs].lastCluster + 1 end
    if first <= last then
      local run = copy(chunk.metadata)
      run.firstCluster, run.lastCluster = first, last
      runs[#runs + 1] = run
    end
  end
  for _, pause in ipairs(pauses) do
    pause.afterCluster, pause.afterByte = boundary[pause.afterByte], nil
  end
  return {
    text = text,
    runs = runs,
    pauses = pauses,
    origins = origins,
    resolvedLocale = locale,
    variant = variant,
  }
end

return Rich
