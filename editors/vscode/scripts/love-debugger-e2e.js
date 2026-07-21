"use strict";

const assert = require("assert");
const childProcess = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { DebugClient } = require("@vscode/debugadapter-testsupport");

const repositoryRoot = path.resolve(__dirname, "../../..");
const debuggerVersion = fs
  .readFileSync(path.join(__dirname, "love-debugger-version.txt"), "utf8")
  .trim();
const requestTimeoutMs = Number.parseInt(
  process.env.TUA_DEBUGGER_TIMEOUT_MS || "20000",
  10,
);
const breakpointLine = 4;

async function main() {
  const startedAt = Date.now();
  const tuaCommand = process.env.TUA_BIN || path.join(repositoryRoot, "target", "debug", "tua");
  const loveCommand = findLoveCommand();
  const debuggerRoot = findDebuggerExtension();
  const adapterPath = path.join(debuggerRoot, "extension", "debugAdapter.js");
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tua-love-debugger-e2e-"));
  let client;
  let targetTerminated = false;

  try {
    const fixture = createFixture(tempRoot);
    buildFixture(tuaCommand, tempRoot);
    assertDebuggerMap(fixture);

    const adapterOutput = [];
    client = new DebugClient(
      process.execPath,
      adapterPath,
      "lua-local",
      {
        cwd: path.dirname(adapterPath),
        env: process.env,
      },
      true,
    );
    client.defaultTimeout = requestTimeoutMs;
    client.on("output", (event) => adapterOutput.push(event.body?.output || ""));

    await client.start();
    const launchConfiguration = {
      type: "lua-local",
      request: "launch",
      name: "Tua LOVE source-map breakpoint e2e",
      program: {
        command: loveCommand,
        communication: "pipe",
      },
      args: [fixture.buildDirectory],
      cwd: tempRoot,
      scriptRoots: [fixture.buildDirectory],
      scriptFiles: debuggerScriptFiles(fixture.buildDirectory),
      stepUnmappedLines: false,
      stopOnEntry: false,
      verbose: true,
    };
    let stackTrace;
    try {
      const results = await client.hitBreakpoint(
        launchConfiguration,
        {
          path: fixture.sourcePath,
          line: breakpointLine,
          verified: true,
        },
        {
          path: fixture.sourcePath,
          line: breakpointLine,
        },
      );
      stackTrace = results[2];
    } catch (error) {
      throw new Error(
        `${error.message}\n${debuggerDiagnostics(adapterOutput)}`,
        { cause: error },
      );
    }

    const mappedFrame = stackTrace.body?.stackFrames?.[0];
    const stoppedAtBreakpoint =
      samePath(mappedFrame?.source?.path, fixture.sourcePath)
      && mappedFrame?.line === breakpointLine;
    assert.ok(
      stoppedAtBreakpoint,
      `LOVE paused at ${mappedFrame?.source?.path || "<unknown>"}:${mappedFrame?.line || 0}, expected ${fixture.sourcePath}:${breakpointLine}\n${debuggerDiagnostics(adapterOutput)}`,
    );

    const threads = await client.threadsRequest();
    const threadId = threads.body?.threads?.[0]?.id;
    assert.ok(threadId, "the stopped LOVE runtime should expose a debug thread");

    const terminated = client.waitForEvent("terminated", requestTimeoutMs);
    await client.continueRequest({ threadId });
    await terminated;
    targetTerminated = true;

    console.log(
      `source-mapped LOVE breakpoint passed in ${Date.now() - startedAt}ms: ${mappedFrame.source.path}:${mappedFrame.line}`,
    );
  } finally {
    if (client) {
      await terminateDebugSession(client, targetTerminated);
    }
    if (process.env.TUA_TEST_KEEP_TEMP === "1") {
      console.log(`kept LOVE debugger test data at ${tempRoot}`);
    } else {
      fs.rmSync(tempRoot, { force: true, recursive: true });
    }
  }
}

function createFixture(root) {
  const sourceDirectory = path.join(root, "src");
  const buildDirectory = path.join(root, "build");
  const sourcePath = path.join(sourceDirectory, "main.tua");

  fs.mkdirSync(sourceDirectory, { recursive: true });
  fs.writeFileSync(
    path.join(root, "tua.toml"),
    [
      'source = "src"',
      'out = "build"',
      'main = "main.tua"',
      'target = "love11-luajit"',
      'extension = "tua"',
      "emit_maps = true",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    sourcePath,
    [
      "local marker: number = 41",
      "",
      "function love.load()",
      "  marker = marker + 1",
      "  if marker == 42 then",
      "    love.event.quit(0)",
      "  end",
      "end",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    path.join(sourceDirectory, "conf.lua"),
    [
      'if os.getenv("LOCAL_LUA_DEBUGGER_VSCODE") == "1" then require("lldebugger").start(); function love.errorhandler(message) error(message, 2) end end',
      "function love.conf(settings)",
      "  settings.window.width = 64",
      "  settings.window.height = 64",
      "  settings.window.visible = false",
      "end",
      "",
    ].join("\n"),
  );

  return { buildDirectory, sourcePath };
}

function buildFixture(tuaCommand, cwd) {
  if (path.isAbsolute(tuaCommand)) {
    assert.ok(fs.existsSync(tuaCommand), `Tua executable was not found at ${tuaCommand}`);
  }

  const result = childProcess.spawnSync(tuaCommand, ["build"], {
    cwd,
    encoding: "utf8",
    env: process.env,
  });
  assert.strictEqual(
    result.status,
    0,
    `failed to build debugger fixture\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
}

function assertDebuggerMap(fixture) {
  const generatedPath = path.join(fixture.buildDirectory, "main.lua");
  const mapPath = `${generatedPath}.map`;

  assert.ok(fs.existsSync(generatedPath), `missing generated LOVE entrypoint: ${generatedPath}`);
  assert.ok(fs.existsSync(mapPath), `missing debugger source map: ${mapPath}`);

  const sourceMap = JSON.parse(fs.readFileSync(mapPath, "utf8"));
  assert.strictEqual(sourceMap.version, 3);
  assert.ok(sourceMap.sources?.includes("src/main.tua"));
  assert.ok(sourceMap.mappings, "debugger source map should contain mappings");
}

function debuggerScriptFiles(directory) {
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

function findDebuggerExtension() {
  const configured = process.env.TUA_DEBUGGER_EXTENSION_PATH;
  const candidates = configured
    ? [configured]
    : [
        path.join(os.homedir(), ".vscode", "extensions"),
        path.join(os.homedir(), ".vscode-insiders", "extensions"),
      ]
        .filter(fs.existsSync)
        .flatMap((directory) =>
          fs
            .readdirSync(directory)
            .filter(
              (entry) =>
                entry === `ismoh-games.second-local-lua-debugger-vscode-${debuggerVersion}`,
            )
            .map((entry) => path.join(directory, entry)),
        );
  const root = candidates.find((candidate) =>
    fs.existsSync(path.join(candidate, "extension", "debugAdapter.js")),
  );

  assert.ok(
    root,
    `Second Local Lua Debugger ${debuggerVersion} was not found; install it or set TUA_DEBUGGER_EXTENSION_PATH`,
  );
  const manifest = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
  assert.strictEqual(manifest.publisher, "ismoh-games");
  assert.strictEqual(manifest.name, "second-local-lua-debugger-vscode");
  assert.strictEqual(
    manifest.version,
    debuggerVersion,
    `expected debugger ${debuggerVersion}, found ${manifest.version}`,
  );

  return root;
}

function findLoveCommand() {
  const candidates = [
    process.env.TUA_LOVE_COMMAND,
    process.platform === "darwin" ? "/Applications/love.app/Contents/MacOS/love" : undefined,
    process.platform === "win32" ? "love.exe" : "love",
  ].filter(Boolean);
  const command = candidates.find(commandWorks);

  assert.ok(command, "LOVE was not found; install LOVE or set TUA_LOVE_COMMAND");
  return command;
}

function commandWorks(command) {
  if (path.isAbsolute(command) && !fs.existsSync(command)) {
    return false;
  }
  const result = childProcess.spawnSync(command, ["--version"], {
    encoding: "utf8",
    timeout: 5000,
  });
  return result.status === 0;
}

function samePath(left, right) {
  return typeof left === "string" && path.resolve(left) === path.resolve(right);
}

function debuggerDiagnostics(output) {
  return `adapter output:\n${output.join("").trim()}`;
}

async function terminateDebugSession(client, targetTerminated) {
  if (!targetTerminated) {
    const terminate = client.terminateRequest({}).catch(() => undefined);
    await Promise.race([
      terminate,
      new Promise((resolve) => setTimeout(resolve, 2000)),
    ]);
  }

  // DebugClient.stop waits forever when this adapter has already terminated
  // its debuggee, so close the pinned test client's adapter process directly.
  if (typeof client.stopAdapter === "function") {
    client.stopAdapter();
  } else {
    await client.stop();
  }
}

main().then(
  () => process.exit(0),
  (error) => {
    console.error(error);
    process.exit(1);
  },
);
