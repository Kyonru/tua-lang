"use strict";
const fs = require("node:fs");
const net = require("node:net");
const os = require("node:os");
const path = require("node:path");
const { createRequire } = require("node:module");
const root = path.resolve(__dirname, "../../..");
const extension = path.join(root, "editors/vscode");
const fromExtension = createRequire(path.join(extension, "package.json"));

async function main() {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "tua-preview-stress-"));
  const project = path.join(temp, "project");
  fs.mkdirSync(path.join(project, "src/assets"), { recursive: true });
  fs.writeFileSync(path.join(project, "tua.toml"), 'source = "src"\nout = "build"\n');
  fs.writeFileSync(path.join(project, "src/main.tua"), 'local image = love.graphics.newImage("assets/player.png")\n');
  fs.copyFileSync(path.join(extension, "test/fixtures/player.png"), path.join(project, "src/assets/player.png"));
  const server = net.createServer();
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  try {
    await fromExtension("@vscode/test-electron").runTests({
      extensionDevelopmentPath: extension,
      extensionTestsPath: path.join(__dirname, "preview-stress-suite.js"),
      extensionTestsEnv: {
        TUA_PREVIEW_STRESS_PORT: String(port),
        TUA_PREVIEW_STRESS_OUT: path.join(__dirname, "preview-stress.json"),
        TUA_SERVER_PATH: path.join(root, "target/release/tua")
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
