local function visit(items, values, index)
  for _, item in ipairs(items) do
    table.insert(values, item)
  end

  for key, value in pairs(values) do
    values[key] = value
  end

  local selected = items[index]
  table.insert(items, selected)
  table.remove(items, index)
  local value = values[index]
  if value ~= nil then
    return value
  end
end

return visit
