"use strict";

const fs = require("node:fs");
const net = require("node:net");
const os = require("node:os");
const path = require("node:path");
const { createRequire } = require("node:module");

const root = path.resolve(__dirname, "../../..");
const extension = path.join(root, "editors/vscode");
const fromExtension = createRequire(path.join(extension, "package.json"));
const { runTests } = fromExtension("@vscode/test-electron");
const { localVSCodeExecutable } = require(path.join(extension, "test/vscode-executable"));

async function main() {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "tua-visible-latency-"));
  const workspace = path.join(temp, "project");
  fs.mkdirSync(path.join(workspace, "src"), { recursive: true });
  fs.mkdirSync(path.join(workspace, "src/classes"), { recursive: true });
  fs.mkdirSync(path.join(workspace, ".vscode"), { recursive: true });
  fs.writeFileSync(path.join(workspace, "tua.toml"), 'source = "src"\nout = "build"\n');
  fs.writeFileSync(path.join(workspace, "src/components.tua"),
    'return { health = 10, name = "enemy", attack = function(amount: number): number return amount end }\n');
  fs.writeFileSync(path.join(workspace, "src/main.tua"), 'local item = require "components"\nitem\n');
  fs.writeFileSync(path.join(workspace, "src/main_alt.tua"), 'local item = require "components"\nitem\n');
  fs.writeFileSync(path.join(workspace, "src/control.js"), 'const item = { health: 10, name: "enemy" };\nitem\n');
  fs.writeFileSync(path.join(workspace, "src/format.tua"), 'local value = 0\nprint(value)\n');
  fs.writeFileSync(path.join(workspace, "src/format_large.tua"),
    Array.from({ length: 1000 }, (_, index) => `local value_${index} = ${index}\n`).join("") + "print(value_999)\n");
  fs.writeFileSync(path.join(workspace, "src/classes/agent.tua"),
    'final class Agent {\n  health: number = 100\n  name: string\n  init(name: string) { self.name = name }\n  takeDamage(amount: number): number { self.health -= amount; return self.health }\n}\nexport type AgentInstance = Agent\nreturn { Agent = Agent }\n');
  fs.writeFileSync(path.join(workspace, "src/factory.tua"),
    'import type { AgentInstance } from "./classes/agent"\nlocal Agent = require("classes.agent").Agent\nlocal function spawn(name: string): AgentInstance\n  return Agent.new(name)\nend\nreturn { spawn = spawn }\n');
  fs.writeFileSync(path.join(workspace, "src/service.tua"),
    'local factory = require "factory"\nlocal function current()\n  return factory.spawn("pilot")\nend\nreturn { current = current }\n');
  fs.writeFileSync(path.join(workspace, "src/cross_file.tua"),
    'local factory = require "factory"\nlocal agent = factory.spawn("pilot")\nprint(agent.health)\nprint(agent)\n');
  fs.writeFileSync(path.join(workspace, "src/cross_file_deep.tua"),
    'local service = require "service"\nlocal agent = service.current()\nprint(agent.health)\nprint(agent)\n');
  fs.writeFileSync(path.join(workspace, "src/service_annotated.tua"),
    'import type { AgentInstance } from "./classes/agent"\nlocal factory = require "factory"\nlocal function current(): AgentInstance\n  return factory.spawn("pilot")\nend\nreturn { current = current }\n');
  fs.writeFileSync(path.join(workspace, "src/cross_file_annotated.tua"),
    'local service = require "service_annotated"\nlocal agent = service.current()\nprint(agent.health)\nprint(agent)\n');
  for (let index = 0; index < 100; index++) {
    fs.writeFileSync(path.join(workspace, "src", `module_${index}.tua`), `return { value = ${index} }\n`);
  }
  fs.writeFileSync(path.join(workspace, ".vscode/settings.json"), JSON.stringify({
    "editor.quickSuggestions": { other: true, comments: false, strings: false },
    "editor.suggestOnTriggerCharacters": true,
    "editor.formatOnSave": true,
    "editor.formatOnSaveMode": "file",
    "tua.completion.inline.enabled": false,
  }));
  const portServer = net.createServer();
  await new Promise(resolve => portServer.listen(0, "127.0.0.1", resolve));
  const port = portServer.address().port;
  await new Promise(resolve => portServer.close(resolve));
  const output = path.join(__dirname, "visible-latency.json");
  try {
    await runTests({
      extensionDevelopmentPath: extension,
      extensionTestsPath: path.join(__dirname, "visible-latency-suite.js"),
      extensionTestsEnv: {
        TUA_VISIBLE_PORT: String(port),
        TUA_VISIBLE_OUT: output,
        TUA_SERVER_PATH: path.join(root, "target/release/tua"),
      },
      launchArgs: [workspace, "--disable-extensions", "--disable-workspace-trust",
        "--skip-welcome", "--skip-release-notes",
        `--remote-debugging-port=${port}`,
        `--user-data-dir=${path.join(temp, "user")}`,
        `--extensions-dir=${path.join(temp, "extensions")}`],
      reuseMachineInstall: true,
      vscodeExecutablePath: localVSCodeExecutable(),
    });
    console.log(fs.readFileSync(output, "utf8"));
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

main().catch(error => { console.error(error); process.exitCode = 1; });
