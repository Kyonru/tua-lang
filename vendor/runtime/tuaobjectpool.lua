local function objectPool(options)
  assert(type(options) == "table", "objectPool options must be a table")
  assert(type(options.create) == "function", "objectPool options.create must be a function")
  assert(options.reset == nil or type(options.reset) == "function", "objectPool options.reset must be a function")
  assert(options.dispose == nil or type(options.dispose) == "function", "objectPool options.dispose must be a function")

  local create = options.create
  local reset = options.reset
  local dispose = options.dispose
  local available = {}
  local active = {}
  local owned = {}
  local activeCount = 0
  local capacity = 0

  local function createObject()
    local object = create()
    assert(object ~= nil, "objectPool options.create returned nil")
    assert(not owned[object], "objectPool options.create returned an object already owned by this pool")

    owned[object] = true
    capacity = capacity + 1
    return object
  end

  local pool = {}

  function pool:acquire()
    local last = #available
    local object

    if last > 0 then
      object = available[last]
      available[last] = nil
    else
      object = createObject()
    end

    active[object] = true
    activeCount = activeCount + 1
    return object
  end

  function pool:release(object)
    if not owned[object] or not active[object] then
      return false
    end

    if reset then
      reset(object)
    end

    active[object] = nil
    activeCount = activeCount - 1
    available[#available + 1] = object
    return true
  end

  function pool:prewarm(count)
    assert(type(count) == "number" and count >= 0 and count == math.floor(count), "objectPool prewarm count must be a non-negative integer")

    local created = 0
    while #available < count do
      available[#available + 1] = createObject()
      created = created + 1
    end

    return created
  end

  function pool:clear()
    if dispose then
      for object in pairs(owned) do
        dispose(object)
      end
    end

    available = {}
    active = {}
    owned = {}
    activeCount = 0
    capacity = 0
  end

  function pool:owns(object)
    return owned[object] == true
  end

  function pool:activeCount()
    return activeCount
  end

  function pool:availableCount()
    return #available
  end

  function pool:capacity()
    return capacity
  end

  local initial = options.initial or 0
  pool:prewarm(initial)

  return pool
end

return objectPool
