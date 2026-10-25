"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const { normalizeInstances } = require("../workspace/run-options");
const { registerDebugInstanceGroups } = require("./debug-instances");
const { COMMANDS } = require("../constants");
const { projectLayoutForWorkspaceRoot } = require("../editor/project-layout");
const { sourceMapPathForGeneratedPath } = require("../editor/source-maps");
const { sourceLocation } = require("../workspace/console-buffer");
const { formatError } = require("../shared/errors");
const {
  activeProjectLayout,
  configuredLoveCommand,
  projectSelectionMessage,
} = require("./run-project");

const DEBUGGER_EXTENSION_ID = "ismoh-games.second-local-lua-debugger-vscode";
const LEGACY_DEBUGGER_EXTENSION_ID = "tomblind.local-lua-debugger-vscode";
const DEBUGGER_EXTENSION_IDS = [DEBUGGER_EXTENSION_ID, LEGACY_DEBUGGER_EXTENSION_ID];
const DEBUGGER_TYPE = "lua-local";
const TUA_LAUNCH_PROPERTY = "tuaProject";
const DEBUG_BOOTSTRAP =
  'if os.getenv("LOCAL_LUA_DEBUGGER_VSCODE") == "1" then require("lldebugger").start(); function love.errorhandler(message) error(message, 2) end end\n';

function registerLoveProjectDebugger({ appendOutputLine, getClient, getServerCommand, activity, inspector }) {
  const preparedOutputs = new Set();
  const instanceGroups = registerDebugInstanceGroups({ activity, inspector, appendOutputLine, build: runBuildCommand,
    prepare: prepareLoveDebugOutput, configuration: createLoveDebugConfiguration, scriptFiles: debuggerScriptFiles });
  const startDisposable = vscode.debug.onDidStartDebugSession?.((session) => {
    for (const prepared of preparedOutputs) {
      if (debugSessionTargetsOutput(session, prepared.outputDirectory) && prepared.activitySession) {
        prepared.activitySession.stop = () => vscode.debug.stopDebugging(session);
        activity?.update(prepared.activitySession, "running");
      }
    }
  }) || { dispose() {} };
  const trackerDisposable = vscode.debug.registerDebugAdapterTrackerFactory?.(DEBUGGER_TYPE, {
    createDebugAdapterTracker(session) {
      return { onDidSendMessage(message) {
        if (message.type !== "event" || message.event !== "output" || typeof message.body?.output !== "string") return;
        for (const prepared of preparedOutputs) {
          if (!debugSessionTargetsOutput(session, prepared.outputDirectory)) continue;
          for (const line of message.body.output.split(/\r?\n/)) {
            const source = message.body.source?.path;
            const location = source?.endsWith(".tua") ? { path: source, line: Math.max(0, (message.body.line || 1) - 1), character: 0 } : sourceLocation(line, prepared.activitySession?.cwd || prepared.outputDirectory);
            activity?.output(prepared.activitySession, line, { location, stream: message.body.category });
          }
        }
      } };
    },
  }) || { dispose() {} };
  const terminationDisposable = vscode.debug.onDidTerminateDebugSession((session) => {
    for (const prepared of preparedOutputs) {
      if (debugSessionTargetsOutput(session, prepared.outputDirectory)) {
        activity?.finish(prepared.activitySession, 0);
        restorePreparedOutput(preparedOutputs, prepared);
      }
    }
  });
  const configurationProviderDisposable = vscode.debug.registerDebugConfigurationProvider(
    DEBUGGER_TYPE,
    {
      async resolveDebugConfigurationWithSubstitutedVariables(folder, configuration) {
        if (configuration?.[TUA_LAUNCH_PROPERTY] !== true) {
          return configuration;
        }

        if (!vscode.workspace.isTrusted) {
          vscode.window.showWarningMessage(
            "Trust this workspace before debugging a LOVE project.",
          );
          return undefined;
        }

        const layout = await debugProjectLayout(folder, getClient);
        if (!layout) {
          vscode.window.showWarningMessage(
            "Open a Tua project workspace before debugging LOVE.",
          );
          return undefined;
        }

        if (!(await ensureDebuggerExtension())) {
          return undefined;
        }

        const prepared = await buildAndPrepareLoveDebugProject({
          appendOutputLine,
          activity,
          getServerCommand,
          layout,
          preparedOutputs,
        });
        if (!prepared) {
          return undefined;
        }

        const launchDirectory = await inspector?.launch(prepared.pendingOutput.activitySession, layout.outputDirectory);
        return createLoveDebugConfiguration({
          layout,
          loveCommand: configuredLoveCommand(),
          overrides: {
            ...configuration,
            ...(launchDirectory ? { tuaInspectorLaunch: launchDirectory } : {}),
            scriptFiles: debuggerScriptFiles(layout.outputDirectory),
          },
        });
      },
    },
  );
  const commandDisposable = vscode.commands.registerCommand(COMMANDS.debugLoveProject, async (options) => {
    if (!vscode.workspace.isTrusted) {
      vscode.window.showWarningMessage("Trust this workspace before debugging a LOVE project.");
      return undefined;
    }

    const layout = await activeProjectLayout(getClient);
    if (!layout) {
      vscode.window.showWarningMessage(projectSelectionMessage("debugging"));
      return undefined;
    }

    if (!(await ensureDebuggerExtension())) {
      return undefined;
    }

    if (options?.instances !== undefined) {
      let instances;
      try { instances = normalizeInstances(options.instances); }
      catch (error) { vscode.window.showErrorMessage(error.message); return undefined; }
      return instanceGroups.start({ layout, instances, title: options.title,
        configurationName: options.configurationName,
        tuaCommand: getServerCommand?.() || "tua", loveCommand: configuredLoveCommand() });
    }

    const preparedResult = await buildAndPrepareLoveDebugProject({
      configurationName: options?.configurationName,
      title: options?.title,
      appendOutputLine,
      activity,
      getServerCommand,
      layout,
      preparedOutputs,
    });
    if (!preparedResult) {
      return undefined;
    }
    const { pendingOutput, prepared } = preparedResult;

    const folder = vscode.workspace.getWorkspaceFolder(
      vscode.Uri.file(layout.configDirectory),
    );
    const launchDirectory = await inspector?.launch(pendingOutput.activitySession, layout.outputDirectory);
    const configuration = createLoveDebugConfiguration({
      layout,
      loveCommand: configuredLoveCommand(),
      overrides: launchDirectory ? { tuaInspectorLaunch: launchDirectory } : {},
    });
    let started;
    try {
      started = await vscode.debug.startDebugging(folder, configuration);
    } catch (error) {
      activity?.finish(pendingOutput.activitySession, 1);
      restorePreparedOutput(preparedOutputs, pendingOutput);
      vscode.window.showErrorMessage(
        `Second Local Lua Debugger did not start the LOVE project: ${formatError(error)}`,
      );
      return undefined;
    }

    if (!started) {
      activity?.finish(pendingOutput.activitySession, 1);
      restorePreparedOutput(preparedOutputs, pendingOutput);
      vscode.window.showErrorMessage("Second Local Lua Debugger did not start the LOVE project.");
      return undefined;
    }

    activity?.update(pendingOutput.activitySession, "running");
    appendOutputLine?.(`[tua] Debugging LOVE from ${layout.outputDirectory}`);
    return {
      bootstrapPath: prepared.bootstrapPath,
      outputDirectory: layout.outputDirectory,
      debuggerType: DEBUGGER_TYPE,
    };
  });

  return vscode.Disposable.from(
    commandDisposable,
    instanceGroups,
    configurationProviderDisposable,
    terminationDisposable,
    startDisposable,
    trackerDisposable,
    {
      dispose() {
        for (const prepared of preparedOutputs) {
          prepared.restore();
        }
        preparedOutputs.clear();
      },
    },
  );
}

async function ensureDebuggerExtension() {
  if (findDebuggerExtension()) {
    return true;
  }

  const install = "Install Second Local Lua Debugger";
  const choice = await vscode.window.showErrorMessage(
    "Tua debugging uses the Second Local Lua Debugger extension for Lua execution, stepping, and inspection.",
    install,
  );
  if (choice === install) {
    await vscode.commands.executeCommand(
      "workbench.extensions.installExtension",
      DEBUGGER_EXTENSION_ID,
    );
  }
  return false;
}

async function buildAndPrepareLoveDebugProject({
  appendOutputLine: writeOutput,
  activity,
  getServerCommand,
  layout,
  preparedOutputs,
  configurationName,
  title = "Debug LOVE Project",
}) {
  const activitySession = activity?.begin({ kind: "debug", title, cwd: layout.configDirectory, configurationName });
  const appendOutputLine = line => { writeOutput?.(line); activity?.output(activitySession, line, { module: "build" }); };
  const tuaCommand = getServerCommand?.() || "tua";
  appendOutputLine?.(`[tua] Building debug project in ${layout.configDirectory}`);
  const buildCode = await runBuildCommand({
    appendOutputLine,
    cwd: layout.configDirectory,
    tuaCommand,
  });
  if (buildCode !== 0) {
    activity?.finish(activitySession, buildCode);
    appendOutputLine?.(`[tua] Debug build failed with exit code ${buildCode}.`);
    vscode.window.showErrorMessage("Tua build failed. Open the Tua output for details.");
    await vscode.commands.executeCommand(COMMANDS.focusOutput);
    return undefined;
  }

  let prepared;
  try {
    prepared = prepareLoveDebugOutput(layout.outputDirectory);
  } catch (error) {
    activity?.finish(activitySession, 1);
    appendOutputLine?.(`[tua] Could not prepare LOVE debug output: ${formatError(error)}`);
    vscode.window.showErrorMessage(`Could not prepare LOVE debug output: ${formatError(error)}`);
    return undefined;
  }

  const pendingOutput = {
    activitySession,
    outputDirectory: layout.outputDirectory,
    restore: prepared.restore,
  };
  preparedOutputs.add(pendingOutput);

  if (!hasDebuggerSourceMap(layout.outputDirectory)) {
    vscode.window.showWarningMessage(
      "No Tua debugger source maps were emitted. Enable emit_maps in tua.toml to bind breakpoints in .tua files.",
    );
  }

  return { pendingOutput, prepared };
}

function findDebuggerExtension(extensions = vscode.extensions) {
  return DEBUGGER_EXTENSION_IDS.map((id) => extensions?.getExtension(id)).find(Boolean);
}

async function debugProjectLayout(folder, getClient) {
  if (folder?.uri?.scheme === "file" && folder.uri.fsPath) {
    return projectLayoutForWorkspaceRoot(folder.uri.fsPath, getClient);
  }
  return activeProjectLayout(getClient);
}

function debugSessionTargetsOutput(session, outputDirectory) {
  const configuration = session?.configuration;
  const type = session?.type || configuration?.type;
  if (type !== DEBUGGER_TYPE || !Array.isArray(configuration?.args)) {
    return false;
  }

  const expected = path.resolve(outputDirectory);
  if (configuration.tuaInspectorOutput === expected) return true;
  return configuration.args.some(
    (argument) => typeof argument === "string" && path.resolve(argument) === expected,
  );
}

function restorePreparedOutput(preparedOutputs, prepared) {
  preparedOutputs.delete(prepared);
  prepared.restore();
}

function createLoveDebugConfiguration({ layout, loveCommand, overrides = {} }) {
  const scriptGlob = debuggerScriptGlob(layout);
  const defaults = {
    type: DEBUGGER_TYPE,
    request: "launch",
    name: "Debug LOVE Project (Tua)",
    program: {
      command: loveCommand,
      communication: "pipe",
    },
    args: [layout.outputDirectory],
    cwd: layout.configDirectory,
    scriptRoots: [layout.outputDirectory],
    stepUnmappedLines: false,
    stopOnEntry: false,
  };
  const extraArgs = Array.isArray(overrides.args) ? overrides.args : [];
  const extraScriptRoots = Array.isArray(overrides.scriptRoots)
    ? overrides.scriptRoots.filter((root) => root !== layout.outputDirectory)
    : [];
  const configuration = {
    ...defaults,
    ...overrides,
    type: DEBUGGER_TYPE,
    request: "launch",
    program: {
      ...defaults.program,
      ...(overrides.program || {}),
    },
    args: [overrides.tuaInspectorLaunch || layout.outputDirectory, ...extraArgs],
    ...(overrides.tuaInspectorLaunch ? { tuaInspectorOutput: path.resolve(layout.outputDirectory) } : {}),
    scriptRoots: [layout.outputDirectory, ...extraScriptRoots],
  };

  if (scriptGlob && !Array.isArray(overrides.scriptFiles)) {
    configuration.scriptFiles = [scriptGlob];
  }

  return configuration;
}

function debuggerScriptGlob(layout) {
  const relative = path.relative(layout.workspaceRoot, layout.outputDirectory);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    return undefined;
  }

  const directory = relative.split(path.sep).filter(Boolean).join("/");
  return directory ? `${directory}/**/*.lua` : "**/*.lua";
}

function debuggerScriptFiles(directory) {
  if (!fs.existsSync(directory)) {
    return [];
  }

  return fs
    .readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        return debuggerScriptFiles(entryPath);
      }
      return entry.isFile() && entry.name.endsWith(".lua") ? [entryPath] : [];
    })
    .sort((left, right) => left.localeCompare(right));
}

function prepareLoveDebugOutput(outputDirectory) {
  const mainPath = path.join(outputDirectory, "main.lua");
  if (!fs.existsSync(mainPath)) {
    throw new Error(`generated LOVE entrypoint was not found at ${mainPath}`);
  }

  const bootstrapPath = path.join(outputDirectory, "conf.lua");
  const trackedPaths = [
    bootstrapPath,
    sourceMapPathForGeneratedPath(bootstrapPath),
    `${bootstrapPath}.map`,
  ];
  const before = snapshotFiles(trackedPaths);
  const source = fs.existsSync(bootstrapPath) ? fs.readFileSync(bootstrapPath, "utf8") : "";
  if (!source.startsWith(DEBUG_BOOTSTRAP)) {
    fs.writeFileSync(bootstrapPath, `${DEBUG_BOOTSTRAP}${source}`);
    shiftTuaSourceMap(bootstrapPath);
    shiftDebuggerSourceMap(bootstrapPath);
  }

  const prepared = snapshotFiles(trackedPaths);
  let restored = false;
  return {
    bootstrapPath,
    restore() {
      if (restored) {
        return;
      }
      restored = true;
      restoreUnchangedFiles(before, prepared);
    },
  };
}

function snapshotFiles(paths) {
  return new Map(
    paths.map((filename) => [
      filename,
      fs.existsSync(filename) ? fs.readFileSync(filename) : undefined,
    ]),
  );
}

function restoreUnchangedFiles(before, prepared) {
  for (const [filename, original] of before) {
    const current = fs.existsSync(filename) ? fs.readFileSync(filename) : undefined;
    const preparedContents = prepared.get(filename);
    if (!sameFileContents(current, preparedContents)) {
      continue;
    }

    if (original === undefined) {
      fs.rmSync(filename, { force: true });
    } else {
      fs.writeFileSync(filename, original);
    }
  }
}

function sameFileContents(left, right) {
  if (left === undefined || right === undefined) {
    return left === right;
  }
  return left.equals(right);
}

function shiftTuaSourceMap(generatedPath) {
  const mapPath = sourceMapPathForGeneratedPath(generatedPath);
  if (!fs.existsSync(mapPath)) {
    return;
  }

  const map = JSON.parse(fs.readFileSync(mapPath, "utf8"));
  if (map?.version !== 1 || !Array.isArray(map.lineMappings)) {
    return;
  }

  map.lineMappings = map.lineMappings.map((mapping) => ({
    ...mapping,
    generatedLine: Number.isInteger(mapping?.generatedLine)
      ? mapping.generatedLine + 1
      : mapping?.generatedLine,
  }));
  fs.writeFileSync(mapPath, `${JSON.stringify(map, null, 2)}\n`);
}

function shiftDebuggerSourceMap(generatedPath) {
  const mapPath = `${generatedPath}.map`;
  if (!fs.existsSync(mapPath)) {
    return;
  }

  const map = JSON.parse(fs.readFileSync(mapPath, "utf8"));
  if (map?.version !== 3 || typeof map.mappings !== "string") {
    return;
  }

  map.mappings = `;${map.mappings}`;
  fs.writeFileSync(mapPath, `${JSON.stringify(map)}\n`);
}

function hasDebuggerSourceMap(directory) {
  if (!fs.existsSync(directory)) {
    return false;
  }

  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory() && hasDebuggerSourceMap(entryPath)) {
      return true;
    }
    if (entry.isFile() && entry.name.endsWith(".lua.map")) {
      return true;
    }
  }

  return false;
}

function runBuildCommand({ appendOutputLine, cwd, tuaCommand, signal }) {
  if (signal?.aborted) return Promise.resolve(1);
  appendOutputLine?.(`$ ${quoteCommandArg(tuaCommand)} build`);

  return new Promise((resolve) => {
    let settled = false;
    let stdout = "";
    let stderr = "";
    const settle = (code) => {
      if (settled) {
        return;
      }
      settled = true;
      signal?.removeEventListener("abort", abort);
      flushBufferedLines(stdout, appendOutputLine);
      flushBufferedLines(stderr, appendOutputLine);
      resolve(code);
    };
    let child;
    const abort = () => child?.kill();

    try {
      child = childProcess.spawn(tuaCommand, ["build"], {
        cwd,
        env: process.env,
        stdio: ["ignore", "pipe", "pipe"],
        windowsHide: true,
      });
    } catch (error) {
      appendOutputLine?.(`[tua] ${tuaCommand} failed: ${formatError(error)}`);
      settle(1);
      return;
    }

    signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) abort();
    child.stdout.on("data", (chunk) => {
      const result = consumeLines(stdout, chunk, appendOutputLine);
      stdout = result;
    });
    child.stderr.on("data", (chunk) => {
      const result = consumeLines(stderr, chunk, appendOutputLine);
      stderr = result;
    });
    child.once("error", (error) => {
      appendOutputLine?.(`[tua] ${tuaCommand} failed: ${formatError(error)}`);
      settle(1);
    });
    child.once("exit", (code, signal) => settle(signal ? 1 : Number.isInteger(code) ? code : 1));
  });
}

function consumeLines(buffer, chunk, appendOutputLine) {
  const normalized = `${buffer}${chunk.toString("utf8")}`
    .replace(/\r\n/gu, "\n")
    .replace(/\r/gu, "\n");
  const lines = normalized.split("\n");
  const remainder = lines.pop() || "";

  for (const line of lines) {
    appendOutputLine?.(line);
  }

  return remainder;
}

function flushBufferedLines(buffer, appendOutputLine) {
  if (buffer) {
    appendOutputLine?.(buffer);
  }
}

function quoteCommandArg(value) {
  if (!/[\s"'\\]/u.test(value)) {
    return value;
  }

  return `"${value.replace(/(["\\])/gu, "\\$1")}"`;
}

module.exports = {
  DEBUG_BOOTSTRAP,
  DEBUGGER_EXTENSION_ID,
  DEBUGGER_EXTENSION_IDS,
  TUA_LAUNCH_PROPERTY,
  createLoveDebugConfiguration,
  debugProjectLayout,
  debugSessionTargetsOutput,
  debuggerScriptFiles,
  debuggerScriptGlob,
  findDebuggerExtension,
  hasDebuggerSourceMap,
  prepareLoveDebugOutput,
  registerLoveProjectDebugger,
};
