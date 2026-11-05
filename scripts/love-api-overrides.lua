local optional_arguments = {
  ["love.window.setMode\0flags"] = true,
}

-- LOVE 11.5 routes these through filesystem::luax_getdata, which accepts
-- File and all Data subclasses, including FileData. Keep the correction shared
-- by every generated catalog; do not widen unrelated FileData-only methods.
-- https://github.com/love2d/love/blob/11.5/src/modules/image/wrap_Image.cpp
local image_data_inputs = {
  ["love.image.newImageData"] = true,
  ["love.image.newCompressedData"] = true,
  ["love.image.isCompressed"] = true,
}

return {
  argument_type = function(target, argument)
    if image_data_inputs[target] and argument.type == "FileData" then
      return "File | Data"
    end
    if target == "love.filesystem.newFileData" and argument.name == "filepath" then
      return "string | File"
    end
    return argument.type
  end,
  argument_is_optional = function(target, argument)
    if argument.default ~= nil then
      return true
    end

    return optional_arguments[target .. "\0" .. tostring(argument.name or "arg")] == true
  end,
}
