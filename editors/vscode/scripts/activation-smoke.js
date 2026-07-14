"use strict";

const assert = require("assert");
const fs = require("fs");
const Module = require("module");
const os = require("os");
const path = require("path");

const extensionPath = path.resolve(__dirname, "..", "src", "extension.js");
const extensionRoot = path.resolve(__dirname, "..");
const originalLoad = Module._load;

main().catch((error) => {
  console.error(error);
  process.exit(1);
});

async function main() {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "tua-vscode-smoke-"));
  const mockServer = path.join(temp, process.platform === "win32" ? "tua-smoke.cmd" : "tua-smoke");
  const workspace = path.join(temp, "workspace");
  const extensionFixture = path.join(temp, "extension");
  const globalStorage = path.join(temp, "global-storage");

  fs.mkdirSync(workspace);
  fs.mkdirSync(extensionFixture);
  writeMockServer(mockServer);

  try {
    await smokeSuccessfulActivation(mockServer, workspace, globalStorage);
    await smokeStartupFailure(path.join(temp, "missing-tua"), workspace);
    await smokeServerSelection(extensionFixture, workspace);
    await smokeUntrustedWorkspaceGuard(workspace);
    await smokeServerPathChangePrompt(extensionFixture, workspace);
  } finally {
    Module._load = originalLoad;
    delete process.env.TUA_SERVER_PATH;
    fs.rmSync(temp, { force: true, recursive: true });
  }
}

async function smokeSuccessfulActivation(mockServer, workspace, globalStorage) {
  const state = createMockState(workspace);
  const previousServerPath = process.env.TUA_SERVER_PATH;
  const sourceSentinel = "DO_NOT_COPY_TUA_SOURCE_CONTENT";

  fs.writeFileSync(path.join(workspace, "private.tua"), `local secret = "${sourceSentinel}"\n`);

  process.env.TUA_SERVER_PATH = mockServer;

  try {
    const extension = loadExtension(state);
    const context = createContext(extensionRoot, globalStorage);

    await extension.activate(context);

    assertRegisteredCommands(state);
    assert.strictEqual(state.clients.length, 1, "activation should create one language client");
    assert.strictEqual(state.clients[0].starts, 1, "activation should start the client");
    assert.strictEqual(
      state.clients[0].serverOptions.run.command,
      mockServer,
      "TUA_SERVER_PATH should take precedence over configured and bundled servers",
    );
    assert.match(state.statusItem.text, /Tua/, "status item should identify Tua");
    assert.strictEqual(
      state.statusItem.command.command,
      "tua.restartServer",
      "running status should restart the server",
    );
    assert(
      state.statusItem.detail.includes("tua 0.0.0-smoke"),
      "status detail should include server version",
    );
    assert.deepStrictEqual(
      state.clients[0].clientOptions.initializationOptions,
      {
        trace: {
          server: "off",
        },
        logVerbosity: "off",
      },
      "activation should pass trace defaults to initialization options",
    );

    await state.commands.get("tua.output.focus")();
    assert.deepStrictEqual(state.outputShows, [true], "output focus command should show the output channel");

    await state.commands.get("tua.copyDebugInfo")();

    const copiedDebugInfo = state.clipboardWrites.at(-1);

    assert(copiedDebugInfo.includes("Server path:"), "debug info should include server path");
    assert(
      copiedDebugInfo.includes("Server version: tua 0.0.0-smoke"),
      "debug info should include server version",
    );
    assert(
      !copiedDebugInfo.includes(sourceSentinel),
      "debug info should not include source file contents",
    );

    await state.commands.get("tua.reportIssue")();

    const issueUrl = state.openedExternals.at(-1).toString();
    const issueBody = decodeURIComponent(issueUrl.split("body=")[1] || "");

    assert(
      issueUrl.startsWith("https://github.com/Kyonru/tua-lang/issues/new?body="),
      "report issue should open the project issue URL",
    );
    assert(
      issueBody.includes("No source code is included."),
      "issue body should tell the user no source code is included",
    );
    assert(issueBody.includes("Server path:"), "issue body should include debug info");
    assert(!issueBody.includes(sourceSentinel), "issue body should not include source file contents");

    await state.commands.get("tua.restartServer")();

    assert.strictEqual(state.clients.length, 2, "restart should create a fresh client");
    assert.strictEqual(state.clients[0].stops, 1, "restart should stop the old client");
    assert.strictEqual(state.clients[1].starts, 1, "restart should start the new client");

    state.setConfiguration("log.verbosity", "info");
    state.configurationListeners[0]?.({
      affectsConfiguration(section) {
        return section === "tua.log.verbosity";
      },
    });
    await waitUntil(() => state.clients[1].notifications.length === 1);

    assert.strictEqual(state.clients.length, 2, "trace updates should not restart the client");
    assert.deepStrictEqual(
      state.clients[1].notifications[0],
      {
        method: "tua/setTrace",
        params: {
          trace: {
            server: "off",
          },
          logVerbosity: "info",
        },
      },
      "trace settings should be sent to the running server",
    );

    await state.commands.get("tua.saveCacheStats")();

    const profileDir = path.join(globalStorage, "profiles");
    const savedProfiles = fs.readdirSync(profileDir).filter((entry) => entry.endsWith(".json"));
    const profilePath = path.join(profileDir, savedProfiles[0] || "");
    const relativeToWorkspace = path.relative(workspace, profilePath);

    assert.strictEqual(savedProfiles.length, 1, "save cache stats should write one profile artifact");
    assert(
      relativeToWorkspace.startsWith("..") || path.isAbsolute(relativeToWorkspace),
      "profile artifacts should be outside the workspace by default",
    );
    assert.deepStrictEqual(
      JSON.parse(fs.readFileSync(profilePath, "utf8")),
      { parse: { hits: 1, misses: 0 } },
      "saved cache stats should contain the server response",
    );

    await state.commands.get("tua.saveCpuProfile")();

    const cpuProfileRequest = state.clients[1].requests.find(
      (request) => request.method === "tua/profileArtifact",
    );

    assert.deepStrictEqual(
      cpuProfileRequest,
      {
        method: "tua/profileArtifact",
        params: {
          kind: "cpuProfile",
          outputDirectory: profileDir,
        },
      },
      "CPU profile command should request an artifact outside the workspace",
    );
    assert(
      state.warningMessages.some((message) => message.includes("CPU profile is not available")),
      "unsupported profile artifacts should fail with a warning",
    );

    await extension.deactivate();

    assert.strictEqual(state.clients[1].stops, 1, "deactivate should stop the active client");
  } finally {
    if (previousServerPath === undefined) {
      delete process.env.TUA_SERVER_PATH;
    } else {
      process.env.TUA_SERVER_PATH = previousServerPath;
    }
  }
}

async function smokeServerSelection(extensionFixture, workspace) {
  const bundledServer = path.join(extensionFixture, "bin", platformExecutableName());
  const workspaceServer = path.join(workspace, "target", "debug", platformExecutableName());

  fs.mkdirSync(path.dirname(bundledServer), { recursive: true });
  fs.mkdirSync(path.dirname(workspaceServer), { recursive: true });
  writeMockServer(bundledServer);
  writeMockServer(workspaceServer);

  const state = createMockState(workspace, {
    quickPickLabel: "Workspace Tua server (debug)",
  });
  const extension = loadExtension(state);
  const context = createContext(extensionFixture);

  await extension.activate(context);

  assert.strictEqual(
    state.clients[0].serverOptions.run.command,
    bundledServer,
    "default server path should resolve to bundled binary when available",
  );

  await state.commands.get("tua.selectServer")();

  assert.strictEqual(state.quickPickCalls.length, 1, "server selection should show a quick pick");
  assert(
    state.quickPickCalls[0].items.some((item) => item.description === workspaceServer),
    "trusted workspace server should be offered",
  );
  assert.deepStrictEqual(
    state.configurationUpdates.at(-1),
    {
      section: "tua",
      key: "server.path",
      value: workspaceServer,
      target: state.vscode.ConfigurationTarget.Workspace,
    },
    "selecting workspace server should persist the selected path",
  );
  assert.strictEqual(state.clients[0].stops, 1, "server selection should stop old client");
  assert.strictEqual(state.clients.length, 2, "server selection should create a fresh client");
  assert.strictEqual(
    state.clients[1].serverOptions.run.command,
    workspaceServer,
    "selected workspace server should be used after restart",
  );

  await extension.deactivate();
}

async function smokeUntrustedWorkspaceGuard(workspace) {
  const workspaceServer = path.join(workspace, "target", "debug", platformExecutableName());

  fs.mkdirSync(path.dirname(workspaceServer), { recursive: true });
  writeMockServer(workspaceServer);

  const state = createMockState(workspace, {
    isTrusted: false,
    serverPath: workspaceServer,
  });
  const extension = loadExtension(state);
  const context = createContext();

  await extension.activate(context);

  assert.strictEqual(state.clients.length, 0, "untrusted workspace server should not be executed");
  assert.match(state.statusItem.text, /failed/, "untrusted workspace server should fail visibly");
  assert(
    state.warningMessages.some((message) => message.includes("Workspace trust is required")),
    "untrusted workspace server should prompt for workspace trust",
  );

  await state.commands.get("tua.selectServer")();

  assert.strictEqual(
    state.executedCommands.at(-1)?.command,
    "workbench.trust.manage",
    "untrusted server selection should route to workspace trust",
  );
}

async function smokeServerPathChangePrompt(extensionFixture, workspace) {
  const bundledServer = path.join(extensionFixture, "bin", platformExecutableName());

  fs.mkdirSync(path.dirname(bundledServer), { recursive: true });
  writeMockServer(bundledServer);

  const state = createMockState(workspace, {
    informationChoice: "Restart",
  });
  const extension = loadExtension(state);
  const context = createContext(extensionFixture);

  await extension.activate(context);
  state.configurationListeners[0]?.({
    affectsConfiguration(section) {
      return section === "tua.server.path";
    },
  });
  await waitUntil(() => state.clients.length === 2);

  assert(
    state.informationMessages.some((message) =>
      message.includes("Tua server path changed. Restart the language server?"),
    ),
    "manual server.path changes should produce an actionable restart prompt",
  );
  assert.strictEqual(state.clients[0].stops, 1, "config prompt restart should stop old client");
  assert.strictEqual(state.clients[1].starts, 1, "config prompt restart should start new client");

  await extension.deactivate();
}

async function smokeStartupFailure(missingServer, workspace) {
  const state = createMockState(workspace);
  const previousServerPath = process.env.TUA_SERVER_PATH;

  process.env.TUA_SERVER_PATH = missingServer;

  try {
    const extension = loadExtension(state);
    const context = createContext();

    await extension.activate(context);

    assertRegisteredCommands(state);
    assert.strictEqual(state.clients.length, 0, "failed startup should not create a client");
    assert.match(state.statusItem.text, /failed/, "failed startup should be visible in status");
    assert.strictEqual(
      state.statusItem.command.command,
      "tua.openSettings",
      "failed status should open settings",
    );

    await state.commands.get("tua.copyDebugInfo")();

    const copiedDebugInfo = state.clipboardWrites.at(-1);

    assert(copiedDebugInfo.includes(missingServer), "failed startup debug info should include server path");
    assert(
      copiedDebugInfo.includes("Last server error:"),
      "failed startup debug info should include the startup failure",
    );
  } finally {
    if (previousServerPath === undefined) {
      delete process.env.TUA_SERVER_PATH;
    } else {
      process.env.TUA_SERVER_PATH = previousServerPath;
    }
  }
}

function platformExecutableName() {
  return process.platform === "win32" ? "tua.exe" : "tua";
}

function writeMockServer(filename) {
  if (process.platform === "win32") {
    fs.writeFileSync(filename, "@echo off\r\necho tua 0.0.0-smoke\r\nexit /b 0\r\n");
    return;
  }

  fs.writeFileSync(
    filename,
    [
      "#!/usr/bin/env node",
      "if (process.argv[2] === '--version') {",
      "  console.log('tua 0.0.0-smoke');",
      "  process.exit(0);",
      "}",
      "process.exit(0);",
      "",
    ].join("\n"),
  );
  fs.chmodSync(filename, 0o755);
}

function loadExtension(state) {
  delete require.cache[extensionPath];

  Module._load = function load(request, parent, isMain) {
    if (request === "vscode") {
      return state.vscode;
    }

    if (request === "vscode-languageclient/node") {
      return {
        LanguageClient: state.LanguageClient,
      };
    }

    return originalLoad.call(this, request, parent, isMain);
  };

  return require(extensionPath);
}

function createMockState(workspace, options = {}) {
  const commands = new Map();
  const clients = [];
  const clipboardWrites = [];
  const configuration = new Map([
    ["server.path", options.serverPath ?? "tua"],
    ["trace.server", options.traceServer ?? "off"],
    ["log.verbosity", options.logVerbosity ?? "off"],
  ]);
  const configurationListeners = [];
  const configurationUpdates = [];
  const executedCommands = [];
  const informationMessages = [];
  const openedExternals = [];
  const outputShows = [];
  const quickPickCalls = [];
  const warningMessages = [];
  let statusItem;

  const vscode = {
    version: "smoke",
    ConfigurationTarget: {
      Global: 1,
      Workspace: 2,
    },
    Disposable: {
      from(...disposables) {
        return {
          dispose() {
            for (const disposable of disposables) {
              disposable.dispose?.();
            }
          },
        };
      },
    },
    LanguageStatusSeverity: {
      Information: 1,
      Warning: 2,
      Error: 3,
    },
    Uri: {
      parse(value) {
        return { toString: () => value };
      },
    },
    commands: {
      registerCommand(command, callback) {
        commands.set(command, callback);
        return { dispose() {} };
      },
      executeCommand(command, ...args) {
        executedCommands.push({ command, args });
        return Promise.resolve();
      },
    },
    env: {
      clipboard: {
        writeText(value) {
          clipboardWrites.push(value);
          return Promise.resolve();
        },
      },
      openExternal(uri) {
        openedExternals.push(uri);
        return Promise.resolve(true);
      },
    },
    languages: {
      createLanguageStatusItem(id, selector) {
        statusItem = {
          id,
          selector,
          dispose() {},
        };
        return statusItem;
      },
    },
    window: {
      createOutputChannel() {
        return {
          appendLine() {},
          dispose() {},
          show(preserveFocus) {
            outputShows.push(preserveFocus);
          },
        };
      },
      showErrorMessage() {
        return Promise.resolve(undefined);
      },
      showInformationMessage(message) {
        informationMessages.push(message);
        return Promise.resolve(options.informationChoice);
      },
      showQuickPick(items) {
        quickPickCalls.push({ items });

        if (options.quickPickLabel) {
          return Promise.resolve(items.find((item) => item.label === options.quickPickLabel));
        }

        return Promise.resolve(items.find((item) => item.kind === "trust"));
      },
      showWarningMessage(message) {
        warningMessages.push(message);
        return Promise.resolve(options.warningChoice);
      },
    },
    workspace: {
      isTrusted: options.isTrusted ?? true,
      workspaceFolders: [
        {
          name: "workspace",
          uri: {
            fsPath: workspace,
            scheme: "file",
          },
        },
      ],
      createFileSystemWatcher() {
        return { dispose() {} };
      },
      getConfiguration() {
        return {
          get(key, defaultValue) {
            return configuration.has(key) ? configuration.get(key) : defaultValue;
          },
          update(key, value, target) {
            configuration.set(key, value);
            configurationUpdates.push({
              section: "tua",
              key,
              value,
              target,
            });
            return Promise.resolve();
          },
        };
      },
      onDidChangeConfiguration(listener) {
        configurationListeners.push(listener);
        return { dispose() {} };
      },
      registerTextDocumentContentProvider() {
        return { dispose() {} };
      },
    },
  };

  class LanguageClient {
    constructor(id, name, serverOptions, clientOptions) {
      this.id = id;
      this.name = name;
      this.serverOptions = serverOptions;
      this.clientOptions = clientOptions;
      this.starts = 0;
      this.stops = 0;
      this.notifications = [];
      this.requests = [];
      clients.push(this);
    }

    start() {
      this.starts += 1;
      return Promise.resolve();
    }

    stop() {
      this.stops += 1;
      return Promise.resolve();
    }

    sendRequest(method, params) {
      this.requests.push({ method, params });

      if (method === "tua/cacheStats") {
        return Promise.resolve({ parse: { hits: 1, misses: 0 } });
      }

      if (method === "tua/profileArtifact") {
        return Promise.reject(new Error("unsupported profile artifact"));
      }

      return Promise.resolve("");
    }

    sendNotification(method, params) {
      this.notifications.push({ method, params });
      return Promise.resolve();
    }
  }

  return {
    get statusItem() {
      return statusItem;
    },
    clipboardWrites,
    clients,
    configurationListeners,
    configurationUpdates,
    commands,
    executedCommands,
    informationMessages,
    LanguageClient,
    openedExternals,
    outputShows,
    quickPickCalls,
    setConfiguration(key, value) {
      configuration.set(key, value);
    },
    vscode,
    warningMessages,
  };
}

function createContext(extensionPath = extensionRoot, globalStoragePath = path.join(os.tmpdir(), "tua-vscode-smoke-storage")) {
  return {
    extensionPath,
    globalStorageUri: {
      fsPath: globalStoragePath,
      scheme: "file",
    },
    subscriptions: [],
  };
}

function assertRegisteredCommands(state) {
  for (const command of [
    "tua.restartServer",
    "tua.output.focus",
    "tua.openSettings",
    "tua.selectServer",
    "tua.copyDebugInfo",
    "tua.copyCacheStats",
    "tua.saveCacheStats",
    "tua.saveCpuProfile",
    "tua.saveHeapSummary",
    "tua.reportIssue",
  ]) {
    assert(state.commands.has(command), `${command} should be registered on activation`);
  }
}

async function waitUntil(predicate) {
  const deadline = Date.now() + 1000;

  while (Date.now() < deadline) {
    if (predicate()) {
      return;
    }

    await new Promise((resolve) => setTimeout(resolve, 10));
  }

  assert(predicate(), "condition should become true before timeout");
}
