"use strict";

const fs = require("node:fs");
const path = require("node:path");
const {
  LspClient, completionItems, pathToFileURL, positionAfter,
} = require("../../../../../editors/vscode/scripts/lsp-profile-common");

const root = __dirname;
const command = path.resolve(root, "../../../../../target/debug/tua");
const modelPath = path.join(root, "src/model.tua");
const modelUri = pathToFileURL(modelPath).href;
const probeUri = pathToFileURL(path.join(root, "src/editor_probe.tua")).href;
const originalModel = fs.readFileSync(modelPath, "utf8");
const changedModel = originalModel.replace(
  "  get(): T {",
  "  extra(): number { return 7 }\n\n  get(): T {",
);
if (changedModel === originalModel) throw Error("could not insert trial method");
const probe = [
  'local Recipes = require("recipes")',
  'local items = Recipes.ingredients()',
  'local potion = Recipes.mix(items[3], items[4])',
  'if potion then',
  '  local jar = Recipes.seal(potion)',
  '  jar:label()',
  'end',
  '',
].join("\n");
const position = positionAfter(probe, "jar:");

async function main() {
  const client = new LspClient(command, root, { requestTimeoutMs: 20000 });
  await client.start();
  try {
    await client.request("initialize", {
      processId: process.pid,
      rootUri: pathToFileURL(root).href,
      capabilities: {},
      workspaceFolders: [{ uri: pathToFileURL(root).href, name: "alchemy" }],
    });
    client.notify("initialized", {});
    client.notify("textDocument/didOpen", {
      textDocument: { uri: probeUri, languageId: "tua", version: 1, text: probe },
    });
    client.notify("textDocument/didOpen", {
      textDocument: { uri: modelUri, languageId: "tua", version: 1, text: originalModel },
    });
    await client.waitForDiagnostics(modelUri, { timeoutMs: 20000 });
    const labels = async () => completionItems(await client.request(
      "textDocument/completion", { textDocument: { uri: probeUri }, position },
    )).map((item) => item.label);
    const before = await labels();
    client.notify("textDocument/didChange", {
      textDocument: { uri: modelUri, version: 2 }, contentChanges: [{ text: changedModel }],
    });
    await client.waitForDiagnostics(modelUri, { timeoutMs: 20000 });
    const changed = await labels();
    client.notify("textDocument/didChange", {
      textDocument: { uri: modelUri, version: 3 }, contentChanges: [{ text: originalModel }],
    });
    await client.waitForDiagnostics(modelUri, { timeoutMs: 20000 });
    const recovered = await labels();
    console.log(JSON.stringify({ before, changed, recovered, position }, null, 2));
    await client.request("shutdown", null);
    const done = client.done();
    client.notify("exit", null);
    await done;
  } catch (error) {
    client.child.kill();
    throw error;
  }
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
