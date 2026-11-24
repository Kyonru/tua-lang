-- Fault-injected filesystem model. Save still performs real JSON/envelope checks.
local Memory = {}
function Memory.new()
  local files, writes, fault, unreadable = {}, 0, nil, nil
  local backend = {
    mkdir = function() return true end,
    info = function(path) return files[path] and { size = #files[path] } or nil end,
    read = function(path)
      if unreadable == path then
        unreadable = nil
        return nil
      end
      return files[path]
    end,
    write = function(path, bytes)
      writes = writes + 1
      local mode = fault
      fault = nil
      if mode == "before" then return false end
      if mode == "torn" then
        files[path] = bytes:sub(1, math.floor(#bytes / 2))
        return false
      end
      files[path] = bytes
      if mode == "after" then return false end
      if mode == "readback" then unreadable = path end
      return true
    end,
    remove = function(path)
      files[path] = nil
      return true
    end,
    hash = function(bytes)
      if love and love.data then
        return love.data.encode("string", "hex", love.data.hash("sha256", bytes))
      end
      -- Standalone fixture only; native runs use real SHA-256 above.
      local sum = 0
      for index = 1, #bytes do sum = (sum * 31 + bytes:byte(index)) % 2147483647 end
      return string.format("%08x", sum):rep(8)
    end,
  }
  return {
    backend = backend,
    writes = function() return writes end,
    fail = function(mode) fault = mode end,
  }
end
return Memory
