local utils = {}

function utils.AABB_collision(ax, ay, aw, ah, bx, by, bw, bh)
  return (
    ax < bx + bw and
    ax + aw > bx and
    ay < by + bh and
    ay + ah > by
  )
end

function utils.easeOutCirc(value, factor)
  local eased = math.sqrt(1 - math.pow(factor - 1, 2))
  return value * eased
end

return utils
