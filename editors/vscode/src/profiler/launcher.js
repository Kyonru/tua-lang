"use strict";
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { luaString } = require("../inspector/launcher");

const TRACE_CATEGORIES = new Set(["ecs", "behaviors", "signals", "audio", "assets"]);

function createProfileLauncher(outputDirectory, shimPath, durationSeconds, traceCategories = []) {
  if (!Array.isArray(traceCategories) || traceCategories.some(category => !TRACE_CATEGORIES.has(category))) {
    throw new Error("invalid performance trace category");
  }
  const originalMain = path.join(outputDirectory, "main.lua");
  if (!fs.statSync(originalMain).isFile()) throw new Error("built game has no main.lua");
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-profile-"));
  const directory = path.join(root, path.basename(outputDirectory));
  const reportPath = path.join(root, "report.json");
  const stopPath = path.join(root, "stop");
  try {
    fs.chmodSync(root, 0o700);
    fs.mkdirSync(directory);
    for (const name of fs.readdirSync(outputDirectory)) {
      if (name === "main.lua") continue;
      const source = path.join(outputDirectory, name);
      const target = path.join(directory, name);
      if (fs.statSync(source).isDirectory()) {
        fs.symlinkSync(source, target, process.platform === "win32" ? "junction" : "dir");
      } else {
        // PhysFS does not treat a symlinked root file as a normal file.
        fs.copyFileSync(source, target);
      }
    }
    const bootstrap = [
      "-- Tua development-only profiler launcher. The build is untouched.",
      `local profiler = assert(loadfile(${luaString(shimPath)}))(${luaString(reportPath)}, ${luaString(stopPath)}, ${durationSeconds}, { tracePath = ${luaString(path.join(path.dirname(shimPath), "trace.lua"))}, trace = { ${traceCategories.map(category => `[${luaString(category)}] = true`).join(", ")} } })`,
      "profiler.beforeGame()",
      `local results = { assert(loadfile(${luaString(originalMain)}))() }`,
      "profiler.install()",
      "return unpack(results)",
      "",
    ].join("\n");
    fs.writeFileSync(path.join(directory, "main.lua"), bootstrap, { mode: 0o600 });
    return {
      directory, reportPath, stopPath,
      stop() { fs.writeFileSync(stopPath, "stop\n", { mode: 0o600 }); },
      dispose() { fs.rmSync(root, { force: true, recursive: true }); },
    };
  } catch (error) {
    fs.rmSync(root, { force: true, recursive: true });
    throw error;
  }
}

module.exports = { createProfileLauncher };
