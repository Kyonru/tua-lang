-- Busted 2.x output adapter: preserve terminal output and write a separate,
-- machine-readable result. Test stdout cannot corrupt the result document.
return function(options)
  local busted = require('busted')
  local json = require('dkjson')
  local handler = require('busted.outputHandlers.plainTerminal')(options)
  local report_path = assert(os.getenv('TUA_TEST_REPORT'))
  busted.subscribe({'suite', 'end'}, function()
    local tests = {}
    for _, group in ipairs({
      {'passed', handler.successes}, {'failed', handler.failures},
      {'errored', handler.errors}, {'skipped', handler.pendings}
    }) do
      for _, result in ipairs(group[2]) do
        local trace = result.trace or {}
        tests[#tests + 1] = {
          name = result.name, status = group[1],
          message = tostring(result.message or ''),
          file = trace.short_src or trace.source,
          line = trace.currentline or trace.linedefined,
        }
      end
    end
    local report = assert(io.open(report_path, 'w'))
    report:write(assert(json.encode({version = 1, tests = tests, duration = handler.getDuration()})))
    report:close()
    return nil, true
  end)
  return handler
end
