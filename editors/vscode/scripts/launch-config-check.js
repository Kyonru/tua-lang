"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const esbuild = require("esbuild");

const extensionRoot = path.resolve(__dirname, "..");
const repositoryRoot = path.resolve(extensionRoot, "..", "..");
const launchPath = path.join(repositoryRoot, ".vscode", "launch.json");
const tasksPath = path.join(repositoryRoot, ".vscode", "tasks.json");
const packagePath = path.join(extensionRoot, "package.json");

const launch = readJson(launchPath);
const tasks = readJson(tasksPath);
const manifest = readJson(packagePath);
const taskByLabel = new Map(tasks.tasks.map((task) => [task.label, task]));
const extensionLaunches = launch.configurations.filter(
  (configuration) => configuration.type === "extensionHost",
);
const debugBuildTask = taskByLabel.get("Build Extension (Debug)");

assert(extensionLaunches.length > 0, "launch.json must define an Extension Host profile");
assert(debugBuildTask, "tasks.json must define Build Extension (Debug)");
assert.equal(debugBuildTask.type, "npm");
assert.equal(debugBuildTask.script, "build:dev");
assert.equal(debugBuildTask.path, "editors/vscode/");
assert.equal(
  manifest.scripts?.["build:dev"],
  "esbuild src/extension.js --bundle --platform=node --format=cjs --external:vscode --sourcemap --sources-content=true --outfile=dist/extension.js",
  "build:dev must produce a debuggable sourcemapped bundle",
);

for (const task of tasks.tasks) {
  for (const dependency of dependenciesOf(task)) {
    assert(taskByLabel.has(dependency), `${task.label} references missing task ${dependency}`);
  }
}

for (const configuration of extensionLaunches) {
  assert.deepEqual(
    configuration.outFiles,
    ["${workspaceFolder}/editors/vscode/dist/**/*.js"],
    `${configuration.name} must debug the bundle that VS Code executes`,
  );
  assert(
    configuration.preLaunchTask,
    `${configuration.name} must build the extension before launch`,
  );
  assert(
    taskDependsOn(configuration.preLaunchTask, "Build Extension (Debug)", new Set()),
    `${configuration.name} must depend on Build Extension (Debug)`,
  );

  const serverPath = configuration.env?.TUA_SERVER_PATH;
  if (serverPath) {
    assert.equal(
      configuration.windows?.env?.TUA_SERVER_PATH,
      `${serverPath}.exe`,
      `${configuration.name} must use the Windows server executable`,
    );
  }
}

smokeBuild();

function taskDependsOn(label, expected, visited) {
  if (label === expected) {
    return true;
  }
  if (visited.has(label)) {
    return false;
  }
  visited.add(label);
  const task = taskByLabel.get(label);
  assert(task, `launch.json references missing task ${label}`);
  return dependenciesOf(task).some((dependency) =>
    taskDependsOn(dependency, expected, visited),
  );
}

function dependenciesOf(task) {
  if (Array.isArray(task.dependsOn)) {
    return task.dependsOn;
  }
  return task.dependsOn ? [task.dependsOn] : [];
}

function smokeBuild() {
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "tua-extension-launch-"));
  const outfile = path.join(scratch, "extension.js");
  try {
    esbuild.buildSync({
      absWorkingDir: extensionRoot,
      bundle: true,
      entryPoints: ["src/extension.js"],
      external: ["vscode"],
      format: "cjs",
      minify: false,
      outfile,
      platform: "node",
      sourcemap: true,
      sourcesContent: true,
    });

    assert(fs.statSync(outfile).size > 0, "development bundle must not be empty");
    const map = readJson(`${outfile}.map`);
    assert(
      map.sources.some((source) => source.replaceAll("\\", "/").endsWith("/src/extension.js")),
      "development source map must include src/extension.js",
    );
    assert(
      map.sourcesContent?.some((source) => source?.includes("async function activate")),
      "development source map must embed extension sources",
    );
  } finally {
    fs.rmSync(scratch, { force: true, recursive: true });
  }
}

function readJson(filename) {
  return JSON.parse(fs.readFileSync(filename, "utf8"));
}
