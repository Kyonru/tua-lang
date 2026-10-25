-- Tua's shader validator. Reads requests on stdin and answers on stdout, one
-- line per request, until stdin closes:
--
--   startup:  READY <major>.<minor>.<revision>
--   request:  SHADER <id> <desktop|gles> <byte-count>\n<source bytes>
--   response: RESULT <id> valid
--             RESULT <id> invalid <escaped LOVE message>
--             RESULT <id> rejected <escaped Lua error>
--
-- Messages escape backslashes as \\ and newlines as \n.

local function escape(message)
  return (tostring(message):gsub("\\", "\\\\"):gsub("\r?\n", "\\n"))
end

local function answer(id, status, message)
  if message then
    io.write("RESULT ", id, " ", status, " ", escape(message), "\n")
  else
    io.write("RESULT ", id, " ", status, "\n")
  end
  io.flush()
end

function love.errorhandler(message)
  io.write("FATAL ", escape(message), "\n")
  io.flush()
  os.exit(1)
end

function love.load()
  local major, minor, revision = love.getVersion()
  io.write(("READY %d.%d.%d\n"):format(major, minor, revision))
  io.flush()
  while true do
    local header = io.read("*l")
    if not header then
      break
    end
    local id, target, size = header:match("^SHADER (%d+) (%a+) (%d+)$")
    if id then
      local source = size == "0" and "" or io.read(tonumber(size)) or ""
      local ok, valid, message = pcall(love.graphics.validateShader, target == "gles", source)
      if not ok then
        answer(id, "rejected", valid)
      elseif valid then
        answer(id, "valid")
      else
        answer(id, "invalid", message)
      end
    end
  end
  love.event.quit()
end
