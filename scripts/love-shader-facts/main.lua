-- Flush each result so a driver stall still leaves usable probe evidence.
io.stdout:setvbuf("no")
-- Records how LOVE compiles and runs shaders, as evidence for
-- internal/shader-editor-support.md.
--
-- Usage: love scripts/love-shader-facts [fixtures-dir]
--
-- Prints one line per fixture and target from love.graphics.validateShader,
-- then one line per Shader:send case. Rerun it on each platform and LOVE
-- version that Tua's shader checks claim to match.

local function read(path)
  local file = assert(io.open(path, "rb"))
  local text = file:read("*a")
  file:close()
  return text
end

local function list(dir)
  local names = {}
  local listing = io.popen('ls "' .. dir .. '"')
  for name in listing:lines() do
    if name:match("%.glsl$") then
      names[#names + 1] = name
    end
  end
  listing:close()
  table.sort(names)
  return names
end

local function first_line(message)
  local reported = tostring(message):match("ERROR: %d+:(%d+): ([^\n]+)")
  if reported then
    return reported
  end
  return (tostring(message):gsub("%s+", " "))
end

local function validate_fixtures(dir)
  local major, minor, revision = love.getVersion()
  print(("LOVE %d.%d.%d validateShader on %s"):format(major, minor, revision, love.system.getOS()))
  for _, name in ipairs(list(dir)) do
    local code = read(dir .. "/" .. name)
    for _, gles in ipairs({ false, true }) do
      local target = gles and "gles" or "desktop"
      local started = love.timer.getTime()
      local ok, valid, message = pcall(love.graphics.validateShader, gles, code)
      local elapsed = (love.timer.getTime() - started) * 1000
      local result
      if not ok then
        result = "rejected: " .. first_line(valid)
      elseif valid then
        result = "ok"
      else
        local line, text = tostring(message):match("ERROR: %d+:(%d+): ([^\n]+)")
        result = line and ("error line " .. line .. ": " .. text) or ("error: " .. first_line(message))
      end
      print(("%-24s %-7s %5.1fms  %s"):format(name, target, elapsed, result))
    end
  end
end

local function send_cases()
  print("Shader:send")
  local shader = love.graphics.newShader([[
extern number n;
extern vec4 v;
extern vec3 c;
extern number arr[2];
extern Image img;
extern mat2 m2;
extern mat4 m4;
extern bool flag;
extern bvec3 switches;
extern ivec2 coords;
extern number unread;
vec4 effect(vec4 color, Image tex, vec2 uv, vec2 screen) {
  return color * n * v * vec4(c, 1.0) * arr[0] * arr[1] * Texel(img, uv)
    * m2[0][0] * m4[0][0] * float(flag) * float(switches.x) * float(coords.x);
}
]])
  local canvas = love.graphics.newCanvas(2, 2)
  local cases = {
    { "unknown uniform", function() shader:send("missing", 1) end },
    { "unread uniform", function() shader:send("unread", 1) end },
    { "number <- number", function() shader:send("n", 1) end },
    { "number <- table", function() shader:send("n", { 1 }) end },
    { "number <- two numbers", function() shader:send("n", 1, 2) end },
    { "vec4 <- 4 numbers", function() shader:send("v", { 1, 2, 3, 4 }) end },
    { "vec4 <- 3 numbers", function() shader:send("v", { 1, 2, 3 }) end },
    { "vec4 <- 5 numbers", function() shader:send("v", { 1, 2, 3, 4, 5 }) end },
    { "vec4 <- number", function() shader:send("v", 1) end },
    { "sendColor vec3 <- 3", function() shader:sendColor("c", { 1, 1, 1 }) end },
    { "sendColor vec3 <- 4", function() shader:sendColor("c", { 1, 1, 1, 1 }) end },
    { "sendColor number", function() shader:sendColor("n", { 1, 1, 1 }) end },
    { "array[2] <- 2 values", function() shader:send("arr", 1, 2) end },
    { "array[2] <- 3 values", function() shader:send("arr", 1, 2, 3) end },
    { "Image <- Canvas", function() shader:send("img", canvas) end },
    { "Image <- number", function() shader:send("img", 1) end },
    { "mat2 <- rows", function() shader:send("m2", { { 1, 0 }, { 0, 1 } }) end },
    { "mat2 <- flat", function() shader:send("m2", { 1, 0, 0, 1 }) end },
    { "mat2 <- Transform", function() shader:send("m2", love.math.newTransform()) end },
    { "mat4 <- Transform", function() shader:send("m4", love.math.newTransform()) end },
    { "bool <- boolean", function() shader:send("flag", true) end },
    { "vec4 <- wrong component", function() shader:send("v", { 1, true, 1, 1 }) end },
    { "mat2 <- too short", function() shader:send("m2", { 1, 0 }) end },
    { "mat2 <- short row", function() shader:send("m2", { { 1 }, { 0, 1 } }) end },
    { "mat2 <- column layout", function() shader:send("m2", "column", { 1, 0, 0, 1 }) end },
    { "number <- numeric string", function() shader:send("n", "0x10") end },
    { "number <- raw Data", function() shader:send("n", love.data.newByteData(4)) end },
    { "bvec3 <- booleans", function() shader:send("switches", { true, false, true }) end },
    { "bvec3 <- numbers", function() shader:send("switches", { 1, 0, 1 }) end },
    { "ivec2 <- numbers", function() shader:send("coords", { 1, 2 }) end },
    { "ivec2 <- short", function() shader:send("coords", { 1 }) end },
    { "bool <- number", function() shader:send("flag", 1) end },
  }
  for _, case in ipairs(cases) do
    local ok, err = pcall(case[2])
    print(("%-24s %s"):format(case[1], ok and "ok" or ("error: " .. first_line(err):gsub("^main%.lua:%d+: ", ""))))
  end
end

-- Report failures on stdout and exit instead of showing LOVE's error screen.
function love.errorhandler(message)
  print("probe failed: " .. tostring(message))
  print(debug.traceback("", 2))
  os.exit(1)
end

function love.load(args)
  local dir = args[1] or "crates/tua-core/src/asset/fixtures/shaders"
  validate_fixtures(dir)
  send_cases()
  love.event.quit()
end
