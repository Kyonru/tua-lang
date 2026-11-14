"use strict";
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { luaString } = require("../inspector/launcher");

const TRACE_CATEGORIES = new Set(["ecs", "behaviors", "signals", "audio", "assets"]);
const INPUT_EVENTS = new Set(["keypressed", "keyreleased", "textinput", "mousemoved", "mousepressed", "mousereleased", "wheelmoved"]);

function temporaryGame(outputDirectory, prefix) {
  const originalMain = path.join(outputDirectory, "main.lua");
  if (!fs.statSync(originalMain).isFile()) throw new Error("built game has no main.lua");
  const root = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  const directory = path.join(root, path.basename(outputDirectory));
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
    return { root, directory, originalMain,
      dispose() { fs.rmSync(root, { force: true, recursive: true }); } };
  } catch (error) { fs.rmSync(root, { force: true, recursive: true }); throw error; }
}

function createProfileLauncher(outputDirectory, shimPath, durationSeconds, traceCategories = [], inputSeed) {
  if (!Array.isArray(traceCategories) || traceCategories.some(category => !TRACE_CATEGORIES.has(category))) {
    throw new Error("invalid performance trace category");
  }
  if (inputSeed !== undefined && (!Number.isSafeInteger(inputSeed) || inputSeed < 1 || inputSeed > 2147483647)) {
    throw new Error("invalid input recording seed");
  }
  const game = temporaryGame(outputDirectory, "tua-profile-");
  const reportPath = path.join(game.root, "report.json");
  const stopPath = path.join(game.root, "stop");
  try {
    const bootstrap = [
      "-- Tua development-only profiler launcher. The build is untouched.",
      `local profiler = assert(loadfile(${luaString(shimPath)}))(${luaString(reportPath)}, ${luaString(stopPath)}, ${durationSeconds}, { tracePath = ${luaString(path.join(path.dirname(shimPath), "trace.lua"))}, trace = { ${traceCategories.map(category => `[${luaString(category)}] = true`).join(", ")} }, inputPath = ${luaString(path.join(path.dirname(shimPath), "input.lua"))}, inputSeed = ${inputSeed === undefined ? "nil" : inputSeed} })`,
      "profiler.beforeGame()",
      `local results = { assert(loadfile(${luaString(game.originalMain)}))() }`,
      "profiler.install()",
      "return unpack(results)",
      "",
    ].join("\n");
    fs.writeFileSync(path.join(game.directory, "main.lua"), bootstrap, { mode: 0o600 });
    return {
      directory: game.directory, reportPath, stopPath,
      stop() { fs.writeFileSync(stopPath, "stop\n", { mode: 0o600 }); },
      dispose: game.dispose,
    };
  } catch (error) {
    game.dispose();
    throw error;
  }
}

function luaInputArgument(value) {
  if (typeof value === "string" && Buffer.byteLength(value) <= 128) return luaString(value);
  if (typeof value === "boolean") return String(value);
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  throw new Error("invalid recorded input argument");
}

function createReplayLauncher(outputDirectory, inputPath, recording) {
  if (!Number.isSafeInteger(recording?.seed) || recording.seed < 1 || recording.seed > 2147483647 ||
      !Array.isArray(recording.events) || recording.events.length > 4096 || recording.dropped !== 0) {
    throw new Error("recorded input is incomplete or invalid");
  }
  const lines = [`return { seed = ${recording.seed}, events = {`];
  let previousFrame = 0;
  for (const event of recording.events) {
    if (!INPUT_EVENTS.has(event?.name) || !Number.isSafeInteger(event.frame) || event.frame < 1 || event.frame > 1000000 || event.frame < previousFrame ||
        !Array.isArray(event.args) || event.args.length > 8) {
      throw new Error("invalid recorded input event");
    }
    previousFrame = event.frame;
    lines.push(`{ frame = ${event.frame}, name = ${luaString(event.name)}, args = { ${event.args.map(luaInputArgument).join(", ")} } },`);
  }
  lines.push("} }");
  const game = temporaryGame(outputDirectory, "tua-replay-");
  try {
    const dataPath = path.join(game.root, "input.lua");
    fs.writeFileSync(dataPath, lines.join("\n"), { mode: 0o600 });
    const bootstrap = [
      "-- Tua development-only input replay launcher. The build is untouched.",
      `local replay = assert(loadfile(${luaString(inputPath)}))().replay(assert(loadfile(${luaString(dataPath)}))())`,
      `local results = { assert(loadfile(${luaString(game.originalMain)}))() }`,
      "replay.install()",
      "return unpack(results)",
      "",
    ].join("\n");
    fs.writeFileSync(path.join(game.directory, "main.lua"), bootstrap, { mode: 0o600 });
    return { directory: game.directory, dispose: game.dispose };
  } catch (error) { game.dispose(); throw error; }
}

module.exports = { createProfileLauncher, createReplayLauncher };
