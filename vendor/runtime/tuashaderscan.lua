-- Small LOVE shader utilities used by Tua's ShaderScan built-in module.
-- The API shape is inspired by idbrii/love-shaderscan (MIT); this runtime is
-- an independent implementation. See docs/shaders.md for the attribution.
-- Copyright (c) 2026 Kyonru. MIT License.

local M = {}
local warned = setmetatable({}, { __mode = "k" })
---@class ShaderScanPreloadEntry
---@field name string
---@field path string
---@class ShaderScanPreloadError
---@field name string
---@field path string
---@field message string
---@class ShaderScanPreloadJob
---@field total number
---@field completed number
---@field successful number
---@field failed number
---@field current_name? string
---@field current_path? string
---@field errors ShaderScanPreloadError[]
---@field done boolean
---@field cancelled boolean
---@field cancel fun(self: ShaderScanPreloadJob)
---@class ShaderScanInstance
---@field s table<string, Shader>
---@field load_shader fun(self: ShaderScanInstance, name: string, path: string): Shader
---@field update fun(self: ShaderScanInstance, dt: number)
---@field safe_send fun(self: ShaderScanInstance, name: string, uniform: string, ...: any): boolean
---@field on_compiled? fun(name: string, path: string, shader: Shader, previous: Shader|nil)
---@field preload fun(self: ShaderScanInstance, entries: ShaderScanPreloadEntry[], on_progress?: fun(job: ShaderScanPreloadJob), on_complete?: fun(job: ShaderScanPreloadJob)): ShaderScanPreloadJob
local Scanner = {}
Scanner.__index = Scanner
local Job = {}
Job.__index = Job

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
    local shorter = path:match("^[^/]+/(.+)$")
    local hint = ""
    if shorter and love.filesystem.getInfo and love.filesystem.getInfo(shorter) then
      hint = ("; %q exists in the built game, so use that path instead"):format(shorter)
    end
    return nil, ("cannot read shader include %q: %s%s"):format(path, tostring(message), hint)
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
      return nil, "shader include path escapes the game root"
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
          return nil, ("%s:%d: include path escapes the game root"):format(current_path, line_number)
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
  if not path then return nil, "shader path escapes the game root" end
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

local function remember_failure(self, name, failed, message)
  if not failed then return end
  local previous = self.loaded[name]
  if previous then
    previous.failed = failed
    previous.retry_path = failed.path
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

local function activate(self, name, loaded)
  local previous = self.loaded[name]
  local old_shader = previous and previous.shader or nil
  if self.on_compiled then
    local ok, message = pcall(self.on_compiled, name, loaded.path, loaded.shader, old_shader)
    if not ok then
      if loaded.shader.release then loaded.shader:release() end
      return nil, ("shader %q setup failed: %s"):format(name, tostring(message)), {
        path = loaded.path,
        dependencies = loaded.dependencies,
        source_by_path = loaded.source_by_path,
      }
    end
  end
  loaded.name = name
  self.loaded[name] = loaded
  self.s[name] = loaded.shader
  if old_shader and old_shader.release then old_shader:release() end
  return loaded.shader
end

local function notify(callback, job, kind)
  if not callback then return end
  local ok, message = pcall(callback, job)
  if not ok then
    print(("[ShaderScan] preload %s callback failed: %s"):format(kind, tostring(message)))
  end
end

local function finish_preload(self, job, cancelled)
  if job.done then return end
  job.done = true
  job.cancelled = cancelled
  if self.preload_job == job then self.preload_job = nil end
  notify(job.on_complete, job, "completion")
end

---@return nil
function Job:cancel()
  if self.done then return end
  if self.processing then
    self.cancel_requested = true
    return
  end
  finish_preload(self.scanner, self, self.completed < self.total)
end

---@param entries ShaderScanPreloadEntry[]
---@param on_progress? fun(job: ShaderScanPreloadJob)
---@param on_complete? fun(job: ShaderScanPreloadJob)
---@return ShaderScanPreloadJob
function Scanner:preload(entries, on_progress, on_complete)
  if self.preload_job then error("a ShaderScan preload is already running", 2) end
  if type(entries) ~= "table" then error("ShaderScan preload entries must be a list", 2) end
  local queue = {}
  local names = {}
  for index, entry in ipairs(entries) do
    if type(entry) ~= "table" or type(entry.name) ~= "string" or entry.name == ""
      or type(entry.path) ~= "string" or entry.path == "" then
      error(("ShaderScan preload entry %d needs a name and path"):format(index), 2)
    end
    if names[entry.name] then
      error(("ShaderScan preload name %q occurs twice"):format(entry.name), 2)
    end
    names[entry.name] = true
    queue[#queue + 1] = { name = entry.name, path = entry.path }
  end
  local job = setmetatable({
    scanner = self, entries = queue, total = #queue,
    completed = 0, successful = 0, failed = 0,
    errors = {}, done = false, cancelled = false,
    current_name = queue[1] and queue[1].name or nil,
    current_path = queue[1] and queue[1].path or nil,
    on_progress = on_progress, on_complete = on_complete,
  }, Job)
  self.preload_job = job
  if job.total == 0 then finish_preload(self, job, false) end
  return job
end

local function process_preload(self)
  local job = self.preload_job
  if not job then return end
  local entry = job.entries[job.completed + 1]
  job.current_name = entry.name
  job.current_path = entry.path
  job.processing = true
  local ok, result = pcall(self.load_shader, self, entry.name, entry.path)
  job.processing = false
  job.completed = job.completed + 1
  if ok then
    job.successful = job.successful + 1
  else
    job.failed = job.failed + 1
    job.errors[#job.errors + 1] = { name = entry.name, path = entry.path, message = tostring(result) }
  end
  notify(job.on_progress, job, "progress")
  if not job.done and (job.cancel_requested or job.completed == job.total) then
    finish_preload(self, job, job.completed < job.total)
  end
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
  if loaded then
    local shader
    shader, message, failed = activate(self, name, loaded)
    if shader then return shader end
  end
  remember_failure(self, name, failed, message)
  error(message, 2)
end

---@param dt number
---@return nil
function Scanner:update(dt)
  process_preload(self)
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
      local replacement, message, failed = load(name, previous.retry_path or previous.path)
      if replacement then
        local shader
        shader, message, failed = activate(self, name, replacement)
        if shader then
          print(("[ShaderScan] reloaded %s (%s)"):format(name, previous.path))
        end
      end
      if message then
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
