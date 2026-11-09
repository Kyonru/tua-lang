"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { runTests, downloadAndUnzipVSCode } = require("@vscode/test-electron");
const { spawnSync } = require("child_process");
const { localVSCodeExecutable, localVSCodeCliArgs } = require("../vscode-executable");

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
      const [cli, ...cliArgs] = localVSCodeCliArgs(vscodeExecutablePath);
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
  const contractsPath = path.join(sourcePath, "contracts.tua");
  const actorsPath = path.join(sourcePath, "actors.tua");
  const genericPath = path.join(sourcePath, "generic.tua");
  const genericClassPath = path.join(sourcePath, "generic_class.tua");
  const protocolStubsPath = path.join(sourcePath, "protocol_stubs.tua");
  const cyclicProtocolAPath = path.join(sourcePath, "cyclic_a.d.tua");
  const cyclicProtocolBPath = path.join(sourcePath, "cyclic_b.d.tua");
  const cyclicProtocolValuePath = path.join(sourcePath, "cyclic_value.tua");
  const cyclicProtocolUsePath = path.join(sourcePath, "cyclic_protocol_use.tua");
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
    contractsPath,
    [
      "export type Box<T> = { value: T }",
      "",
      "export protocol Updatable {",
      "  --- advances the actor by one frame",
      "  update(dt: number): nil",
      "  draw(): nil",
      "}",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    actorsPath,
    [
      'import type { Box, Updatable } from "./contracts"',
      "final class Particle implements Updatable {",
      "  update(dt: number): nil {}",
      "  draw(): nil {}",
      "}",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    genericPath,
    [
      "local function identity<T>(value: T): T",
      "  return value",
      "end",
      "",
      "return { identity = identity }",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    genericClassPath,
    [
      "open class Box<T> {",
      "  value: T",
      "",
      "  init(value: T) {",
      "    self.value = value",
      "  }",
      "",
      "  get(): T {",
      "    return self.value",
      "  }",
      "}",
      "",
      "final class NumberBox extends Box<number> {",
      "  init(value: number) {",
      "    super(value)",
      "  }",
      "}",
      "",
      "export type BoxInstance<T> = Box<T>",
      "export type NumberBoxInstance = NumberBox",
      "return { Box = Box, NumberBox = NumberBox }",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    protocolStubsPath,
    [
      'import type { Updatable } from "./contracts"',
      "open class Base {",
      "  update(dt: number): nil {}",
      "}",
      "final class Actor extends Base implements Updatable {",
      "}",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    cyclicProtocolAPath,
    [
      'import type { B } from "./cyclic_b"',
      "export protocol A {",
      "  f(value: B): nil",
      "  next(): B",
      "}",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    cyclicProtocolBPath,
    [
      'import type { A } from "./cyclic_a"',
      "export protocol B {",
      "  --- accepts the paired A protocol",
      "  g(value: A): nil",
      "  previous(): A",
      "}",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    cyclicProtocolValuePath,
    [
      'import type { A } from "./cyclic_a"',
      'import type { B } from "./cyclic_b"',
      "final class Value implements A, B {",
      "  f(value: B): nil {}",
      "  next(): B { return self }",
      "  g(value: A): nil {}",
      "  previous(): A { return self }",
      "}",
      "return { Value = Value }",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    cyclicProtocolUsePath,
    [
      'import type { B } from "./cyclic_b"',
      'import type { A } from "./cyclic_a"',
      'local ValueModule = require("cyclic_value")',
      "local a: A = ValueModule.Value.new()",
      "local target: B = a:next()",
      "local returned_a: A = target:previous()",
      "local function consume(value: B, paired: A)",
      "  value:g(paired)",
      "end",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    mainPath,
    [
      'import type { Box, Updatable } from "./contracts"',
      'import type { BoxInstance, NumberBoxInstance } from "./generic_class"',
      'local Generic = require("generic")',
      'local BoxModule = require("generic_class")',
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
      "local score: Box<number> = { value = 4 }",
      "local score_value = score.value",
      'local generic_text = Generic.identity("ready")',
      "local imported_box = BoxModule.Box.new(42)",
      "local typed_box: BoxInstance<number> = imported_box",
      "local specialized_box = BoxModule.NumberBox.new(42)",
      "local typed_specialized_box: NumberBoxInstance = specialized_box",
      "local imported_box_value: number = typed_box.value",
      "local imported_box_method: number = typed_box:get()",
      "local specialized_parent_value: number = typed_specialized_box.value",
      "local specialized_parent_method: number = typed_specialized_box:get()",
      "",
      "local function tick(value: Updatable, dt: number)",
      "  value:update(dt)",
      "end",
      "",
      "return {",
      "  current_health = current_health,",
      "  uppercase_name = uppercase_name,",
      "  greeting = greeting,",
      "  screen_width = screen_width,",
      "  generic_text = generic_text,",
      "  imported_box_value = imported_box_value,",
      "  imported_box_method = imported_box_method,",
      "}",
      "",
    ].join("\n"),
  );

  return {
    actorsPath,
    configPath,
    cyclicProtocolAPath,
    cyclicProtocolBPath,
    cyclicProtocolValuePath,
    cyclicProtocolUsePath,
    contractsPath,
    genericPath,
    genericClassPath,
    helperPath,
    mainPath,
    protocolStubsPath,
    serverPath: process.env.TUA_REAL_SERVER_PATH,
  };
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
