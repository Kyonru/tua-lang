-- Private descriptor identity shared by Save and Dialogue without loading persistence.
-- Weak keys do not keep an unused shape alive. Public fields are not trusted as callbacks.
local descriptors = setmetatable({}, { __mode = "k" })
local Shape = {}

function Shape.register(value, defaults, copy)
  descriptors[value] = { defaults = defaults, copy = copy }
  return value
end

function Shape.get(value)
  local descriptor = descriptors[value]
  assert(descriptor, "context needs Save.shape descriptors")
  return descriptor.copy, descriptor.defaults
end

return Shape
