"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const {
  projectLayoutForUri,
  projectLayoutForWorkspaceRoot,
} = require("../editor/project-layout");
const { loadSourceMapsFromDirectory, remapTracebackLine } = require("../editor/source-maps");
const { formatError } = require("../shared/errors");

const TASK_TYPE = "tua";
const TASK_NAME = "Run LOVE Project";
const RUNNING_CONTEXT = "tua.loveProjectRunning";

function registerLoveProjectRunner({ appendOutputLine, getClient, getServerCommand, activity }) {
  let activeExecution;
  const disposables = [
    vscode.commands.registerCommand(COMMANDS.runLoveProject, () =>
      runLoveProject({ getClient, getServerCommand }),
    ),
    vscode.commands.registerCommand(COMMANDS.restartLoveProject, () =>
      runLoveProject({ getClient, getServerCommand, restart: true }),
    ),
    vscode.commands.registerCommand(COMMANDS.stopLoveProject, () => stopLoveProject()),
  ];

  void setRunningContext(false);

  if (vscode.tasks?.onDidEndTask) {
    disposables.push(
      vscode.tasks.onDidEndTask((event) => {
        if (event.execution === activeExecution) {
          activeExecution = undefined;
          void setRunningContext(false);
        }
      }),
    );
  }

  if (vscode.tasks?.onDidEndTaskProcess) {
    disposables.push(
      vscode.tasks.onDidEndTaskProcess((event) => {
        if (event.execution === activeExecution) {
          activeExecution = undefined;
          void setRunningContext(false);
        }
      }),
    );
  }

  async function runLoveProject(options) {
    if (!vscode.workspace.isTrusted) {
      vscode.window.showWarningMessage("Trust this workspace before running LOVE project tasks.");
      return undefined;
    }

    if (!vscode.tasks?.executeTask) {
      vscode.window.showErrorMessage("This VS Code version does not support task execution.");
      return undefined;
    }

    const layout = await activeProjectLayout(options.getClient);
    if (!layout) {
      vscode.window.showWarningMessage(projectSelectionMessage("running"));
      return undefined;
    }

    if (options.restart && activeExecution?.terminate) {
      activeExecution.terminate();
    }

    const task = createLoveProjectTask({
      activity,
      appendOutputLine,
      layout,
      loveCommand: configuredLoveCommand(),
      tuaCommand: options.getServerCommand?.() || "tua",
    });

    activeExecution = await vscode.tasks.executeTask(task);
    await setRunningContext(true);

    return {
      outputDirectory: layout.outputDirectory,
      taskName: TASK_NAME,
      workspaceRoot: layout.configDirectory,
    };
  }

  async function stopLoveProject() {
    if (!activeExecution?.terminate) {
      await setRunningContext(false);
      vscode.window.showInformationMessage("No Tua LOVE project is currently running.");
      return false;
    }

    activeExecution.terminate();
    activeExecution = undefined;
    await setRunningContext(false);
    return true;
  }

  return vscode.Disposable.from(...disposables);
}

function createLoveProjectTask({ appendOutputLine, layout, loveCommand, tuaCommand, activity }) {
  const terminalOptions = {
    activity,
    title: TASK_NAME,
    appendOutputLine,
    cwd: layout.configDirectory,
    loveCommand,
    outputDirectory: layout.outputDirectory,
    tuaCommand,
  };
  const execution = new vscode.CustomExecution(
    async () => new LoveProjectTerminal(terminalOptions),
  );
  const task = new vscode.Task(
    {
      type: TASK_TYPE,
      command: "runLoveProject",
      projectRoot: layout.configDirectory,
    },
    vscode.TaskScope.Workspace,
    TASK_NAME,
    "Tua",
    execution,
    [],
  );

  task.detail = `Build ${layout.configDirectory} and run LOVE from ${layout.outputDirectory}.`;
  task.presentationOptions = {
    clear: false,
    focus: true,
    panel: vscode.TaskPanelKind?.Dedicated,
    reveal: vscode.TaskRevealKind?.Always,
  };

  if (vscode.TaskGroup?.Build) {
    task.group = vscode.TaskGroup.Build;
  }

  return task;
}

async function activeProjectLayout(getClient) {
  const document = vscode.window.activeTextEditor?.document;
  if (document?.uri?.scheme === "file") {
    const layout = await projectLayoutForUri(document.uri, getClient);
    if (layout) {
      return layout;
    }
  }

  const workspaceFolders = (vscode.workspace.workspaceFolders || []).filter(
    (folder) => folder.uri?.scheme === "file",
  );
  if (workspaceFolders.length !== 1) {
    return undefined;
  }

  return projectLayoutForWorkspaceRoot(workspaceFolders[0].uri.fsPath, getClient);
}

function projectSelectionMessage(action) {
  const workspaceFolders = (vscode.workspace.workspaceFolders || []).filter(
    (folder) => folder.uri?.scheme === "file",
  );

  return workspaceFolders.length > 1
    ? `Open a file from the Tua project you want to use before ${action} LOVE.`
    : `Open a Tua project workspace before ${action} LOVE.`;
}

function configuredLoveCommand() {
  const configured = vscode.workspace.getConfiguration("tua").get("love.command", "love");
  const command = typeof configured === "string" && configured.trim() ? configured.trim() : "love";

  return resolveLoveCommand(command);
}

function resolveLoveCommand(command, options = {}) {
  const platform = options.platform || process.platform;
  const env = options.env || process.env;
  const fileExists = options.fileExists || executableFileExists;

  if (!isDefaultLoveCommand(command, platform)) {
    return command;
  }

  if (commandExistsOnPath(command, env, platform, fileExists)) {
    return command;
  }

  return (options.commonCommands || commonLoveCommands(platform)).find(fileExists) || command;
}

class LoveProjectTerminal {
  constructor({ appendOutputLine, cwd, loveCommand, outputDirectory, tuaCommand, activity, kind = "run", title }) {
    this.closeEmitter = new vscode.EventEmitter();
    this.writeEmitter = new vscode.EventEmitter();
    this.onDidClose = this.closeEmitter.event;
    this.onDidWrite = this.writeEmitter.event;
    this.appendOutputLine = appendOutputLine;
    this.cwd = cwd;
    this.loveCommand = loveCommand;
    this.outputDirectory = outputDirectory;
    this.tuaCommand = tuaCommand;
    this.activeProcess = undefined;
    this.finished = false;
    this.lineBuffer = "";
    this.sourceMaps = [];
    this.activity = activity;
    this.session = activity?.begin({ cwd, kind, title });
    if (this.session) this.session.stop = () => this.close();
  }

  open() {
    void this.run();
  }

  close() {
    this.stopped = true;
    this.killActiveProcess();
    this.finish(1);
  }

  async run() {
    this.writeLine(`[tua] Building project in ${this.cwd}`);
    const buildCode = await this.runProcess(this.tuaCommand, ["build"]);

    if (this.finished) {
      return;
    }

    if (buildCode !== 0) {
      this.writeLine(`[tua] Build failed with exit code ${buildCode}.`);
      this.finish(buildCode);
      return;
    }

    this.sourceMaps = this.loadSourceMaps();
    this.writeLine(`[tua] Starting LOVE from ${this.outputDirectory}`);
    const loveCode = await this.runProcess(this.loveCommand, [this.outputDirectory], { onStart: () => this.activity?.update(this.session, "running") });

    if (!this.finished) {
      this.finish(loveCode);
    }
  }

  runProcess(command, args, { onStart } = {}) {
    this.writeLine(`$ ${formatCommand(command, args)}`);

    return new Promise((resolve) => {
      let settled = false;
      let child;

      const settle = (code) => {
        if (settled) {
          return;
        }

        settled = true;
        this.flushOutputLine();
        if (this.activeProcess === child) {
          this.activeProcess = undefined;
        }
        resolve(code);
      };

      try {
        child = childProcess.spawn(command, args, {
          cwd: this.cwd,
          env: process.env,
          stdio: ["ignore", "pipe", "pipe"],
          windowsHide: true,
          detached: process.platform !== "win32",
        });
      } catch (error) {
        this.writeLine(`[tua] Failed to start ${command}: ${formatError(error)}`);
        settle(1);
        return;
      }

      this.activeProcess = child;
      child.once("spawn", () => onStart?.());
      child.stdout.on("data", (chunk) => this.writeProcessOutput(chunk));
      child.stderr.on("data", (chunk) => this.writeProcessOutput(chunk));
      child.once("error", (error) => {
        this.writeLine(`[tua] ${command} failed: ${formatError(error)}`);
        const hint = missingExecutableHint(command, error);
        if (hint) {
          this.writeLine(`[tua] ${hint}`);
        }
        settle(1);
      });
      child.once("exit", (code, signal) => {
        if (signal) {
          settle(1);
          return;
        }

        settle(Number.isInteger(code) ? code : 1);
      });
    });
  }

  writeProcessOutput(chunk) {
    if (this.finished) {
      return;
    }

    const normalized = chunk.toString("utf8").replace(/\r\n/gu, "\n").replace(/\r/gu, "\n");
    const lines = `${this.lineBuffer}${normalized}`.split("\n");
    this.lineBuffer = lines.pop() || "";

    for (const line of lines) {
      this.writeProcessLine(line);
    }
  }

  flushOutputLine() {
    if (!this.lineBuffer) {
      return;
    }

    this.writeProcessLine(this.lineBuffer);
    this.lineBuffer = "";
  }

  writeProcessLine(line) {
    this.activity?.line(this.session, line);
    this.writeTerminalLine(line);

    if (this.sourceMaps.length === 0) {
      return;
    }

    const mapped = remapTracebackLine(line, this.sourceMaps);
    if (!mapped) {
      return;
    }

    for (const mappedLine of mapped.split("\n")) {
      this.writeLine(`[tua] ${mappedLine}`);
    }
  }

  loadSourceMaps() {
    try {
      const maps = loadSourceMapsFromDirectory(this.outputDirectory);
      if (maps.length === 0) {
        this.writeLine(
          `[tua] No source maps found in ${this.outputDirectory}; enable emit_maps for Tua traceback links.`,
        );
      }

      return maps;
    } catch (error) {
      this.writeLine(`[tua] Could not load source maps: ${formatError(error)}`);
      return [];
    }
  }

  writeLine(line) {
    if (this.finished) {
      return;
    }

    this.appendOutputLine?.(line);
    this.writeTerminalLine(line);
  }

  writeTerminalLine(line) {
    if (this.finished) {
      return;
    }

    this.writeEmitter.fire(`${line}\r\n`);
  }

  killActiveProcess() {
    if (!this.activeProcess || this.activeProcess.killed) {
      return;
    }

    const child = this.activeProcess;
    if (Number.isInteger(child.pid) && process.platform !== "win32") {
      try { process.kill(-child.pid, "SIGTERM"); return; } catch { /* Already exited or not a process group. */ }
    }
    if (Number.isInteger(child.pid) && process.platform === "win32") {
      childProcess.execFile("taskkill", ["/pid", String(child.pid), "/T", "/F"], () => child.kill());
    } else child.kill();
  }

  finish(code) {
    if (this.finished) {
      return;
    }

    this.finished = true;
    this.activity?.finish(this.session, code, this.stopped);
    this.closeEmitter.fire(code);
    this.closeEmitter.dispose();
    this.writeEmitter.dispose();
  }
}

function setRunningContext(running) {
  return vscode.commands.executeCommand("setContext", RUNNING_CONTEXT, running);
}

function formatCommand(command, args) {
  return [command, ...args.map((arg) => quoteCommandArg(String(arg)))].join(" ");
}

function quoteCommandArg(value) {
  if (!/[\s"'\\]/u.test(value)) {
    return value;
  }

  return `"${value.replace(/(["\\])/gu, "\\$1")}"`;
}

function missingExecutableHint(command, error) {
  if (error?.code !== "ENOENT" || !isLoveExecutableCommand(command)) {
    return undefined;
  }

  const macPath = "/Applications/love.app/Contents/MacOS/love";

  return `Could not find LOVE. Install LOVE, add it to PATH, or set \`tua.love.command\` to the full executable path such as \`${macPath}\`.`;
}

function isLoveExecutableCommand(command) {
  const normalized = path.basename(command).toLowerCase();

  return normalized === "love" || normalized === "love.exe";
}

function isDefaultLoveCommand(command, platform) {
  const normalized = command.toLowerCase();

  return normalized === "love" || (platform === "win32" && normalized === "love.exe");
}

function commandExistsOnPath(command, env, platform, fileExists) {
  if (!isBareCommand(command)) {
    return fileExists(command);
  }

  const pathValue =
    typeof env.PATH === "string" ? env.PATH : typeof env.Path === "string" ? env.Path : "";
  if (!pathValue) {
    return false;
  }

  const extensions =
    platform === "win32" && path.extname(command) === "" ? ["", ".exe", ".cmd", ".bat"] : [""];

  return pathValue
    .split(path.delimiter)
    .filter(Boolean)
    .some((directory) =>
      extensions.some((extension) => fileExists(path.join(directory, `${command}${extension}`))),
    );
}

function isBareCommand(command) {
  return !command.includes("/") && !command.includes("\\");
}

function commonLoveCommands(platform) {
  if (platform === "darwin") {
    return ["/Applications/love.app/Contents/MacOS/love"];
  }

  if (platform === "win32") {
    return [
      "C:\\Program Files\\LOVE\\love.exe",
      "C:\\Program Files (x86)\\LOVE\\love.exe",
    ];
  }

  return [];
}

function executableFileExists(filename) {
  try {
    return fs.existsSync(filename) && fs.statSync(filename).isFile();
  } catch {
    return false;
  }
}

module.exports = {
  LoveProjectTerminal,
  activeProjectLayout,
  configuredLoveCommand,
  createLoveProjectTask,
  projectSelectionMessage,
  registerLoveProjectRunner,
  resolveLoveCommand,
};
