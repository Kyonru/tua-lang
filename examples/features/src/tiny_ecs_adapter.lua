-- Feature: tiny-ecs-compatible runtime adapter.
-- Tua's ECS support is editor/checker metadata. Runtime behavior stays normal
-- Lua, so projects can bind `ecs` to tiny-ecs or a compatible adapter.

local M = {}

function M.world()
  local world = {
    next_id = 0,
    components = {},
  }

  function world:create()
    self.next_id = self.next_id + 1
    return self.next_id
  end

  function world:add(entity, component, value)
    self.components[entity] = self.components[entity] or {}
    self.components[entity][component.name] = value
  end

  function world:get(entity, component)
    return self.components[entity] and self.components[entity][component.name]
  end

  function world:has(entity, component)
    return self:get(entity, component) ~= nil
  end

  function world:remove(entity, component)
    if self.components[entity] then
      self.components[entity][component.name] = nil
    end
  end

  function world:query(components)
    return {
      each = function(_callback)
        -- Tiny adapter placeholder: real tiny-ecs projects would delegate here.
      end,
      count = function()
        return #components
      end,
    }
  end

  return world
end

function M.component(name, schema)
  return {
    name = name,
    schema = schema,
  }
end

function M.system(world, components, update)
  return {
    enabled = true,
    update = update,
    query = world:query(components),
  }
end

return M
