"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const net = require("node:net");
const os = require("node:os");
const path = require("node:path");
const { runTests, downloadAndUnzipVSCode } = require("@vscode/test-electron");
const { spawnSync } = require("node:child_process");
const { localVSCodeExecutable, localVSCodeCliArgs } = require("../vscode-executable");

async function main() {
  let extension = path.resolve(__dirname, "../..");
  let server = path.resolve(process.env.TUA_REAL_SERVER_PATH ||
    path.join(extension, "bin", process.platform === "win32" ? "tua.exe" : "tua"));
  const vsix = process.env.TUA_TEST_VSIX;
  if (!vsix) assert(fs.statSync(server, { throwIfNoEntry: false })?.isFile(), `real Tua server missing: ${server}`);
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "tua-sidebar-host-"));
  const project = path.join(temp, "project");
  const source = path.join(project, "src");
  fs.mkdirSync(source, { recursive: true });
  fs.writeFileSync(path.join(project, "tua.toml"), 'source = "src"\nout = "build"\nmain = "main.tua"\n');
  fs.writeFileSync(path.join(source, "main.tua"), 'local helper = require "helper"\nreturn helper\n');
  fs.writeFileSync(path.join(source, "helper.lua"), "return { old = 1 }\n");
  fs.writeFileSync(path.join(source, "extra.tua"), "return { extra = 2 }\n");
  const listener = net.createServer();
  await new Promise(resolve => listener.listen(0, "127.0.0.1", resolve));
  const port = listener.address().port;
  await new Promise(resolve => listener.close(resolve));
  try {
    let vscodeExecutablePath = localVSCodeExecutable();
    if (vsix) {
      vscodeExecutablePath ||= await downloadAndUnzipVSCode();
      const [cli, ...cliArgs] = localVSCodeCliArgs(vscodeExecutablePath);
      const installed = path.join(temp, "extensions");
      const result = spawnSync(cli, [...cliArgs, "--user-data-dir", path.join(temp, "user"),
        "--extensions-dir", installed, "--install-extension", path.resolve(vsix), "--force"],
      { stdio: "inherit", timeout: 120000, shell: process.platform === "win32" });
      if (result.error || result.status !== 0) throw result.error || new Error("VSIX installation failed");
      const entry = fs.readdirSync(installed).find(name => name.toLowerCase().startsWith("solenodontelabs.tua-"));
      assert(entry, "installed Tua extension not found");
      extension = path.join(installed, entry);
      server = path.join(extension, "bin", process.platform === "win32" ? "tua.exe" : "tua");
      assert(fs.statSync(server, { throwIfNoEntry: false })?.isFile(), `bundled Tua server missing: ${server}`);
    }
    await runTests({
      extensionDevelopmentPath: extension,
      extensionTestsPath: path.join(__dirname, "suite/sidebar-filesystem.js"),
      extensionTestsEnv: {
        ...(vsix ? {} : { TUA_SERVER_PATH: server }),
        TUA_SIDEBAR_CDP_PORT: String(port),
        TUA_SIDEBAR_EVIDENCE: process.env.TUA_SIDEBAR_EVIDENCE || path.join(temp, "sidebar-evidence.json"),
      },
      launchArgs: [project, "--disable-extensions", "--disable-workspace-trust",
        "--skip-welcome", "--skip-release-notes", `--remote-debugging-port=${port}`,
        `--user-data-dir=${path.join(temp, "user")}`,
        `--extensions-dir=${path.join(temp, "extensions")}`],
      reuseMachineInstall: true,
      vscodeExecutablePath,
    });
  } finally {
    if (process.env.TUA_TEST_KEEP_TEMP === "1") {
      console.log(`kept sidebar VS Code test data at ${temp}`);
    } else {
      fs.rmSync(temp, { recursive: true, force: true });
    }
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
