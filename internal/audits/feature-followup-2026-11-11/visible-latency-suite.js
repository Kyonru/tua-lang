"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { BrowserSession, waitFor } = require(path.resolve(__dirname, "../../../editors/vscode/test/webview/cdp"));

function percentile(values, fraction) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1)];
}

async function page(port) {
  const target = await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${port}/json/list`);
    return (await response.json()).find(item => item.type === "page");
  }, "VS Code page", 15000);
  return BrowserSession.connect(target.webSocketDebuggerUrl);
}

async function menuVisible(browser) {
  return browser.evaluate(`(() => { const w = document.querySelector('.suggest-widget');
    return !!(w && w.classList.contains('visible') && getComputedStyle(w).display !== 'none'); })()`);
}

async function focus(browser) {
  await waitFor(() => browser.evaluate("Boolean(document.querySelector('.monaco-editor.focused .native-edit-context'))"),
    "focused editor context", 10000);
  assert(await browser.evaluate(`(() => { const e=document.querySelector('.monaco-editor.focused .native-edit-context');
    e.focus(); return document.activeElement===e; })()`));
}

async function closeMenu(browser) {
  await browser.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
  await browser.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
  await waitFor(async () => !(await menuVisible(browser)), "suggestion menu closed", 5000);
}

async function armMenuObserver(browser, labels = ["health", "name"]) {
  await browser.evaluate(`(() => {
    const expected = ${JSON.stringify(labels)};
    window.__tuaMenuSample = { start: performance.now(), elapsed: null, labels: [] };
    const visible=() => { const w=document.querySelector('.suggest-widget');
      if (!w || !w.classList.contains('visible') || getComputedStyle(w).display==='none') return;
      const labels=[...w.querySelectorAll('.label-name')].map(x=>x.textContent.trim());
      if (!expected.every(label => labels.includes(label))) return;
      window.__tuaMenuSample.elapsed=performance.now()-window.__tuaMenuSample.start;
      window.__tuaMenuSample.labels=labels; window.__tuaMenuSample.observer.disconnect();
    };
    window.__tuaMenuSample.observer=new MutationObserver(visible);
    window.__tuaMenuSample.observer.observe(document.body,{subtree:true,childList:true,attributes:true,characterData:true});
  })()`);
}

async function menuSampleResult(browser, labels = ["health", "name"]) {
  let sample;
  try {
    sample = await waitFor(() => browser.evaluate("window.__tuaMenuSample && window.__tuaMenuSample.elapsed !== null ? {elapsed:window.__tuaMenuSample.elapsed,labels:window.__tuaMenuSample.labels} : null"),
      "typed dot to visible typed suggestions", 10000);
  } catch (error) {
    const visible = await browser.evaluate(`(() => { const w=document.querySelector('.suggest-widget'); return { visible: w?.className, labels: [...(w?.querySelectorAll('.label-name') || [])].map(x => x.textContent.trim()).slice(0,40), text:w?.textContent?.slice(0,800) }; })()`);
    throw new Error(`${error.message}: ${JSON.stringify(visible)}`);
  }
  assert(labels.every(label => sample.labels.includes(label)));
  return sample.elapsed;
}

async function completionSample(browser, editor, options = {}) {
  const symbol = options.symbol || "item";
  const position = options.position || new vscode.Position(1, 4);
  const labels = options.labels || ["health", "name"];
  await closeMenu(browser);
  const line = editor.document.lineAt(position.line).text;
  assert.equal(line.slice(position.character - symbol.length, position.character), symbol);
  if (line.charAt(position.character) === ".") {
    await editor.edit(builder => builder.delete(new vscode.Range(position,
      position.translate(0, 1))), { undoStopBefore: false, undoStopAfter: false });
  }
  editor.selection = new vscode.Selection(position, position);
  await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
  await focus(browser);
  await armMenuObserver(browser, labels);
  await browser.send("Input.insertText", { text: "." });
  return menuSampleResult(browser, labels);
}

async function hoverSample(browser, editor) {
  await closeMenu(browser);
  const text = editor.document.getText();
  const offset = text.lastIndexOf("print(agent.)");
  if (offset >= 0) {
    const dot = offset + "print(agent".length;
    await editor.edit(builder => builder.delete(new vscode.Range(editor.document.positionAt(dot),
      editor.document.positionAt(dot + 1))));
  }
  editor.selection = new vscode.Selection(1, 8, 1, 8);
  await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
  await focus(browser);
  await browser.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
  await browser.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
  await browser.evaluate(`(() => {
    window.__tuaHoverSample = { start: performance.now(), elapsed: null, text: "" };
    const check = () => { const h = document.querySelector('.monaco-hover:not(.hidden)');
      if (!h || getComputedStyle(h).display === 'none') return;
      const value = h.textContent || '';
      if (!value.includes('Agent')) return;
      window.__tuaHoverSample.elapsed = performance.now() - window.__tuaHoverSample.start;
      window.__tuaHoverSample.text = value;
      window.__tuaHoverSample.observer.disconnect();
    };
    window.__tuaHoverSample.observer = new MutationObserver(check);
    window.__tuaHoverSample.observer.observe(document.body,{subtree:true,childList:true,attributes:true,characterData:true});
  })()`);
  await vscode.commands.executeCommand("editor.action.showHover");
  const result = await waitFor(() => browser.evaluate("window.__tuaHoverSample?.elapsed !== null ? { elapsed: window.__tuaHoverSample.elapsed, text: window.__tuaHoverSample.text } : null"), "visible imported-class hover", 10000);
  assert(result.text.includes("Agent"));
  return result;
}

async function pointerHoverSample(browser, editor) {
  await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
  await focus(browser);
  await browser.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
  await browser.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
  const rect = await browser.evaluate(`(() => {
    const line = [...document.querySelectorAll('.monaco-editor.focused .view-line')]
      .find(node => node.textContent.includes('agent') && node.textContent.includes('factory.spawn'));
    if (!line) return null;
    const target = line.textContent.indexOf('agent');
    const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT);
    let node, offset = 0;
    while ((node = walker.nextNode())) {
      const end = offset + node.textContent.length;
      if (target >= offset && target + 1 <= end) {
        const range = document.createRange();
        range.setStart(node, target - offset);
        range.setEnd(node, target - offset + 1);
        const r = range.getBoundingClientRect();
        return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
      }
      offset = end;
    }
    return null;
  })()`);
  if (!rect) {
    const lines = await browser.evaluate("[...document.querySelectorAll('.monaco-editor.focused .view-line')].map(node => node.textContent).slice(0,8)");
    throw new Error("imported class local must have a visible text range: " + JSON.stringify(lines));
  }
  await browser.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: 4, y: 4 });
  await browser.evaluate(`(() => {
    window.__tuaPointerHover = { start: performance.now(), elapsed: null, text: "" };
    const check = () => { const h = document.querySelector('.monaco-hover:not(.hidden)');
      if (!h || getComputedStyle(h).display === 'none') return;
      const value = h.textContent || '';
      if (!value.includes('local agent: Agent')) return;
      window.__tuaPointerHover.elapsed = performance.now() - window.__tuaPointerHover.start;
      window.__tuaPointerHover.text = value;
      window.__tuaPointerHover.observer.disconnect();
    };
    window.__tuaPointerHover.observer = new MutationObserver(check);
    window.__tuaPointerHover.observer.observe(document.body,{subtree:true,childList:true,attributes:true,characterData:true});
  })()`);
  await browser.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: rect.x, y: rect.y });
  return waitFor(() => browser.evaluate("window.__tuaPointerHover?.elapsed !== null ? { elapsed: window.__tuaPointerHover.elapsed, text: window.__tuaPointerHover.text } : null"),
    "pointer hover on imported class", 10000);
}

async function manualSample(browser) {
  await closeMenu(browser);
  await focus(browser);
  await armMenuObserver(browser);
  await vscode.commands.executeCommand("editor.action.triggerSuggest");
  return menuSampleResult(browser);
}

async function formatSample(browser, document, editor, value) {
  const current = document.getText();
  await editor.edit(builder => builder.replace(new vscode.Range(document.positionAt(0),
    document.positionAt(current.length)), `local   value=${value}\nprint(value)\n`));
  await focus(browser);
  const start = performance.now();
  await document.save();
  const elapsed = performance.now() - start;
  assert(document.getText().includes(`local value = ${value}`), `format-on-save did not apply: ${document.getText()}`);
  return elapsed;
}

async function largeFormatSample(document, editor, value) {
  const firstLine = document.lineAt(0).range;
  await editor.edit(builder => builder.replace(firstLine, `local   value_0=${value}`));
  const start = performance.now();
  await document.save();
  const elapsed = performance.now() - start;
  assert(document.getText().startsWith(`local value_0 = ${value}\n`));
  return elapsed;
}

async function run() {
  const extension = vscode.extensions.getExtension("SolenodonteLabs.tua");
  assert(extension);
  await extension.activate();
  const port = Number(process.env.TUA_VISIBLE_PORT);
  const browser = await page(port);
  const report = { version: extension.packageJSON.version, workspaceFiles: 113,
    completion: { coldMs: null, warmMs: [], manualMs: [], javascriptControlMs: [], fileSwitchMs: [] }, formatOnSave: { samplesMs: [], largeSamplesMs: [] },
    crossFileClass: { completionColdMs: null, completionWarmMs: [], hoverVisibleMs: [], pointerHoverMs: [], hoverProtocolMs: null, hoverText: null, deep: {}, annotated: {} },
    environment: { platform: process.platform, arch: process.arch, vscode: vscode.version } };
  try {
    const [folder] = vscode.workspace.workspaceFolders;
    const document = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(folder.uri, "src/main.tua"));
    const editor = await vscode.window.showTextDocument(document, { preview: false });
    report.completion.coldMs = await completionSample(browser, editor);
    for (let index = 0; index < 12; index++) report.completion.warmMs.push(await completionSample(browser, editor));
    for (let index = 0; index < 12; index++) report.completion.manualMs.push(await manualSample(browser));
    for (const file of ["src/main_alt.tua", "src/main.tua"]) {
      const switched = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(folder.uri, file));
      report.completion.fileSwitchMs.push(await completionSample(browser,
        await vscode.window.showTextDocument(switched, { preview: false })));
    }
    const crossFile = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(folder.uri, "src/cross_file.tua"));
    const crossEditor = await vscode.window.showTextDocument(crossFile, { preview: false });
    const crossOptions = { symbol: "agent", position: new vscode.Position(3, 11), labels: ["health", "name", "takeDamage"] };
    report.crossFileClass.completionColdMs = await completionSample(browser, crossEditor, crossOptions);
    for (let index = 0; index < 12; index++) report.crossFileClass.completionWarmMs.push(await completionSample(browser, crossEditor, crossOptions));
    for (let index = 0; index < 8; index++) {
      const hovered = await hoverSample(browser, crossEditor);
      report.crossFileClass.hoverVisibleMs.push(hovered.elapsed);
      report.crossFileClass.hoverText = hovered.text;
    }
    for (let index = 0; index < 3; index++) {
      const hovered = await pointerHoverSample(browser, crossEditor);
      report.crossFileClass.pointerHoverMs.push(hovered.elapsed);
    }
    const protocolStart = performance.now();
    const protocolHovers = await vscode.commands.executeCommand("vscode.executeHoverProvider", crossFile.uri, new vscode.Position(1, 8));
    report.crossFileClass.hoverProtocolMs = performance.now() - protocolStart;
    report.crossFileClass.hoverProtocol = protocolHovers.map(hover => hover.contents.map(content => typeof content === "string" ? content : content.value).join("\n")).join("\n");
    const deep = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(folder.uri, "src/cross_file_deep.tua"));
    const deepEditor = await vscode.window.showTextDocument(deep, { preview: false });
    try {
      report.crossFileClass.deep.visibleAnySuggestionMs = await completionSample(browser, deepEditor,
        { symbol: "agent", position: new vscode.Position(3, 11), labels: [] });
    } catch (error) {
      report.crossFileClass.deep.visibleError = error.message;
    }
    report.crossFileClass.deep.widget = await browser.evaluate(`(() => { const w=document.querySelector('.suggest-widget'); return {visible:!!w?.classList.contains('visible'),labels:[...(w?.querySelectorAll('.label-name')||[])].map(x=>x.textContent.trim()).slice(0,30)}; })()`);
    report.crossFileClass.deep.hoverVisibleMs = [];
    for (let index = 0; index < 5; index++) {
      const hovered = await hoverSample(browser, deepEditor);
      report.crossFileClass.deep.hoverVisibleMs.push(hovered.elapsed);
      report.crossFileClass.deep.hoverVisibleText = hovered.text;
    }
    const deepStart = performance.now();
    const deepItems = await vscode.commands.executeCommand("vscode.executeCompletionItemProvider", deep.uri, new vscode.Position(3, 12));
    report.crossFileClass.deep.protocolMs = performance.now() - deepStart;
    report.crossFileClass.deep.labels = deepItems.items.map(item => item.label);
    const deepHovers = await vscode.commands.executeCommand("vscode.executeHoverProvider", deep.uri, new vscode.Position(1, 8));
    report.crossFileClass.deep.hover = deepHovers.map(hover => hover.contents.map(content => typeof content === "string" ? content : content.value).join("\n")).join("\n");
    const annotated = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(folder.uri, "src/cross_file_annotated.tua"));
    const annotatedEditor = await vscode.window.showTextDocument(annotated, { preview: false });
    report.crossFileClass.annotated.visibleColdMs = await completionSample(browser, annotatedEditor, crossOptions);
    report.crossFileClass.annotated.visibleWarmMs = [];
    for (let index = 0; index < 5; index++) {
      report.crossFileClass.annotated.visibleWarmMs.push(await completionSample(browser, annotatedEditor, crossOptions));
    }
    const controlDocument = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(folder.uri, "src/control.js"));
    const controlEditor = await vscode.window.showTextDocument(controlDocument, { preview: false });
    for (let index = 0; index < 12; index++) report.completion.javascriptControlMs.push(await completionSample(browser, controlEditor));
    const formatDocument = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(folder.uri, "src/format.tua"));
    const formatEditor = await vscode.window.showTextDocument(formatDocument, { preview: false });
    for (let index = 0; index < 12; index++) report.formatOnSave.samplesMs.push(await formatSample(browser, formatDocument, formatEditor, index));
    const largeDocument = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(folder.uri, "src/format_large.tua"));
    const largeEditor = await vscode.window.showTextDocument(largeDocument, { preview: false });
    for (let index = 0; index < 5; index++) report.formatOnSave.largeSamplesMs.push(await largeFormatSample(largeDocument, largeEditor, index));
    report.completion.warmP50Ms = percentile(report.completion.warmMs, 0.5);
    report.completion.warmP95Ms = percentile(report.completion.warmMs, 0.95);
    report.completion.manualP50Ms = percentile(report.completion.manualMs, 0.5);
    report.completion.manualP95Ms = percentile(report.completion.manualMs, 0.95);
    report.completion.javascriptControlP50Ms = percentile(report.completion.javascriptControlMs, 0.5);
    report.completion.javascriptControlP95Ms = percentile(report.completion.javascriptControlMs, 0.95);
    report.crossFileClass.completionWarmP50Ms = percentile(report.crossFileClass.completionWarmMs, 0.5);
    report.crossFileClass.completionWarmP95Ms = percentile(report.crossFileClass.completionWarmMs, 0.95);
    report.crossFileClass.hoverVisibleP50Ms = percentile(report.crossFileClass.hoverVisibleMs, 0.5);
    report.crossFileClass.hoverVisibleP95Ms = percentile(report.crossFileClass.hoverVisibleMs, 0.95);
    report.environment.quickSuggestionsDelayMs = vscode.workspace.getConfiguration("editor").get("quickSuggestionsDelay");
    report.formatOnSave.p50Ms = percentile(report.formatOnSave.samplesMs, 0.5);
    report.formatOnSave.p95Ms = percentile(report.formatOnSave.samplesMs, 0.95);
    report.formatOnSave.largeP50Ms = percentile(report.formatOnSave.largeSamplesMs, 0.5);
    report.formatOnSave.largeP95Ms = percentile(report.formatOnSave.largeSamplesMs, 0.95);
    fs.writeFileSync(process.env.TUA_VISIBLE_OUT, JSON.stringify(report, null, 2) + "\n");
    console.log("VISIBLE_LATENCY", JSON.stringify(report));
  } finally {
    browser.dispose();
  }
}

module.exports = { run };
