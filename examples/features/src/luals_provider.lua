---@meta tua.features.luals_annotations

-- Feature: literal unions preserve exact string choices for completion and diagnostics.
-- Feature: optional shorthand such as `string?` is parsed as `string | nil`.

-- Feature: function type aliases are also parsed, for example:
--   ---@alias ImageLoader fun(path: string, mode?: LoadMode): Image

-- Feature: other supported LuaCATS forms include:
--   ---@alias ImageList Image[]
--   ---@alias PointTuple [number, number]
--   ---@alias SourceMap { [string]: Source }
--   ---@alias ImageMap table<string, Image>

-- Feature: LuaLS tags that Tua keeps as shallow metadata instead of executing.
---@module "features.luals_annotations"
---@enum AssetKind
---@diagnostic disable-next-line: undefined-global
---@nodiscard

---@param path string image path
---@param mode "static" | "stream" loader mode
---@return Image loaded image
local function loadImage(path, mode)
  return love.graphics.newImage(path)
end

-- Feature: deprecated API metadata reaches Tua consumers through module exports.
---@deprecated use loadImage instead
local function legacyLoadImage(path, mode)
  return loadImage(path, mode)
end

return {
  loadImage = loadImage,
  legacyLoadImage = legacyLoadImage,
}
