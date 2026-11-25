-- Dialogue text is scalar and uses named placeholders only. Replacement values
-- are never parsed again as formatting syntax, markup, or dialogue directives.
local function dialoguePlaceholders(text)
  local names, cursor = {}, 1
  while cursor <= #text do
    local pair = text:sub(cursor, cursor + 1)
    if pair == "%%" then cursor = cursor + 2
    elseif pair == "%{" then
      local ending = text:find("}", cursor + 2, true)
      assert(ending, "unterminated dialogue placeholder")
      local name = text:sub(cursor + 2, ending - 1):match("^%s*(.-)%s*$")
      assert(name:match("^[A-Za-z_][A-Za-z0-9_]*$"), "invalid dialogue placeholder")
      names[name], cursor = true, ending + 1
    else cursor = cursor + 1 end
  end
  return names
end

local function dialogueFormat(text, args)
  local output, cursor = {}, 1
  while cursor <= #text do
    local pair = text:sub(cursor, cursor + 1)
    if pair == "%%" then
      output[#output + 1], cursor = "%", cursor + 2
    elseif pair == "%{" then
      local ending = text:find("}", cursor + 2, true)
      assert(ending, "unterminated dialogue placeholder")
      local name = text:sub(cursor + 2, ending - 1):match("^%s*(.-)%s*$")
      local value = args[name]
      assert(type(value) == "string" or type(value) == "number" or type(value) == "boolean",
        "missing or invalid dialogue argument: " .. name)
      output[#output + 1], cursor = tostring(value), ending + 1
    else
      output[#output + 1], cursor = text:sub(cursor, cursor), cursor + 1
    end
  end
  return table.concat(output)
end

return { placeholders = dialoguePlaceholders, format = dialogueFormat }
