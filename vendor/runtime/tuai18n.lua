-- Tua's configured localization adapter. Kept separate from the pinned Smiti18n runtime.
local config = require("tuai18n_config")
-- LuaJIT's default package.path may omit ./?/init.lua. Requiring the entry
-- module explicitly also works with Lua 5.1 and LOVE's module search path.
local smiti = require("smiti18n.init")
local previous_locale = smiti.getLocale()

for _, catalog in ipairs(config.catalogs) do
  local data = require(catalog.module)
  if type(data) ~= "table" or type(data[catalog.locale]) ~= "table" then
    error("i18n catalog " .. catalog.module .. " must return a table rooted at " .. catalog.locale)
  end
  smiti.load(data)
end

smiti.setFallbackLocale(config.fallback_locale)
smiti.setLocale(previous_locale or config.default_locale)

local I18n = {}

function I18n.t(key, args, locale)
  if locale == nil then
    return smiti.translate(key, args)
  end
  -- Smiti includes the current locale after data.locale. A per-call override
  -- must instead fall back directly to the configured fallback locale.
  local previous = smiti.getLocale()
  local data = {}
  if args then
    for name, value in pairs(args) do data[name] = value end
  end
  data.locale = locale
  smiti.setLocale(locale)
  local ok, result = pcall(smiti.translate, key, data)
  smiti.setLocale(previous)
  if not ok then error(result, 0) end
  return result
end

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

function I18n.dialogue(key, source, args, locale)
  assert(type(source) == "string" and type(args) == "table", "invalid dialogue translation")
  local value, resolvedLocale = smiti.resolve(key, locale)
  if value == nil then value = source end
  if type(value) == "table" then
    assert(type(value.other) == "string" and value[1] == nil,
      "dialogue translation must be text or a plural entry: " .. key)
    local count = args.count
    assert(type(count) == "number" and count == count and math.abs(count) < math.huge,
      "dialogue plural needs a finite count")
    value = value[smiti.plural.get(smiti.variants.root(resolvedLocale), count)] or value.other
  end
  assert(type(value) == "string", "dialogue translation must be text: " .. key)
  local expected, actual = dialoguePlaceholders(source), dialoguePlaceholders(value)
  for name in pairs(expected) do assert(actual[name], "missing dialogue placeholder: " .. name) end
  for name in pairs(actual) do assert(expected[name], "extra dialogue placeholder: " .. name) end
  return dialogueFormat(value, args)
end

function I18n.setLocale(locale)
  smiti.setLocale(locale)
end

function I18n.getLocale()
  return smiti.getLocale()
end

function I18n.getLocales()
  local result = {}
  for index, catalog in ipairs(config.catalogs) do result[index] = catalog.locale end
  return result
end

I18n.formatNumber = smiti.formatNumber
I18n.formatPrice = smiti.formatPrice
I18n.formatDate = smiti.formatDate

return I18n
