"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const path = require("node:path");
const { buildSync } = require("esbuild");
function bundled(file) {
  const { outputFiles } = buildSync({ entryPoints: [path.resolve(__dirname, "../../src", file)], bundle: true, minify: true, platform: "node", format: "cjs", write: false });
  const context = { module: { exports: {} }, require };
  vm.runInNewContext(outputFiles[0].text, context);
  return context.module.exports;
}
test("minified sprite webviews preserve callable playback helpers", () => {
  const { spritePreviewRuntime } = bundled("love/sprite-preview/runtime.js");
  const script = spritePreviewRuntime([], "demo.tua");
  const helpers = script.slice(script.indexOf("const expandAnimationFrames ="), script.indexOf("function saveState()"));
  const result = vm.runInNewContext(helpers + '; ({frames: expandAnimationFrames({from:0,to:3,row:0}).length, playing:advancePlayback({playing:true,offset:0,elapsed:0},[0.1],false,0.2).playing})');
  assert.equal(result.frames, 4); assert.equal(result.playing, false);
});
test("minified graph webviews preserve helper closure dependencies", () => {
  const { stateMachinePreviewHtml, GRAPH_LAYOUT } = bundled("state-machines/template.js");
  const html = stateMachinePreviewHtml({ cspSource: "test" }, { source: "demo.tua", machines: [] });
  const restore = html.slice(html.indexOf("const restoreGraphState ="), html.indexOf("const restored ="));
  const begin = html.indexOf("const { statePastelHue, connectionRoutes");
  const helpers = html.slice(begin, html.indexOf("for (const [index, machine]", begin));
  const context = { config: GRAPH_LAYOUT };
  const result = vm.runInNewContext(restore + helpers + `
    const machine={initial:'idle',states:[{name:'idle'},{name:'run'}]};
    const edges=[{source:'idle',target:'run',transition:{event:'start'}}];
    const layout=stateLayout(machine,edges,config);
    ({states:layout.states.length,routes:layout.routes.length,hue:statePastelHue(0),selection:restoreGraphState({source:'demo'},null).selection.kind,path:orthogonalPath([{x:0,y:0},{x:20,y:20}])});`, context);
  assert.equal(result.states, 2); assert.equal(result.routes, 1);
  assert.equal(result.hue, 210); assert.equal(result.selection, "machine");
  assert.match(result.path, /^M /);
});
