"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const path = require("node:path");
const { buildSync } = require("esbuild");
const { createGraphViewport } = require("../../src/shared/graph-viewport");
function bundled(file) {
  const { outputFiles } = buildSync({ entryPoints: [path.resolve(__dirname, "../../src", file)], bundle: true, minify: true, platform: "node", format: "cjs", write: false });
  const context = { module: { exports: {} }, require };
  vm.runInNewContext(outputFiles[0].text, context);
  return context.module.exports;
}
test("graph viewport handles pinch zoom and single-pointer pan", () => {
  const previousDocument = global.document;
  const listeners = new Map();
  const stage = { style: {}, append() {} };
  const scroll = {
    clientWidth: 400, clientHeight: 300, scrollLeft: 100, scrollTop: 50,
    classList: { add() {}, remove() {} },
    addEventListener(name, listener) { listeners.set(name, listener); },
    getBoundingClientRect() { return { left: 0, top: 0 }; },
    setPointerCapture() {},
  };
  const content = { style: {}, before() {} };
  const event = (id, x, y) => ({ pointerId: id, pointerType: "touch", clientX: x, clientY: y,
    target: { closest: () => false }, preventDefault() {} });
  global.document = { createElement: () => stage };
  try {
    const viewport = createGraphViewport(scroll, content);
    viewport.setSize(800, 600);
    listeners.get("pointerdown")(event(1, 100, 100));
    listeners.get("pointerdown")(event(2, 200, 100));
    listeners.get("pointermove")(event(2, 300, 100));
    assert.equal(viewport.scale(), 2);
    listeners.get("pointerup")(event(2, 300, 100));
    const left = scroll.scrollLeft;
    listeners.get("pointermove")(event(1, 120, 100));
    assert.equal(scroll.scrollLeft, left - 20);
    assert.equal(stage.style.width, "1600px");
  } finally {
    global.document = previousDocument;
  }
});
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


test("backward transitions remain inside the layout when the canvas stretches", () => {
  const { stateLayout, connectionGeometry, GRAPH_LAYOUT: config } = bundled("state-machines/template.js");
  const machine = {initial: "title", states: [{name: "title"}, {name: "playing"}, {name: "over"}]};
  const edges = [
    {source: "title", target: "playing", transition: {event: "play"}},
    {source: "playing", target: "over", transition: {event: "crash"}},
    {source: "over", target: "playing", transition: {event: "play"}},
  ];
  const layout = stateLayout(machine, edges, config);
  const rect = name => {
    const state = layout.states.find(state => state.name === name);
    return {left: state.x, right: state.x + config.nodeWidth, top: state.y,
      bottom: state.y + config.nodeHeight, centerY: state.y + config.nodeHeight / 2};
  };
  const geometry = height => connectionGeometry(edges[2], layout.routes[2],
    rect("over"), rect("playing"), layout, {height, width: layout.width}, config);
  const normal = geometry(layout.height);
  const stretched = geometry(layout.height + 800);
  assert.equal(layout.routes[2].kind, "backward");
  assert.equal(stretched.pathData, normal.pathData);
  assert.equal(stretched.labelY, normal.labelY);
  assert.ok(stretched.labelY > rect("over").bottom);
  assert.ok(stretched.points.every(point => point.y >= 0 && point.y < layout.height));
});

test("crowded state-machine routes keep distinct source and target ports", () => {
  const { stateLayout, connectionRoutes, connectionGeometry, GRAPH_LAYOUT: config } = bundled("state-machines/template.js");
  const machine = { initial: "idle", states: [{ name: "idle" }, { name: "run" }] };
  const edges = Array.from({ length: 9 }, (_, index) => ({
    source: "idle", target: "run", transition: { event: `event${index}` },
  }));
  const layout = stateLayout(machine, edges, config);
  assert.equal(new Set(layout.routes.map(route => route.sourceOffset)).size, edges.length);
  assert.equal(new Set(layout.routes.map(route => route.targetOffset)).size, edges.length);
  const rect = name => {
    const state = layout.states.find(item => item.name === name);
    return { left: state.x, right: state.x + config.nodeWidth, top: state.y,
      bottom: state.y + config.nodeHeight, centerY: state.y + config.nodeHeight / 2 };
  };
  const paths = edges.map((edge, index) => connectionGeometry(edge, layout.routes[index],
    rect("idle"), rect("run"), layout, layout, config).pathData);
  assert.equal(new Set(paths).size, edges.length);

  const converging = Array.from({ length: 9 }, (_, index) => ({
    source: `source${index}`, target: "run", transition: { event: `event${index}` },
  }));
  const depth = new Map([...converging.map(edge => [edge.source, 0]), ["run", 1]]);
  const incoming = connectionRoutes(converging, depth);
  assert.equal(new Set(incoming.map(route => route.targetOffset)).size, converging.length);
});
