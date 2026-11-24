-- Reveal consumes timing fields only; rendering metadata remains host-owned.
local Timing = {}
local function finite(value)
  return type(value) == "number" and value == value and math.abs(value) < math.huge
end
local function integer(value)
  return finite(value) and value % 1 == 0
end
local function array(value, limit)
  assert(
    type(value) == "table" and getmetatable(value) == nil and #value <= limit,
    "invalid rich timing array"
  )
  local count = 0
  for key in pairs(value) do
    assert(integer(key) and key >= 1 and key <= #value, "rich timing arrays must be dense")
    count = count + 1
  end
  assert(count == #value, "rich timing arrays must be dense")
end

function Timing.same(saved, view)
  if saved == nil or view == nil then return saved == nil and view == nil end
  if type(view) ~= "table"
    or saved.text ~= view.text
    or type(view.runs) ~= "table"
    or type(view.pauses) ~= "table" then
    return false
  end
  if #saved.runs ~= #view.runs or #saved.pauses ~= #view.pauses then return false end
  for index, run in ipairs(saved.runs) do
    local other = view.runs[index]
    if type(other) ~= "table"
      or run.firstCluster ~= other.firstCluster
      or run.lastCluster ~= other.lastCluster
      or run.rate ~= other.rate then
      return false
    end
  end
  for index, pause in ipairs(saved.pauses) do
    local other = view.pauses[index]
    if type(other) ~= "table"
      or pause.afterCluster ~= other.afterCluster
      or pause.seconds ~= other.seconds then
      return false
    end
  end
  return true
end

function Timing.compile(view, text, units, defaultRate)
  if view == nil then return nil end
  assert(type(view) == "table" and view.text == text, "rich presentation text mismatch")
  array(view.runs, 4096)
  array(view.pauses, 256)
  local saved = { text = text, runs = {}, pauses = {} }
  local expected = 1
  for index, run in ipairs(view.runs) do
    assert(
      type(run) == "table"
      and run.firstCluster == expected
      and integer(run.lastCluster)
      and run.lastCluster >= expected
      and run.lastCluster <= #units,
      "rich runs must cover each cluster once"
    )
    assert(
      run.rate == nil or finite(run.rate) and run.rate > 0 and run.rate <= 1000,
      "invalid rich reveal rate"
    )
    saved.runs[index] = { firstCluster = expected, lastCluster = run.lastCluster, rate = run.rate }
    expected = run.lastCluster + 1
  end
  assert(expected == #units + 1, "rich runs must cover all clusters")
  local previous, total, rounding = 0, 0, 0
  for index, pause in ipairs(view.pauses) do
    assert(
      type(pause) == "table"
      and integer(pause.afterCluster)
      and pause.afterCluster >= previous
      and pause.afterCluster <= #units,
      "invalid rich pause boundary"
    )
    assert(
      finite(pause.seconds) and pause.seconds >= 0 and pause.seconds <= 60,
      "invalid rich pause duration"
    )
    previous = pause.afterCluster
    local corrected = pause.seconds - rounding
    local sum = total + corrected
    rounding, total = (sum - total) - corrected, sum
    assert(total <= 120, "rich pause duration limit exceeded")
    saved.pauses[index] = { afterCluster = pause.afterCluster, seconds = pause.seconds }
  end
  local deadlines, elapsed, runIndex, pauseIndex = {}, 0, 1, 1
  for boundary = 0, #units do
    while saved.pauses[pauseIndex] and saved.pauses[pauseIndex].afterCluster == boundary do
      elapsed = elapsed + saved.pauses[pauseIndex].seconds
      pauseIndex = pauseIndex + 1
    end
    if boundary < #units then
      while boundary >= saved.runs[runIndex].lastCluster do runIndex = runIndex + 1 end
      elapsed = elapsed + 1 / (saved.runs[runIndex].rate or defaultRate)
      deadlines[boundary + 1] = elapsed
    end
  end
  return { saved = saved, deadlines = deadlines, duration = elapsed }
end

function Timing.visible(schedule, elapsed)
  local low, high = 1, #schedule.deadlines
  while low <= high do
    local middle = math.floor((low + high) / 2)
    if schedule.deadlines[middle] <= elapsed then low = middle + 1 else high = middle - 1 end
  end
  return high
end
return Timing
