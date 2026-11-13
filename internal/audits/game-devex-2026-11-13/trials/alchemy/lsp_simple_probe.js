"use strict";

const path = require("node:path");
const { LspClient, completionItems, pathToFileURL, positionAfter } =
  require("../../../../../editors/vscode/scripts/lsp-profile-common");

const root = __dirname;
const command = path.resolve(root, "../../../../../target/debug/tua");
const uri = pathToFileURL(path.join(root, "src/simple_editor_probe.tua")).href;
const completed = process.argv.includes("--completed");
const source = 'local object = { label = function(self) end }\n' +
  (completed ? 'object:label()\n' : 'object:\n');
const cursor = positionAfter(source, "object:");

async function main() {
  const client = new LspClient(command, root, { requestTimeoutMs: 20000 });
  await client.start();
  await client.request("initialize", {
    processId: process.pid,
    rootUri: pathToFileURL(root).href,
    capabilities: {},
    workspaceFolders: [{ uri: pathToFileURL(root).href, name: "alchemy" }],
  });
  client.notify("initialized", {});
  client.notify("textDocument/didOpen", {
    textDocument: { uri, languageId: "tua", version: 1, text: source },
  });
  const result = await client.request("textDocument/completion", {
    textDocument: { uri }, position: cursor,
  });
  const diagnostic = await client.waitForDiagnostics(uri, { timeoutMs: 20000 });
  console.log(JSON.stringify({ source, cursor,
    labels: completionItems(result).map((item) => item.label),
    diagnostics: diagnostic.params?.diagnostics || [],
  }, null, 2));
  await client.request("shutdown", null);
  const done = client.done();
  client.notify("exit", null);
  await done;
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
