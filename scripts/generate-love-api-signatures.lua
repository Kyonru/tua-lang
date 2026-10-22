local root = assert(arg[1], "usage: lua scripts/generate-love-api-signatures.lua /path/to/love-api [out]")
local out = arg[2] or "crates/tua-core/src/love_api_signatures.rs"

package.path = root .. "/?.lua;" .. root .. "/?/init.lua;" .. package.path

local chunk = assert(loadfile(root .. "/love_api.lua"))
local api = chunk("love_api")
local overrides = dofile("scripts/love-api-overrides.lua")

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

local function argument_label(target, arg)
  local name = arg.name or "arg"

  if name == "..." then
    if arg.type ~= nil then
      return "...: " .. type_label(arg.type)
    end

    return "..."
  end

  local suffix = overrides.argument_is_optional(target, arg) and "?" or ""
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
    args[#args + 1] = argument_label(target, arg)
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

local function collect_enum_values()
  local enums = {}

  for _, module in ipairs(api.modules or {}) do
    for _, enum in ipairs(module.enums or {}) do
      if enum.name then
        local values = {}

        for _, constant in ipairs(enum.constants or {}) do
          if constant.name then
            values[#values + 1] = constant.name
          end
        end

        if #values > 0 then
          enums[enum.name] = values
        end
      end
    end
  end

  return enums
end

local function enum_type_names(type_name, enum_values)
  local names = {}
  local seen = {}

  for part in type_label(type_name):gmatch("[^|]+") do
    part = part:match("^%s*(.-)%s*$")

    if enum_values[part] and not seen[part] then
      seen[part] = true
      names[#names + 1] = part
    end
  end

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

local enum_values = collect_enum_values()
local enum_case_by_key = {}
local enum_cases = {}
local object_method_cases = {}

for _, entry in ipairs(entries) do
  local type_name, method_name = entry.name:match("^([^:]+):(.+)$")

  if type_name and method_name then
    object_method_cases[#object_method_cases + 1] = {
      target = entry.name,
      type_name = type_name,
      method_name = method_name,
    }
  end

  local types_by_index = {}

  for _, signature in ipairs(entry.signatures) do
    for index, parameter in ipairs(signature.parameters) do
      local types = enum_type_names(parameter.type, enum_values)

      if #types > 0 then
        local zero_index = index - 1
        types_by_index[zero_index] = types_by_index[zero_index] or {}

        for _, type_name in ipairs(types) do
          types_by_index[zero_index][type_name] = true
        end
      end
    end
  end

  for index, type_set in pairs(types_by_index) do
    local types = {}

    for type_name in pairs(type_set) do
      types[#types + 1] = { name = type_name }
    end

    sort_by_name(types)

    local key_parts = { entry.name, tostring(index) }

    for _, type_entry in ipairs(types) do
      key_parts[#key_parts + 1] = type_entry.name
    end

    local key = table.concat(key_parts, "\0")

    if not enum_case_by_key[key] then
      enum_case_by_key[key] = true
      enum_cases[#enum_cases + 1] = {
        name = entry.name,
        parameter_index = index,
        types = types,
      }
    end
  end
end

table.sort(enum_cases, function(left, right)
  if left.name ~= right.name then
    return left.name < right.name
  end

  return left.parameter_index < right.parameter_index
end)

local revision_file = assert(io.open("scripts/love-api-revision.txt", "r"))
local revision = assert(revision_file:read("*l"))
revision_file:close()

local file = assert(io.open(out, "w"))

file:write("// Generated from love2d-community/love-api ", tostring(api.version or "unknown"), " function variants.\n")
file:write("// Source revision: ", revision, "\n")
file:write([[// Source data: https://github.com/love2d-community/love-api
// Regenerate with: make generate-love-api LOVE_API_ROOT=/path/to/love-api

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
      file:write(
        "                        label: ",
        rust_raw(argument_label(entry.name, parameter)),
        ",\n"
      )
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

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LoveEnumCompletionCase {
    pub target: &'static str,
    pub parameter_index: usize,
    pub enum_types: &'static [&'static str],
}

#[cfg(test)]
pub(crate) fn enum_completion_cases() -> &'static [LoveEnumCompletionCase] {
    ENUM_COMPLETION_CASES
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LoveObjectMethodCase {
    pub target: &'static str,
    pub type_name: &'static str,
    pub method_name: &'static str,
}

#[cfg(test)]
pub(crate) fn object_method_cases() -> &'static [LoveObjectMethodCase] {
    OBJECT_METHOD_CASES
}

#[cfg(test)]
const ENUM_COMPLETION_CASES: &[LoveEnumCompletionCase] = &[
]])

for _, case in ipairs(enum_cases) do
  file:write("    LoveEnumCompletionCase {\n")
  file:write("        target: ", rust_raw(case.name), ",\n")
  file:write("        parameter_index: ", tostring(case.parameter_index), ",\n")
  file:write("        enum_types: &[\n")

  for _, type_entry in ipairs(case.types) do
    file:write("            ", rust_raw(type_entry.name), ",\n")
  end

  file:write("        ],\n")
  file:write("    },\n")
end

file:write([[];

#[cfg(test)]
const OBJECT_METHOD_CASES: &[LoveObjectMethodCase] = &[
]])

for _, case in ipairs(object_method_cases) do
  file:write("    LoveObjectMethodCase {\n")
  file:write("        target: ", rust_raw(case.target), ",\n")
  file:write("        type_name: ", rust_raw(case.type_name), ",\n")
  file:write("        method_name: ", rust_raw(case.method_name), ",\n")
  file:write("    },\n")
end

file:write([[];
]])

file:close()
