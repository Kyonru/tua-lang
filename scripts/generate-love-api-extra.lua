local root = assert(arg[1], "usage: lua scripts/generate-love-api-extra.lua /path/to/love-api [out]")
local out = arg[2] or "crates/tua-core/src/love_api_extra.rs"

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

local function function_detail(receiver, fn)
  local variant = fn.variants and fn.variants[1] or {}
  local args = {}

  for _, arg in ipairs(variant.arguments or {}) do
    local name = arg.name or "arg"
    local suffix = arg.default ~= nil and "?" or ""

    if name == "..." then
      args[#args + 1] = "..."
    else
      args[#args + 1] = name .. suffix .. ": " .. type_label(arg.type)
    end
  end

  local returns = {}

  for _, ret in ipairs(variant.returns or {}) do
    returns[#returns + 1] = type_label(ret.type)
  end

  local detail = "function " .. receiver .. ":" .. fn.name .. "(" .. table.concat(args, ", ") .. ")"

  if #returns > 0 then
    detail = detail .. ": " .. table.concat(returns, ", ")
  end

  return detail
end

local function documentation(item)
  local docs = {}

  if item.description and item.description ~= "" then
    docs[#docs + 1] = item.description
  end

  local variant = item.variants and item.variants[1]

  if variant and variant.description and variant.description ~= "" and variant.description ~= item.description then
    docs[#docs + 1] = variant.description
  end

  return table.concat(docs, "\n\n")
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

local function collect_enums()
  local enums = {}

  for _, module in ipairs(api.modules or {}) do
    for _, enum in ipairs(module.enums or {}) do
      if enum.name then
        enums[#enums + 1] = enum
      end
    end
  end

  return sort_by_name(enums)
end

local function sorted_type_names(types)
  local names = {}

  for name in pairs(types) do
    names[#names + 1] = { name = name }
  end

  sort_by_name(names)

  return names
end

local types = collect_api_types()
local type_names = sorted_type_names(types)
local enums = collect_enums()

local revision_file = assert(io.open("scripts/love-api-revision.txt", "r"))
local revision = assert(revision_file:read("*l"))
revision_file:close()

local file = assert(io.open(out, "w"))

file:write("// Generated from love2d-community/love-api ", tostring(api.version or "unknown"), " modules/enums and modules/types.\n")
file:write("// Source revision: ", revision, "\n")
file:write([[// Source data: https://github.com/love2d-community/love-api
// Regenerate with: make generate-love-api LOVE_API_ROOT=/path/to/love-api

use crate::lua::{LuaItem, LuaItemKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LoveApiTypedItem {
    type_name: &'static str,
    item: LuaItem,
}

pub(crate) fn type_names() -> &'static [&'static str] {
    TYPE_NAMES
}

pub(crate) fn type_supertypes() -> &'static [(&'static str, &'static str)] {
    TYPE_SUPERTYPES
}

pub(crate) fn type_field_items(type_name: &str, prefix: &str) -> Vec<LuaItem> {
    TYPE_ITEMS
        .iter()
        .filter(|entry| entry.type_name == type_name)
        .map(|entry| entry.item)
        .filter(|item| item.label.starts_with(prefix))
        .collect()
}

pub(crate) fn enum_value_items(type_name: &str, prefix: &str) -> Vec<LuaItem> {
    ENUM_ITEMS
        .iter()
        .filter(|entry| entry.type_name == type_name)
        .map(|entry| entry.item)
        .filter(|item| item.label.starts_with(prefix))
        .collect()
}

pub(crate) fn all_items() -> impl Iterator<Item = LuaItem> {
    TYPE_ITEMS
        .iter()
        .chain(ENUM_ITEMS.iter())
        .map(|entry| entry.item)
}

]])

file:write("const TYPE_NAMES: &[&str] = &[\n")
for _, entry in ipairs(type_names) do
  file:write("    ", rust_raw(entry.name), ",\n")
end
file:write("];\n\n")

file:write("const TYPE_SUPERTYPES: &[(&str, &str)] = &[\n")
for _, entry in ipairs(type_names) do
  local type_item = types[entry.name]

  for _, supertype in ipairs(type_item.supertypes or {}) do
    file:write("    (", rust_raw(entry.name), ", ", rust_raw(supertype), "),\n")
  end
end
file:write("];\n\n")

file:write("const TYPE_ITEMS: &[LoveApiTypedItem] = &[\n")
for _, entry in ipairs(type_names) do
  local type_name = entry.name
  for _, fn in ipairs(collect_type_functions(types, type_name)) do
    file:write("    LoveApiTypedItem {\n")
    file:write("        type_name: ", rust_raw(type_name), ",\n")
    file:write("        item: LuaItem {\n")
    file:write("            label: ", rust_raw(fn.name), ",\n")
    file:write("            kind: LuaItemKind::Function,\n")
    file:write("            detail: ", rust_raw(function_detail(type_name, fn)), ",\n")
    file:write("            documentation: ", rust_raw(documentation(fn)), ",\n")
    file:write("        },\n")
    file:write("    },\n")
  end
end
file:write("];\n\n")

file:write("const ENUM_ITEMS: &[LoveApiTypedItem] = &[\n")
for _, enum in ipairs(enums) do
  for _, constant in ipairs(enum.constants or {}) do
    file:write("    LoveApiTypedItem {\n")
    file:write("        type_name: ", rust_raw(enum.name), ",\n")
    file:write("        item: LuaItem {\n")
    file:write("            label: ", rust_raw(constant.name), ",\n")
    file:write("            kind: LuaItemKind::Value,\n")
    file:write("            detail: ", rust_raw(enum.name), ",\n")
    file:write("            documentation: ", rust_raw(constant.description or enum.description or ""), ",\n")
    file:write("        },\n")
    file:write("    },\n")
  end
end
file:write("];\n")
file:close()
