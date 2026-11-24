"use strict";
// Reuse the repository's protocol client and statistics; this is not widget latency.
const fs = require("fs");
const path = require("path");
const assert = require("node:assert/strict");
const { LspClient, pathToFileURL, completionItems, statsForValues } = require("../../editors/vscode/scripts/lsp-profile-common");
const compiler = path.resolve(process.argv[2]);
const root = path.resolve(process.argv[3]);
const output = path.resolve(process.argv[4]);
const scene = fs.readFileSync(path.join(root, "src/scene.tdialogue"), "utf8");
const characters = fs.readFileSync(path.join(root, "src/characters.tua"), "utf8");
const uri = pathToFileURL(path.join(root, "src/scene.tdialogue")).href;
const characterUri = pathToFileURL(path.join(root, "src/characters.tua")).href;
const line = scene.split("\n").findIndex((line) => line.startsWith("Cast.Guide:"));
const samples = {};
async function measure(client, label, method, position, save = true) {
  const start = process.hrtime.bigint();
  const result = await client.request(method, { textDocument: { uri }, position });
  if (save) (samples[label] ||= []).push(Number(process.hrtime.bigint() - start) / 1e6);
  return result;
}
(async () => {
  for (let trial = 0; trial < 5; trial++) {
    const client = new LspClient(compiler, root, { env: { ...process.env, TUA_PROFILE: "0" } });
    await client.start();
    try {
      await client.request("initialize", { processId: process.pid, rootUri: pathToFileURL(root).href, capabilities: {} });
      client.notify("initialized", {});
      for (const [fileUri, text, languageId] of [[uri, scene, "tdialogue"], [characterUri, characters, "tua"]]) {
        client.notify("textDocument/didOpen", { textDocument: { uri: fileUri, version: 1, languageId, text } });
      }
      const first = await measure(client, "first_completion_after_open", "textDocument/completion", { line, character: 5 });
      assert(completionItems(first).some((item) => item.label === "Guide"));
      for (let n = 0; n < 5; n++) await measure(client, "warmup", "textDocument/completion", { line, character: 5 }, false);
      for (let n = 0; n < 20; n++) {
        const completion = await measure(client, "warm_completion", "textDocument/completion", { line, character: 5 });
        assert(completionItems(completion).some((item) => item.label === "Guide"));
        assert(await measure(client, "warm_hover", "textDocument/hover", { line, character: 7 }));
        const definition = await measure(client, "warm_definition", "textDocument/definition", { line, character: 7 });
        assert(definition && (!Array.isArray(definition) || definition.length > 0));
      }
      for (let n = 0; n < 10; n++) {
        const text = n % 2 === 0 ? characters.replace("return { Guide = Guide }", "return { Guide = Guide, Scout = Guide }") : characters;
        client.notify("textDocument/didChange", { textDocument: { uri: characterUri, version: n + 2 }, contentChanges: [{ text }] });
        const changed = await measure(client, "completion_after_unsaved_import_edit", "textDocument/completion", { line, character: 5 });
        assert.equal(completionItems(changed).some((item) => item.label === "Scout"), n % 2 === 0);
      }
      await client.request("shutdown", null);
      const done = client.done();
      client.notify("exit", null);
      await done;
    } finally {
      if (client.child && client.child.exitCode === null) client.child.kill();
    }
  }
  fs.writeFileSync(output, JSON.stringify({ workload: "one v1 script, typed imported character, five fresh servers", unit: "protocol_roundtrip_ms", instrumentation: "no server profiler; client clock includes transport and scheduling, not editor paint", warmup: 5, repetitions: 5, samples, summary: Object.fromEntries(Object.entries(samples).map(([name, values]) => [name, statsForValues(values)])) }, null, 2) + "\n");
})().catch((error) => { console.error(error); process.exitCode = 1; });
