"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { runTests, downloadAndUnzipVSCode, resolveCliArgsFromVSCodeExecutablePath } = require("@vscode/test-electron");
const { spawnSync } = require("child_process");
const { localVSCodeExecutable } = require("../vscode-executable");

async function main() {
  let extensionDevelopmentPath = path.resolve(__dirname, "..", "..");
  const extensionTestsPath = path.resolve(__dirname, "suite", "index.js");
  let serverPath = path.resolve(
    process.env.TUA_REAL_SERVER_PATH ||
      path.join(extensionDevelopmentPath, "bin", process.platform === "win32" ? "tua.exe" : "tua"),
  );

  const vsix = process.env.TUA_TEST_VSIX;
  if (!vsix) assertRealServer(serverPath);

  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tua-vsc-real-"));
  const workspacePath = path.join(tempRoot, "project");
  const fixtures = createProject(workspacePath);

  try {
    let vscodeExecutablePath = localVSCodeExecutable();
    if (vsix) {
      vscodeExecutablePath ||= await downloadAndUnzipVSCode();
      const [cli, ...cliArgs] = resolveCliArgsFromVSCodeExecutablePath(vscodeExecutablePath);
      const installed = path.join(tempRoot, "extensions");
      const result = spawnSync(cli, [...cliArgs, "--user-data-dir", path.join(tempRoot, "user-data"),
        "--extensions-dir", installed, "--install-extension", path.resolve(vsix), "--force"],
        { stdio: "inherit", timeout: 120000, shell: process.platform === "win32" });
      if (result.error || result.status !== 0) throw result.error || new Error("VSIX installation failed");
      const entry = fs.readdirSync(installed).find(name => name.toLowerCase().startsWith("solenodontelabs.tua-"));
      if (!entry) throw new Error("installed Tua extension not found");
      extensionDevelopmentPath = path.join(installed, entry);
      serverPath = path.join(extensionDevelopmentPath, "bin", process.platform === "win32" ? "tua.exe" : "tua");
      assertRealServer(serverPath);
      // Exercise bundled discovery, without a development server override.
      delete process.env.TUA_SERVER_PATH;
    }
    await runTests({
      extensionDevelopmentPath,
      extensionTestsPath,
      extensionTestsEnv: {
        TUA_REAL_SERVER_FIXTURES: JSON.stringify(fixtures),
        TUA_REAL_SERVER_PATH: serverPath,
        ...(vsix ? {} : { TUA_SERVER_PATH: serverPath }),
      },
      launchArgs: [
        workspacePath,
        "--disable-extensions",
        "--disable-workspace-trust",
        `--user-data-dir=${path.join(tempRoot, "user-data")}`,
        `--extensions-dir=${path.join(tempRoot, "extensions")}`,
      ],
      reuseMachineInstall: true,
      vscodeExecutablePath,
    });
  } finally {
    if (process.env.TUA_TEST_KEEP_TEMP === "1") {
      console.log(`kept real-server VS Code test data at ${tempRoot}`);
    } else {
      fs.rmSync(tempRoot, { force: true, recursive: true });
    }
  }
}

function assertRealServer(serverPath) {
  const stats = fs.statSync(serverPath, { throwIfNoEntry: false });
  if (!stats?.isFile()) {
    throw new Error(
      `Real Tua server not found at ${serverPath}. Run \`make extension-server\` first or set TUA_REAL_SERVER_PATH.`,
    );
  }

  if (process.platform !== "win32") {
    fs.accessSync(serverPath, fs.constants.X_OK);
  }
}

function createProject(workspacePath) {
  const sourcePath = path.join(workspacePath, "src");
  const mainPath = path.join(sourcePath, "main.tua");
  const helperPath = path.join(sourcePath, "helper.lua");
  const configPath = path.join(workspacePath, "tua.toml");

  fs.mkdirSync(sourcePath, { recursive: true });
  fs.writeFileSync(
    configPath,
    ['source = "src"', 'out = "build"', 'main = "main.tua"', ""].join("\n"),
  );
  fs.writeFileSync(
    helperPath,
    [
      "local helper = {}",
      "",
      "---@param name string",
      "---@return string",
      "function helper.greet(name)",
      '  return "Hello, " .. name',
      "end",
      "",
      "return helper",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    mainPath,
    [
      "type Player = {",
      "  health: number,",
      "  name: string,",
      "}",
      "",
      "local player: Player={health=100,name=\"Ada\"}",
      "local helper = require(\"helper\")",
      "local current_health = player.health",
      "local uppercase_name = string.upper(player.name)",
      "local greeting = helper.greet(player.name)",
      "local screen_width = love.graphics.getWidth()",
      "",
      "return {",
      "  current_health = current_health,",
      "  uppercase_name = uppercase_name,",
      "  greeting = greeting,",
      "  screen_width = screen_width,",
      "}",
      "",
    ].join("\n"),
  );

  return { configPath, helperPath, mainPath, serverPath: process.env.TUA_REAL_SERVER_PATH };
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
