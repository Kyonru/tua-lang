-- love.graphics.validateShader needs a graphics context, so the probe opens a
-- 1x1 borderless window off screen instead of running headless.
function love.conf(t)
  t.identity = "tua-shader-facts"
  t.window.title = "tua shader facts"
  t.window.width = 1
  t.window.height = 1
  t.window.borderless = true
  t.window.x = -4000
  t.window.y = -4000
  t.modules.audio = false
  t.modules.sound = false
  t.modules.joystick = false
  t.modules.physics = false
end
