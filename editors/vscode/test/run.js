"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { runTests } = require("@vscode/test-electron");

async function main() {
  const extensionDevelopmentPath = path.resolve(__dirname, "..");
  const extensionTestsPath = path.resolve(__dirname, "suite", "index.js");
  const serverFixturePath = path.resolve(__dirname, "fixtures", "mock-lsp-server.js");
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tua-vscode-test-"));
  const workspacePath = path.join(tempRoot, "workspace");
  const documentPath = path.join(workspacePath, "suggestions.tua");
  const logPath = path.join(tempRoot, "lsp-events.jsonl");

  fs.mkdirSync(workspacePath);
  fs.writeFileSync(documentPath, "local seed = 1\n\n");
  const serverPath = createServerLauncher(tempRoot, serverFixturePath);

  try {
    await runTests({
      extensionDevelopmentPath,
      extensionTestsPath,
      extensionTestsEnv: {
        TUA_SERVER_PATH: serverPath,
        TUA_TEST_DOCUMENT: documentPath,
        TUA_TEST_LSP_LOG: logPath,
      },
      launchArgs: [
        workspacePath,
        "--disable-extensions",
        `--user-data-dir=${path.join(tempRoot, "user-data")}`,
        `--extensions-dir=${path.join(tempRoot, "extensions")}`,
      ],
      reuseMachineInstall: true,
      vscodeExecutablePath: localVSCodeExecutable(),
    });
  } finally {
    fs.rmSync(tempRoot, { force: true, recursive: true });
  }
}

function createServerLauncher(tempRoot, serverFixturePath) {
  if (process.platform === "win32") {
    const launcherPath = path.join(tempRoot, "tua-vscode-test.cmd");
    fs.writeFileSync(
      launcherPath,
      `@echo off\r\n"${process.execPath}" "${serverFixturePath}" %*\r\n`,
    );
    return launcherPath;
  }

  const launcherPath = path.join(tempRoot, "tua-vscode-test");
  fs.writeFileSync(
    launcherPath,
    `#!/bin/sh\nexec "${process.execPath}" "${serverFixturePath}" "$@"\n`,
  );
  fs.chmodSync(launcherPath, 0o755);
  return launcherPath;
}

function localVSCodeExecutable() {
  if (process.env.VSCODE_EXECUTABLE_PATH) {
    return process.env.VSCODE_EXECUTABLE_PATH;
  }

  if (process.platform === "darwin") {
    const applicationPath = "/Applications/Visual Studio Code.app/Contents/MacOS/Electron";
    if (fs.existsSync(applicationPath)) {
      return applicationPath;
    }
  }

  return undefined;
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
