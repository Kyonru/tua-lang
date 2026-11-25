"use strict";
// Native transport qualification using the existing extension inspector, not a second inspector.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");
const root = path.resolve(__dirname, "../../../../../..");
const { ActivityStore } = require(path.join(root, "editors/vscode/src/workspace/activity-store"));
const { InspectorSessions } = require(path.join(root, "editors/vscode/src/inspector/sessions"));
const activity = new ActivityStore();
const inspector = new InspectorSessions({ activity, enabled: () => true,
  assetsDirectory: path.join(root, "editors/vscode/assets/inspector"),
  options: () => ({ updateInterval: 100, categories: ["ecs", "states", "pools"] }) });
async function main() {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "tua-arena-inspector-"));
  let child;
  let logs = "";
  try {
    const game = path.join(temporary, "game");
    fs.cpSync(path.resolve(__dirname, "../build"), game, { recursive: true });
    fs.writeFileSync(path.join(game, "conf.lua"), `function love.conf(t)
  t.window.visible = false
  t.modules.audio = false
end
`);
    fs.writeFileSync(path.join(game, "main.lua"), `local Session = require("session")
local session = Session.new()
session.combat.options.enemyCount = 32
session.update(0, 0, 0, "confirm")
collectgarbage("collect")
function love.update()
  session.trigger.held = true
  session.trigger.x, session.trigger.y = 1500, 200
  session.update(1 / 60, 0, 0, "none")
end
`);
    const session = activity.begin({ title: "Arena C8", kind: "run", cwd: root });
    const launcher = await inspector.launch(session, game);
    child = spawn(process.env.TUA_LOVE || "/Applications/love.app/Contents/MacOS/love", [launcher],
      { stdio: ["ignore", "pipe", "pipe"] });
    child.stdout.on("data", data => { logs += data; });
    child.stderr.on("data", data => { logs += data; });
    child.on("error", error => { logs += error.message; });
    let snapshot;
    for (let index = 0; index < 100; index++) {
      const current = inspector.entries.get(session.id)?.snapshot;
      if (current?.objects.some(object => object.kind === "states" && object.data.current === "playing")) {
        snapshot = current;
        break;
      }
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    assert(snapshot, logs);
    assert(snapshot.objects.some(object => object.kind === "ecs"));
    assert(snapshot.objects.some(object => object.kind === "pools"));
    const summary = { verified: ["live ECS", "playing state", "bullet pool"],
      scope: "native inspector transport, not installed VS Code UI", snapshot };
    fs.writeFileSync(process.argv[2], JSON.stringify(summary, null, 2) + "\n");
    console.log("ARENA_INSPECTOR_OK live ECS, state machine and bullet pool");
  } finally {
    if (child && child.exitCode === null) {
      child.kill();
      await new Promise(resolve => child.once("exit", resolve));
    }
    inspector.dispose();
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
