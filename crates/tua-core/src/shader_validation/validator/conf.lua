-- love.graphics.validateShader needs a graphics context, so the validator
-- opens a 1x1 borderless window off screen. LOVE has no hidden-window mode.
function love.conf(t)
  t.identity = "tua-shader-validator"
  t.window.title = "Tua shader validator"
  t.window.width = 1
  t.window.height = 1
  t.window.borderless = true
  t.window.x = -4000
  t.window.y = -4000
  t.modules.audio = false
  t.modules.sound = false
  t.modules.joystick = false
  t.modules.physics = false
  t.modules.video = false
end
