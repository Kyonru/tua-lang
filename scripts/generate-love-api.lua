local root = assert(arg[1], "usage: lua scripts/generate-love-api.lua /path/to/love-api [out]")
local out = arg[2] or "crates/tua-core/src/love_api.rs"

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

local function rust_string(value)
  value = tostring(value or "")
  value = value:gsub("\\", "\\\\"):gsub('"', '\\"')
  return '"' .. value .. '"'
end

local function sort_items(items)
  table.sort(items, function(left, right)
    local left_path = table.concat(left.path, "\0")
    local right_path = table.concat(right.path, "\0")

    if left_path ~= right_path then
      return left_path < right_path
    end

    return left.label < right.label
  end)
  return items
end

local function type_label(label)
  label = tostring(label or "any")
  label = label:gsub("%s+or%s+", " | ")
  return label
end

local function function_detail(target, fn)
  local variant = fn.variants and fn.variants[1] or {}
  local arguments = {}

  for _, argument in ipairs(variant.arguments or {}) do
    local name = argument.name or "arg"
    local suffix = argument.default ~= nil and "?" or ""

    if name == "..." then
      if argument.type ~= nil then
        arguments[#arguments + 1] = "...: " .. type_label(argument.type)
      else
        arguments[#arguments + 1] = "..."
      end
    else
      arguments[#arguments + 1] = name .. suffix .. ": " .. type_label(argument.type)
    end
  end

  local returns = {}

  for _, value in ipairs(variant.returns or {}) do
    returns[#returns + 1] = type_label(value.type)
  end

  local detail = "function " .. target .. "(" .. table.concat(arguments, ", ") .. ")"

  if #returns > 0 then
    detail = detail .. ": " .. table.concat(returns, ", ")
  end

  return detail
end

local function documentation(item)
  return item.description or ""
end

local function add_function(items, path, target, fn)
  items[#items + 1] = {
    path = path,
    label = fn.name,
    kind = "Function",
    detail = function_detail(target, fn),
    documentation = documentation(fn),
  }
end

local items = {}
local callback_labels = {}

for _, fn in ipairs(api.functions or {}) do
  add_function(items, { "love" }, "love." .. fn.name, fn)
end

for _, fn in ipairs(api.callbacks or {}) do
  add_function(items, { "love" }, "love." .. fn.name, fn)
  callback_labels[#callback_labels + 1] = fn.name
end

for _, module in ipairs(api.modules or {}) do
  items[#items + 1] = {
    path = { "love" },
    label = module.name,
    kind = "Module",
    detail = "LÖVE " .. module.name .. " module",
    documentation = module.description or "",
  }

  for _, fn in ipairs(module.functions or {}) do
    add_function(items, { "love", module.name }, "love." .. module.name .. "." .. fn.name, fn)
  end
end

sort_items(items)
table.sort(callback_labels)

local revision_file = assert(io.open("scripts/love-api-revision.txt", "r"))
local revision = assert(revision_file:read("*l"))
revision_file:close()

local file = assert(io.open(out, "w"))

file:write("// Generated from love2d-community/love-api ", tostring(api.version or "unknown"), ".\n")
file:write("// Source revision: ", revision, "\n")
file:write([[// Source data: https://github.com/love2d-community/love-api
// Regenerate with: make generate-love-api LOVE_API_ROOT=/path/to/love-api

use crate::lua::{LuaItem, LuaItemKind};

pub(crate) fn builtin_definition_document(id: &str) -> Option<String> {
    ITEMS
        .iter()
        .map(lua_item)
        .find(|item| crate::lua::builtin_definition_id(*item) == id)
        .map(crate::lua::builtin_definition_source_for_item)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LoveApiItem {
    path: &'static [&'static str],
    label: &'static str,
    kind: LuaItemKind,
    detail: &'static str,
    documentation: &'static str,
}

pub(crate) fn field_items(receiver_parts: &[String], prefix: &str) -> Vec<LuaItem> {
    ITEMS
        .iter()
        .filter(|item| path_matches(item.path, receiver_parts))
        .filter(|item| item.label.starts_with(prefix))
        .map(lua_item)
        .collect()
}

pub(crate) fn hover_item(parts: &[String]) -> Option<LuaItem> {
    let (label, receiver) = parts.split_last()?;

    ITEMS
        .iter()
        .find(|item| path_matches(item.path, receiver) && item.label == label)
        .map(lua_item)
}

pub(crate) fn callback_item(label: &str) -> Option<LuaItem> {
    if CALLBACK_LABELS.binary_search(&label).is_err() {
        return None;
    }

    ITEMS
        .iter()
        .find(|item| item.path == ["love"] && item.label == label)
        .map(lua_item)
}

fn path_matches(path: &[&str], receiver_parts: &[String]) -> bool {
    path.len() == receiver_parts.len()
        && path
            .iter()
            .zip(receiver_parts)
            .all(|(left, right)| *left == right)
}

fn lua_item(item: &LoveApiItem) -> LuaItem {
    LuaItem {
        label: item.label,
        kind: item.kind,
        detail: item.detail,
        documentation: item.documentation,
    }
}

const ITEMS: &[LoveApiItem] = &[
]])

for _, item in ipairs(items) do
  file:write("    LoveApiItem {\n")
  file:write("        path: &[")

  for index, path in ipairs(item.path) do
    if index > 1 then
      file:write(", ")
    end

    file:write(rust_string(path))
  end

  file:write("],\n")
  file:write("        label: ", rust_string(item.label), ",\n")
  file:write("        kind: LuaItemKind::", item.kind, ",\n")
  file:write("        detail: ", rust_raw(item.detail), ",\n")
  file:write("        documentation: ", rust_raw(item.documentation), ",\n")
  file:write("    },\n")
end

file:write("];\n\n")
file:write("const CALLBACK_LABELS: &[&str] = &[\n")
for _, label in ipairs(callback_labels) do
  file:write("    ", rust_string(label), ",\n")
end
file:write("];\n")
file:close()
