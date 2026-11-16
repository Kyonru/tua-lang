"use strict";
const fs = require("node:fs");
const path = require("node:path");
const { stripVTControlCharacters } = require("node:util");

class ConsoleBuffer {
  constructor(limit = 2000) { this.limit = limit; this.entries = []; this.serial = 0; }
  append(session, raw, options = {}) {
    const text = stripVTControlCharacters(raw).slice(0, 16000);
    if (!text.trim()) return;
    const tagged = text.match(/^\[(trace|debug|info|warn|warning|error|fatal)\](?:\s*\[([^\]]+)\])?/i);
    const reversed = text.match(/^\[([^\]]+)\]\s*\[(trace|debug|info|warn|warning|error|fatal)\]/i);
    let severity = options.severity || tagged?.[1]?.toLowerCase() || reversed?.[2]?.toLowerCase();
    if (!severity) severity = /\b(error|failed|panic|exception)\b/i.test(text) || (options.location && options.stream === "stderr")
      ? "error" : options.stream === "stderr" ? "warn" : "info";
    if (severity === "warning") severity = "warn";
    if (severity === "fatal") severity = "error";
    const module = options.module || tagged?.[2] || reversed?.[1] || "game";
    const location = options.location || sourceLocation(text, session.cwd);
    const previous = this.entries.at(-1);
    if (previous?.session.id === session.id && previous.text === text && previous.severity === severity && previous.module === module &&
        previous.location?.path === location?.path && previous.location?.line === location?.line && previous.location?.character === location?.character) {
      previous.count++;
      previous.time = Date.now();
      return previous;
    }
    const entry = { id: String(++this.serial), session, text, severity, module, location, count: 1, time: Date.now() };
    this.entries.push(entry);
    if (this.entries.length > this.limit) this.entries.splice(0, this.entries.length - this.limit);
    return entry;
  }
  filtered({ severity, module, session } = {}) {
    return this.entries.filter(entry => (!severity || entry.severity === severity) && (!module || entry.module === module) && (!session || entry.session.id === session));
  }
  clear() { this.entries = []; }
}

function sourceLocation(text, cwd) {
  const match = text.match(/(?:^|\s)((?:[A-Za-z]:[\\/]|\/|\.\.?\/|src\/)[^\n]*?\.tua):(\d+)(?::(\d+))?/);
  if (!match) return undefined;
  return { path: projectSourcePath(path.resolve(cwd, match[1]), cwd), line: Math.max(0, Number(match[2]) - 1), character: Math.max(0, Number(match[3] || 1) - 1) };
}

function projectSourcePath(filePath, cwd) {
  if (!cwd) return filePath;
  const direct = path.relative(cwd, filePath);
  if (direct && direct !== ".." && !direct.startsWith(`..${path.sep}`) && !path.isAbsolute(direct)) {
    return filePath;
  }
  let realRoot;
  let realFile;
  try {
    realRoot = fs.realpathSync.native(cwd);
    realFile = path.join(fs.realpathSync.native(path.dirname(filePath)), path.basename(filePath));
  } catch { return filePath; }
  const relative = path.relative(realRoot, realFile);
  return relative && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative)
    ? path.join(cwd, relative)
    : filePath;
}

module.exports = { ConsoleBuffer, projectSourcePath, sourceLocation };
