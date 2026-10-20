-- Headless: the benchmark only needs timing, the event loop, and the
-- filesystem, so no window is opened.
function love.conf(t)
  t.identity = "tua-ecs-benchmark"
  t.window = false
  t.modules.window = false
  t.modules.graphics = false
  t.modules.image = false
  t.modules.font = false
  t.modules.video = false
  t.modules.audio = false
  t.modules.sound = false
  t.modules.joystick = false
  t.modules.physics = false
  t.modules.touch = false
  t.modules.mouse = false
  t.modules.keyboard = false
end
