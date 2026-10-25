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
--
-- Editors keep this process alive between validations. Blocking the main
-- thread on stdin would stop window event processing, and macOS would report
-- the app as not responding, so a thread reads stdin and the main loop keeps
-- pumping events while it waits for requests.

local READER = [[
local requests = love.thread.getChannel("tua-shader-requests")
while true do
  local header = io.read("*l")
  if not header then
    break
  end
  local id, target, size = header:match("^SHADER (%d+) (%a+) (%d+)$")
  if id then
    local source = size == "0" and "" or io.read(tonumber(size)) or ""
    requests:push({ id = id, target = target, source = source })
  end
end
requests:push({ eof = true })
]]

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

local function fatal(message)
  io.write("FATAL ", escape(message), "\n")
  io.flush()
  os.exit(1)
end

function love.errorhandler(message)
  fatal(message)
end

local function validate(request)
  local ok, valid, message =
    pcall(love.graphics.validateShader, request.target == "gles", request.source)
  if not ok then
    answer(request.id, "rejected", valid)
  elseif valid then
    answer(request.id, "valid")
  else
    answer(request.id, "invalid", message)
  end
end

function love.run()
  local major, minor, revision = love.getVersion()
  local requests = love.thread.getChannel("tua-shader-requests")
  love.thread.newThread(READER):start()
  io.write(("READY %d.%d.%d\n"):format(major, minor, revision))
  io.flush()

  return function()
    love.event.pump()
    for name, a, b in love.event.poll() do
      if name == "quit" then
        return 0
      elseif name == "threaderror" then
        fatal(b)
      end
    end
    -- Waiting on the channel with a short timeout keeps the window responsive.
    local request = requests:demand(0.05)
    while request do
      if request.eof then
        return 0
      end
      validate(request)
      request = requests:pop()
    end
  end
end
