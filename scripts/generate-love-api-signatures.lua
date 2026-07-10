local root = assert(arg[1], "usage: lua scripts/generate-love-api-signatures.lua /path/to/love-api [out]")
local out = arg[2] or "crates/tua-core/src/love_api_signatures.rs"

package.path = root .. "/?.lua;" .. root .. "/?/init.lua;" .. package.path

local chunk = assert(loadfile(root .. "/love_api.lua"))
local api = chunk("love_api")

local function rust_raw(value)
  value = tostring(value or "")

  for count = 0, 8 do
    local hashes = string.rep("#", count)
    if not value:find('"' .. hashes, 1, true) then
      if count == 0 then
        return 'r"' .. value .. '"'
      end

      return "r" .. hashes .. '"' .. value .. '"' .. hashes
    end
  end

  error("could not encode raw Rust string")
end

local function sort_by_name(items)
  table.sort(items, function(left, right)
    return left.name < right.name
  end)
  return items
end

local function type_label(label)
  label = tostring(label or "any")
  label = label:gsub("%s+or%s+", " | ")
  return label
end

local function argument_label(arg)
  local name = arg.name or "arg"

  if name == "..." then
    return "..."
  end

  local suffix = arg.default ~= nil and "?" or ""
  return name .. suffix .. ": " .. type_label(arg.type)
end

local function argument_documentation(arg)
  local docs = {}

  if arg.description and arg.description ~= "" then
    docs[#docs + 1] = arg.description
  end

  if arg.default ~= nil then
    docs[#docs + 1] = "Default: `" .. tostring(arg.default) .. "`."
  end

  return table.concat(docs, "\n\n")
end

local function return_labels(variant)
  local returns = {}

  for _, ret in ipairs(variant.returns or {}) do
    returns[#returns + 1] = type_label(ret.type)
  end

  return returns
end

local function signature_label(target, variant)
  local args = {}

  for _, arg in ipairs(variant.arguments or {}) do
    args[#args + 1] = argument_label(arg)
  end

  local detail = "function " .. target .. "(" .. table.concat(args, ", ") .. ")"
  local returns = return_labels(variant)

  if #returns > 0 then
    detail = detail .. ": " .. table.concat(returns, ", ")
  end

  return detail
end

local function variant_documentation(item, variant)
  local docs = {}

  if item.description and item.description ~= "" then
    docs[#docs + 1] = item.description
  end

  if
    variant.description
    and variant.description ~= ""
    and variant.description ~= item.description
  then
    docs[#docs + 1] = variant.description
  end

  local returns = {}

  for _, ret in ipairs(variant.returns or {}) do
    if ret.description and ret.description ~= "" then
      local label = type_label(ret.type)
      local name = ret.name or ""

      if name ~= "" then
        returns[#returns + 1] = "- `" .. name .. ": " .. label .. "`: " .. ret.description
      else
        returns[#returns + 1] = "- `" .. label .. "`: " .. ret.description
      end
    end
  end

  if #returns > 0 then
    docs[#docs + 1] = "Returns:\n" .. table.concat(returns, "\n")
  end

  return table.concat(docs, "\n\n")
end

local function signature_entry(target, item, variant)
  return {
    label = signature_label(target, variant),
    documentation = variant_documentation(item, variant),
    parameters = variant.arguments or {},
  }
end

local function add_function(entries, target, item)
  local variants = item.variants or {}

  if #variants == 0 then
    variants = { {} }
  end

  local signatures = {}

  for _, variant in ipairs(variants) do
    signatures[#signatures + 1] = signature_entry(target, item, variant)
  end

  entries[#entries + 1] = {
    name = target,
    signatures = signatures,
  }
end

local function collect_api_types()
  local types = {}

  local function add(type_item)
    if type_item and type_item.name then
      types[type_item.name] = type_item
    end
  end

  for _, type_item in ipairs(api.types or {}) do
    add(type_item)
  end

  for _, module in ipairs(api.modules or {}) do
    for _, type_item in ipairs(module.types or {}) do
      add(type_item)
    end
  end

  return types
end

local function collect_type_functions(types, type_name, seen)
  seen = seen or {}

  if seen[type_name] then
    return {}
  end

  seen[type_name] = true

  local type_item = types[type_name]
  local by_name = {}
  local ordered = {}

  local function push(fn)
    if not fn or not fn.name or by_name[fn.name] then
      return
    end

    by_name[fn.name] = true
    ordered[#ordered + 1] = fn
  end

  if not type_item then
    return ordered
  end

  for _, fn in ipairs(type_item.functions or {}) do
    push(fn)
  end

  for _, supertype in ipairs(type_item.supertypes or {}) do
    for _, fn in ipairs(collect_type_functions(types, supertype, seen)) do
      push(fn)
    end
  end

  return ordered
end

local function sorted_type_names(types)
  local names = {}

  for name in pairs(types) do
    names[#names + 1] = { name = name }
  end

  sort_by_name(names)

  return names
end

local entries = {}

for _, fn in ipairs(api.functions or {}) do
  add_function(entries, "love." .. fn.name, fn)
end

for _, fn in ipairs(api.callbacks or {}) do
  add_function(entries, "love." .. fn.name, fn)
end

for _, module in ipairs(sort_by_name(api.modules or {})) do
  for _, fn in ipairs(module.functions or {}) do
    add_function(entries, "love." .. module.name .. "." .. fn.name, fn)
  end
end

local types = collect_api_types()

for _, entry in ipairs(sorted_type_names(types)) do
  local type_name = entry.name

  for _, fn in ipairs(collect_type_functions(types, type_name)) do
    add_function(entries, type_name .. ":" .. fn.name, fn)
  end
end

sort_by_name(entries)

local file = assert(io.open(out, "w"))

file:write([[// Generated from love2d-community/love-api ]] .. tostring(api.version or "unknown") .. [[ function variants.
// Source data: https://github.com/love2d-community/love-api
// Regenerate with: lua scripts/generate-love-api-signatures.lua /path/to/love-api

use crate::lua::{LuaSignature, LuaSignatureParameter};

pub(crate) fn signatures_for_target(target: &str) -> Option<&'static [LuaSignature]> {
    match target {
]])

for _, entry in ipairs(entries) do
  file:write("        ", rust_raw(entry.name), " => Some(&[\n")

  for _, signature in ipairs(entry.signatures) do
    file:write("            LuaSignature {\n")
    file:write("                label: ", rust_raw(signature.label), ",\n")
    file:write("                documentation: ", rust_raw(signature.documentation), ",\n")
    file:write("                parameters: &[\n")

    for _, parameter in ipairs(signature.parameters) do
      file:write("                    LuaSignatureParameter {\n")
      file:write("                        label: ", rust_raw(argument_label(parameter)), ",\n")
      file:write("                        documentation: ", rust_raw(argument_documentation(parameter)), ",\n")
      file:write("                    },\n")
    end

    file:write("                ],\n")
    file:write("            },\n")
  end

  file:write("        ]),\n")
end

file:write([[        _ => None,
    }
}
]])

file:close()
