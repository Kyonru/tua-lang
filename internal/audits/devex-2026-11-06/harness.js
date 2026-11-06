"use strict";
// Audit-only helpers; reuse the repository's stdio framing and timeout client.
const fs = require("node:fs"), os = require("node:os"), path = require("node:path");
const { pathToFileURL } = require("node:url");
const repo = path.resolve(__dirname, "../../..");
const { LspClient, delay, completionItems } = require(path.join(repo, "editors/vscode/scripts/lsp-profile-common"));
const binary = path.resolve(process.env.TUA_AUDIT_BINARY || path.join(repo, "target/release/tua"));
function position(text, needle, offset = 0, last = false) {
  const index = (last ? text.lastIndexOf(needle) : text.indexOf(needle));
  if (index < 0) throw new Error(`missing probe anchor ${needle}`);
  const prefix = text.slice(0, index + offset), lines = prefix.split("\n");
  return { line: lines.length - 1, character: lines.at(-1).length }; // JS uses UTF-16
}
function makeProject(files, config = 'source="src"\nout="build"\n') {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-devex-audit-"));
  fs.writeFileSync(path.join(root, "tua.toml"), config);
  for (const [name, text] of Object.entries(files)) { const file = path.join(root, name); fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, text); }
  return root;
}
async function connect(root, { capabilities = {}, folders = [root] } = {}) {
  const client = new LspClient(binary, root, { requestTimeoutMs: 30000 });
  const inbound = [], notifications = [];
  const handle = client.handleMessage.bind(client);
  client.handleMessage = message => {
    if (message.method) notifications.push(message);
    if (message.method && message.id !== undefined) {
      client.send({jsonrpc: "2.0", id: message.id, result: message.method === "workspace/configuration" ? (message.params.items || []).map(() => ({})) : null});
      return;
    }
    handle(message);
  };
  const ask = async (label, method, params) => {
    const start = performance.now();
    try { const result = await client.request(method, params); inbound.push({label, method, params, result, ms: performance.now() - start}); return result; }
    catch(error) { inbound.push({label, method, params, error: String(error), ms: performance.now() - start}); throw error; }
  };
  await client.start();
  const initialized = await ask("initialize", "initialize", { processId: process.pid, rootUri: pathToFileURL(root).href, capabilities, workspaceFolders: folders.map(root => ({uri: pathToFileURL(root).href, name: path.basename(root)})) });
  client.notify("initialized", {});
  const docs = new Map();
  function open(name, text = fs.readFileSync(path.resolve(root, name), "utf8")) {
    const uri = pathToFileURL(path.resolve(root, name)).href;
    const doc = {uri, text, version: 1}; docs.set(uri, doc);
    client.notify("textDocument/didOpen", {textDocument: {uri, text, version: 1, languageId: name.endsWith('.lua') ? 'lua' : 'tua'}});
    return doc;
  }
  function change(doc, text) { doc.text = text; client.notify("textDocument/didChange", {textDocument: {uri: doc.uri, version: ++doc.version}, contentChanges: [{text}]}); }
  async function query(label, method, doc, needle, offset = 0, extras = {}) {
    const params = {textDocument: {uri: doc.uri}, ...extras};
    if (needle !== undefined) params.position = position(doc.text, needle, offset, true);
    return ask(label, method, params);
  }
  async function close() { try { await client.request("shutdown", null); client.notify("exit", null); } finally { client.child.kill(); } }
  return {client, initialized, inbound, notifications, ask, open, change, query, close};
}
function save(file, value) { fs.mkdirSync(path.dirname(file), {recursive:true}); fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n'); }
module.exports = {repo, binary, position, makeProject, connect, save, delay, completionItems, pathToFileURL};
