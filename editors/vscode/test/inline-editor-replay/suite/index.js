"use strict";

const assert = require("assert/strict");
const fs = require("fs");
const net = require("net");
const vscode = require("vscode");

const delay = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));

async function run() {
  const extension = vscode.extensions.getExtension("SolenodonteLabs.tua");
  assert(extension, "the Tua extension should be available in the extension host");
  await extension.activate();

  assert.equal(
    vscode.workspace.getConfiguration("tua").get("completion.inline.enabled"),
    true,
    "editor replay workspace must start with local inline completion enabled",
  );
  assert.equal(
    vscode.workspace
      .getConfiguration("tua")
      .get("completion.multiToken.allowModelIntegration"),
    false,
    "editor replay workspace must start with model integration disabled",
  );

  await waitForServer();
  if (process.env.TUA_EDITOR_REPLAY_SKIP_WARMUP !== "1") {
    await waitForPatternCorpusIdle();
  }
  const networkProof = await proveNetworkIsolation();
  fs.writeFileSync(
    process.env.TUA_EDITOR_REPLAY_NETWORK_PROOF,
    `${JSON.stringify(networkProof)}\n`,
  );
  fs.writeFileSync(process.env.TUA_EDITOR_REPLAY_READY, "ready\n");

  let handledAction = 0;
  const deadline = Date.now() + 45_000;
  while (Date.now() < deadline) {
    const action = readJson(process.env.TUA_EDITOR_REPLAY_ACTION);
    if (action && action.id > handledAction) {
      let actionResult = {};
      if (action.type === "prepare-editor") {
        await prepareEditor(action);
      } else if (action.type === "type-text") {
        assert.equal(typeof action.value, "string");
        await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
        const editor = vscode.window.activeTextEditor;
        assert(editor, "typing requires an active editor");
        for (const character of action.value) {
          const position = editor.selection.active;
          const inserted = await editor.edit(
            (builder) => builder.insert(position, character),
            { undoStopBefore: false, undoStopAfter: false },
          );
          assert.equal(inserted, true, "VS Code rejected a replay edit");
          const next = position.translate(0, character.length);
          editor.selection = new vscode.Selection(next, next);
          await delay(25);
        }
      } else if (action.type === "debug-inline-counters") {
        await vscode.commands.executeCommand("tua.copyDebugInfo");
        const debugInfo = await vscode.env.clipboard.readText();
        const match = /^Inline completion counters: (\{.*\})$/mu.exec(debugInfo);
        assert(match, "Tua debug info omitted inline completion counters");
        const counters = JSON.parse(match[1]);
        assert(counters.deterministic, "Tua debug info omitted deterministic counters");
        actionResult = { inlineCounters: counters.deterministic };
      } else if (action.type === "trigger-suggest") {
        await vscode.commands.executeCommand("editor.action.triggerSuggest");
      } else {
        assert.equal(action.type, "trigger-inline");
        await vscode.commands.executeCommand("editor.action.inlineSuggest.trigger");
      }
      handledAction = action.id;
      fs.writeFileSync(
        process.env.TUA_EDITOR_REPLAY_ACK,
        `${JSON.stringify({ id: action.id, triggeredAt: Date.now(), ...actionResult })}\n`,
      );
    }
    if (fs.existsSync(process.env.TUA_EDITOR_REPLAY_DONE)) {
      const editor = vscode.window.activeTextEditor;
      fs.writeFileSync(
        process.env.TUA_EDITOR_REPLAY_RESULT,
        `${JSON.stringify({
          path: editor?.document.uri.fsPath,
          version: editor?.document.version,
          line: editor?.selection.active.line,
          character: editor?.selection.active.character,
          lineText: editor?.document.lineAt(editor.selection.active.line).text,
        })}\n`,
      );
      return;
    }
    await delay(20);
  }

  throw new Error("timed out waiting for the CDP editor-replay driver");
}

async function prepareEditor(action) {
  assert.equal(typeof action.path, "string");
  assert(Number.isInteger(action.line) && action.line >= 0);
  const [folder] = vscode.workspace.workspaceFolders || [];
  assert(folder, "editor replay requires one workspace folder");
  const document = await vscode.workspace.openTextDocument(
    vscode.Uri.joinPath(folder.uri, ...action.path.split("/")),
  );
  const editor = await vscode.window.showTextDocument(document, {
    preserveFocus: false,
    preview: false,
  });
  assert.equal(
    vscode.workspace.getConfiguration("editor", document.uri).get("suggest.preview"),
    true,
    "the Tua language default must enable suggestion preview",
  );
  const position = document.lineAt(action.line).range.end;
  editor.selection = new vscode.Selection(position, position);
  editor.revealRange(new vscode.Range(position, position));
}

async function proveNetworkIsolation() {
  assertNotAborted();
  const loopback = await proveLoopback();
  assertNotAborted();
  const outbound = await new Promise((resolve, reject) => {
    const socket = net.createConnection({ host: "1.1.1.1", port: 53 });
    const timer = setTimeout(() => {
      socket.destroy();
      reject(new Error("outbound network probe timed out instead of being denied"));
    }, 1_000);
    socket.once("connect", () => {
      clearTimeout(timer);
      socket.destroy();
      reject(new Error("outbound network probe unexpectedly connected"));
    });
    socket.once("error", (error) => {
      clearTimeout(timer);
      if (error.code !== "EPERM" && error.code !== "EACCES") {
        reject(new Error(`outbound network probe failed inconclusively: ${error.code}`));
        return;
      }
      resolve({ denied: true, errorCode: error.code });
    });
  });
  return {
    measuredAt: new Date().toISOString(),
    processId: process.pid,
    loopback,
    outbound,
  };
}

async function proveLoopback() {
  const server = net.createServer();
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const port = server.address().port;
  try {
    await new Promise((resolve, reject) => {
      const socket = net.createConnection({ host: "127.0.0.1", port });
      const timer = setTimeout(() => {
        socket.destroy();
        reject(new Error("loopback network probe timed out"));
      }, 1_000);
      socket.once("connect", () => {
        clearTimeout(timer);
        socket.destroy();
        resolve();
      });
      socket.once("error", (error) => {
        clearTimeout(timer);
        reject(error);
      });
    });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
  return { connected: true, address: "127.0.0.1", port };
}

async function waitForServer() {
  const deadline = Date.now() + 20_000;
  let last = "";
  while (Date.now() < deadline) {
    assertNotAborted();
    try {
      await vscode.commands.executeCommand("tua.copyDebugInfo");
      last = await vscode.env.clipboard.readText();
      if (last.includes("Server state: running") && !last.includes("Last server error:")) {
        return;
      }
    } catch {}
    await delay(50);
  }
  throw new Error(`Tua language server did not start. Last debug info: ${last}`);
}

async function waitForPatternCorpusIdle() {
  const deadline = Date.now() + 30_000;
  let last;
  while (Date.now() < deadline) {
    assertNotAborted();
    try {
      await vscode.commands.executeCommand("tua.copyCacheStats");
      last = JSON.parse(await vscode.env.clipboard.readText());
      const corpus = last.patternCorpus;
      const warm = last.workspaceWarm;
      if (
        corpus &&
        warm &&
        warm.queuedHigh === 0 &&
        warm.queuedLow === 0 &&
        warm.warmed + warm.failed >= warm.discovered &&
        corpus.queued === 0 &&
        corpus.inFlight === 0 &&
        corpus.remergePending === false &&
        corpus.remergeInFlight === false
      ) {
        return;
      }
    } catch {}
    await delay(100);
  }
  throw new Error(`Tua pattern corpus did not become idle: ${JSON.stringify(last)}`);
}

function assertNotAborted() {
  if (fs.existsSync(process.env.TUA_EDITOR_REPLAY_DONE)) {
    throw new Error("editor replay startup was aborted by the driver");
  }
}

function readJson(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch (error) {
    if (error.code === "ENOENT" || error instanceof SyntaxError) {
      return undefined;
    }
    throw error;
  }
}

module.exports = { run };
