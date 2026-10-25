"use strict";
const childProcess = require("node:child_process");

function probeVersion(command) {
  return new Promise(resolve => childProcess.execFile(command, ["--version"], { timeout: 3000, maxBuffer: 65536, windowsHide: true }, (error, stdout, stderr) => {
    resolve(error ? { ok: false, detail: error.code === "ENOENT" ? "Executable not found" : error.killed ? "Version check timed out" : "Executable could not report its version" }
      : { ok: true, detail: String(stdout || stderr).trim().split(/\r?\n/)[0] || "Executable available" });
  }));
}

function emitMapsSetting(text) {
  const section = text.search(/^\s*\[/m);
  const root = section < 0 ? text : text.slice(0, section);
  const match = /^(\s*(?:emit_maps|"emit_maps"|'emit_maps')\s*=\s*)(true|false)(?=\s*(?:#.*)?$)/m.exec(root);
  return match ? { enabled: match[2] === "true", start: match.index + match[1].length, end: match.index + match[0].length } : { enabled: true };
}

module.exports = { probeVersion, emitMapsSetting };
