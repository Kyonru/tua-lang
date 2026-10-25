"use strict";

const fs = require("fs");
const vscode = require("vscode");
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
    label: ".love archive",
    description: "dist/<name>.love, playable with LOVE on any desktop",
  },
  {
    target: "windows",
    label: "Windows",
    description: "a fused .exe folder; needs the zipped LOVE for Windows",
    setting: "love.windowsRuntime",
    pick: { canSelectFolders: true, canSelectFiles: false, title: "Select the extracted LOVE for Windows folder" },
  },
  {
    target: "macos",
    label: "macOS",
    description: "a fused .app; needs love.app",
    setting: "love.macosApp",
    pick: { canSelectFolders: true, canSelectFiles: true, title: "Select love.app" },
  },
];

// Run one `tua` command in a task terminal, with LOVE traceback remapping
// once the build output exists.
class TuaCommandTerminal extends LoveProjectTerminal {
  constructor(options) {
    super(options);
    this.tuaArgs = options.tuaArgs;
  }

  async run() {
    const code = await this.runProcess(this.tuaCommand, this.tuaArgs);
    if (!this.finished) {
      this.finish(code);
    }
  }

  writeProcessLine(line, stream) {
    if (line.startsWith("dev: started") || line.startsWith("dev: sent reload")) {
      this.sourceMaps = this.loadSourceMaps();
    }
    super.writeProcessLine(line, stream);
  }
}

function registerGameCommands({ appendOutputLine, getClient, getServerCommand, activity }) {
  return vscode.Disposable.from(
    vscode.commands.registerCommand("tua.buildProject", () => runTuaTask({
      appendOutputLine, activity, getClient, getServerCommand,
      action: "building", taskName: "Build Project", args: () => ["build"],
    })),
    vscode.commands.registerCommand(COMMANDS.runLoveProjectWithHotReload, () =>
      runTuaTask({
        appendOutputLine,
        activity,
        getClient,
        getServerCommand,
        action: "running",
        taskName: HOT_RELOAD_TASK_NAME,
        args: () => ["dev", "--love", configuredLoveCommand()],
      }),
    ),
    vscode.commands.registerCommand(COMMANDS.packageGame, async (options) => {
      const choice = options?.target ? PACKAGE_TARGETS.find(item => item.target === options.target) : await vscode.window.showQuickPick(PACKAGE_TARGETS, {
        title: "Package Game",
        placeHolder: "Choose what to build",
      });
      if (!choice) {
        return undefined;
      }
      const args = ["package", "--target", choice.target];
      if (choice.setting) {
        const runtime = await loveRuntime(choice);
        if (!runtime) {
          return undefined;
        }
        args.push("--love", runtime);
      }
      return runTuaTask({
        appendOutputLine,
        activity,
        getClient,
        getServerCommand,
        action: "packaging",
        taskName: `${PACKAGE_TASK_NAME} (${choice.label})`,
        args: () => args,
      });
    }),
  );
}

async function runTuaTask({ appendOutputLine, getClient, getServerCommand, activity, action, taskName, args }) {
  if (!vscode.workspace.isTrusted) {
    vscode.window.showWarningMessage("Trust this workspace before running Tua game tasks.");
    return undefined;
  }
  if (!vscode.tasks?.executeTask) {
    vscode.window.showErrorMessage("This VS Code version does not support task execution.");
    return undefined;
  }
  const layout = await activeProjectLayout(getClient);
  if (!layout) {
    vscode.window.showWarningMessage(projectSelectionMessage(action));
    return undefined;
  }

  const tuaArgs = args();
  const options = {
    activity,
    kind: tuaArgs[0],
    title: taskName,
    appendOutputLine,
    cwd: layout.configDirectory,
    loveCommand: configuredLoveCommand(),
    outputDirectory: layout.outputDirectory,
    tuaArgs,
    tuaCommand: getServerCommand?.() || "tua",
  };
  const task = new vscode.Task(
    { type: TASK_TYPE, command: options.tuaArgs[0], projectRoot: layout.configDirectory },
    vscode.TaskScope.Workspace,
    taskName,
    "Tua",
    new vscode.CustomExecution(async () => new TuaCommandTerminal(options)),
    [],
  );
  task.detail = `tua ${options.tuaArgs.join(" ")} in ${layout.configDirectory}`;
  task.presentationOptions = {
    clear: true,
    focus: false,
    panel: vscode.TaskPanelKind?.Dedicated,
    reveal: vscode.TaskRevealKind?.Always,
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
  if (candidate && fs.existsSync(candidate)) {
    return candidate;
  }

  const picked = await vscode.window.showOpenDialog({
    ...choice.pick,
    canSelectMany: false,
    openLabel: "Use This LOVE",
  });
  const runtime = picked?.[0]?.fsPath;
  if (!runtime) {
    return undefined;
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

module.exports = {
  PACKAGE_TARGETS,
  TuaCommandTerminal,
  registerGameCommands,
};
