"use strict";

// Replay real editor requests against unsaved overlays. Project files are read,
// never rewritten. Scenarios and traces should stay local when they use private code.
const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const { LspClient, pathToFileURL, positionAfter, completionItems, delay, statsForValues } = require("./lsp-profile-common");

async function profileEditor(command, scenario, output) {
  const root = path.resolve(scenario.root);
  const documents = Object.fromEntries(Object.entries(scenario.documents).map(([id, relative]) => {
    const filename = path.resolve(root, relative);
    const source = fs.readFileSync(filename, "utf8");
    return [id, { source, original: source, uri: pathToFileURL(filename).href, version: 1, languageId: filename.endsWith(".lua") ? "lua" : "tua" }];
  }));
  const report = { server: path.resolve(command), serverSha256: hash(fs.readFileSync(command)), documents: Object.fromEntries(Object.entries(documents).map(([id, doc]) => [id, { bytes: Buffer.byteLength(doc.source), sha256: hash(doc.source) }])), timings: [], failures: [] };
  const client = new LspClient(path.resolve(command), root, { requestTimeoutMs: scenario.timeoutMs || 60000, env: { ...process.env, TUA_PROFILE: process.env.TUA_PROFILE || "0" } });
  const persist = () => {
    fs.mkdirSync(path.dirname(output), { recursive: true });
    const groups = new Map();
    for (const timing of report.timings) { if (!groups.has(timing.label)) groups.set(timing.label, []); groups.get(timing.label).push(timing.ms); }
    report.summary = Object.fromEntries([...groups].map(([label, values]) => [label, statsForValues(values)]));
    fs.writeFileSync(output, `${JSON.stringify(report, null, 2)}\n`);
    fs.writeFileSync(`${output}.trace`, client.stderr);
  };
  const document = id => { if (!documents[id]) throw new Error(`unknown document ${id}`); return documents[id]; };
  const change = (doc, text) => { doc.source = text; client.notify("textDocument/didChange", { textDocument: { uri: doc.uri, version: ++doc.version }, contentChanges: [{ text }] }); };
  async function request(step) {
    const doc = document(step.document);
    const params = { textDocument: { uri: doc.uri }, ...step.params };
    if (step.after !== undefined) params.position = probePosition(doc.source, step.after);
    if (step.request === "tua/localInlineCompletion") params.documentVersion = doc.version;
    const start = performance.now();
    const result = await client.request(step.request, params);
    const timing = { label: step.label || step.request, ms: performance.now() - start, nonNull: result !== null };
    if (step.request === "textDocument/completion") timing.count = completionItems(result).length;
    if (step.expectNonNull && result === null) report.failures.push(`${timing.label}: null result`);
    if (step.expectLabels) {
      const labels = new Set(completionItems(result).map(item => item.label));
      for (const label of step.expectLabels) if (!labels.has(label)) report.failures.push(`${timing.label}: missing ${label}`);
    }
    if (step.expectHover && !JSON.stringify(result).includes(step.expectHover)) report.failures.push(`${timing.label}: missing hover text ${step.expectHover}`);
    report.timings.push(timing);
    console.log(JSON.stringify(timing));
    if (step.applyFormatting && result?.length) {
      if (result.length !== 1 || result[0].range.start.line !== 0 || result[0].range.start.character !== 0) throw new Error("expected a full-document formatting edit");
      change(doc, result[0].newText);
    }
    persist();
  }
  await client.start();
  try {
    await client.request("initialize", { processId: process.pid, rootUri: pathToFileURL(root).href, capabilities: scenario.capabilities || {}, workspaceFolders: [{ uri: pathToFileURL(root).href, name: path.basename(root) }] });
    client.notify("initialized", {});
    if (scenario.settings) client.notify("workspace/didChangeConfiguration", { settings: { tua: scenario.settings } });
    for (const step of scenario.steps) {
      if (step.open) {
        const doc = document(step.open);
        client.notify("textDocument/didOpen", { textDocument: { uri: doc.uri, languageId: doc.languageId, version: doc.version, text: doc.source } });
      } else if (step.change) {
        const doc = document(step.change);
        change(doc, step.replace ? replaceOnce(doc.original, step.replace.from, step.replace.to) : doc.original);
      } else if (step.save) {
        client.notify("textDocument/didSave", { textDocument: { uri: document(step.save).uri } });
      } else if (step.parallel) {
        await Promise.all(step.parallel.map(request));
      } else if (step.request) {
        await request(step);
      } else if (Number.isFinite(step.waitMs) && step.waitMs >= 0 && step.waitMs <= 5000) {
        await delay(step.waitMs);
      } else { throw new Error("invalid replay step"); }
    }
    report.cacheStats = await client.request("tua/cacheStats", null);
    await client.request("shutdown", null);
    client.notify("exit", null);
  } finally { persist(); client.child.kill(); }
  if (report.failures.length) throw new Error(report.failures.join("\n"));
  return report;
}

function hash(value) { return crypto.createHash("sha256").update(value).digest("hex"); }
function probePosition(source, needle) {
  if (!needle || source.indexOf(needle) < 0) throw new Error("probe text is absent");
  if (source.indexOf(needle) !== source.lastIndexOf(needle)) throw new Error("probe text is ambiguous; include surrounding text");
  return positionAfter(source, needle);
}
function replaceOnce(source, from, to) { probePosition(source, from); return source.replace(from, () => to); }

if (require.main === module) {
  const [command, scenarioPath, output] = process.argv.slice(2);
  if (!command || !scenarioPath || !output) { console.error("Usage: node lsp-editor-profile.js <tua> <scenario.json> <output.json>"); process.exitCode = 1; }
  else profileEditor(command, JSON.parse(fs.readFileSync(scenarioPath, "utf8")), path.resolve(output)).catch(error => { console.error(error.stack); process.exitCode = 1; });
}
module.exports = { profileEditor, probePosition, replaceOnce };
