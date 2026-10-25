"use strict";
// Isolated native tree benchmark; no user settings or running games are changed.
const fs = require("node:fs"), os = require("node:os"), path = require("node:path");
const { performance } = require("node:perf_hooks");
const percentile = (values, q) => [...values].sort((a, b) => a - b)[Math.min(values.length - 1, Math.ceil(values.length * q) - 1)];
exports.run = async () => {
  const vscode = require("vscode"), assert = require("node:assert/strict");
  const { BrowserSession } = require("../test/webview/cdp");
  const api = await vscode.extensions.getExtension("local.inspector-benchmark").activate();
  await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
  await vscode.commands.executeCommand("fixture.inspector.focus");
  const objects = api.tree.getChildren()[0].children;
  for (const object of objects) await api.view.reveal(object, { expand: true });
  await api.view.reveal(objects[0], { focus: true, select: true });
  const targets = await (await fetch(`http://127.0.0.1:${process.env.TUA_UI_PORT}/json/list`)).json();
  const browser = await BrowserSession.connect(targets.find(item => item.type === "page").webSocketDebuggerUrl);
  const report = { objects: 16, fieldsPerObject: 128, expandedValueRows: 2048, updateInterval: 500, secondsPerPhase: Number(process.env.TUA_UI_SECONDS || 15), phases: {} };
  try {
    for (const phase of ["paused", "live"]) {
      if (phase === "live") api.start();
      const input = [], lag = [], began = performance.now(); let previous = performance.now(), key = 0;
      const timer = setInterval(() => { const now = performance.now(); lag.push(Math.max(0, now - previous - 20)); previous = now; }, 20);
      try {
        while (performance.now() - began < report.secondsPerPhase * 1000) {
          const before = await browser.evaluate("document.activeElement?.getAttribute('aria-activedescendant')");
          const start = performance.now(), down = key++ % 2 === 0;
          await browser.send("Input.dispatchKeyEvent", { type: "keyDown", key: down ? "ArrowDown" : "ArrowUp", code: down ? "ArrowDown" : "ArrowUp", windowsVirtualKeyCode: down ? 40 : 38 });
          await browser.send("Input.dispatchKeyEvent", { type: "keyUp", key: down ? "ArrowDown" : "ArrowUp", code: down ? "ArrowDown" : "ArrowUp", windowsVirtualKeyCode: down ? 40 : 38 });
          let changed = false;
          while (performance.now() - start < 2000) {
            const after = await browser.evaluate("document.activeElement?.getAttribute('aria-activedescendant')");
            if (after && after !== before) { changed = true; break; }
            await new Promise(resolve => setTimeout(resolve, 5));
          }
          assert(changed, "Tree must respond to keyboard navigation while snapshots update");
          input.push(performance.now() - start);
          await new Promise(resolve => setTimeout(resolve, 100));
        }
      } finally { clearInterval(timer); }
      report.phases[phase] = { inputSamples: input.length, inputP50Ms: percentile(input, .5), inputP95Ms: percentile(input, .95), inputP99Ms: percentile(input, .99), inputMaxMs: Math.max(...input), eventLoopLagP99Ms: percentile(lag, .99), eventLoopLagMaxMs: Math.max(...lag) };
    }
    api.stop();
    report.treeBuild = { count: api.times.length, p50Ms: percentile(api.times, .5), p99Ms: percentile(api.times, .99), maxMs: Math.max(...api.times) };
    report.rendererErrors = browser.errors;
    assert.equal(browser.errors.length, 0);
    fs.writeFileSync(process.env.TUA_UI_OUTPUT, JSON.stringify(report, null, 2) + "\n");
    console.log(JSON.stringify(report));
  } finally { api.stop(); browser.dispose(); }
};
async function main() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-inspector-ui-benchmark-"));
  const extensionRoot = path.resolve(__dirname, "..");
  const net = require("node:net"), server = net.createServer();
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve)); const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  fs.copyFileSync(path.join(extensionRoot, "assets/logo.svg"), path.join(root, "logo.svg"));
  fs.writeFileSync(path.join(root, "package.json"), JSON.stringify({ name: "inspector-benchmark", publisher: "local", version: "1.0.0", engines: { vscode: "^1.94.0" }, main: "extension.js", activationEvents: ["onView:fixture.inspector"], contributes: { viewsContainers: { activitybar: [{ id: "fixture", title: "Tua", icon: "logo.svg" }] }, views: { fixture: [{ id: "fixture.inspector", name: "Runtime Inspector" }] } } }));
  fs.writeFileSync(path.join(root, "extension.js"), `const vscode=require('vscode');
const {performance}=require('node:perf_hooks');
if(process.env.TUA_PRESENTATION_MODULE)require.cache[${JSON.stringify(path.join(extensionRoot, "src/inspector/presentation.js"))}]={exports:require(process.env.TUA_PRESENTATION_MODULE)};
const {InspectorTree}=require(${JSON.stringify(path.join(extensionRoot, "src/inspector/view.js"))});
exports.activate=context=>{
 let sequence=0,timer;
 const snapshot=()=>({objects:Array.from({length:16},(_,id)=>({id:id+1,kind:'stores',label:'store_'+(id+1),data:{state:Object.fromEntries(Array.from({length:128},(_,i)=>['stat'+String(i).padStart(3,'0'),i%8===0?sequence:i])),updates:sequence,subscribers:1,watchers:1}}))});
 const entry={session:{id:'host',active:true},snapshot:snapshot()};
 const tree=new InspectorTree({entries:new Map([['host',entry]])});
 const times=[],original=tree.getChildren.bind(tree);
 tree.getChildren=node=>{const start=performance.now();const result=original(node);if(!node)times.push(performance.now()-start);return result;};
 const view=vscode.window.createTreeView('fixture.inspector',{treeDataProvider:tree});context.subscriptions.push(tree,view,{dispose(){clearInterval(timer)}});
 return{tree,view,times,start(){timer=setInterval(()=>{sequence++;entry.previousSnapshot=entry.snapshot;entry.snapshot=snapshot();tree.changed.fire()},500)},stop(){clearInterval(timer)}};
};`);
  try {
    await require("@vscode/test-electron").runTests({ extensionDevelopmentPath: root, extensionTestsPath: __filename, vscodeExecutablePath: require("../test/vscode-executable").localVSCodeExecutable(), extensionTestsEnv: { TUA_UI_PORT: String(port), TUA_UI_OUTPUT: process.env.TUA_UI_OUTPUT || path.join(os.tmpdir(), "tua-inspector-ui.json") }, launchArgs: ["--disable-extensions", "--disable-workspace-trust", "--skip-welcome", "--skip-release-notes", `--remote-debugging-port=${port}`, `--user-data-dir=${root}/user`, `--extensions-dir=${root}/extensions`] });
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
}
if (require.main === module) main().catch(error => { console.error(error); process.exitCode = 1; });
