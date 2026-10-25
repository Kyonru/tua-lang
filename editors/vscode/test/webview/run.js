"use strict";

const fs = require("fs");
const net = require("net");
const os = require("os");
const path = require("path");
const { runTests } = require("@vscode/test-electron");
const { localVSCodeExecutable } = require("../vscode-executable");

async function main() {
  const server = net.createServer();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "tua-webview-test-"));
  try {
    await runTests({
      extensionDevelopmentPath: path.resolve(__dirname, "../.."),
      extensionTestsPath: path.join(__dirname, "suite.js"),
      extensionTestsEnv: { TUA_PREVIEW_CDP_PORT: String(port) },
      vscodeExecutablePath: localVSCodeExecutable(),
      launchArgs: [
        "--disable-extensions", "--disable-workspace-trust", "--skip-welcome", "--skip-release-notes",
        `--user-data-dir=${path.join(temp, "user")}`,
        `--extensions-dir=${path.join(temp, "extensions")}`,
        `--remote-debugging-port=${port}`,
      ],
    });
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
