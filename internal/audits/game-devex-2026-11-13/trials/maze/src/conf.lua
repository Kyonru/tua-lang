function love.conf(t)
  t.window.title = "Tua stealth maze DevEx trial"
end

-- An audit-only breadcrumb. Normal runs keep LOVE's own error screen.
if os.getenv("TUA_MAZE_SMOKE") == "1" then
  print("TUA_MAZE_CONF_LOADED")
  function love.errorhandler(message)
    local output = io.open("/private/tmp/tua-maze-love-error.txt", "w")
    if output then
      output:write(tostring(message), "\n", debug.traceback(), "\n")
      output:close()
    end
    io.stderr:write("TUA_MAZE_ERROR ", tostring(message), "\n")
    os.exit(1)
  end
end
