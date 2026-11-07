local function defaultValue()
  error("default failure")
  return 0
end
local function crash(value)
  if value == nil then
    value = defaultValue()
  end
  return value
end
crash()
