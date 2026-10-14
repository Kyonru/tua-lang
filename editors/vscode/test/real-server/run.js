"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { runTests } = require("@vscode/test-electron");
const { localVSCodeExecutable } = require("../vscode-executable");

async function main() {
  const extensionDevelopmentPath = path.resolve(__dirname, "..", "..");
  const extensionTestsPath = path.resolve(__dirname, "suite", "index.js");
  const serverPath = path.resolve(
    process.env.TUA_REAL_SERVER_PATH ||
      path.join(extensionDevelopmentPath, "bin", process.platform === "win32" ? "tua.exe" : "tua"),
  );

  assertRealServer(serverPath);

  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tua-vsc-real-"));
  const workspacePath = path.join(tempRoot, "project");
  const fixtures = createProject(workspacePath);

  try {
    await runTests({
      extensionDevelopmentPath,
      extensionTestsPath,
      extensionTestsEnv: {
        TUA_REAL_SERVER_FIXTURES: JSON.stringify(fixtures),
        TUA_REAL_SERVER_PATH: serverPath,
        TUA_SERVER_PATH: serverPath,
      },
      launchArgs: [
        workspacePath,
        "--disable-extensions",
        "--disable-workspace-trust",
        `--user-data-dir=${path.join(tempRoot, "user-data")}`,
        `--extensions-dir=${path.join(tempRoot, "extensions")}`,
      ],
      reuseMachineInstall: true,
      vscodeExecutablePath: localVSCodeExecutable(),
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
