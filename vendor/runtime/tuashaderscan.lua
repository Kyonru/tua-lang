-- Small LOVE shader utilities used by Tua's ShaderScan built-in module.
-- The API shape is inspired by idbrii/love-shaderscan (MIT); this runtime is
-- an independent implementation. See docs/shaders.md for the attribution.
-- Copyright (c) 2026 Kyonru. MIT License.

local M = {}
local warned = setmetatable({}, { __mode = "k" })

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
