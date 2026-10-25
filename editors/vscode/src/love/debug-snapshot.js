"use strict";
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { loadSourceMapForGeneratedPath } = require("../editor/source-maps");

// Running debuggers must not lose their bootstrap or source maps to a later build.
function createDebugSnapshot(outputDirectory) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-debug-"));
  const output = path.join(root, path.basename(outputDirectory));
  try {
    fs.cpSync(outputDirectory, output, { recursive: true });
    relocateMaps(outputDirectory, output);
    return { outputDirectory: output, dispose: () => fs.rmSync(root, { recursive: true, force: true }) };
  } catch (error) {
    fs.rmSync(root, { recursive: true, force: true });
    throw error;
  }
}

function relocateMaps(original, copied) {
  for (const entry of fs.readdirSync(copied, { withFileTypes: true })) {
    const from = path.join(original, entry.name);
    const to = path.join(copied, entry.name);
    if (entry.isDirectory()) relocateMaps(from, to);
    else if (entry.isFile() && entry.name.endsWith(".lua.map")) {
      const map = JSON.parse(fs.readFileSync(to, "utf8"));
      if (map.version === 3) {
        const sourceRoot = path.resolve(path.dirname(from), map.sourceRoot || "");
        // Both supported Lua adapters understand absolute sources, but the legacy
        // adapter concatenates sourceRoot with mapDir even when sourceRoot is absolute.
        map.sources = map.sources.map(source => path.resolve(sourceRoot, source).split(path.sep).join("/"));
        map.sourceRoot = ".";
        fs.writeFileSync(to, JSON.stringify(map));
      }
    } else if (entry.isFile() && entry.name.endsWith(".lua.map.json")) {
      const map = JSON.parse(fs.readFileSync(to, "utf8"));
      const resolved = loadSourceMapForGeneratedPath(from.slice(0, -".map.json".length));
      if (resolved?.sourcePath) {
        map.source = resolved.sourcePath;
        map.generated = to.slice(0, -".map.json".length);
        fs.writeFileSync(to, JSON.stringify(map));
      }
    }
  }
}

module.exports = { createDebugSnapshot };
