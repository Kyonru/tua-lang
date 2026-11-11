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
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "tua-starfall-latency-"));
  const baselineVsix = process.env.TUA_STARFALL_BASELINE_VSIX;
  const testedExtension = baselineVsix ? path.join(temp, "unpacked", "extension") : extension;
  if (baselineVsix) {
    fs.mkdirSync(path.join(temp, "unpacked"));
    execFileSync("unzip", ["-q", path.resolve(baselineVsix), "-d", path.join(temp, "unpacked")]);
  }
  const project = path.join(temp, "starfall");
  fs.cpSync(path.join(root, "examples/starfall"), project, { recursive: true });
  fs.mkdirSync(path.join(project, ".vscode"), { recursive: true });
  fs.writeFileSync(path.join(project, ".vscode/settings.json"), JSON.stringify({
    "editor.quickSuggestions": { other: true, comments: false, strings: false },
    "editor.quickSuggestionsDelay": 10,
    "editor.suggestOnTriggerCharacters": true,
    "tua.completion.inline.enabled": false,
  }));
  const server = net.createServer();
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  const output = process.env.TUA_STARFALL_OUT || path.join(__dirname,
    baselineVsix ? "starfall-visible-baseline.json" : "starfall-visible.json");
  try {
    await fromExtension("@vscode/test-electron").runTests({
      extensionDevelopmentPath: testedExtension,
      extensionTestsPath: path.join(__dirname, "starfall-visible-suite.js"),
      extensionTestsEnv: {
        TUA_STARFALL_PORT: String(port),
        TUA_STARFALL_OUT: output,
        TUA_COMPLETION_TRACE: "1",
        TUA_COMPLETION_TRACE_OUTPUT: path.join(temp, "completion-trace.jsonl"),
        TUA_STARFALL_BASELINE: baselineVsix ? "1" : "0",
        TUA_TEST_REVISION: baselineVsix ? "previous packaged 0.7.0" : "current",
        ...(!baselineVsix ? { TUA_SERVER_PATH: process.env.TUA_TEST_SERVER || path.join(root, "target/release/tua") } : {}),
      },
      launchArgs: [project, "--disable-extensions", "--disable-workspace-trust",
        "--skip-welcome", "--skip-release-notes", `--remote-debugging-port=${port}`,
        `--user-data-dir=${path.join(temp, "user")}`, `--extensions-dir=${path.join(temp, "extensions")}`],
      reuseMachineInstall: true,
      vscodeExecutablePath: require(path.join(extension, "test/vscode-executable")).localVSCodeExecutable(),
    });
    console.log(fs.readFileSync(output, "utf8"));
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

main().catch(error => { console.error(error); process.exitCode = 1; });
