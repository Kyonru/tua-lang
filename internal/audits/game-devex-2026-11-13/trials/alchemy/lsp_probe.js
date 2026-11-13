"use strict";

const fs = require("node:fs");
const path = require("node:path");
const {
  LspClient,
  completionItems,
  pathToFileURL,
  positionAfter,
} = require("../../../../../editors/vscode/scripts/lsp-profile-common");

const root = __dirname;
const command = path.resolve(root, "../../../../../target/debug/tua");
const uri = pathToFileURL(path.join(root, "src/editor_probe.tua")).href;
const incomplete = process.argv.includes("--incomplete");
const source = [
  'import type { PotionInstance } from "./model"',
  'local Recipes = require("recipes")',
  'local items = Recipes.ingredients()',
  'local potion = Recipes.mix(items[3], items[4])',
  'if potion then',
  '  local jar = Recipes.seal(potion)',
  incomplete ? '  jar:' : '  jar:label()',
  'end',
  '',
].join("\n");

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
      textDocument: { uri, languageId: "tua", version: 1, text: source },
    });

    const run = async (name, method, position) => {
      const start = performance.now();
      const result = await client.request(method, { textDocument: { uri }, position });
      return { name, ms: +(performance.now() - start).toFixed(2), result };
    };

    const cursor = positionAfter(source, "jar:");
    const completion = await run("cold jar colon", "textDocument/completion", cursor);
    const warmCompletion = await run("warm jar colon", "textDocument/completion", cursor);
    const hover = await run("jar hover", "textDocument/hover", { line: 6, character: 3 });
    const signature = await run("mix signature", "textDocument/signatureHelp", positionAfter(source, "Recipes.mix("));
    const definition = await run("mix definition", "textDocument/definition", positionAfter(source, "Recipes.mi"));
    const diagnostics = await client.waitForDiagnostics(uri, { timeoutMs: 20000 });

    const output = {
      command,
      uri,
      source,
      cursor,
      mode: incomplete ? "incomplete colon" : "completed member call",
      completion: {
        ms: completion.ms,
        labels: completionItems(completion.result).map((item) => item.label),
      },
      warmCompletion: {
        ms: warmCompletion.ms,
        labels: completionItems(warmCompletion.result).map((item) => item.label),
      },
      hover: { ms: hover.ms, result: hover.result },
      signature: { ms: signature.ms, result: signature.result },
      definition: { ms: definition.ms, result: definition.result },
      diagnostics: diagnostics.params?.diagnostics || [],
    };
    fs.writeFileSync(path.join(root, incomplete ? "LSP-incomplete.json" : "LSP.json"), JSON.stringify(output, null, 2) + "\n");
    console.log(JSON.stringify({
      mode: output.mode,
      sourceLine: source.split("\n")[cursor.line],
      cursor,
      completion: output.completion,
      warmCompletion: output.warmCompletion,
      hover: output.hover,
      signature: output.signature,
      definition: output.definition,
      diagnosticCount: output.diagnostics.length,
    }, null, 2));
    await client.request("shutdown", null);
    const done = client.done();
    client.notify("exit", null);
    await done;
  } catch (error) {
    client.child.kill();
    throw error;
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
