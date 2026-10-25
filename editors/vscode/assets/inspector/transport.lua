-- Bounded non-blocking loopback transport. No disk writes or remote hosts.
local socket = require("socket")
return function(port, token, snapshot)
  local client, connected, pending, offset, incoming
  local paused, requested, stopped = false, true, false
  local nextConnect, nextSnapshot = 0, 0
  local function disconnect()
    if client then client:close() end
    client, connected, pending, incoming = nil, false, nil, ""
    nextConnect = socket.gettime() + 1
  end
  local function tick()
    if stopped then return end
    local now = socket.gettime()
    if not client then
      if now < nextConnect then return end
      client = assert(socket.tcp())
      client:settimeout(0)
      client:connect("127.0.0.1", port)
      incoming = ""
      nextConnect = now + 2
    end
    if not connected then
      if not client:getpeername() then
        if now > nextConnect then disconnect() end
        return
      end
      connected = true
      pending = '{"type":"hello","version":1,"token":"' .. token .. '"}\n'
      offset = 1
      requested = true
    end
    local data, err, partial = client:receive(1024)
    incoming = incoming .. (data or partial or "")
    if err == "closed" then stopped = true; disconnect(); return end
    if #incoming > 4096 then disconnect(); return end
    local consumed = 0
    while consumed < 8 do
      local line, rest = incoming:match("^([^\n]*)\n(.*)$")
      if not line then break end
      incoming = rest
      if line == "stop" then stopped = true; disconnect(); return
      elseif line == "pause" then paused = true
      elseif line == "resume" then paused = false; requested = true
      elseif line == "snapshot" then requested = true end
      consumed = consumed + 1
    end
    if not pending and (requested or (not paused and now >= nextSnapshot)) then
      pending = snapshot() .. "\n"
      offset = 1
      requested = false
      nextSnapshot = now + 0.5
    end
    if pending then
      -- At most 16 KiB per frame. Slow readers cannot grow an output queue.
      local sent, sendError, last = client:send(pending, offset, math.min(#pending, offset + 16383))
      offset = (sent or last or (offset - 1)) + 1
      if offset > #pending then pending = nil end
      if sendError and sendError ~= "timeout" then disconnect() end
    end
  end
  return function()
    local ok = pcall(tick)
    if not ok then disconnect() end
  end
end
