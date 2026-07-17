"use strict";

const fs = require("fs");
const path = require("path");
const { normalizePath, normalizeRelativePath, replaceExtension } = require("./project-layout");

function loadSourceMapForGeneratedPath(generatedPath) {
  const mapPath = sourceMapPathForGeneratedPath(generatedPath);
  if (!fs.existsSync(mapPath)) {
    return undefined;
  }

  const parsed = JSON.parse(fs.readFileSync(mapPath, "utf8"));
  if (parsed?.version !== 1) {
    throw new Error(`unsupported source map version in ${mapPath}`);
  }

  const lineMappings = Array.isArray(parsed.lineMappings)
    ? parsed.lineMappings
        .map((mapping) => ({
          generatedLine: integerLine(mapping?.generatedLine),
          sourceLine: integerLine(mapping?.sourceLine),
        }))
        .filter((mapping) => mapping.generatedLine && mapping.sourceLine)
    : [];
  const root = sourceMapRoot(generatedPath, parsed.generated) || path.dirname(mapPath);
  const source = typeof parsed.source === "string" ? parsed.source : undefined;
  const generated = typeof parsed.generated === "string" ? parsed.generated : undefined;

  return {
    generated,
    generatedPath: generated ? path.resolve(root, generated) : path.resolve(generatedPath),
    lineMappings,
    mapPath,
    root,
    source,
    sourcePath: source ? resolveSourceMapPath(root, source) : undefined,
  };
}

function loadSourceMapsFromDirectory(directory) {
  const maps = [];

  collectSourceMapPaths(directory, maps);

  return maps
    .sort()
    .map((mapPath) => loadSourceMapForGeneratedPath(mapPath.slice(0, -".map.json".length)))
    .filter(Boolean);
}

function sourceMapPathForGeneratedPath(generatedPath) {
  return replaceExtension(generatedPath, ".lua.map.json");
}

function sourceMapRoot(generatedPath, generated) {
  if (typeof generated !== "string" || path.isAbsolute(generated)) {
    return undefined;
  }

  const normalizedGeneratedPath = normalizePath(generatedPath);
  const relativeGeneratedPath = normalizeRelativePath(generated);
  const suffix = `${path.sep}${relativeGeneratedPath}`;

  if (!normalizedGeneratedPath.endsWith(suffix)) {
    return undefined;
  }

  return (
    generatedPath.slice(0, generatedPath.length - suffix.length) ||
    path.parse(generatedPath).root
  );
}

function resolveSourceMapPath(root, mappedPath) {
  if (typeof mappedPath !== "string" || !mappedPath) {
    return undefined;
  }

  return path.resolve(root, mappedPath);
}

function nearestMapping(mappings, field, targetLine) {
  let best;
  let bestDistance = Number.MAX_SAFE_INTEGER;

  for (const mapping of mappings) {
    const line = mapping[field];
    const distance = Math.abs(line - targetLine);
    if (
      distance < bestDistance ||
      (distance === bestDistance &&
        (!best || mapping.generatedLine < best.generatedLine))
    ) {
      best = mapping;
      bestDistance = distance;
    }
  }

  return best;
}

function lookupSourceLocation(maps, generatedPath, generatedLine) {
  const matches = [];

  for (const map of maps) {
    const mapping = map.lineMappings.find((entry) => entry.generatedLine === generatedLine);
    if (!mapping || !map.sourcePath) {
      continue;
    }

    const score = pathMatchScore(generatedPath, map.generated || map.generatedPath);
    if (score === undefined) {
      continue;
    }

    matches.push({
      generatedLine,
      generatedPath,
      score,
      sourceLine: mapping.sourceLine,
      sourcePath: map.sourcePath,
    });
  }

  matches.sort((left, right) => right.score - left.score);

  if (matches.length === 0 || (matches.length > 1 && matches[0].score === matches[1].score)) {
    return undefined;
  }

  return matches[0];
}

function remapTracebackLine(line, maps) {
  const locations = tracebackLocations(line, maps);
  if (locations.length === 0) {
    return undefined;
  }

  return locations
    .map(
      (location) =>
        `${location.sourcePath}:${location.sourceLine} (generated ${location.generatedPath}:${location.generatedLine})`,
    )
    .join("\n");
}

function tracebackLocations(line, maps) {
  const locations = [];
  let cursor = 0;

  while (cursor < line.length) {
    const relativeLua = line.slice(cursor).indexOf(".lua:");
    if (relativeLua === -1) {
      break;
    }

    const luaStart = cursor + relativeLua;
    const pathEnd = luaStart + ".lua".length;
    const lineStart = pathEnd + 1;
    const lineEnd = consumeDigits(line, lineStart);

    if (lineEnd === lineStart) {
      cursor = lineStart;
      continue;
    }

    const pathStart = tracebackPathStart(line, luaStart);
    const generatedPath = line.slice(pathStart, pathEnd);
    const generatedLine = Number.parseInt(line.slice(lineStart, lineEnd), 10);
    const location = lookupSourceLocation(maps, generatedPath, generatedLine);

    if (location) {
      locations.push(location);
    }

    cursor = lineEnd;
  }

  return locations;
}

function pathMatchScore(tracebackPath, generatedPath) {
  const normalizedTraceback = normalizeTracebackPath(tracebackPath);
  const normalizedGenerated = normalizeTracebackPath(generatedPath);

  if (normalizedTraceback === normalizedGenerated) {
    return normalizedGenerated.length + 10_000;
  }

  if (normalizedTraceback.endsWith(`/${normalizedGenerated}`)) {
    return normalizedGenerated.length;
  }

  if (normalizedGenerated.endsWith(`/${normalizedTraceback}`)) {
    return normalizedTraceback.length;
  }

  return undefined;
}

function normalizeTracebackPath(filePath) {
  return filePath.replace(/\\/gu, "/").replace(/^\.\//u, "");
}

function tracebackPathStart(line, luaStart) {
  let pathStart = luaStart;

  for (let index = luaStart - 1; index >= 0; index -= 1) {
    const character = line[index];
    if (isTracebackPathBoundary(character)) {
      break;
    }

    pathStart = index;
  }

  return pathStart;
}

function isTracebackPathBoundary(character) {
  return /\s/u.test(character) || character === "\"" || character === "'" || character === "[" || character === "(";
}

function consumeDigits(input, index) {
  let cursor = index;

  while (cursor < input.length && input[cursor] >= "0" && input[cursor] <= "9") {
    cursor += 1;
  }

  return cursor;
}

function collectSourceMapPaths(entryPath, paths) {
  if (!fs.existsSync(entryPath)) {
    return;
  }

  const stat = fs.statSync(entryPath);
  if (stat.isFile()) {
    if (isSourceMapPath(entryPath)) {
      paths.push(entryPath);
    }
    return;
  }

  if (!stat.isDirectory()) {
    return;
  }

  for (const entry of fs.readdirSync(entryPath)) {
    collectSourceMapPaths(path.join(entryPath, entry), paths);
  }
}

function isSourceMapPath(filePath) {
  return path.basename(filePath).endsWith(".lua.map.json");
}

function integerLine(value) {
  return Number.isInteger(value) && value > 0 ? value : undefined;
}

module.exports = {
  loadSourceMapForGeneratedPath,
  loadSourceMapsFromDirectory,
  lookupSourceLocation,
  nearestMapping,
  remapTracebackLine,
  resolveSourceMapPath,
  sourceMapPathForGeneratedPath,
};
