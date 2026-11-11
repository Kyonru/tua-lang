"use strict";
const fs = require("node:fs");
const net = require("node:net");
const os = require("node:os");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { createRequire } = require("node:module");
const root = path.resolve(__dirname, "../../..");
const extension = path.join(root, "editors/vscode");
const fromExtension = createRequire(path.join(extension, "package.json"));

async function main() {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "tua-preview-stress-"));
  const project = path.join(temp, "project");
  fs.mkdirSync(path.join(project, "src/assets"), { recursive: true });
  fs.writeFileSync(path.join(project, "tua.toml"), 'source = "src"\nout = "build"\n');
  fs.writeFileSync(path.join(project, "src/main.tua"), 'local image = love.graphics.newImage("assets/player.png")\nlocal modules = require("module_000")\n');
  for (let index = 0; index < 120; index++) {
    const next = index < 119 ? `local _next = require("module_${String(index + 1).padStart(3, "0")}")\n` : "";
    const returned = `return { value = ${index} }\n`;
    fs.writeFileSync(path.join(project, "src", `module_${String(index).padStart(3, "0")}.tua`), next + returned);
  }
  fs.copyFileSync(path.join(extension, "test/fixtures/player.png"), path.join(project, "src/assets/player.png"));
  for (const [name, source] of [
    ["sfx_move.wav", "examples/love-full/game/assets/sounds/sfx_move.wav"],
    ["sfx_bump.ogg", "examples/love-full/game/assets/sounds/sfx_bump.ogg"],
    ["sniglet.otf", "examples/love-full/game/assets/sniglet.otf"],
    ["wave.glsl", "examples/features/src/assets/wave.glsl"],
  ]) fs.copyFileSync(path.join(root, source), path.join(project, "src/assets", name));
  const server = net.createServer();
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  const graphFile = path.join(temp, "real-module-graph.json");
  const releaseServer = path.join(root, "target/release/tua");
  execFileSync("python3", [path.join(__dirname, "real-module-graph-probe.py"), project, graphFile, releaseServer]);
  try {
    await fromExtension("@vscode/test-electron").runTests({
      extensionDevelopmentPath: extension,
      extensionTestsPath: path.join(__dirname, "preview-stress-suite.js"),
      extensionTestsEnv: {
        TUA_PREVIEW_STRESS_PORT: String(port),
        TUA_PREVIEW_STRESS_OUT: path.join(__dirname, "preview-stress.json"),
        TUA_REAL_GRAPH_JSON: graphFile,
        TUA_SERVER_PATH: releaseServer
      },
      launchArgs: [project, "--disable-extensions", "--disable-workspace-trust",
        "--skip-welcome", "--skip-release-notes", `--remote-debugging-port=${port}`,
        `--user-data-dir=${path.join(temp, "user")}`, `--extensions-dir=${path.join(temp, "extensions")}`],
      reuseMachineInstall: true,
      vscodeExecutablePath: require(path.join(extension, "test/vscode-executable")).localVSCodeExecutable()
    });
    console.log(fs.readFileSync(path.join(__dirname, "preview-stress.json"), "utf8"));
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
