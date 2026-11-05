-- Loaded as LuaCov configuration, outside the generated project tree.
-- Delegate executable-line classification to LuaCov's reporter API.
package.preload['luacov.reporter.tua'] = function()
  local reporter = require('luacov.reporter')
  local json = require('dkjson')
  local Report = setmetatable({}, reporter.ReporterBase)
  Report.__index = Report
  local function array() return setmetatable({}, {__jsontype = 'array'}) end
  function Report:on_start() self.results = array(); self.errors = array() end
  function Report:on_new_file(filename)
    self.current = {path = filename, lines = array()}
    self.results[#self.results + 1] = self.current
  end
  function Report:on_hit_line(_, line, _, hits)
    self.current.lines[#self.current.lines + 1] = {line = line, hits = hits}
  end
  function Report:on_mis_line(filename, line, text)
    self:on_hit_line(filename, line, text, 0)
  end
  function Report:on_file_error(filename, operation, message)
    self.errors[#self.errors + 1] = filename .. ': ' .. operation .. ': ' .. message
  end
  function Report:on_end()
    self:write(assert(json.encode({files = self.results, errors = self.errors})))
  end
  return {report = function() reporter.report(Report) end}
end
return {
  statsfile = assert(os.getenv('TUA_COVERAGE_STATS')),
  reportfile = assert(os.getenv('TUA_COVERAGE_REPORT')),
  runreport = true,
  reporter = 'tua',
  includeuntestedfiles = true,
}
