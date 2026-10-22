"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { runTests } = require("@vscode/test-electron");
const { localVSCodeExecutable } = require("./vscode-executable");

async function main() {
  const extensionDevelopmentPath = path.resolve(__dirname, "..");
  const extensionTestsPath = path.resolve(__dirname, "suite", "index.js");
  const serverFixturePath = path.resolve(__dirname, "fixtures", "mock-lsp-server.js");
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tua-vscode-test-"));
  const workspaceOne = path.join(tempRoot, "workspace-one");
  const workspaceTwo = path.join(tempRoot, "workspace-two");
  const workspacePath = path.join(tempRoot, "tua-integration.code-workspace");
  const logPath = path.join(tempRoot, "lsp-events.jsonl");

  const fixtures = createWorkspaceFixtures(workspaceOne, workspaceTwo);
  fs.writeFileSync(
    workspacePath,
    `${JSON.stringify(
      {
        folders: [{ path: workspaceOne }, { path: workspaceTwo }],
        settings: {
          "tua.preview.enabled": true,
          "tua.completion.inline.enabled": true,
          "tua.completion.multiToken.allowModelIntegration": false,
        },
      },
      null,
      2,
    )}\n`,
  );
  const serverPath = createServerLauncher(tempRoot, serverFixturePath);

  try {
    await runTests({
      extensionDevelopmentPath,
      extensionTestsPath,
      extensionTestsEnv: {
        TUA_SERVER_PATH: serverPath,
        TUA_TEST_FIXTURES: JSON.stringify(fixtures),
        TUA_TEST_LSP_LOG: logPath,
      },
      launchArgs: [
        workspacePath,
        "--disable-extensions",
        "--disable-workspace-trust",
        `--user-data-dir=${path.join(tempRoot, "user-data")}`,
        `--extensions-dir=${path.join(tempRoot, "extensions")}`,
      ],
      reuseMachineInstall: true,
      vscodeExecutablePath: localVSCodeExecutable(),
    });
  } finally {
    if (process.env.TUA_TEST_KEEP_TEMP === "1") {
      console.log(`kept VS Code integration test data at ${tempRoot}`);
    } else {
      fs.rmSync(tempRoot, { force: true, recursive: true });
    }
  }
}

function createWorkspaceFixtures(workspaceOne, workspaceTwo) {
  const sourceOne = path.join(workspaceOne, "src");
  const sourceTwo = path.join(workspaceTwo, "src");
  const assetDirectory = path.join(sourceOne, "assets");

  fs.mkdirSync(assetDirectory, { recursive: true });
  fs.mkdirSync(sourceTwo, { recursive: true });
  fs.writeFileSync(path.join(workspaceOne, "tua.toml"), 'source = "src"\nout = "build"\n');
  fs.writeFileSync(path.join(workspaceTwo, "tua.toml"), 'source = "src"\nout = "build"\n');

  const fixtures = {
    ecs: path.join(sourceOne, "ecs.tua"),
    intelligence: path.join(sourceOne, "intelligence.tua"),
    moduleGraphRoot: path.join(sourceOne, "module-graph-main.tua"),
    moduleGraphPlayer: path.join(sourceOne, "module_graph_player.tua"),
    moduleGraphShared: path.join(sourceOne, "module_graph_shared.lua"),
    missingSprite: path.join(sourceOne, "missing-sprite.tua"),
    oldModule: path.join(sourceOne, "old_module.tua"),
    playerAsset: path.join(assetDirectory, "player.png"),
    renameSource: path.join(sourceOne, "rename-source.tua"),
    rootTwo: path.join(sourceTwo, "secondary.tua"),
    sprite: path.join(sourceOne, "sprites.tua"),
    suggestions: path.join(sourceOne, "suggestions.tua"),
    workspaceOne,
    workspaceTwo,
  };

  fs.writeFileSync(fixtures.suggestions, "local seed = 1\n\n");
  fs.writeFileSync(
    fixtures.ecs,
    [
      'local Position = ecs.component("Position", { x = "number", y = "number" })',
      'local Velocity = ecs.component("Velocity", { x = "number", y = "number" })',
      'local movement = ecs.system(world, { Position, Velocity }, function(_, position, velocity, dt)',
      '  position.x += velocity.x * dt',
      'end)',
      '',
    ].join("\n"),
  );
  fs.writeFileSync(
    fixtures.intelligence,
    [
      "local target_value = 42",
      "local function greet(name: string): string",
      "  return name",
      "end",
      "local hover_probe = target_value",
      "local call_probe = greet(",
      "local inlay_probe = target_value",
      "local events = signal()",
      'events:on("player.hit", function() end)',
      'events:emit("player.hit")',
      "",
    ].join("\n"),
  );
  fs.writeFileSync(fixtures.oldModule, "return { value = 1 }\n");
  fs.writeFileSync(
    fixtures.moduleGraphRoot,
    [
      'local player = require("module_graph_player")',
      'local missing = require("missing_module")',
      "return { start = player.spawn, missing = missing }",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    fixtures.moduleGraphPlayer,
    'local shared = require("module_graph_shared")\nreturn { Player = {}, shared = shared }\n',
  );
  fs.writeFileSync(
    fixtures.moduleGraphShared,
    'local player = require("module_graph_player")\nreturn { version = 1, player = player }\n',
  );
  fs.writeFileSync(
    fixtures.renameSource,
    'local dependency = require("old_module")\nreturn dependency\n',
  );
  fs.writeFileSync(fixtures.rootTwo, "local root_two_value = 2\nreturn root_two_value\n");
  fs.writeFileSync(
    fixtures.sprite,
    spriteSource("assets/player.png", {
      idle: "{ from = 0, to = 3, fps = 8 }",
    }),
  );
  fs.writeFileSync(
    fixtures.missingSprite,
    spriteSource("assets/missing.png", {
      idle: "{ from = 0, to = 3, fps = 8 }",
    }),
  );
  fs.writeFileSync(
    fixtures.playerAsset,
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  );

  return fixtures;
}

function spriteSource(imagePath, animations) {
  const animationLines = Object.entries(animations).map(
    ([name, definition]) => `    ${name} = ${definition},`,
  );

  return [
    "local Player = spriteSheet({",
    `  image = "${imagePath}",`,
    "  frame = { width = 16, height = 16 },",
    "  animations = {",
    ...animationLines,
    "  },",
    "})",
    "",
  ].join("\n");
}

function createServerLauncher(tempRoot, serverFixturePath) {
  if (process.platform === "win32") {
    const launcherPath = path.join(tempRoot, "tua-vscode-test.cmd");
    fs.writeFileSync(
      launcherPath,
      `@echo off\r\n"${process.execPath}" "${serverFixturePath}" %*\r\n`,
    );
    return launcherPath;
  }

  const launcherPath = path.join(tempRoot, "tua-vscode-test");
  fs.writeFileSync(
    launcherPath,
    `#!/bin/sh\nexec "${process.execPath}" "${serverFixturePath}" "$@"\n`,
  );
  fs.chmodSync(launcherPath, 0o755);
  return launcherPath;
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
