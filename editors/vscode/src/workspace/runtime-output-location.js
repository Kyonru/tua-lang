"use strict";

const { lookupSourceLocation, tracebackLocations } = require("../editor/source-maps");
const { projectSourcePath, sourceLocation } = require("./console-buffer");

// Use the same source maps for task output and debugger output. A traceback
// line may mention several frames; the first mapped frame is the error site.
function runtimeOutputLocation(line, cwd, maps = [], source, sourceLine) {
  if (source?.endsWith(".tua") && Number.isInteger(sourceLine) && sourceLine > 0) {
    return { path: projectSourcePath(source, cwd), line: sourceLine - 1, character: 0 };
  }
  if (source?.endsWith(".lua") && Number.isInteger(sourceLine) && sourceLine > 0) {
    const mapped = lookupSourceLocation(maps, source, sourceLine);
    if (mapped) return { path: projectSourcePath(mapped.sourcePath, cwd), line: mapped.sourceLine - 1, character: 0 };
  }
  const mapped = tracebackLocations(line, maps)[0];
  if (mapped) return { path: projectSourcePath(mapped.sourcePath, cwd), line: mapped.sourceLine - 1, character: 0 };
  return sourceLocation(line, cwd);
}

module.exports = { runtimeOutputLocation };
