local optional_arguments = {
  ["love.window.setMode\0flags"] = true,
}

return {
  argument_is_optional = function(target, argument)
    if argument.default ~= nil then
      return true
    end

    return optional_arguments[target .. "\0" .. tostring(argument.name or "arg")] == true
  end,
}
