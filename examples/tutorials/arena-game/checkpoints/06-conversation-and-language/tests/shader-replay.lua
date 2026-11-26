local host
local function setup(context)
  host = require("__tua_replay_game")
  local HitShader = require("hit-shader")
  local controller = HitShader.new(true)
  assert(controller.state.loading and controller.state.completed == 0)
  controller.update(0)
  assert(
    controller.state.ready and not controller.state.loading and controller.state.completed == 1
  )
  assert(controller.state.failed == 0 and controller.scanner.s.hit:hasUniform("flash"))
  local canvas = love.graphics.newCanvas(16, 8)
  love.graphics.push("all")
  love.graphics.setCanvas(canvas)
  love.graphics.clear(0, 0, 0, 0)
  love.graphics.setColor(0.2, 0.3, 0.4, 1)
  controller.draw(1, function()
    love.graphics.rectangle("fill", 0, 0, 8, 8)
  end)
  assert(love.graphics.getShader() == nil)
  love.graphics.rectangle("fill", 8, 0, 8, 8)
  love.graphics.pop()
  local pixels = canvas:newImageData()
  local r, g, b = pixels:getPixel(2, 2)
  assert(r > 0.95 and g > 0.8 and b > 0.55, "flash must affect the selected draw")
  local nr, ng, nb = pixels:getPixel(12, 2)
  assert(
    math.abs(nr - 0.2) < 0.02 and math.abs(ng - 0.3) < 0.02 and math.abs(nb - 0.4) < 0.02,
    "shader must not leak into later draws"
  )
  pixels:release()
  canvas:release()
  local external = love.graphics.newShader(
    "vec4 effect(vec4 c, Image t, vec2 uv, vec2 p) { return c * Texel(t, uv); }"
  )
  love.graphics.setShader(external)
  local ok = pcall(function()
    controller.draw(1, function()
      error("render fixture")
    end)
  end)
  assert(not ok and love.graphics.getShader() == external)
  love.graphics.setShader()
  external:release()
  local previous = controller.scanner.s.hit
  local originalCompile = love.graphics.newShader
  love.graphics.newShader = function()
    return originalCompile("invalid shader fixture")
  end
  controller.retry()
  controller.update(0)
  assert(
    controller.state.failed == 1 and controller.scanner.s.hit == previous,
    "failed replacement must retain last good shader"
  )
  local failed = HitShader.new(true)
  failed.update(0)
  assert(not failed.state.ready and failed.state.failed == 1)
  local drawn = false
  failed.draw(1, function()
    drawn = true
  end)
  assert(drawn and love.graphics.getShader() == nil, "initial failure still draws normally")
  love.graphics.newShader = originalCompile
  failed.retry()
  failed.update(0)
  assert(failed.state.ready and failed.state.error == "" and failed.state.failed == 0)
  failed.dispose()
  local initialize = controller.scanner.on_compiled
  controller.scanner.on_compiled = function()
    error("setup fixture")
  end
  controller.retry()
  controller.update(0)
  assert(controller.state.failed == 1 and controller.scanner.s.hit == previous)
  controller.scanner.on_compiled = initialize
  controller.retry()
  controller.update(0)
  assert(controller.scanner.s.hit ~= previous and controller.state.failed == 0)
  local originalRead = love.filesystem.read
  love.filesystem.read = function(path, ...)
    if path == "shaders/lib/flash.glsl" then
      return nil, "missing include fixture"
    end
    return originalRead(path, ...)
  end
  local missing = HitShader.new(true)
  missing.update(0)
  assert(not missing.state.ready and missing.state.error:find("flash.glsl", 1, true))
  love.filesystem.read = originalRead
  missing.retry()
  missing.update(0)
  assert(missing.state.ready)
  missing.dispose()
  local immediate = HitShader.new(false)
  assert(immediate.state.ready and not immediate.state.loading)
  immediate.dispose()
  local cancelled = HitShader.new(true)
  cancelled.dispose()
  cancelled.update(1)
  assert(not cancelled.state.ready and not cancelled.state.loading)
  controller.dispose()
  controller.dispose()
  assert(controller.scanner.s.hit == nil)
  context.key("return", true)
  host.update(0)
  context.key("return", false)
  host.update(0)
  print(
    "ARENA_SHADER_NATIVE_OK preload, pixels, scope/error restoration, compile/setup/include failure, retry, disposal"
  )
end
local function beforeFrame(context)
  local position
  host.combat.enemies:each(function(_, p)
    if not position then
      position = p
    end
  end)
  if position then
    local x, y = host.camera:toScreen(position.x, position.y)
    context.pointer(x, y, true)
  end
end
local function verify(context)
  assert(host.hitShader.state.ready and host.combat.state.hits > 0)
  assert(love.graphics.getShader() == nil)
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }
