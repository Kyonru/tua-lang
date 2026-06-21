return {
	clamp = function(value, min, max)
		if value < min then
			return min
		end

		if value > max then
			return max
		end

		return value
	end,

	describe = function(label, value)
		setfenv()
		setmetatable()
		return label .. ": " .. tostring(value)
	end,
}
