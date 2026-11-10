-- Small LOVE shader utilities used by Tua's ShaderScan built-in module.
-- The API shape is inspired by idbrii/love-shaderscan (MIT); this runtime is
-- an independent implementation. See docs/shaders.md for the attribution.
-- Copyright (c) 2026 Kyonru. MIT License.

local M = {}
local warned = setmetatable({}, { __mode = "k" })
---@class ShaderScanInstance
---@field s table<string, Shader>
local Scanner = {}
Scanner.__index = Scanner

local function normalize(path)
  path = path:gsub("\\", "/")
  if path:sub(1, 1) == "/" or path:match("^%a:") then
    return nil
  end
  local parts = {}
  for part in path:gmatch("[^/]+") do
    if part == ".." then
      if #parts == 0 then return nil end
      parts[#parts] = nil
    elseif part ~= "." and part ~= "" then
      parts[#parts + 1] = part
    end
  end
  return table.concat(parts, "/")
end

local function read(path)
  local source, message = love.filesystem.read(path)
  if not source then
    return nil, ("cannot read shader include %q: %s"):format(path, tostring(message))
  end
  return source
end

local function include_path(line)
  return line:match('^%s*#include%s+"([^"]+)"%s*$')
end

local function expand(path, root_source)
  local source_by_path = {}
  local expanded = {}
  local stack = {}
  local lines = {}
  local source_map = {}
  local dependencies = {}

  local function visit(current_path, supplied_source)
    current_path = normalize(current_path)
    if not current_path then
      return nil, "shader include path escapes the project root"
    end
    -- Include-once semantics make cycles finite and avoid duplicate helper
    -- declarations when a shared include is reached by multiple branches.
    if expanded[current_path] then return true end
    expanded[current_path] = true
    source_map[current_path] = source_map[current_path] or {}
    dependencies[current_path] = true
    local source = supplied_source
    if source == nil then
      local message
      source, message = read(current_path)
      if not source then
        source_by_path[current_path] = false
        return nil, message
      end
    end
    source_by_path[current_path] = source
    stack[#stack + 1] = current_path
    local line_number = 0
    for line in (source .. "\n"):gmatch("(.-)\n") do
      line_number = line_number + 1
      local requested = include_path(line)
      if requested then
        local normalized = normalize(requested)
        if not normalized then
          stack[#stack] = nil
          return nil, ("%s:%d: include path escapes the project root"):format(current_path, line_number)
        end
        local ok, message = visit(normalized)
        if not ok then
          stack[#stack] = nil
          return nil, ("%s:%d: %s"):format(current_path, line_number, message)
        end
      else
        lines[#lines + 1] = line
        source_map[#lines] = { path = current_path, line = line_number, text = line }
      end
    end
    stack[#stack] = nil
    return true
  end

  local ok, message = visit(path, root_source)
  if not ok then return nil, source_map, dependencies, source_by_path, message end
  return table.concat(lines, "\n") .. "\n", source_map, dependencies, source_by_path
end

local function diagnostic_location(message, source_map, fallback)
  local number = tonumber(message:match("Line%s+(%d+):") or message:match("ERROR:%s*%d+:(%d+):"))
  local origin = number and source_map[number]
  if not origin then return fallback .. ": " .. tostring(message) end
  return ("%s:%d: %s"):format(origin.path, origin.line, tostring(message))
end

local function load(name, path)
  path = normalize(path)
  if not path then return nil, "shader path escapes the project root" end
  local source, message = read(path)
  if not source then
    return nil, message, {
      path = path,
      dependencies = { [path] = true },
      source_by_path = { [path] = false },
    }
  end
  local code, source_map, dependencies, source_by_path, expand_error = expand(path, source)
  if not code then
    return nil, expand_error, {
      path = path,
      dependencies = dependencies,
      source_by_path = source_by_path,
    }
  end
  local ok, shader = pcall(love.graphics.newShader, code)
  if not ok then
    local location = diagnostic_location(shader, source_map, path)
    return nil, ("shader %q failed to compile: %s"):format(name, location), {
      path = path,
      dependencies = dependencies,
      source_by_path = source_by_path,
    }
  end
  return {
    shader = shader,
    path = path,
    code = code,
    source_map = source_map,
    dependencies = dependencies,
    source_by_path = source_by_path,
  }
end

---@return ShaderScanInstance
function M.new()
  return setmetatable({ s = {}, loaded = {}, elapsed = 0 }, Scanner)
end

---@param name string
---@param path string
---@return Shader
function Scanner:load_shader(name, path)
  local loaded, message, failed = load(name, path)
  if not loaded then
    if failed then
      local previous = self.loaded[name]
      if previous then
        previous.failed = failed
        previous.last_error = message
      else
        self.loaded[name] = {
          path = failed.path,
          dependencies = failed.dependencies,
          source_by_path = failed.source_by_path,
          failed = failed,
          last_error = message,
        }
      end
    end
    error(message, 2)
  end
  loaded.name = name
  loaded.failed = nil
  self.s[name] = loaded.shader
  self.loaded[name] = loaded
  return loaded.shader
end

---@param dt number
---@return nil
function Scanner:update(dt)
  self.elapsed = self.elapsed + math.max(dt or 0, 0)
  if self.elapsed < 0.25 then return end
  self.elapsed = 0
  for name, previous in pairs(self.loaded) do
    local observed = previous.failed or previous
    local changed = false
    for path in pairs(observed.dependencies) do
      local source = read(path)
      if (source or false) ~= observed.source_by_path[path] then
        changed = true
        break
      end
    end
    if changed then
      local replacement, message, failed = load(name, previous.path)
      if replacement then
        replacement.name = name
        self.loaded[name] = replacement
        self.s[name] = replacement.shader
        if previous.shader and previous.shader.release then previous.shader:release() end
        print(("[ShaderScan] reloaded %s (%s)"):format(name, previous.path))
      else
        previous.failed = failed or observed
        if previous.last_error ~= message then
          print(("[ShaderScan] %s"):format(message))
          previous.last_error = message
        end
      end
    end
  end
end

---@param name string
---@param uniform string
---@vararg any
---@return boolean
function Scanner:safe_send(name, uniform, ...)
  return M.safe_send(self.s[name], uniform, ...)
end

---@param shader Shader
---@param name string
---@vararg any
---@return boolean
function M.safe_send(shader, name, ...)
  if shader:hasUniform(name) then
    shader:send(name, ...)
    return true
  end

  local names = warned[shader]
  if not names then
    names = {}
    warned[shader] = names
  end
  if not names[name] then
    names[name] = true
    print(("[ShaderScan] shader has no active uniform %q; skipping send"):format(name))
  end
  return false
end

return M
