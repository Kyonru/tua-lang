describe("plain #integration", function()
  for _, name in ipairs({"dynamic A", "dynamic B"}) do
    it(name, function() print("{broken json text}"); assert.is_true(true) end)
  end
end)
