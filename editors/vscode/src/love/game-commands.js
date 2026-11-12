"use strict";

const fs = require("fs");
const path = require("path");
const { execFile } = require("child_process");
const vscode = require("vscode");
const { isPathInside, projectLayoutForWorkspaceRoot } = require("../editor/project-layout");
const { normalizeInstances } = require("../workspace/run-options");
const { HotReloadInstancesTerminal } = require("./hot-reload-instances");
const { COMMANDS } = require("../constants");
const {
  LoveProjectTerminal,
  activeProjectLayout,
  configuredLoveCommand,
  projectSelectionMessage,
} = require("./run-project");

const TASK_TYPE = "tua";
const HOT_RELOAD_TASK_NAME = "Run LOVE Project With Hot Reload";
const PACKAGE_TASK_NAME = "Package Game";

const PACKAGE_TARGETS = [
  {
    target: "love",
    label: "Any desktop (.love)",
    description: "Linux, Windows, or macOS · requires LÖVE to play",
  },
  {
    target: "windows",
    label: "Windows app (.exe)",
    description: "Includes LÖVE for Windows; share the resulting folder as a zip",
    setting: "love.windowsRuntime",
    pick: { canSelectFolders: true, canSelectFiles: false, title: "Select the extracted LOVE for Windows folder" },
  },
  {
    target: "macos",
    label: "macOS app (.app)",
    description: "Includes love.app; sign and notarize before distribution",
    setting: "love.macosApp",
    pick: { canSelectFolders: true, canSelectFiles: true, title: "Select love.app" },
  },
  {
    target: "web",
    label: "Web (HTML5)",
    description: "Static browser build · needs a LÖVE 11.5 web player and hosting headers",
    setting: "love.webPlayer",
    pick: { canSelectFolders: true, canSelectFiles: false, title: "Select the LÖVE 11.5 standalone web player folder" },
  },
];

// Run one `tua` command in a task terminal, with LOVE traceback remapping
// once the build output exists.
class TuaCommandTerminal extends LoveProjectTerminal {
  constructor(options) {
    super(options);
    this.tuaArgs = options.tuaArgs;
    this.onComplete = options.onComplete;
    this.packageArtifacts = [];
  }

  async run() {
    const code = await this.runProcess(this.tuaCommand, this.tuaArgs);
    if (!this.finished) {
      this.finish(code);
      if (this.onComplete) {
        void Promise.resolve().then(() => this.onComplete(code, this.packageArtifacts)).catch(error =>
          vscode.window.showErrorMessage(`Could not show the package result: ${error.message}`));
      }
    }
  }

  writeProcessLine(line, stream) {
    if (this.tuaArgs[0] === "package" && stream !== "stderr") {
      const artifact = packagedArtifactFromLine(line, this.cwd);
      if (artifact) this.packageArtifacts.push(artifact);
    }
    if (line.startsWith("dev: started") || line.startsWith("dev: sent reload")) {
      this.sourceMaps = this.loadSourceMaps();
    }
    super.writeProcessLine(line, stream);
  }
}

function registerGameCommands({ appendOutputLine, getClient, getServerCommand, activity, inspector, listPackageTargets = listCustomPackageTargets }) {
  return vscode.Disposable.from(
    vscode.commands.registerCommand("tua.testProject", () => runTuaTask({
      appendOutputLine, activity, getClient, getServerCommand,
      action: "testing", taskName: "Run Unit Tests", args: () => ["test"],
    })),
    vscode.commands.registerCommand("tua.buildProject", () => runTuaTask({
      appendOutputLine, activity, getClient, getServerCommand,
      action: "building", taskName: "Build Project", args: () => ["build"],
    })),
    vscode.commands.registerCommand(COMMANDS.runLoveProjectWithHotReload, presentation =>
      runTuaTask({
        appendOutputLine,
        activity,
        inspector,
        getClient,
        getServerCommand,
        showConsole: presentation?.console === true,
        action: "running",
        taskName: presentation?.title || HOT_RELOAD_TASK_NAME,
        instances: presentation?.instances,
        configurationName: presentation?.configurationName,
        args: () => presentation?.instances === undefined ? ["dev", "--love", configuredLoveCommand()] : ["dev", "--no-launch"],
      }),
    ),
    vscode.commands.registerCommand(COMMANDS.packageGame, async (options) => {
      if (!vscode.workspace.isTrusted) {
        vscode.window.showWarningMessage("Trust this workspace before packaging a game.");
        return undefined;
      }
      if (!vscode.tasks?.executeTask) {
        vscode.window.showErrorMessage("This VS Code version does not support task execution.");
        return undefined;
      }
      let layout;
      try { layout = await packageProjectLayout(getClient); }
      catch (error) { vscode.window.showErrorMessage(`Could not find the Tua project: ${error.message}`); return undefined; }
      if (!layout) {
        vscode.window.showWarningMessage("No Tua project was found. Open a workspace containing tua.toml before packaging.");
        return undefined;
      }
      let customTargets = [];
      try { customTargets = await listPackageTargets(layout, getServerCommand?.() || "tua"); }
      catch (error) {
        if (options?.target?.startsWith("custom:")) {
          vscode.window.showErrorMessage(`Could not load custom package targets: ${error.message}`);
          return undefined;
        }
        vscode.window.showWarningMessage(`Custom package targets are unavailable: ${error.message}`);
      }
      const targets = [...PACKAGE_TARGETS, ...customTargets];
      const choice = options?.target ? targets.find(item => item.target === options.target) : await vscode.window.showQuickPick(
        targets.map(item => ({ ...item, detail: packageTargetDetail(item) })),
        { title: `Package ${path.basename(layout.configDirectory)} · target platform`, placeHolder: "Choose a platform or portable archive · output in this project's dist/" },
      );
      if (options?.target && !choice) {
        vscode.window.showErrorMessage(`Unknown package target: ${options.target}`);
        return undefined;
      }
      if (!choice) {
        return undefined;
      }
      const args = ["package", "--target", choice.target];
      if (choice.setting) {
        const runtime = await loveRuntime(choice);
        if (!runtime) {
          return undefined;
        }
        args.push(choice.target === "web" ? "--web-player" : "--love", runtime);
      }
      return runTuaTask({
        appendOutputLine,
        activity,
        getClient,
        getServerCommand,
        showConsole: options?.console === true,
        configurationName: options?.configurationName,
        action: "packaging",
        taskName: `${PACKAGE_TASK_NAME} (${choice.label})`,
        args: () => args,
        layout,
        onComplete: (code, artifacts) => notifyPackageResult(code, artifacts, layout.configDirectory),
      });
    }),
  );
}

async function runTuaTask({ appendOutputLine, getClient, getServerCommand, activity, action, taskName, args, showConsole = false, instances, configurationName, inspector, layout, onComplete }) {
  if (!vscode.workspace.isTrusted) {
    vscode.window.showWarningMessage("Trust this workspace before running Tua tasks.");
    return undefined;
  }
  if (!vscode.tasks?.executeTask) {
    vscode.window.showErrorMessage("This VS Code version does not support task execution.");
    return undefined;
  }
  layout ||= await activeProjectLayout(getClient);
  if (!layout) {
    vscode.window.showWarningMessage(action === "testing"
      ? "Open a file in the Tua project you want to test."
      : projectSelectionMessage(action));
    return undefined;
  }

  try { instances = instances === undefined ? undefined : normalizeInstances(instances); }
  catch (error) { vscode.window.showErrorMessage(error.message); return undefined; }
  const tuaArgs = args();
  // Inspector-enabled single-player dev runs use the same editor-owned watcher
  // as multiplayer so the shim launches only the game, never the compiler.
  if (tuaArgs[0] === "dev" && instances === undefined && inspector?.enabled()) {
    instances = [{ name: "Game", args: [] }];
    tuaArgs.splice(0, tuaArgs.length, "dev", "--no-launch");
  }
  const options = {
    activity,
    inspector,
    instances,
    configurationName,
    kind: tuaArgs[0],
    title: taskName,
    appendOutputLine,
    cwd: layout.configDirectory,
    loveCommand: configuredLoveCommand(),
    outputDirectory: layout.outputDirectory,
    tuaArgs,
    tuaCommand: getServerCommand?.() || "tua",
    onComplete,
  };
  const task = new vscode.Task(
    { type: TASK_TYPE, command: options.tuaArgs[0], projectRoot: layout.configDirectory },
    vscode.TaskScope.Workspace,
    taskName,
    "Tua",
    new vscode.CustomExecution(async () => instances ? new HotReloadInstancesTerminal(options) : new TuaCommandTerminal(options)),
    [],
  );
  task.detail = `tua ${options.tuaArgs.join(" ")} in ${layout.configDirectory}`;
  task.presentationOptions = {
    clear: true,
    focus: false,
    panel: vscode.TaskPanelKind?.Dedicated,
    reveal: showConsole ? vscode.TaskRevealKind?.Never : vscode.TaskRevealKind?.Always,
  };

  await vscode.tasks.executeTask(task);
  return { taskName, args: options.tuaArgs, workspaceRoot: layout.configDirectory };
}

// The configured LOVE runtime for a fused target, or one the user picks now.
async function loveRuntime(choice) {
  const configuration = vscode.workspace.getConfiguration("tua");
  const configured = configuration.get(choice.setting, "");
  const fallback =
    choice.target === "macos" && process.platform === "darwin" ? "/Applications/love.app" : "";
  const candidate = (typeof configured === "string" && configured.trim()) || fallback;
  if (candidate && validLoveRuntime(choice, candidate)) {
    return candidate;
  }
  if (candidate && configured) {
    vscode.window.showWarningMessage(`The configured ${choice.label} LÖVE runtime is unavailable or incomplete. Select another one.`);
  }
  let runtime;
  while (!runtime) {
    const picked = await vscode.window.showOpenDialog({
      ...choice.pick,
      canSelectMany: false,
      openLabel: choice.target === "web" ? "Use This Web Player" : "Use This LÖVE",
    });
    const selected = picked?.[0]?.fsPath;
    if (!selected) return undefined;
    if (validLoveRuntime(choice, selected)) runtime = selected;
    else vscode.window.showWarningMessage(runtimeRequirement(choice));
  }
  const remember = await vscode.window.showInformationMessage(
    `Use ${runtime} for future ${choice.label} packages?`,
    "Remember",
  );
  if (remember === "Remember") {
    await configuration.update(choice.setting, runtime, vscode.ConfigurationTarget.Global);
  }
  return runtime;
}

async function packageProjectLayout(getClient) {
  const active = await activeProjectLayout(getClient);
  if (active) return active;
  const folders = (vscode.workspace.workspaceFolders || []).filter(folder => folder.uri?.scheme === "file");
  if (folders.length < 2) return undefined;
  const results = await Promise.allSettled(folders.map(folder =>
    projectLayoutForWorkspaceRoot(folder.uri.fsPath, getClient),
  ));
  const layouts = results.filter(result => result.status === "fulfilled").map(result => result.value).filter(Boolean);
  if (!layouts.length) {
    const failure = results.find(result => result.status === "rejected");
    if (failure) throw failure.reason;
  }
  if (layouts.length === 1) return layouts[0];
  if (layouts.length < 2) return undefined;
  const picked = await vscode.window.showQuickPick(layouts.map(layout => ({
    label: path.basename(layout.configDirectory), description: layout.configDirectory, layout,
  })), { title: "Package Game", placeHolder: "Choose the Tua project to package" });
  return picked?.layout;
}

function packageTargetDetail(choice) {
  if (choice.target === "windows") return "Creates an app folder in dist/windows/ · requires an extracted Windows LÖVE runtime";
  if (choice.target === "macos") return "Creates an app bundle in dist/macos/ · requires love.app";
  if (choice.target === "web") return "Creates a static site in dist/web/ · requires a standalone LÖVE 11.5 web player";
  if (choice.target.startsWith("custom:")) return `Runs the configured command · output in dist/custom/${choice.target.slice(7)}/`;
  return "Creates a .love archive in dist/ · no extra runtime needed to build";
}

function listCustomPackageTargets(layout, command, run = execFile) {
  return new Promise((resolve, reject) => run(command, ["package", "--list-targets"], {
    cwd: layout.configDirectory, timeout: 5000, maxBuffer: 65536, windowsHide: true,
  }, (error, stdout) => {
    if (error) return reject(error);
    try {
      const data = JSON.parse(stdout);
      if (!Array.isArray(data) || data.length > 64) throw new Error("invalid target list");
      const seen = new Set();
      const targets = data.map(item => {
        if (typeof item?.target !== "string" || !/^custom:[A-Za-z0-9_-]+$/u.test(item.target) || seen.has(item.target)) throw new Error("invalid custom target");
        seen.add(item.target);
        return {
          target: item.target,
          label: typeof item.label === "string" && item.label ? item.label : item.target.slice(7),
          description: "Custom pipeline from tua.toml",
        };
      });
      resolve(targets);
    } catch (failure) { reject(failure); }
  }));
}

function validLoveRuntime(choice, candidate) {
  if (!candidate || !fs.existsSync(candidate)) return false;
  if (choice.target === "windows") return fs.existsSync(path.join(candidate, "love.exe"));
  if (choice.target === "web") return ["player.js", "style.css", "license.txt", "11.5/love.js", "11.5/love.wasm", "11.5/license.txt", "lua/normalize1.lua", "lua/normalize2.lua"].every(file => fs.existsSync(path.join(candidate, file)));
  return choice.target === "macos" && candidate.endsWith(".app") && fs.existsSync(path.join(candidate, "Contents/Info.plist"));
}

function runtimeRequirement(choice) {
  return choice.target === "windows"
    ? "Choose the extracted 64-bit LÖVE for Windows folder containing love.exe."
    : choice.target === "web"
      ? "Choose the LÖVE 11.5 standalone player folder containing player.js and 11.5/love.wasm."
      : "Choose a love.app bundle containing Contents/Info.plist.";
}

function packagedArtifactFromLine(line, projectDirectory) {
  const match = /^packaged (.+?)(?: \([\d.]+ (?:B|KiB|MiB|GiB)\))?$/u.exec(line);
  if (!match) return undefined;
  const artifact = path.resolve(projectDirectory, match[1]);
  return isPathInside(path.join(projectDirectory, "dist"), artifact) && artifact !== path.join(projectDirectory, "dist")
    ? artifact : undefined;
}

async function notifyPackageResult(code, artifacts, projectDirectory) {
  if (code !== 0) {
    vscode.window.showErrorMessage("Packaging failed. Check the Package Game task output for the error.");
    return;
  }
  const artifact = artifacts.at(-1);
  if (!artifact || !fs.existsSync(artifact)) {
    vscode.window.showWarningMessage("Packaging finished, but its output could not be located. Check the Package Game task output.");
    return;
  }
  const location = projectDirectory ? path.relative(projectDirectory, artifact) : artifact;
  const action = await vscode.window.showInformationMessage(
    `Packaged ${location}.`,
    "Reveal Package", "Copy Path",
  );
  if (action === "Reveal Package") await vscode.commands.executeCommand("revealFileInOS", vscode.Uri.file(artifact));
  if (action === "Copy Path") await vscode.env.clipboard.writeText(artifact);
}

module.exports = {
  PACKAGE_TARGETS,
  TuaCommandTerminal,
  packagedArtifactFromLine,
  packageProjectLayout,
  packageTargetDetail,
  listCustomPackageTargets,
  notifyPackageResult,
  validLoveRuntime,
  registerGameCommands,
};
