"use strict";

const assert = require("assert");
const fs = require("fs");
const Module = require("module");
const os = require("os");
const path = require("path");
const vm = require("vm");

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
    await smokeDebugLspLogging(mockServer, workspace);
    await smokeStartupFailure(path.join(temp, "missing-tua"), workspace);
    await smokeServerSelection(extensionFixture, workspace);
    await smokeUntrustedWorkspaceGuard(workspace);
    await smokeServerPathChangePrompt(extensionFixture, workspace);
  } finally {
    Module._load = originalLoad;
    delete process.env.TUA_SERVER_PATH;
    delete process.env.TUA_DEBUG_LSP;
    fs.rmSync(temp, { force: true, recursive: true });
  }
}

async function smokeSuccessfulActivation(mockServer, workspace, globalStorage) {
  const state = createMockState(workspace, { quickPickLabel: "Types Only" });
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
      typeof state.clients[0].clientOptions.middleware?.provideInlayHints,
      "function",
      "activation should install the inlay hint mode middleware",
    );
    assert.deepStrictEqual(
      state.clients[0].clientOptions.documentSelector,
      [
        { scheme: "file", language: "tua" },
        { scheme: "file", pattern: "**/*.tua" },
        { scheme: "untitled", language: "tua" },
      ],
      "document selector should cover .tua files even when VS Code language mode is wrong",
    );

    assert.strictEqual(
      state.documentChangeListeners.length,
      0,
      "activation should leave automatic suggestions to VS Code instead of observing every edit",
    );

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

    await waitUntil(
      () => state.treeViews[0]?.treeDataProvider.getChildren().length === 2,
    );
    assert(
      state.clients[0].requests.some((request) => request.method === "tua/assetInventory"),
      "activation should load compiler-backed asset inventory after the server starts",
    );
    assert.strictEqual(state.treeViews.length, 1, "activation should create one Asset Explorer");
    assert.strictEqual(state.treeViews[0].id, "tua.assets");

    const assetCategories = state.treeViews[0].treeDataProvider.getChildren();
    const imageCategory = assetCategories.find((node) => node.category === "image");
    const audioCategory = assetCategories.find((node) => node.category === "audio");
    const usedGroup = state.treeViews[0].treeDataProvider
      .getChildren(imageCategory)
      .find((node) => node.usage === "used");
    const unusedGroup = state.treeViews[0].treeDataProvider
      .getChildren(audioCategory)
      .find((node) => node.usage === "unused");
    const usedAsset = state.treeViews[0].treeDataProvider.getChildren(usedGroup)[0];
    const unusedAsset = state.treeViews[0].treeDataProvider.getChildren(unusedGroup)[0];
    const assetReference = state.treeViews[0].treeDataProvider.getChildren(usedAsset)[0];

    assert.strictEqual(usedAsset.asset.path, "assets/player.png");
    assert.strictEqual(unusedAsset.asset.path, "assets/jump.wav");
    assert.strictEqual(assetReference.reference.loaderKind, "image");
    assert.strictEqual(
      state.diagnosticCollections[0].entries.get(unusedAsset.asset.uri)[0].code,
      "TL2009",
      "unused asset diagnostics should be published from the compiler inventory",
    );

    await state.commands.get("tua.assets.findReferences")(usedAsset);
    await state.commands.get("tua.assets.openReference")(assetReference);
    await state.commands.get("tua.assets.rename")(usedAsset);
    await state.commands.get("tua.assets.reveal")(unusedAsset);

    assert(
      state.executedCommands.some(
        (entry) =>
          entry.command === "editor.action.showReferences" &&
          entry.args[0].toString().endsWith("/src/main.tua"),
      ),
      "Asset Explorer should anchor VS Code's references view in source",
    );
    assert(
      state.executedCommands.some(
        (entry) =>
          entry.command === "vscode.open" &&
          entry.args[0].toString().endsWith("/src/main.tua"),
      ),
      "Asset Explorer references should navigate to the compiler range",
    );
    assert(
      state.executedCommands.some((entry) => entry.command === "revealInExplorer"),
      "Asset Explorer should reveal assets for normal VS Code file rename",
    );
    assert.strictEqual(
      state.workspaceEdits[0].renames[0].newUri.fsPath,
      path.join(workspace, "assets", "hero.png"),
      "Asset Explorer rename should use a workspace file edit so LSP participants can update paths",
    );

    await state.commands.get("tua.output.focus")();
    assert.deepStrictEqual(state.outputShows, [true], "output focus command should show the output channel");

    await state.commands.get("tua.source.organizeImports")();
    await state.commands.get("tua.source.sortImports")();
    await state.commands.get("tua.source.removeUnusedImports")();
    await state.commands.get("tua.source.fixAll")();

    assert.deepStrictEqual(
      state.executedCommands.slice(-4),
      [
        {
          command: "editor.action.codeAction",
          args: [{ kind: "source.organizeImports", apply: "first" }],
        },
        {
          command: "editor.action.codeAction",
          args: [{ kind: "source.sortImports", apply: "first" }],
        },
        {
          command: "editor.action.codeAction",
          args: [{ kind: "source.removeUnusedImports", apply: "first" }],
        },
        {
          command: "editor.action.codeAction",
          args: [{ kind: "source.fixAll", apply: "first" }],
        },
      ],
      "source-action commands should dispatch matching VS Code code actions",
    );

    state.setConfiguration("preview.enabled", true);
    fs.mkdirSync(path.join(workspace, "src", "assets"), { recursive: true });
    fs.writeFileSync(path.join(workspace, "src", "assets", "player.png"), Buffer.from([0x89, b("P"), b("N"), b("G")]));
    const spriteDocument = mockDocument(
      path.join(workspace, "src", "sprites.tua"),
      "tua",
      "file",
      `local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = { from = 0, to = 3, fps = 8 },
  },
})
`,
    );
    state.setActiveDocument(spriteDocument);

    assert.strictEqual(
      await state.commands.get("tua.inlayHints.selectMode")(),
      "types",
      "the inlay hint command should return the selected mode",
    );
    assert.deepStrictEqual(
      state.quickPickCalls.at(-1).items.map((item) => item.label),
      ["Full", "Compact", "Types Only", "Parameters Only", "Off"],
      "the inlay hint command should expose all presentation modes in one quick pick",
    );

    const explanation = await state.commands.get("tua.explainType")();
    assert.strictEqual(explanation.type, "any");
    assert(
      state.quickPickCalls.at(-1).options.title.includes("Why is sprite any?"),
      "an any explanation should use the diagnostic command title",
    );
    assert(
      state.quickPickCalls.at(-1).items.some((item) => item.detail.includes("dynamic escape hatch")),
      "the type explanation should show compiler-provided inference details",
    );

    const initialPreview = await state.commands.get("tua.previewSprites")();

    assert.deepStrictEqual(
      initialPreview,
      { animationCount: 1, missingAssetCount: 0, spriteCount: 1 },
      "sprite preview should report the rendered sprite summary",
    );

    assert.strictEqual(
      state.webviewPanels.length,
      1,
      `sprite preview command should open one webview panel; warnings=${state.warningMessages.join(" | ")} information=${state.informationMessages.join(" | ")}`,
    );
    assert(
      state.webviewPanels[0].html.includes("Player") &&
        state.webviewPanels[0].html.includes("assets/player.png") &&
        state.webviewPanels[0].html.includes("Sprite sheet") &&
        state.webviewPanels[0].html.includes("animation-tabs") &&
        state.webviewPanels[0].html.includes("Fit") &&
        state.webviewPanels[0].html.includes("Reload") &&
        state.webviewPanels[0].html.indexOf('class="stage"') <
          state.webviewPanels[0].html.indexOf('class="inspector"'),
      "sprite preview webview should include parsed sprite metadata and preview controls",
    );
    assert.strictEqual(
      typeof state.webviewPanels[0].messageListener,
      "function",
      "sprite preview webview should handle reload messages",
    );

    spriteDocument.setText(
      spriteDocument.getTextContent().replace(
        "idle = { from = 0, to = 3, fps = 8 },",
        [
          "idle = { from = 0, to = 3, fps = 8 },",
          "    run = { from = 4, to = 9, fps = 12 },",
        ].join("\n"),
      ),
    );
    const reloadResult = await state.commands.get("tua.previewSprites.reload")();
    assert.deepStrictEqual(
      reloadResult,
      { animationCount: 2, missingAssetCount: 0, spriteCount: 1 },
      "sprite preview reload should use the latest document contents",
    );
    assert(
      state.webviewPanels[0].html.includes('"name":"run"'),
      "sprite preview reload should render newly added animations",
    );

    await state.commands.get("tua.previewSprites")();
    assert.strictEqual(
      state.webviewPanels.length,
      1,
      "reopening the sprite preview should reuse the existing panel",
    );
    assert.deepStrictEqual(
      state.webviewPanels[0].reveals,
      [
        { column: 2, preserveFocus: false },
        { column: 2, preserveFocus: false },
      ],
      "reload and reopen should reveal the preview without preserving focus",
    );

    spriteDocument.setText(
      spriteDocument.getTextContent().replace(
        "run = { from = 4, to = 9, fps = 12 },",
        [
          "run = { from = 4, to = 9, fps = 12 },",
          "    jump = { from = 10, to = 12, fps = 10 },",
        ].join("\n"),
      ),
    );
    await state.webviewPanels[0].messageListener({ command: "reload" });
    assert(
      state.webviewPanels[0].html.includes('"name":"jump"'),
      "the webview reload message should render current document contents",
    );
    assert.deepStrictEqual(
      state.webviewPanels[0].reveals.at(-1),
      { column: 2, preserveFocus: false },
      "the webview reload message should reveal the refreshed preview",
    );

    const missingSpriteDocument = mockDocument(
      path.join(workspace, "src", "missing-sprites.tua"),
      "tua",
      "file",
      spriteDocument.getTextContent().replace("assets/player.png", "assets/missing.png"),
    );
    state.setActiveDocument(missingSpriteDocument);
    const missingPreview = await state.commands.get("tua.previewSprites")();
    assert.deepStrictEqual(
      missingPreview,
      { animationCount: 3, missingAssetCount: 1, spriteCount: 1 },
      "sprite preview should report missing assets without failing to open",
    );
    assert(
      state.webviewPanels[0].html.includes('"imagePath":"assets/missing.png"') &&
        state.webviewPanels[0].html.includes("Could not find ") &&
        state.webviewPanels[0].html.includes("TL5004") &&
        state.webviewPanels[0].html.includes("missing sprite image asset"),
      "sprite preview should render a clear missing-asset state",
    );

    state.setActiveDocument(spriteDocument);
    const spriteHover = await state.hoverProviders
      .at(-1)
      ?.provider.provideHover(spriteDocument, { line: 0, character: 8 });

    assert(
      spriteHover?.contents?.[0]?.value.includes("command:tua.previewSprites") &&
        spriteHover.contents[0].value.includes("Preview Player"),
      "sprite sheet variable hover should link to the preview command",
    );
    assert.deepStrictEqual(
      spriteHover.contents[0].isTrusted,
      { enabledCommands: ["tua.previewSprites"] },
      "sprite preview hover should trust only the preview command",
    );
    assert(
      state.clients[0].requests.filter((request) => request.method === "tua/inspect").length >= 5,
      "sprite preview rendering, reload, and hover should request compiler-backed inspection data",
    );

    await state.commands.get("tua.showSignalGraph")();

    assert.strictEqual(state.webviewPanels.length, 2, "signal graph should open one webview panel");
    const signalPanel = state.webviewPanels[1];
    assert(
      signalPanel.html.includes("Signal Graph") &&
        signalPanel.html.includes("player.hit") &&
        signalPanel.html.includes("Listeners") &&
        signalPanel.html.includes("Emitters") &&
        signalPanel.html.includes("Create typed listener") &&
        signalPanel.html.includes("Create declaration from emit") &&
        signalPanel.html.includes("schema conflict") &&
        signalPanel.html.includes("openLocation"),
      "signal graph should render event, file, role, and navigation data",
    );
    assert(
      state.clients[0].requests.some((request) => request.method === "tua/signalGraph"),
      "signal graph should request compiler-backed graph data",
    );

    await signalPanel.messageListener({
      command: "openLocation",
      uri: `file://${path.join(workspace, "src", "events.tua")}`,
      range: {
        start: { line: 1, character: 13 },
        end: { line: 1, character: 25 },
      },
    });
    assert(
      state.executedCommands.some((entry) => entry.command === "vscode.open"),
      "signal graph call locations should open in the editor",
    );

    await signalPanel.messageListener({
      command: "createTypedListener",
      eventId: "events:player.hit",
      callId: "events-player-hit-declare",
    });
    await signalPanel.messageListener({
      command: "createDeclarationFromEmit",
      eventId: "events:enemy.spawned",
      callId: "events-enemy-spawned-emit",
    });

    const signalInsertions = state.workspaceEdits.flatMap((edit) => edit.insertions || []);
    assert(
      signalInsertions.some((insertion) =>
        insertion.newText.includes(
          'events:on("player.hit", function(player: Player, damage: number)',
        ),
      ),
      "signal graph should create typed listener snippets from resolved payloads",
    );
    assert(
      signalInsertions.some((insertion) =>
        insertion.newText.includes(
          'events:declare("enemy.spawned", function(enemy: Enemy, x: number) end)',
        ),
      ),
      "signal graph should create declaration snippets from inferred emit payloads",
    );

    await state.commands.get("tua.whyModuleIncluded")();

    assert.strictEqual(state.webviewPanels.length, 3, "module graph should open one webview panel");
    const modulePanel = state.webviewPanels[2];
    assert(
      modulePanel.html.includes("Module Graph") &&
        modulePanel.html.includes("Unresolved") &&
        modulePanel.html.includes("Why included") &&
        modulePanel.html.includes('"status":"missing"') &&
        modulePanel.html.includes('"name":"spawn"') &&
        modulePanel.html.includes('"initialTab":"why"') &&
        modulePanel.html.includes(`"focusUri":"${spriteDocument.uri.toString()}"`) &&
        modulePanel.html.includes("openLocation"),
      "module graph should render relationships, unresolved imports, exports, inclusion paths, and navigation",
    );
    assertWebviewScriptsParse(modulePanel.html, "module graph");
    assert(
      state.clients[0].requests.some((request) => request.method === "tua/moduleGraph"),
      "module graph should request compiler-backed graph data",
    );

    await modulePanel.messageListener({
      command: "openLocation",
      uri: spriteDocument.uri.toString(),
      range: {
        start: { line: 0, character: 6 },
        end: { line: 0, character: 12 },
      },
    });
    assert.strictEqual(
      state.executedCommands.at(-1).command,
      "vscode.open",
      "module graph import and export locations should open in the editor",
    );

    await state.commands.get("tua.showEcsExplorer")();

    assert.strictEqual(state.webviewPanels.length, 4, "ECS Explorer should open one webview panel");
    const ecsPanel = state.webviewPanels[3];
    assert(
      ecsPanel.html.includes("ECS Explorer") &&
        ecsPanel.html.includes('"runtimeName":"Position"') &&
        ecsPanel.html.includes('"access":"readWrite"') &&
        ecsPanel.html.includes("Relevant queries") &&
        ecsPanel.html.includes("Insight, not a diagnostic") &&
        ecsPanel.html.includes("openLocation"),
      "ECS Explorer should render components, system access, queries, insights, and navigation",
    );
    assertWebviewScriptsParse(ecsPanel.html, "ECS Explorer");
    assert(
      state.clients[0].requests.some((request) => request.method === "tua/ecsGraph"),
      "ECS Explorer should request compiler-backed graph data",
    );

    await ecsPanel.messageListener({
      command: "openLocation",
      uri: `file://${path.join(workspace, "src", "ecs.tua")}`,
      range: {
        start: { line: 0, character: 6 },
        end: { line: 0, character: 14 },
      },
    });
    assert.strictEqual(
      state.executedCommands.at(-1).command,
      "vscode.open",
      "ECS component and query locations should open in the editor",
    );

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

    await state.commands.get("tua.resetCompletionStats")();

    const resetRequest = state.clients[1].requests.find(
      (request) => request.method === "tua/resetCompletionStats",
    );

    assert.deepStrictEqual(
      resetRequest,
      {
        method: "tua/resetCompletionStats",
        params: undefined,
      },
      "reset completion stats command should send the LSP reset request",
    );
    assert(
      state.informationMessages.some((message) =>
        message.includes("Tua completion learning stats reset"),
      ),
      "reset completion stats should report success",
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

async function smokeDebugLspLogging(mockServer, workspace) {
  const state = createMockState(workspace);
  const previousServerPath = process.env.TUA_SERVER_PATH;
  const previousDebugLsp = process.env.TUA_DEBUG_LSP;

  process.env.TUA_SERVER_PATH = mockServer;
  process.env.TUA_DEBUG_LSP = "1";

  try {
    const extension = loadExtension(state);
    const context = createContext();

    await extension.activate(context);

    assert.deepStrictEqual(
      state.clients[0].clientOptions.initializationOptions,
      {
        trace: {
          server: "messages",
        },
        logVerbosity: "debug",
      },
      "debug LSP env flag should enable server trace and debug logs",
    );
    assert(
      state.outputLines.some((line) => line.includes("TUA_DEBUG_LSP=1 enabled")),
      "debug LSP env flag should announce extension-side debug logging",
    );
    assert(
      state.outputLines.some((line) => line.includes("language client initialization options")),
      "debug LSP env flag should log the trace settings sent to the server",
    );

    assert.strictEqual(
      state.documentChangeListeners.length,
      0,
      "debug logging should not restore the removed suggestion edit listener",
    );

    await extension.deactivate();
  } finally {
    if (previousServerPath === undefined) {
      delete process.env.TUA_SERVER_PATH;
    } else {
      process.env.TUA_SERVER_PATH = previousServerPath;
    }

    if (previousDebugLsp === undefined) {
      delete process.env.TUA_DEBUG_LSP;
    } else {
      process.env.TUA_DEBUG_LSP = previousDebugLsp;
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

function mockDocument(filename, languageId = "tua", scheme = "file", text = "") {
  let version = 1;

  return {
    fileName: filename,
    languageId,
    get version() {
      return version;
    },
    getText(range) {
      if (!range) {
        return text;
      }

      return text.slice(positionOffset(text, range.start), positionOffset(text, range.end));
    },
    getWordRangeAtPosition(position) {
      const line = text.split(/\r?\n/)[position.line] || "";
      let start = Math.max(0, Math.min(position.character, line.length));
      let end = start;

      while (start > 0 && /[A-Za-z0-9_]/.test(line[start - 1])) {
        start -= 1;
      }

      while (end < line.length && /[A-Za-z0-9_]/.test(line[end])) {
        end += 1;
      }

      if (start === end) {
        return undefined;
      }

      return {
        start: { line: position.line, character: start },
        end: { line: position.line, character: end },
      };
    },
    setText(value) {
      text = value;
      version += 1;
    },
    getTextContent() {
      return text;
    },
    uri: {
      fsPath: filename,
      path: filename,
      scheme,
      toString() {
        return `${scheme}://${filename}`;
      },
    },
  };
}

function positionOffset(text, position) {
  const lines = text.split(/\r?\n/);
  let offset = 0;

  for (let line = 0; line < position.line; line += 1) {
    offset += (lines[line] || "").length + 1;
  }

  return offset + position.character;
}

function b(character) {
  return character.charCodeAt(0);
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
  clearExtensionModuleCache();

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

function clearExtensionModuleCache() {
  const sourceRoot = path.join(extensionRoot, "src") + path.sep;

  for (const modulePath of Object.keys(require.cache)) {
    if (modulePath === extensionPath || modulePath.startsWith(sourceRoot)) {
      delete require.cache[modulePath];
    }
  }
}

function createMockState(workspace, options = {}) {
  const commands = new Map();
  const clients = [];
  const clipboardWrites = [];
  const configuration = new Map([
    ["server.path", options.serverPath ?? "tua"],
    ["trace.server", options.traceServer ?? "off"],
    ["log.verbosity", options.logVerbosity ?? "off"],
    ["preview.enabled", options.previewEnabled ?? false],
  ]);
  const configurationListeners = [];
  const diagnosticCollections = [];
  const documentChangeListeners = [];
  const documentSaveListeners = [];
  const configurationUpdates = [];
  const executedCommands = [];
  const fileWatchers = [];
  const informationMessages = [];
  const openedExternals = [];
  const outputLines = [];
  const outputShows = [];
  const quickPickCalls = [];
  const hoverProviders = [];
  const treeViews = [];
  const webviewPanels = [];
  const warningMessages = [];
  const workspaceEdits = [];
  let activeTextEditor;
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
    Diagnostic: class Diagnostic {
      constructor(range, message, severity) {
        this.range = range;
        this.message = message;
        this.severity = severity;
      }
    },
    DiagnosticSeverity: {
      Error: 0,
      Warning: 1,
      Information: 2,
      Hint: 3,
    },
    EventEmitter: class EventEmitter {
      constructor() {
        this.listeners = [];
        this.event = (listener) => {
          this.listeners.push(listener);
          return { dispose() {} };
        };
      }

      dispose() {
        this.listeners = [];
      }

      fire(value) {
        for (const listener of this.listeners) {
          listener(value);
        }
      }
    },
    Hover: class Hover {
      constructor(contents, range) {
        this.contents = Array.isArray(contents) ? contents : [contents];
        this.range = range;
      }
    },
    MarkdownString: class MarkdownString {
      constructor(value = "", supportThemeIcons = false) {
        this.value = value;
        this.supportThemeIcons = supportThemeIcons;
      }
    },
    Position: class Position {
      constructor(line, character) {
        this.line = line;
        this.character = character;
      }
    },
    Range: class Range {
      constructor(start, end) {
        this.start = start;
        this.end = end;
      }
    },
    Location: class Location {
      constructor(uri, range) {
        this.uri = uri;
        this.range = range;
      }
    },
    ThemeIcon: class ThemeIcon {
      constructor(id) {
        this.id = id;
      }
    },
    TreeItem: class TreeItem {
      constructor(label, collapsibleState = 0) {
        this.label = label;
        this.collapsibleState = collapsibleState;
      }
    },
    TreeItemCollapsibleState: {
      None: 0,
      Collapsed: 1,
      Expanded: 2,
    },
    Uri: {
      file(filename) {
        return {
          fsPath: filename,
          path: filename,
          scheme: "file",
          toString() {
            return `file://${filename}`;
          },
        };
      },
      parse(value) {
        const fsPath = value.startsWith("file://") ? decodeURIComponent(value.slice(7)) : undefined;

        return {
          fsPath,
          path: fsPath,
          scheme: value.split(":", 1)[0],
          toString: () => value,
        };
      },
      joinPath(base, ...segments) {
        const filename = path.resolve(base.fsPath, ...segments);
        return {
          fsPath: filename,
          path: filename,
          scheme: base.scheme,
          toString() {
            return `${base.scheme}://${filename}`;
          },
        };
      },
    },
    WorkspaceEdit: class WorkspaceEdit {
      constructor() {
        this.renames = [];
        this.insertions = [];
      }

      renameFile(oldUri, newUri, renameOptions) {
        this.renames.push({ oldUri, newUri, options: renameOptions });
      }

      insert(uri, position, newText) {
        this.insertions.push({ uri, position, newText });
      }
    },
    ViewColumn: {
      Beside: 2,
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
      createDiagnosticCollection(name) {
        const collection = {
          name,
          entries: new Map(),
          clear() {
            collection.entries.clear();
          },
          dispose() {
            collection.entries.clear();
          },
          set(uri, diagnostics) {
            collection.entries.set(uri.toString(), diagnostics);
          },
        };

        diagnosticCollections.push(collection);
        return collection;
      },
      createLanguageStatusItem(id, selector) {
        statusItem = {
          id,
          selector,
          dispose() {},
        };
        return statusItem;
      },
      registerHoverProvider(selector, provider) {
        hoverProviders.push({ selector, provider });
        return { dispose() {} };
      },
    },
    window: {
      get activeTextEditor() {
        return activeTextEditor;
      },
      createOutputChannel() {
        return {
          appendLine(line) {
            outputLines.push(line);
          },
          dispose() {},
          show(preserveFocus) {
            outputShows.push(preserveFocus);
          },
        };
      },
      createTreeView(id, viewOptions) {
        const visibilityListeners = [];
        const view = {
          id,
          treeDataProvider: viewOptions.treeDataProvider,
          showCollapseAll: viewOptions.showCollapseAll,
          dispose() {},
          onDidChangeVisibility(listener) {
            visibilityListeners.push(listener);
            return { dispose() {} };
          },
          fireVisibility(visible) {
            for (const listener of visibilityListeners) {
              listener({ visible });
            }
          },
        };

        treeViews.push(view);
        return view;
      },
      createWebviewPanel(viewType, title, column, options) {
        const panel = {
          viewType,
          title,
          column,
          options,
          html: "",
          reveals: [],
          reveal(revealColumn, preserveFocus) {
            panel.reveals.push({ column: revealColumn, preserveFocus });
          },
          onDidDispose(listener) {
            panel.disposeListener = listener;
            return { dispose() {} };
          },
          dispose() {
            panel.disposeListener?.();
          },
          webview: {
            cspSource: "mock-csp",
            asWebviewUri(uri) {
              return {
                toString() {
                  return `webview:${uri.fsPath}`;
                },
              };
            },
            onDidReceiveMessage(listener) {
              panel.messageListener = listener;
              return { dispose() {} };
            },
          },
        };

        Object.defineProperty(panel.webview, "html", {
          get() {
            return panel.html;
          },
          set(value) {
            panel.html = value;
          },
        });

        webviewPanels.push(panel);
        return panel;
      },
      showErrorMessage() {
        return Promise.resolve(undefined);
      },
      showInformationMessage(message) {
        informationMessages.push(message);
        return Promise.resolve(options.informationChoice);
      },
      showInputBox() {
        return Promise.resolve(options.inputBoxValue ?? "hero.png");
      },
      setStatusBarMessage() {
        return { dispose() {} };
      },
      showQuickPick(items, pickOptions = {}) {
        quickPickCalls.push({ items, options: pickOptions });

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
      createFileSystemWatcher(glob) {
        const createListeners = [];
        const changeListeners = [];
        const deleteListeners = [];
        const watcher = {
          glob,
          dispose() {},
          onDidCreate(listener) {
            createListeners.push(listener);
            return { dispose() {} };
          },
          onDidChange(listener) {
            changeListeners.push(listener);
            return { dispose() {} };
          },
          onDidDelete(listener) {
            deleteListeners.push(listener);
            return { dispose() {} };
          },
        };

        fileWatchers.push(watcher);
        return watcher;
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
      asRelativePath(uri) {
        return uri.fsPath ? path.relative(workspace, uri.fsPath) : uri.toString();
      },
      applyEdit(edit) {
        workspaceEdits.push(edit);
        return Promise.resolve(true);
      },
      onDidChangeConfiguration(listener) {
        configurationListeners.push(listener);
        return { dispose() {} };
      },
      onDidChangeTextDocument(listener) {
        documentChangeListeners.push(listener);
        return { dispose() {} };
      },
      onDidSaveTextDocument(listener) {
        documentSaveListeners.push(listener);
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

      if (method === "tua/resetCompletionStats") {
        return Promise.resolve({ removedFiles: 1 });
      }

      if (method === "tua/profileArtifact") {
        return Promise.reject(new Error("unsupported profile artifact"));
      }

      if (method === "tua/signalGraph") {
        const eventsUri = `file://${path.join(workspace, "src", "events.tua")}`;
        const combatUri = `file://${path.join(workspace, "src", "combat.tua")}`;

        return Promise.resolve({
          events: [
            {
              id: "events:player.hit",
              name: "player.hit",
              bus: "events",
              busUri: eventsUri,
              resolvedPayload: [
                { name: "player", type: "Player" },
                { name: "damage", type: "number" },
              ],
              schemaConflict: false,
              calls: [
                {
                  id: "events-player-hit-declare",
                  uri: eventsUri,
                  receiver: "events",
                  method: "declare",
                  role: "declaration",
                  range: {
                    start: { line: 1, character: 13 },
                    end: { line: 1, character: 25 },
                  },
                  callRange: {
                    start: { line: 1, character: 0 },
                    end: { line: 1, character: 74 },
                  },
                  insertionPosition: { line: 1, character: 0 },
                  indentation: "",
                  payload: [
                    { name: "player", type: "Player" },
                    { name: "damage", type: "number" },
                  ],
                  inferredPayload: null,
                },
                {
                  id: "events-player-hit-emit",
                  uri: combatUri,
                  receiver: "events",
                  method: "emit",
                  role: "emit",
                  range: {
                    start: { line: 8, character: 12 },
                    end: { line: 8, character: 24 },
                  },
                  callRange: {
                    start: { line: 8, character: 0 },
                    end: { line: 8, character: 36 },
                  },
                  insertionPosition: { line: 8, character: 0 },
                  indentation: "",
                  payload: null,
                  inferredPayload: [
                    { name: "player", type: "Player" },
                    { name: "damage", type: "number" },
                  ],
                },
              ],
            },
            {
              id: "events:enemy.spawned",
              name: "enemy.spawned",
              bus: "events",
              busUri: eventsUri,
              resolvedPayload: [
                { name: "enemy", type: "Enemy" },
                { name: "x", type: "number" },
              ],
              schemaConflict: false,
              calls: [
                {
                  id: "events-enemy-spawned-emit",
                  uri: combatUri,
                  receiver: "events",
                  method: "emit",
                  role: "emit",
                  range: {
                    start: { line: 12, character: 12 },
                    end: { line: 12, character: 27 },
                  },
                  callRange: {
                    start: { line: 12, character: 0 },
                    end: { line: 12, character: 44 },
                  },
                  insertionPosition: { line: 12, character: 0 },
                  indentation: "",
                  payload: null,
                  inferredPayload: [
                    { name: "enemy", type: "Enemy" },
                    { name: "x", type: "number" },
                  ],
                },
              ],
            },
            {
              id: "events:score.changed",
              name: "score.changed",
              bus: "events",
              busUri: eventsUri,
              resolvedPayload: null,
              schemaConflict: true,
              calls: [
                {
                  id: "events-score-declare",
                  uri: eventsUri,
                  receiver: "events",
                  method: "declare",
                  role: "declaration",
                  range: {
                    start: { line: 14, character: 16 },
                    end: { line: 14, character: 29 },
                  },
                  callRange: {
                    start: { line: 14, character: 0 },
                    end: { line: 14, character: 68 },
                  },
                  insertionPosition: { line: 14, character: 0 },
                  indentation: "",
                  payload: [{ name: "score", type: "number" }],
                  inferredPayload: null,
                },
                {
                  id: "events-score-listener",
                  uri: combatUri,
                  receiver: "events",
                  method: "on",
                  role: "listener",
                  range: {
                    start: { line: 16, character: 10 },
                    end: { line: 16, character: 23 },
                  },
                  callRange: {
                    start: { line: 16, character: 0 },
                    end: { line: 16, character: 62 },
                  },
                  insertionPosition: { line: 16, character: 0 },
                  indentation: "",
                  payload: [{ name: "score", type: "string" }],
                  inferredPayload: null,
                },
              ],
            },
          ],
        });
      }

      if (method === "tua/moduleGraph") {
        return Promise.resolve(mockModuleGraph(activeTextEditor?.document, workspace));
      }

      if (method === "tua/ecsGraph") {
        return Promise.resolve(mockEcsGraph(workspace));
      }

      if (method === "tua/assetInventory") {
        const player = path.join(workspace, "assets", "player.png");
        const jump = path.join(workspace, "assets", "jump.wav");
        const source = path.join(workspace, "src", "main.tua");

        return Promise.resolve({
          analysisRevision: 1,
          inventory: {
            assets: [
              {
                uri: `file://${player}`,
                path: "assets/player.png",
                category: "image",
                categoryLabel: "Images",
                extension: "png",
                dimensions: { width: 32, height: 16 },
                usageCount: 1,
                references: [
                  {
                    uri: `file://${source}`,
                    range: {
                      start: { line: 2, character: 21 },
                      end: { line: 2, character: 40 },
                    },
                    loaderKind: "image",
                  },
                ],
              },
              {
                uri: `file://${jump}`,
                path: "assets/jump.wav",
                category: "audio",
                categoryLabel: "Audio",
                extension: "wav",
                dimensions: null,
                usageCount: 0,
                references: [],
              },
            ],
            diagnostics: [
              {
                uri: `file://${jump}`,
                diagnostic: {
                  range: {
                    start: { line: 0, character: 0 },
                    end: { line: 0, character: 0 },
                  },
                  severity: 4,
                  code: "TL2009",
                  source: "tua",
                  message: "asset `assets/jump.wav` has no static Tua or Lua references",
                },
              },
            ],
          },
        });
      }

      if (method === "tua/inspect") {
        return Promise.resolve(mockSpriteInspection(activeTextEditor?.document));
      }

      if (method === "tua/explainType") {
        return Promise.resolve(mockTypeExplanation(activeTextEditor?.document));
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
    diagnosticCollections,
    documentChangeListeners,
    documentSaveListeners,
    executedCommands,
    fileWatchers,
    informationMessages,
    hoverProviders,
    LanguageClient,
    openedExternals,
    outputLines,
    outputShows,
    quickPickCalls,
    setActiveDocument(document) {
      activeTextEditor = { document };
    },
    setConfiguration(key, value) {
      configuration.set(key, value);
    },
    fireTextDocumentChange(document, text) {
      activeTextEditor = { document };

      for (const listener of documentChangeListeners) {
        listener({
          document,
          contentChanges: [{ text }],
        });
      }
    },
    vscode,
    treeViews,
    webviewPanels,
    warningMessages,
    workspaceEdits,
  };
}

function mockModuleGraph(document, workspace) {
  const rootUri = document?.uri?.toString?.() || `file://${path.join(workspace, "src", "main.tua")}`;
  const playerUri = `file://${path.join(workspace, "src", "player.tua")}`;
  const sharedUri = `file://${path.join(workspace, "src", "shared.tua")}`;
  const importRange = {
    start: { line: 0, character: 16 },
    end: { line: 0, character: 24 },
  };

  return {
    analysisRevision: 7,
    graph: {
      modules: [
        {
          uri: rootUri,
          sourceKind: "tua",
          root: true,
          inCycle: false,
          dependencyCount: 2,
          unresolvedDependencyCount: 1,
          reverseDependencyCount: 0,
          exports: [
            {
              name: "spawn",
              type: "function(name: string): Player",
              documentation: "Creates a player.",
              origin: "tua",
              range: {
                start: { line: 0, character: 6 },
                end: { line: 0, character: 11 },
              },
            },
          ],
          inclusionPath: { rootUri, steps: [] },
        },
        {
          uri: playerUri,
          sourceKind: "tua",
          root: false,
          inCycle: true,
          dependencyCount: 1,
          unresolvedDependencyCount: 0,
          reverseDependencyCount: 2,
          exports: [{ name: "Player", type: "Player", origin: "tua", range: importRange }],
          inclusionPath: {
            rootUri,
            steps: [
              {
                sourceUri: rootUri,
                targetUri: playerUri,
                specifier: "player",
                kind: "require",
                range: importRange,
              },
            ],
          },
        },
        {
          uri: sharedUri,
          sourceKind: "lua",
          root: false,
          inCycle: true,
          dependencyCount: 1,
          unresolvedDependencyCount: 0,
          reverseDependencyCount: 1,
          exports: [{ name: "version", type: "number", origin: "lua", range: importRange }],
          inclusionPath: {
            rootUri,
            steps: [
              {
                sourceUri: rootUri,
                targetUri: playerUri,
                specifier: "player",
                kind: "require",
                range: importRange,
              },
              {
                sourceUri: playerUri,
                targetUri: sharedUri,
                specifier: "shared",
                kind: "import",
                range: importRange,
              },
            ],
          },
        },
      ],
      edges: [
        {
          sourceUri: rootUri,
          specifier: "player",
          kind: "require",
          range: importRange,
          inCycle: false,
          resolution: { status: "resolved", targetUri: playerUri },
        },
        {
          sourceUri: rootUri,
          specifier: "missing",
          kind: "require",
          range: importRange,
          inCycle: false,
          resolution: {
            status: "missing",
            searchRootUris: [`file://${path.join(workspace, "src")}`],
          },
        },
        {
          sourceUri: playerUri,
          specifier: "shared",
          kind: "import",
          range: importRange,
          inCycle: true,
          resolution: { status: "resolved", targetUri: sharedUri },
        },
        {
          sourceUri: sharedUri,
          specifier: "player",
          kind: "require",
          range: importRange,
          inCycle: true,
          resolution: { status: "resolved", targetUri: playerUri },
        },
      ],
      cycles: [{ id: "cycle:0", moduleUris: [playerUri, sharedUri] }],
    },
  };
}

function mockEcsGraph(workspace) {
  const uri = `file://${path.join(workspace, "src", "ecs.tua")}`;
  const range = {
    start: { line: 0, character: 6 },
    end: { line: 0, character: 14 },
  };
  const componentId = "component:position";
  const systemId = "system:movement";

  return {
    analysisRevision: 12,
    graph: {
      components: [
        {
          id: componentId,
          uri,
          localName: "Position",
          runtimeName: "Position",
          range,
          nameRange: range,
          fields: [{ name: "x", type: "number", range }],
        },
      ],
      systems: [
        {
          id: systemId,
          uri,
          name: "movement",
          range,
          nameRange: range,
          callbackRange: range,
          components: [
            {
              reference: "Position",
              componentId,
              access: "readWrite",
              range,
            },
          ],
        },
      ],
      queries: [
        {
          id: `${systemId}:query`,
          uri,
          name: "movement",
          kind: "system",
          ownerSystemId: systemId,
          range,
          nameRange: range,
          components: [
            {
              reference: "Position",
              componentId,
              access: "readWrite",
              range,
            },
          ],
        },
      ],
      insights: [
        {
          id: `${systemId}:allocation`,
          kind: "callbackAllocation",
          title: "Per-entity callback allocations",
          message: "This is a performance insight, not an error.",
          uri,
          range,
          subjectId: systemId,
        },
      ],
    },
  };
}

function mockSpriteInspection(document) {
  const source = document?.getText?.() || "";
  const name = /\blocal\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*spriteSheet\s*\(/.exec(source)?.[1];

  if (!name) {
    return {
      analysisRevision: 1,
      documentVersion: document?.version,
      items: [],
    };
  }

  const imagePath = /\bimage\s*=\s*["']([^"']+)["']/.exec(source)?.[1] || "";
  const frame = /\bframe\s*=\s*\{[^}]*\bwidth\s*=\s*(\d+)[^}]*\bheight\s*=\s*(\d+)/s.exec(
    source,
  );
  const animations = [...source.matchAll(
    /^[ \t]*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*\{[^\n}]*\bfrom\s*=\s*(\d+)[^\n}]*\bto\s*=\s*(\d+)[^\n}]*\bfps\s*=\s*(\d+(?:\.\d+)?)([^\n}]*)\}/gm,
  )]
    .map((match) => ({
      name: match[1],
      from: Number(match[2]),
      to: Number(match[3]),
      fps: Number(match[4]),
      loop: !/\bloop\s*=\s*false\b/.test(match[5]),
    }));
  const resolvedImagePath = path.resolve(path.dirname(document.uri.fsPath), imagePath);
  const imageExists = fs.existsSync(resolvedImagePath);

  return {
    analysisRevision: 1,
    documentVersion: document.version,
    items: [
      {
        kind: "spriteSheet",
        name,
        range: {
          start: { line: 0, character: 0 },
          end: { line: Math.max(0, source.split(/\r?\n/).length - 1), character: 0 },
        },
        selectionRange: {
          start: { line: 0, character: 6 },
          end: { line: 0, character: 6 + name.length },
        },
        image: {
          path: imagePath,
          uri: imageExists ? `file://${resolvedImagePath}` : null,
          status: imageExists ? "valid" : "missing",
        },
        frame: frame
          ? {
              width: Number(frame[1]),
              height: Number(frame[2]),
            }
          : null,
        animations,
        diagnostics: imageExists
          ? []
          : [
              {
                code: "TL5004",
                message: `missing sprite image asset \`${imagePath}\``,
                severity: 1,
              },
            ],
      },
    ],
  };
}

function mockTypeExplanation(document) {
  return {
    analysisRevision: document?.version || 0,
    documentVersion: document?.version,
    explanation: {
      expression: "sprite",
      type: "any",
      isAny: true,
      range: {
        start: { line: 0, character: 6 },
        end: { line: 0, character: 12 },
      },
      steps: [
        {
          kind: "dynamic",
          title: "Explicit `any` annotation",
          detail: "`sprite` uses Tua's dynamic escape hatch.",
          type: "any",
          uri: document?.uri.toString(),
          range: {
            start: { line: 0, character: 6 },
            end: { line: 0, character: 12 },
          },
        },
      ],
    },
  };
}

function createContext(extensionPath = extensionRoot, globalStoragePath = path.join(os.tmpdir(), "tua-vscode-smoke-storage")) {
  return {
    extensionPath,
    globalState: createMemento(),
    globalStorageUri: {
      fsPath: globalStoragePath,
      scheme: "file",
    },
    subscriptions: [],
    workspaceState: createMemento(),
  };
}

function createMemento() {
  const values = new Map();

  return {
    get(key, defaultValue) {
      return values.has(key) ? values.get(key) : defaultValue;
    },
    update(key, value) {
      if (value === undefined) {
        values.delete(key);
      } else {
        values.set(key, value);
      }
      return Promise.resolve();
    },
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
    "tua.resetCompletionStats",
    "tua.saveCpuProfile",
    "tua.saveHeapSummary",
    "tua.explainType",
    "tua.inlayHints.selectMode",
    "tua.previewSprites",
    "tua.previewSprites.reload",
    "tua.assets.refresh",
    "tua.assets.open",
    "tua.assets.openReference",
    "tua.assets.findReferences",
    "tua.assets.rename",
    "tua.assets.reveal",
    "tua.showSignalGraph",
    "tua.showModuleGraph",
    "tua.showEcsExplorer",
    "tua.whyModuleIncluded",
    "tua.source.organizeImports",
    "tua.source.sortImports",
    "tua.source.removeUnusedImports",
    "tua.source.fixAll",
    "tua.reportIssue",
  ]) {
    assert(state.commands.has(command), `${command} should be registered on activation`);
  }
}

function assertWebviewScriptsParse(html, label) {
  const scripts = [...html.matchAll(/<script[^>]*>([\s\S]*?)<\/script>/g)];

  assert(scripts.length > 0, `${label} should include an executable webview script`);
  for (const [, script] of scripts) {
    new vm.Script(script, { filename: `${label.replaceAll(" ", "-")}-webview.js` });
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

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
