"use strict";

const assert = require("assert/strict");
const fs = require("fs");
const Module = require("module");
const os = require("os");
const path = require("path");
const test = require("node:test");
const { fileURLToPath, pathToFileURL } = require("node:url");

class EventEmitter {
  constructor() {
    this.listeners = new Set();
    this.event = (listener) => {
      this.listeners.add(listener);
      return { dispose: () => this.listeners.delete(listener) };
    };
  }

  fire(value) {
    for (const listener of this.listeners) {
      listener(value);
    }
  }

  dispose() {
    this.listeners.clear();
  }
}

const vscodeMock = {
  EventEmitter,
  Disposable: { from: (...items) => ({ dispose: () => items.forEach((item) => item.dispose?.()) }) },
  commands: { registerCommand: () => ({ dispose() {} }) },
  workspace: { getConfiguration: () => ({ get: (_key, fallback) => fallback }) },
  window: {},
};

const originalLoad = Module._load;
Module._load = function load(request, parent, isMain) {
  if (request === "vscode") {
    return vscodeMock;
  }
  return originalLoad.call(this, request, parent, isMain);
};
const { PACKAGE_TARGETS, TuaCommandTerminal, notifyPackageResult, packagedArtifactFromLine, packageProjectLayout, packageTargetDetail, registerGameCommands, validLoveRuntime } = require("../../src/love/game-commands");
Module._load = originalLoad;

test("unit testing refuses to execute code in an untrusted workspace", async () => {
  const commands = new Map();
  const warnings = [];
  const previousRegister = vscodeMock.commands.registerCommand;
  vscodeMock.commands.registerCommand = (name, callback) => {
    commands.set(name, callback);
    return { dispose() {} };
  };
  vscodeMock.workspace.isTrusted = false;
  vscodeMock.window.showWarningMessage = (message) => warnings.push(message);
  try {
    registerGameCommands({ getServerCommand: () => { throw new Error("must not run"); } });
    await commands.get("tua.testProject")();
    assert.match(warnings[0], /Trust this workspace/);
  } finally {
    vscodeMock.commands.registerCommand = previousRegister;
    delete vscodeMock.workspace.isTrusted;
    delete vscodeMock.window.showWarningMessage;
  }
});

function runTerminal(script, outputDirectory) {
  const lines = [];
  const terminal = new TuaCommandTerminal({
    cwd: os.tmpdir(),
    outputDirectory,
    tuaArgs: ["-e", script],
    tuaCommand: process.execPath,
  });
  terminal.onDidWrite((text) => lines.push(text.replace(/\r\n$/u, "")));
  return new Promise((resolve) => {
    terminal.onDidClose((code) => resolve({ code, lines }));
    terminal.open();
  });
}

test("the task terminal streams the tua command and reports its exit code", async () => {
  const { code, lines } = await runTerminal(
    "console.log('rebuild: rebuilt 1 file(s)'); process.exitCode = 3",
    os.tmpdir(),
  );

  assert.equal(code, 3);
  assert(lines[0].startsWith("$ "), "the command line is echoed first");
  assert(lines.includes("rebuild: rebuilt 1 file(s)"));
});

test("source maps load once the dev game has started", async () => {
  const missing = path.join(os.tmpdir(), `tua-no-build-${process.pid}-${Date.now()}`);
  const { lines } = await runTerminal("console.log('dev: started love build')", missing);

  assert(lines.includes("dev: started love build"));
  assert(
    lines.some((line) => line.includes("source maps") || line.includes("Could not load source maps")),
    lines.join("\n"),
  );
});

test("fused package targets name the setting that holds their LOVE runtime", () => {
  assert.deepEqual(
    PACKAGE_TARGETS.map((target) => [target.target, target.setting]),
    [
      ["love", undefined],
      ["windows", "love.windowsRuntime"],
      ["macos", "love.macosApp"],
      ["web", "love.webPlayer"],
    ],
  );
});

test("package picker explains the selectable platforms and their outputs", () => {
  assert.deepEqual(PACKAGE_TARGETS.map(item => item.target), ["love", "windows", "macos", "web"]);
  assert.match(PACKAGE_TARGETS[0].description, /Linux, Windows, or macOS/u);
  assert.match(packageTargetDetail(PACKAGE_TARGETS[0]), /\.love archive in dist/u);
  assert.match(packageTargetDetail(PACKAGE_TARGETS[1]), /dist\/windows/u);
  assert.match(packageTargetDetail(PACKAGE_TARGETS[2]), /dist\/macos/u);
  assert.match(packageTargetDetail(PACKAGE_TARGETS[3]), /dist\/web/u);
});

test("runtime selection accepts the expected LÖVE bundle for each fused target", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-package-runtime-test-"));
  try {
    const windows = path.join(root, "love-win64");
    const macos = path.join(root, "love.app");
    const web = path.join(root, "love-web");
    fs.mkdirSync(windows);
    fs.mkdirSync(path.join(macos, "Contents"), { recursive: true });
    fs.writeFileSync(path.join(windows, "love.exe"), "");
    fs.writeFileSync(path.join(macos, "Contents/Info.plist"), "");
    for (const file of ["player.js", "style.css", "license.txt", "11.5/love.js", "11.5/love.wasm", "11.5/license.txt", "lua/normalize1.lua", "lua/normalize2.lua"]) {
      const target = path.join(web, file);
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.writeFileSync(target, "");
    }
    assert(validLoveRuntime(PACKAGE_TARGETS[1], windows));
    assert(!validLoveRuntime(PACKAGE_TARGETS[1], macos));
    assert(validLoveRuntime(PACKAGE_TARGETS[2], macos));
    assert(!validLoveRuntime(PACKAGE_TARGETS[2], windows));
    assert(validLoveRuntime(PACKAGE_TARGETS[3], web));
    fs.rmSync(path.join(web, "11.5/love.wasm"));
    assert(!validLoveRuntime(PACKAGE_TARGETS[3], web));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("only package outputs inside dist become revealable artifacts", () => {
  const project = path.join(os.tmpdir(), "tua-game-project");
  assert.equal(packagedArtifactFromLine("packaged dist/My Game.love (184.2 KiB)", project), path.join(project, "dist/My Game.love"));
  assert.equal(packagedArtifactFromLine("packaged dist/windows/My Game", project), path.join(project, "dist/windows/My Game"));
  assert.equal(packagedArtifactFromLine("packaged ../elsewhere.love", project), undefined);
  assert.equal(packagedArtifactFromLine("packaged dist", project), undefined);
  assert.equal(packagedArtifactFromLine("built 2 file(s)", project), undefined);
});

test("package task reports the artifact printed by the CLI after exit", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-package-task-test-"));
  try {
    const result = new Promise(resolve => {
      const terminal = new TuaCommandTerminal({ cwd: root, outputDirectory: path.join(root, "build"),
        tuaCommand: "tua", tuaArgs: ["package", "--target", "love"],
        onComplete: (code, artifacts) => resolve({ code, artifacts }), });
      terminal.runProcess = async () => {
        fs.mkdirSync(path.join(root, "dist"));
        fs.writeFileSync(path.join(root, "dist", "demo.love"), "test");
        terminal.writeProcessLine("packaged dist/demo.love (4 B)", "stdout");
        return 0;
      };
      void terminal.run();
    });
    const completed = await result;
    assert.equal(completed.code, 0);
    assert.deepEqual(completed.artifacts, [path.join(root, "dist", "demo.love")]);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("successful package feedback reveals or copies the actual output; failure does not", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-package-result-test-"));
  const artifact = path.join(root, "dist", "game.love");
  fs.mkdirSync(path.dirname(artifact));
  fs.writeFileSync(artifact, "sample");
  const messages = [], commands = [], copied = [];
  vscodeMock.window.showInformationMessage = async (message) => { messages.push(message); return "Reveal Package"; };
  vscodeMock.window.showErrorMessage = message => { messages.push(message); };
  vscodeMock.window.showWarningMessage = message => { messages.push(message); };
  vscodeMock.commands.executeCommand = async (...args) => { commands.push(args); };
  vscodeMock.Uri = { file: fsPath => ({ fsPath }) };
  vscodeMock.env = { clipboard: { writeText: async value => { copied.push(value); } } };
  try {
    await notifyPackageResult(0, [artifact], root);
    assert.match(messages[0], /Packaged dist[/\\]game\.love/u);
    assert.deepEqual(commands[0], ["revealFileInOS", { fsPath: artifact }]);
    vscodeMock.window.showInformationMessage = async () => "Copy Path";
    await notifyPackageResult(0, [artifact], root);
    assert.deepEqual(copied, [artifact]);
    await notifyPackageResult(1, [artifact]);
    assert.match(messages.at(-1), /Packaging failed/u);
    await notifyPackageResult(0, []);
    assert.match(messages.at(-1), /could not be located/u);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    delete vscodeMock.window.showInformationMessage;
    delete vscodeMock.window.showErrorMessage;
    delete vscodeMock.window.showWarningMessage;
    delete vscodeMock.commands.executeCommand;
    delete vscodeMock.Uri;
    delete vscodeMock.env;
  }
});

test("package command resolves the project before offering target platforms", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-package-picker-test-"));
  const commands = new Map();
  const saved = {
    registerCommand: vscodeMock.commands.registerCommand,
    workspace: { ...vscodeMock.workspace },
    window: { ...vscodeMock.window },
    Uri: vscodeMock.Uri, Task: vscodeMock.Task, CustomExecution: vscodeMock.CustomExecution,
    TaskScope: vscodeMock.TaskScope, tasks: vscodeMock.tasks,
  };
  let selectedTask, requestedLayout = false, offeredPlatforms = false;
  const asUri = file => pathToFileURL(file).toString();
  try {
    vscodeMock.commands.registerCommand = (name, callback) => { commands.set(name, callback); return { dispose() {} }; };
    Object.assign(vscodeMock.workspace, {
      isTrusted: true,
      workspaceFolders: [{ uri: { scheme: "file", fsPath: root } }],
    });
    Object.assign(vscodeMock.window, {
      activeTextEditor: undefined,
    });
    vscodeMock.Uri = { file: file => ({ toString: () => asUri(file) }), parse: uri => ({ fsPath: fileURLToPath(uri) }) };
    vscodeMock.Task = class { constructor(definition, _scope, name, _source, execution) { this.definition = definition; this.name = name; this.execution = execution; } };
    vscodeMock.CustomExecution = class { constructor(factory) { this.factory = factory; } };
    vscodeMock.TaskScope = { Workspace: 1 };
    vscodeMock.tasks = { executeTask: async task => { selectedTask = task; } };
    const client = { sendRequest: async (method) => {
      assert.equal(method, "tua/projectLayout");
      requestedLayout = true;
      return { projectUri: asUri(root), sourceUri: asUri(path.join(root, "src")), outputUri: asUri(path.join(root, "build")), workspaceUri: asUri(root) };
    } };
    registerGameCommands({ getClient: () => client, getServerCommand: () => process.execPath });
    // A selected fused target asks for its runtime; use the portable target in
    // this task check to keep the test independent of platform installations.
    vscodeMock.window.showQuickPick = async (items, picker) => {
      assert(requestedLayout, "project resolution comes before target selection");
      assert.match(picker.title, /target platform/u);
      assert.deepEqual(items.map(item => item.target), ["love", "windows", "macos", "web"]);
      offeredPlatforms = true;
      return items[0];
    };
    const result = await commands.get("tua.packageGame")();
    assert(offeredPlatforms);
    assert.deepEqual(result.args, ["package", "--target", "love"]);
    assert.equal(selectedTask.definition.projectRoot, root);
    assert.match(selectedTask.name, /Any desktop/u);
    const windows = path.join(root, "love-win64");
    fs.mkdirSync(windows);
    fs.writeFileSync(path.join(windows, "love.exe"), "");
    vscodeMock.workspace.getConfiguration = () => ({ get: (key, fallback) => key === "love.windowsRuntime" ? windows : fallback });
    vscodeMock.window.showQuickPick = async items => items[1];
    const windowsResult = await commands.get("tua.packageGame")();
    assert.deepEqual(windowsResult.args, ["package", "--target", "windows", "--love", windows]);
    const web = path.join(root, "love-web");
    for (const file of ["player.js", "style.css", "license.txt", "11.5/love.js", "11.5/love.wasm", "11.5/license.txt", "lua/normalize1.lua", "lua/normalize2.lua"]) {
      const target = path.join(web, file);
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.writeFileSync(target, "");
    }
    vscodeMock.workspace.getConfiguration = () => ({ get: (key, fallback) => key === "love.webPlayer" ? web : fallback });
    const webResult = await commands.get("tua.packageGame")({ target: "web" });
    assert.deepEqual(webResult.args, ["package", "--target", "web", "--web-player", web]);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    vscodeMock.commands.registerCommand = saved.registerCommand;
    Object.assign(vscodeMock.workspace, saved.workspace);
    Object.assign(vscodeMock.window, saved.window);
    for (const key of Object.keys(vscodeMock.workspace)) if (!(key in saved.workspace)) delete vscodeMock.workspace[key];
    for (const key of Object.keys(vscodeMock.window)) if (!(key in saved.window)) delete vscodeMock.window[key];
    vscodeMock.Uri = saved.Uri;
    vscodeMock.Task = saved.Task;
    vscodeMock.CustomExecution = saved.CustomExecution;
    vscodeMock.TaskScope = saved.TaskScope;
    vscodeMock.tasks = saved.tasks;
  }
});

test("multi-root packaging asks which Tua project to build when no file is active", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-package-multiroot-test-"));
  const projects = [path.join(root, "one"), path.join(root, "two")];
  for (const project of projects) fs.mkdirSync(project);
  const previousFolders = vscodeMock.workspace.workspaceFolders;
  const previousEditor = vscodeMock.window.activeTextEditor;
  const previousPicker = vscodeMock.window.showQuickPick;
  const previousUri = vscodeMock.Uri;
  const asUri = file => pathToFileURL(file).toString();
  try {
    vscodeMock.workspace.workspaceFolders = projects.map(project => ({ uri: { scheme: "file", fsPath: project } }));
    vscodeMock.window.activeTextEditor = undefined;
    vscodeMock.Uri = { file: file => ({ toString: () => asUri(file) }), parse: uri => ({ fsPath: fileURLToPath(uri) }) };
    vscodeMock.window.showQuickPick = async (items, picker) => {
      assert.match(picker.placeHolder, /Choose the Tua project/u);
      assert.deepEqual(items.map(item => item.label), ["one", "two"]);
      return items[1];
    };
    const client = { sendRequest: async (_method, params) => {
      const project = projects.find(item => params.uri.includes(path.basename(item)));
      return { projectUri: asUri(project), sourceUri: asUri(path.join(project, "src")), outputUri: asUri(path.join(project, "build")), workspaceUri: asUri(project) };
    } };
    const selected = await packageProjectLayout(() => client);
    assert.equal(selected.configDirectory, projects[1]);
    await assert.rejects(
      packageProjectLayout(() => ({ sendRequest: async () => { throw new Error("server unavailable"); } })),
      /server unavailable/u,
    );
  } finally {
    vscodeMock.workspace.workspaceFolders = previousFolders;
    vscodeMock.window.activeTextEditor = previousEditor;
    vscodeMock.window.showQuickPick = previousPicker;
    vscodeMock.Uri = previousUri;
    fs.rmSync(root, { recursive: true, force: true });
  }
});
