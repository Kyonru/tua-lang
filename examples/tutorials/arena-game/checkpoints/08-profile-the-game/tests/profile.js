"use strict";
// Reuse the extension's profiler launcher; do not install instrumentation in the game.
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const root = path.resolve(__dirname, "../../../../../..");
const { createProfileLauncher } = require(path.join(root, "editors/vscode/src/profiler/launcher"));
const project = path.resolve(__dirname, "..");
const output = process.argv[2];
if (!output) throw new Error("Usage: node tests/profile.js /absolute/output.json");
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "tua-arena-profile-"));
let launcher;
try {
  const game = path.join(temporary, "game");
  let build = path.join(project, "build");
  if (process.argv[3] === "--baseline") {
    const reference = path.join(temporary, "reference");
    fs.cpSync(project, reference, { recursive: true,
      filter: file => file !== path.join(project, "build") });
    fs.copyFileSync(path.join(__dirname, "reference-combat.tua"), path.join(reference, "src/combat.tua"));
    const compiled = spawnSync(process.env.TUA_BIN || path.join(root, "target/release/tua"),
      ["build"], { cwd: reference, encoding: "utf8" });
    if (compiled.status !== 0) throw new Error(compiled.stdout + compiled.stderr);
    build = path.join(reference, "build");
  }
  fs.cpSync(build, game, { recursive: true });
  fs.copyFileSync(path.join(__dirname, "benchmark.lua"), path.join(game, "main.lua"));
  fs.writeFileSync(path.join(game, "conf.lua"), `io.stdout:setvbuf("no")
if jit then jit.off() end
function love.conf(t)
  t.window.visible = false
  t.window.vsync = 0
  t.modules.audio = false
end
`);
  const revision = spawnSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).stdout.trim();
  launcher = createProfileLauncher(game, path.join(root, "editors/vscode/assets/profiler/shim.lua"),
    0, [], undefined, { gcPolicy: "running", revision,
      benchmark: { workload: "arena-128-one-second-episodes-v2", warmupUpdates: 120, repetitions: 1, revision } });
  const run = spawnSync(process.env.TUA_LOVE || "/Applications/love.app/Contents/MacOS/love",
    [launcher.directory], { encoding: "utf8", timeout: 60000,
      env: { ...process.env, ARENA_MODE: "profile", ARENA_ENEMIES: "128" } });
  if (run.error || run.status !== 0) throw new Error(run.stdout + run.stderr + run.error);
  fs.copyFileSync(launcher.reportPath, output);
  const report = JSON.parse(fs.readFileSync(output, "utf8"));
  console.log(JSON.stringify({ samples: report.sampleCount, frames: report.frameCount,
    durationMs: report.durationMs, metadata: report.metadata }));
} finally {
  if (launcher) launcher.dispose();
  fs.rmSync(temporary, { recursive: true, force: true });
}
