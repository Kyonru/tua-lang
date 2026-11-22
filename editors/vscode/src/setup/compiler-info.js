"use strict";
const { execFile } = require("node:child_process");

function parseCompilerInfo(output) {
  let value;
  try { value = JSON.parse(output); } catch { return null; }
  if (!value || value.schemaVersion !== 1) return null;
  const short = text => typeof text === "string" && /^[\w.+-]{1,128}$/.test(text) ? text : null;
  if (!short(value.version) || !short(value.target)) return null;
  return {
    schemaVersion: 1, version: value.version, target: value.target,
    profile: short(value.profile),
    revision: typeof value.revision === "string" && /^[a-f0-9]{40,64}$/.test(value.revision) ? value.revision : null,
    dirty: typeof value.dirty === "boolean" ? value.dirty : null,
    executableSha256: typeof value.executableSha256 === "string" && /^[a-f0-9]{64}$/.test(value.executableSha256) ? value.executableSha256 : null,
  };
}

function readCompilerInfo(command, cwd, run = execFile) {
  return new Promise(resolve => {
    // Optional capability: legacy compilers can reject the flag or return plain
    // version text. Neither case prevents the language server from starting.
    run(command, ["--version", "--json"], { cwd, timeout: 3000, maxBuffer: 16384, windowsHide: true },
      (error, stdout) => resolve(error ? null : parseCompilerInfo(stdout)));
  });
}

function compilerInfoLines(info) {
  return [
    `Compiler revision: ${info?.revision || "unknown"}`,
    `Compiler build dirty: ${typeof info?.dirty === "boolean" ? info.dirty : "unknown"}`,
    `Compiler target: ${info?.target || "unknown"}`,
    `Compiler profile: ${info?.profile || "unknown"}`,
    `Compiler SHA-256: ${info?.executableSha256 || "unknown"}`,
  ];
}
module.exports = { parseCompilerInfo, readCompilerInfo, compilerInfoLines };
