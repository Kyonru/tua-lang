"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");
const vscode = require("vscode");
const { COMMANDS, DOCUMENT_SELECTOR, STATUS_SELECTOR } = require("./constants");
const { registerEditorDevex } = require("./editor");
const { registerEcsDevex } = require("./ecs");
const { registerServerHealthFeature } = require("./health");
const { registerLoveDevex } = require("./love");
const { registerModuleDevex } = require("./modules");
const { registerObjectPoolDevex } = require("./object-pools");
const { registerSignalDevex } = require("./signals");
const { createDebugLogger, DEBUG_LSP_ENV } = require("./shared/debug-log");
const { isTuaDocument } = require("./shared/documents");
const { formatError } = require("./shared/errors");

let client;
let editorDevex;
let loveDevex;
let extensionContext;
let languageClientModule;
let outputChannel;
let statusItem;
let currentServerInfo;
let starting = false;
let suppressNextServerPathPrompt = false;

let debugLogger;

async function activate(context) {
  extensionContext = context;
  outputChannel = vscode.window.createOutputChannel("tua");
  debugLogger = createDebugLogger(outputChannel);
  context.subscriptions.push(outputChannel);
  context.subscriptions.push(registerBuiltinDocumentProvider(outputChannel));
  context.subscriptions.push(registerLifecycleCommands());
  editorDevex = registerEditorDevex({
    appendOutputLine: (line) => outputChannel?.appendLine(line),
    context,
    configurationTarget,
    getClient: () => client,
    getServerCommand: () => currentServerInfo?.command || configuredServerPath(extensionContext),
    isTuaDocument,
  });
  context.subscriptions.push(editorDevex);
  loveDevex = registerLoveDevex({
    appendOutputLine: (line) => outputChannel?.appendLine(line),
    configurationTarget,
    getClient: () => client,
    getServerCommand: () => currentServerInfo?.command || configuredServerPath(extensionContext),
    isTuaDocument,
  });
  context.subscriptions.push(loveDevex);
  context.subscriptions.push(registerSignalDevex({ getClient: () => client }));
  context.subscriptions.push(registerModuleDevex({ getClient: () => client }));
  context.subscriptions.push(registerEcsDevex({ getClient: () => client }));
  context.subscriptions.push(registerObjectPoolDevex({ getClient: () => client }));
  context.subscriptions.push(registerServerHealthFeature({ getClient: () => client }));
  context.subscriptions.push(vscode.workspace.onDidChangeConfiguration(handleConfigurationChange));
  statusItem = createStatusItem();
  context.subscriptions.push(statusItem);
  debugLog(`${DEBUG_LSP_ENV}=1 enabled; verbose extension/LSP trace is active.`);

  await startLanguageServer();
}

function debugLog(message, options) {
  debugLogger?.log(message, options);
}

function debugLspLoggingEnabled() {
  return Boolean(debugLogger?.isEnabled());
}

function registerLifecycleCommands() {
  return vscode.Disposable.from(
    vscode.commands.registerCommand(COMMANDS.initProject, initializeProject),
    vscode.commands.registerCommand(COMMANDS.restartServer, restartLanguageServer),
    vscode.commands.registerCommand(COMMANDS.focusOutput, () => outputChannel?.show(true)),
    vscode.commands.registerCommand(COMMANDS.openSettings, () =>
      vscode.commands.executeCommand("workbench.action.openSettings", "tua"),
    ),
    vscode.commands.registerCommand(COMMANDS.selectServer, selectServerVersion),
    vscode.commands.registerCommand(COMMANDS.copyDebugInfo, copyDebugInfo),
    vscode.commands.registerCommand(COMMANDS.copyCacheStats, copyCacheStats),
    vscode.commands.registerCommand(COMMANDS.saveCacheStats, saveCacheStats),
    vscode.commands.registerCommand(COMMANDS.resetCompletionStats, resetCompletionStats),
    vscode.commands.registerCommand(
      COMMANDS.toggleExperimentalCompletions,
      toggleExperimentalCompletions,
    ),
    vscode.commands.registerCommand(COMMANDS.saveCpuProfile, () =>
      saveServerProfileArtifact("cpuProfile", "CPU profile"),
    ),
    vscode.commands.registerCommand(COMMANDS.saveHeapSummary, () =>
      saveServerProfileArtifact("heapSummary", "heap/allocation summary"),
    ),
    vscode.commands.registerCommand(COMMANDS.organizeImports, () =>
      runSourceAction("source.organizeImports"),
    ),
    vscode.commands.registerCommand(COMMANDS.sortImports, () => runSourceAction("source.sortImports")),
    vscode.commands.registerCommand(COMMANDS.removeUnusedImports, () =>
      runSourceAction("source.removeUnusedImports"),
    ),
    vscode.commands.registerCommand(COMMANDS.fixAll, () => runSourceAction("source.fixAll")),
    vscode.commands.registerCommand(COMMANDS.reportIssue, reportIssue),
  );
}

function runSourceAction(kind) {
  return vscode.commands.executeCommand("editor.action.codeAction", {
    kind,
    apply: "first",
  });
}

function createStatusItem() {
  const item = vscode.languages.createLanguageStatusItem("tua.languageServer", STATUS_SELECTOR);
  item.name = "Tua Language Server";
  item.text = "$(circle-slash) Tua stopped";
  item.detail = "Tua language server is stopped.";
  item.busy = false;
  item.severity = vscode.LanguageStatusSeverity.Warning;
  item.command = {
    command: COMMANDS.restartServer,
    title: "Restart Tua Language Server",
  };

  return item;
}

async function startLanguageServer() {
  if (starting || !extensionContext || !outputChannel) {
    return;
  }

  starting = true;

  const serverCommand = configuredServerPath(extensionContext);
  const cwd = workspaceCwd();
  const baseDetail = serverDetail(serverCommand, cwd);
  currentServerInfo = {
    command: serverCommand,
    cwd,
    version: undefined,
    state: "starting",
  };

  if (!vscode.workspace.isTrusted && isWorkspaceLocalPath(serverCommand)) {
    const error = new Error("Workspace trust is required before running a workspace-local Tua server binary.");
    showWorkspaceTrustWarning(error);
    setServerStatus("failed", `${baseDetail}\n${formatError(error)}`);
    currentServerInfo.state = "failed";
    currentServerInfo.error = formatError(error);
    starting = false;

    return;
  }

  setServerStatus("starting", baseDetail);
  outputChannel.appendLine(`Starting tua language server: ${serverCommand}`);
  if (cwd) {
    outputChannel.appendLine(`Workspace cwd: ${cwd}`);
  }

  const preflight = await checkServerCommand(serverCommand, cwd);

  if (!preflight.ok) {
    showServerStartupWarning(serverCommand, preflight.error);
    setServerStatus("failed", `${baseDetail}\n${formatError(preflight.error)}`);
    currentServerInfo.state = "failed";
    currentServerInfo.error = formatError(preflight.error);
    starting = false;

    return;
  }

  outputChannel.appendLine(`Tua language server version: ${preflight.version || "unknown"}`);
  currentServerInfo.version = preflight.version;

  const languageClient = loadLanguageClient();
  if (!languageClient) {
    currentServerInfo.state = "failed";
    currentServerInfo.error = "Missing vscode-languageclient dependency.";
    starting = false;

    return;
  }

  const { LanguageClient } = languageClient;
  const serverOptions = {
    run: serverExecutable(serverCommand, cwd),
    debug: serverExecutable(serverCommand, cwd),
  };
  const initializationOptions = traceConfiguration();

  debugLog(
    `language client initialization options: trace.server=${initializationOptions.trace.server} logVerbosity=${initializationOptions.logVerbosity}`,
  );

  const clientOptions = {
    documentSelector: DOCUMENT_SELECTOR,
    initializationOptions,
    middleware: editorDevex?.middleware,
    outputChannel,
    synchronize: {
      configurationSection: "tua",
      fileEvents: [
        vscode.workspace.createFileSystemWatcher("**/*.{tua,lua}"),
        vscode.workspace.createFileSystemWatcher("**/tua.toml"),
      ],
    },
  };

  client = new LanguageClient("tua", "tua language server", serverOptions, clientOptions);

  try {
    await client.start();
    currentServerInfo.state = "running";
    setServerStatus("running", serverDetail(serverCommand, cwd, preflight.version));
    void loveDevex?.refreshAssets?.();
  } catch (error) {
    outputChannel.appendLine(`Failed to start tua language server: ${formatError(error)}`);
    showServerStartupWarning(serverCommand, error);
    client = undefined;
    currentServerInfo.state = "failed";
    currentServerInfo.error = formatError(error);
    setServerStatus("failed", `${baseDetail}\n${formatError(error)}`);
  } finally {
    starting = false;
  }
}

function loadLanguageClient() {
  if (languageClientModule) {
    return languageClientModule;
  }

  try {
    languageClientModule = require("vscode-languageclient/node");

    return languageClientModule;
  } catch (error) {
    outputChannel.appendLine(`Failed to load vscode-languageclient: ${formatError(error)}`);
    vscode.window.showErrorMessage(
      "The tua extension is missing its language client dependency. Reinstall the extension or run npm install in editors/vscode.",
    );
    setServerStatus("failed", `Missing language client dependency.\n${formatError(error)}`);

    return undefined;
  }
}

async function restartLanguageServer() {
  if (!extensionContext) {
    return;
  }

  outputChannel?.show(true);
  outputChannel?.appendLine("Restarting tua language server...");
  await stopLanguageServer({ restarting: true });
  await startLanguageServer();
}

async function initializeProject() {
  if (!extensionContext) {
    return undefined;
  }

  const workspaceFolder = await pickWorkspaceFolderForInit();
  if (!workspaceFolder) {
    return undefined;
  }

  const tuaCommand = configuredServerPath(extensionContext);
  if (!vscode.workspace.isTrusted && isWorkspaceLocalPath(tuaCommand)) {
    showWorkspaceTrustWarning(
      new Error(
        "Workspace trust is required before initializing a project with a workspace-local Tua executable.",
      ),
    );
    return undefined;
  }

  const cwd = workspaceFolder.uri.fsPath;
  outputChannel?.appendLine(`[tua] Initializing project in ${cwd}`);
  outputChannel?.appendLine(`$ ${tuaCommand} init`);

  try {
    const result = await runCommandCapture(tuaCommand, ["init"], cwd);
    appendCommandOutput(result.stdout);
    appendCommandOutput(result.stderr);
    vscode.window.showInformationMessage(`Initialized Tua project in ${workspaceFolder.name}.`);

    return {
      command: tuaCommand,
      workspaceRoot: cwd,
    };
  } catch (error) {
    outputChannel?.appendLine(`[tua] Failed to initialize project: ${formatError(error)}`);
    outputChannel?.show(true);
    vscode.window.showErrorMessage(`Could not initialize Tua project: ${formatError(error)}`);
    return undefined;
  }
}

async function pickWorkspaceFolderForInit() {
  const folders = (vscode.workspace.workspaceFolders || []).filter(
    (folder) => folder.uri?.scheme === "file",
  );

  if (folders.length === 0) {
    vscode.window.showWarningMessage("Open a folder before initializing a Tua project.");
    return undefined;
  }

  if (folders.length === 1) {
    return folders[0];
  }

  const selected = await vscode.window.showQuickPick(
    folders.map((folder) => ({
      label: folder.name,
      description: folder.uri.fsPath,
      folder,
    })),
    {
      placeHolder: "Select the workspace folder to initialize as a Tua project",
      ignoreFocusOut: true,
    },
  );

  return selected?.folder;
}

async function selectServerVersion() {
  if (!extensionContext) {
    return;
  }

  const picks = serverVersionPicks(extensionContext);
  const selected = await vscode.window.showQuickPick(picks, {
    placeHolder: "Select the Tua server binary to use",
    ignoreFocusOut: true,
  });

  if (!selected) {
    return;
  }

  if (selected.kind === "trust") {
    await vscode.commands.executeCommand("workbench.trust.manage");
    return;
  }

  if (selected.kind === "settings") {
    await vscode.commands.executeCommand(COMMANDS.openSettings);
    return;
  }

  suppressNextServerPathPrompt = true;
  await vscode.workspace
    .getConfiguration("tua")
    .update("server.path", selected.serverPath, configurationTarget());
  await restartLanguageServer();
}

function handleConfigurationChange(event) {
  const serverPathChanged = event.affectsConfiguration("tua.server.path");
  const traceChanged =
    event.affectsConfiguration("tua.trace.server") ||
    event.affectsConfiguration("tua.log.verbosity");

  if (traceChanged) {
    sendTraceConfiguration();
  }

  if (!serverPathChanged) {
    return;
  }

  if (suppressNextServerPathPrompt) {
    suppressNextServerPathPrompt = false;
    return;
  }

  vscode.window
    .showInformationMessage("Tua server path changed. Restart the language server?", "Restart")
    .then((choice) => {
      if (choice === "Restart") {
        restartLanguageServer();
      }
    });
}

function serverVersionPicks(context) {
  const picks = [];
  const bundled = bundledServerPath(context);

  if (bundled) {
    picks.push({
      label: "Bundled Tua server",
      description: bundled,
      detail: "Use the server packaged with this extension.",
      kind: "server",
      serverPath: "tua",
    });
  }

  if (vscode.workspace.isTrusted) {
    for (const candidate of workspaceServerPaths()) {
      picks.push({
        label: `Workspace Tua server (${path.basename(path.dirname(candidate))})`,
        description: candidate,
        detail: "Use a binary from this trusted workspace.",
        kind: "server",
        serverPath: candidate,
      });
    }
  } else if (hasWorkspaceFolders()) {
    picks.push({
      label: "Trust workspace to use workspace binaries",
      description: "Workspace trust required",
      detail: "Tua will not execute workspace-local server binaries until the workspace is trusted.",
      kind: "trust",
    });
  }

  picks.push({
    label: "Configure custom server path...",
    description: "Open settings",
    detail: "Set tua.server.path manually.",
    kind: "settings",
  });

  return picks;
}

function workspaceServerPaths() {
  const executable = platformExecutableName();
  const candidates = [];

  for (const folder of vscode.workspace.workspaceFolders || []) {
    if (folder.uri.scheme !== "file") {
      continue;
    }

    candidates.push(
      path.join(folder.uri.fsPath, "target", "debug", executable),
      path.join(folder.uri.fsPath, "target", "release", executable),
      path.join(folder.uri.fsPath, "editors", "vscode", "bin", executable),
    );
  }

  return [...new Set(candidates)].filter((candidate) => fs.existsSync(candidate));
}

function configurationTarget() {
  return hasWorkspaceFolders() ? vscode.ConfigurationTarget.Workspace : vscode.ConfigurationTarget.Global;
}

function runCommandCapture(command, args, cwd) {
  return new Promise((resolve, reject) => {
    let settled = false;
    let stdout = "";
    let stderr = "";
    let child;

    const finishResolve = () => {
      if (settled) {
        return;
      }

      settled = true;
      resolve({ stdout, stderr });
    };

    const finishReject = (error) => {
      if (settled) {
        return;
      }

      settled = true;
      reject(error);
    };

    try {
      child = childProcess.spawn(command, args, {
        cwd,
        env: process.env,
        stdio: ["ignore", "pipe", "pipe"],
        windowsHide: true,
      });
    } catch (error) {
      finishReject(error);
      return;
    }

    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8");
    });

    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8");
    });

    child.once("error", (error) => {
      finishReject(error);
    });

    child.once("exit", (code) => {
      if (code === 0) {
        finishResolve();
        return;
      }

      const detail = [stderr.trim(), stdout.trim()].find(Boolean);
      finishReject(
        new Error(
          `\`${formatCommand(command, args)}\` exited with status ${code}${detail ? `: ${detail}` : ""}`,
        ),
      );
    });
  });
}

function appendCommandOutput(text) {
  if (!text) {
    return;
  }

  for (const line of text.split(/\r?\n/).map((entry) => entry.trimEnd())) {
    if (line) {
      outputChannel?.appendLine(line);
    }
  }
}

function formatCommand(command, args) {
  return [command, ...args].join(" ");
}

async function copyDebugInfo() {
  const info = debugInfoText();
  await vscode.env.clipboard.writeText(info);
  vscode.window.showInformationMessage("Tua debug information copied to clipboard.");
}

async function copyCacheStats() {
  try {
    const stats = await requestCacheStats();
    await vscode.env.clipboard.writeText(JSON.stringify(stats, null, 2));
    vscode.window.showInformationMessage("Tua cache statistics copied to clipboard.");
  } catch (error) {
    if (!client) {
      vscode.window.showWarningMessage("Tua language server is not running.");
      return;
    }

    outputChannel?.appendLine(`Failed to read tua cache stats: ${formatError(error)}`);
    vscode.window.showErrorMessage(`Failed to read Tua cache stats: ${formatError(error)}`);
  }
}

async function saveCacheStats() {
  try {
    const stats = await requestCacheStats();
    const directory = profileOutputDirectory();
    const filename = `cache-stats-${timestampForFilename(new Date())}.json`;
    const outputPath = path.join(directory, filename);

    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(outputPath, `${JSON.stringify(stats, null, 2)}\n`, "utf8");
    outputChannel?.appendLine(`Tua cache statistics saved to: ${outputPath}`);
    vscode.window.showInformationMessage(`Tua cache statistics saved to ${outputPath}`);
  } catch (error) {
    if (!client) {
      vscode.window.showWarningMessage("Tua language server is not running.");
      return;
    }

    outputChannel?.appendLine(`Failed to save tua cache stats: ${formatError(error)}`);
    vscode.window.showErrorMessage(`Failed to save Tua cache stats: ${formatError(error)}`);
  }
}

function requestCacheStats() {
  if (!client) {
    return Promise.reject(new Error("Tua language server is not running."));
  }

  return client.sendRequest("tua/cacheStats");
}

async function resetCompletionStats() {
  if (!client) {
    vscode.window.showWarningMessage("Tua language server is not running.");
    return;
  }

  try {
    const result = await client.sendRequest("tua/resetCompletionStats");
    const removedFiles =
      result && Number.isFinite(result.removedFiles) ? Number(result.removedFiles) : 0;
    const message =
      removedFiles === 1
        ? "Tua completion learning stats reset. Removed 1 workspace stats file."
        : `Tua completion learning stats reset. Removed ${removedFiles} workspace stats files.`;

    outputChannel?.appendLine(message);
    vscode.window.showInformationMessage(message);
  } catch (error) {
    outputChannel?.appendLine(`Failed to reset tua completion learning stats: ${formatError(error)}`);
    vscode.window.showErrorMessage(
      `Failed to reset Tua completion learning stats: ${formatError(error)}`,
    );
  }
}

async function toggleExperimentalCompletions() {
  const configuration = vscode.workspace.getConfiguration("tua");
  const sequenceRankingEnabled = configuration.get(
    "completion.sequenceRanking.enabled",
    false,
  );
  const multiTokenEnabled = configuration.get("completion.multiToken.enabled", false);
  const enabled = !(sequenceRankingEnabled && multiTokenEnabled);
  const target = configurationTarget();

  await configuration.update("completion.sequenceRanking.enabled", enabled, target);
  await configuration.update("completion.multiToken.enabled", enabled, target);

  const scope = hasWorkspaceFolders() ? "for this workspace" : "globally";
  const message = `Tua experimental completion features ${enabled ? "enabled" : "disabled"} ${scope}.`;
  outputChannel?.appendLine(message);
  vscode.window.showInformationMessage(message);
}

async function saveServerProfileArtifact(kind, label) {
  if (!client) {
    vscode.window.showWarningMessage("Tua language server is not running.");
    return;
  }

  const directory = profileOutputDirectory();

  try {
    fs.mkdirSync(directory, { recursive: true });

    const result = await client.sendRequest("tua/profileArtifact", {
      kind,
      outputDirectory: directory,
    });
    const outputPaths = profileArtifactPaths(result);
    const outputLabel = outputPaths.length > 0 ? outputPaths.join(", ") : directory;

    outputChannel?.appendLine(`Tua ${label} saved to: ${outputLabel}`);
    vscode.window.showInformationMessage(`Tua ${label} saved to ${outputLabel}`);
  } catch (error) {
    outputChannel?.appendLine(`Tua ${label} unavailable: ${formatError(error)}`);
    vscode.window.showWarningMessage(`Tua ${label} is not available from this server: ${formatError(error)}`);
  }
}

function profileArtifactPaths(result) {
  if (typeof result === "string" && result.trim()) {
    return [result.trim()];
  }

  if (!result || typeof result !== "object") {
    return [];
  }

  if (typeof result.path === "string" && result.path.trim()) {
    return [result.path.trim()];
  }

  if (Array.isArray(result.paths)) {
    return result.paths.filter((entry) => typeof entry === "string" && entry.trim());
  }

  return [];
}

function profileOutputDirectory() {
  if (extensionContext?.globalStorageUri?.scheme === "file" && extensionContext.globalStorageUri.fsPath) {
    return path.join(extensionContext.globalStorageUri.fsPath, "profiles");
  }

  return path.join(os.tmpdir(), "tua-vscode-profiles");
}

function timestampForFilename(date) {
  return date.toISOString().replaceAll(":", "-").replace(/\.\d{3}Z$/, "Z");
}

function traceConfiguration() {
  const config = vscode.workspace.getConfiguration("tua");
  const configuredTrace = config.get("trace.server", "off");
  const configuredVerbosity = config.get("log.verbosity", "off");

  if (debugLspLoggingEnabled()) {
    return {
      trace: {
        server: configuredTrace === "verbose" ? "verbose" : "messages",
      },
      logVerbosity: configuredVerbosity === "trace" ? "trace" : "debug",
    };
  }

  return {
    trace: {
      server: configuredTrace,
    },
    logVerbosity: configuredVerbosity,
  };
}

function sendTraceConfiguration() {
  if (!client) {
    return;
  }

  Promise.resolve(client.sendNotification("tua/setTrace", traceConfiguration())).catch((error) => {
    outputChannel?.appendLine(`Failed to update tua trace settings: ${formatError(error)}`);
    vscode.window.showErrorMessage(`Failed to update Tua trace settings: ${formatError(error)}`);
  });
}

function debugInfoText() {
  const packageInfo = extensionContext ? require(path.join(extensionContext.extensionPath, "package.json")) : {};
  const lines = [
    `Tua extension: ${packageInfo.version || "unknown"}`,
    `VS Code: ${vscode.version}`,
    `Platform: ${process.platform} ${process.arch}`,
    `Workspace trusted: ${String(vscode.workspace.isTrusted)}`,
    `Server state: ${currentServerInfo?.state || "unknown"}`,
    `Server path: ${currentServerInfo?.command || configuredServerPath(extensionContext)}`,
    `Server version: ${currentServerInfo?.version || "unknown"}`,
  ];

  if (currentServerInfo?.cwd) {
    lines.push(`Workspace cwd: ${currentServerInfo.cwd}`);
  }

  if (currentServerInfo?.error) {
    lines.push(`Last server error: ${currentServerInfo.error}`);
  }

  return lines.join("\n");
}

async function reportIssue() {
  const body = encodeURIComponent(`<!-- Review before submitting. No source code is included. -->\n\n\`\`\`\n${debugInfoText()}\n\`\`\`\n`);
  const uri = vscode.Uri.parse(`https://github.com/Kyonru/tua-lang/issues/new?body=${body}`);
  await vscode.env.openExternal(uri);
}

async function stopLanguageServer(options = {}) {
  const activeClient = client;
  client = undefined;

  if (!activeClient) {
    if (!options.restarting) {
      setServerStatus("stopped", "Tua language server is stopped.");
    }

    return undefined;
  }

  if (!options.restarting) {
    setServerStatus("stopped", "Stopping Tua language server...");
  }

  try {
    await activeClient.stop();
  } catch (error) {
    outputChannel?.appendLine(`Failed to stop tua language server: ${formatError(error)}`);
  } finally {
    if (!options.restarting) {
      setServerStatus("stopped", "Tua language server is stopped.");
    }
  }

  return undefined;
}

function setServerStatus(state, detail) {
  if (!statusItem) {
    return;
  }

  statusItem.detail = detail;
  statusItem.command = statusCommand(state);

  if (state === "starting") {
    statusItem.text = "$(sync~spin) Tua starting";
    statusItem.busy = true;
    statusItem.severity = vscode.LanguageStatusSeverity.Information;
  } else if (state === "running") {
    statusItem.text = "$(check) Tua";
    statusItem.busy = false;
    statusItem.severity = vscode.LanguageStatusSeverity.Information;
  } else if (state === "failed") {
    statusItem.text = "$(error) Tua failed";
    statusItem.busy = false;
    statusItem.severity = vscode.LanguageStatusSeverity.Error;
  } else {
    statusItem.text = "$(circle-slash) Tua stopped";
    statusItem.busy = false;
    statusItem.severity = vscode.LanguageStatusSeverity.Warning;
  }
}

function statusCommand(state) {
  if (state === "failed") {
    return {
      command: COMMANDS.openSettings,
      title: "Open Tua Settings",
    };
  }

  return {
    command: COMMANDS.restartServer,
    title: "Restart Tua Language Server",
  };
}

function serverDetail(command, cwd, version) {
  const lines = [`Server: ${command}`];

  if (version) {
    lines.push(`Version: ${version}`);
  }

  if (cwd) {
    lines.push(`Workspace: ${cwd}`);
  }

  return lines.join("\n");
}

function registerBuiltinDocumentProvider(outputChannel) {
  return vscode.workspace.registerTextDocumentContentProvider("tua", {
    async provideTextDocumentContent(uri) {
      if (!client) {
        return "";
      }

      try {
        return (await client.sendRequest("tua/builtinDocument", { uri: uri.toString() })) || "";
      } catch (error) {
        outputChannel.appendLine(`Failed to load tua builtin document: ${formatError(error)}`);
        return "";
      }
    },
  });
}

async function deactivate() {
  debugLogger?.flush();

  return stopLanguageServer();
}

function configuredServerPath(context) {
  const envOverride = process.env.TUA_SERVER_PATH;
  if (typeof envOverride === "string" && envOverride.trim()) {
    return resolveServerCommand(context, envOverride.trim());
  }

  const configured = vscode.workspace.getConfiguration("tua").get("server.path");
  if (typeof configured === "string" && configured.trim() && configured.trim() !== "tua") {
    return resolveServerCommand(context, configured.trim());
  }

  const bundled = bundledServerPath(context);
  if (bundled) {
    return bundled;
  }

  const fallback = typeof configured === "string" && configured.trim() ? configured.trim() : "tua";

  return resolveServerCommand(context, fallback);
}

function bundledServerPath(context) {
  const executable = platformExecutableName();
  const candidates = [
    path.join(context.extensionPath, "bin", executable),
    path.resolve(context.extensionPath, "..", "..", "target", "debug", executable),
  ];

  return candidates.find((candidate) => fs.existsSync(candidate));
}

function platformExecutableName() {
  return process.platform === "win32" ? "tua.exe" : "tua";
}

function workspaceCwd() {
  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];

  return workspaceFolder?.uri.scheme === "file" ? workspaceFolder.uri.fsPath : undefined;
}

function hasWorkspaceFolders() {
  return Boolean(vscode.workspace.workspaceFolders?.length);
}

function isWorkspaceLocalPath(command) {
  if (!command || isBareCommand(command)) {
    return false;
  }

  const resolved = path.resolve(command);

  return (vscode.workspace.workspaceFolders || []).some((folder) => {
    if (folder.uri.scheme !== "file") {
      return false;
    }

    return isPathInside(resolved, folder.uri.fsPath);
  });
}

function isPathInside(child, parent) {
  const relative = path.relative(parent, child);

  return relative === "" || Boolean(relative && !relative.startsWith("..") && !path.isAbsolute(relative));
}

function resolveServerCommand(context, command) {
  if (isBareCommand(command) || path.isAbsolute(command)) {
    return command;
  }

  const candidates = candidateRelativeServerPaths(context, command);
  const existing = candidates.find((candidate) => fs.existsSync(candidate));

  return existing || candidates[0] || command;
}

function isBareCommand(command) {
  return !command.includes("/") && !command.includes("\\");
}

function candidateRelativeServerPaths(context, command) {
  const bases = [
    vscode.workspace.isTrusted ? workspaceCwd() : undefined,
    process.cwd(),
    context.extensionPath,
    path.resolve(context.extensionPath, "..", ".."),
  ].filter(Boolean);
  const unique = [...new Set(bases)];

  return unique.map((base) => path.resolve(base, command));
}

function serverExecutable(command, cwd) {
  return {
    command,
    args: ["lsp"],
    options: {
      cwd,
    },
  };
}

function checkServerCommand(command, cwd) {
  return new Promise((resolve) => {
    let settled = false;
    let stderr = "";
    let stdout = "";
    const child = childProcess.spawn(command, ["--version"], {
      cwd,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });

    const timeout = setTimeout(() => {
      child.kill();
      finish({
        ok: false,
        error: new Error(`\`${command} --version\` timed out`),
      });
    }, 5000);

    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8");
    });

    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8");
    });

    child.once("error", (error) => {
      finish({ ok: false, error });
    });

    child.once("exit", (code) => {
      if (code === 0) {
        finish({ ok: true, version: firstLine(stdout) || firstLine(stderr) });
      } else {
        finish({
          ok: false,
          error: new Error(
            `\`${command} --version\` exited with status ${code}${stderr ? `: ${stderr.trim()}` : ""}`,
          ),
        });
      }
    });

    function finish(result) {
      if (settled) {
        return;
      }

      settled = true;
      clearTimeout(timeout);
      resolve(result);
    }
  });
}

function firstLine(text) {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean);
}

function showServerStartupWarning(command, error) {
  const detail = formatError(error);
  const message =
    `Could not start the tua language server with \`${command}\`. ` +
    "Install a VSIX bundled with the tua server, set `tua.server.path`, or launch with `TUA_SERVER_PATH`.";

  vscode.window
    .showWarningMessage(`${message}${detail ? ` ${detail}` : ""}`, "Open Settings")
    .then((choice) => {
      if (choice === "Open Settings") {
        vscode.commands.executeCommand("workbench.action.openSettings", "tua.server.path");
      }
    });
}

function showWorkspaceTrustWarning(error) {
  vscode.window
    .showWarningMessage(formatError(error), "Manage Workspace Trust", "Open Settings")
    .then((choice) => {
      if (choice === "Manage Workspace Trust") {
        vscode.commands.executeCommand("workbench.trust.manage");
      } else if (choice === "Open Settings") {
        vscode.commands.executeCommand(COMMANDS.openSettings);
      }
    });
}

module.exports = {
  activate,
  deactivate,
};
