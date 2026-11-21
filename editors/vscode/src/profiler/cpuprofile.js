"use strict";

const { profileMetadata } = require("./metadata");
const path = require("node:path");
const { pathToFileURL } = require("node:url");
const { lookupSourceLocation } = require("../editor/source-maps");

function stackFrames(stack, maps, outputDirectory, lookup) {
  const frames = stack.split("\n").map(line => {
    const label = line.trim();
    const match = /^(.*\.lua):(\d+)$/u.exec(label);
    if (!match) return { functionName: label || "(unknown)", scriptId: "0", url: "", lineNumber: 0, columnNumber: 0 };
    const generatedPath = path.isAbsolute(match[1].replace(/^@/u, "")) ? match[1].replace(/^@/u, "") :
      path.join(outputDirectory, match[1].replace(/^@/u, ""));
    const generatedLine = Number(match[2]);
    const mapped = lookup ? lookup(generatedPath, generatedLine) : lookupSourceLocation(maps, generatedPath, generatedLine);
    const sourcePath = mapped?.sourcePath || generatedPath;
    const sourceLine = mapped?.sourceLine || generatedLine;
    return { functionName: `${path.basename(sourcePath)}:${sourceLine}`, scriptId: "0",
      url: pathToFileURL(sourcePath).href, lineNumber: Math.max(0, sourceLine - 1), columnNumber: 0 };
  });
  return frames.reverse(); // LuaJIT dumpstack lists the active frame first.
}

function toCpuProfile(report, maps, outputDirectory, lookup) {
  if (!report.sampleEvents?.length) return undefined;
  const nodes = [{ id: 1, callFrame: { functionName: "(root)", scriptId: "0", url: "", lineNumber: 0, columnNumber: 0 }, children: [] }];
  const children = new Map();
  const leafIds = report.stacks.map(item => {
    let parent = nodes[0];
    for (const frame of stackFrames(item.stack, maps, outputDirectory, lookup)) {
      const key = `${parent.id}\0${frame.url}\0${frame.lineNumber}\0${frame.functionName}`;
      let child = children.get(key);
      if (!child) {
        child = { id: nodes.length + 1, callFrame: frame, children: [] };
        nodes.push(child);
        parent.children.push(child.id);
        children.set(key, child);
      }
      parent = child;
    }
    return parent.id;
  });
  const samples = [], timeDeltas = [];
  // The ring retains only the latest callbacks. Do not render evicted time as
  // an empty lead-in to the exported flame chart.
  const startTime = report.sampleEventsDropped ? Math.round(report.sampleEvents[0].atMs * 1000) : 0;
  let lastUs = startTime;
  for (const event of report.sampleEvents) {
    const nowUs = Math.max(lastUs, Math.round(event.atMs * 1000));
    samples.push(leafIds[event.stackId - 1]);
    timeDeltas.push(nowUs - lastUs);
    lastUs = nowUs;
  }
  return {
    nodes, startTime, endTime: Math.max(lastUs, Math.round((report.durationMs || 0) * 1000)),
    samples, timeDeltas,
    tua: { source: "LuaJIT statistical profiler", capturedSamples: report.sampleCount,
      metadata: profileMetadata(report.metadata), durationMs: report.durationMs,
      capturedFrames: report.frameCount, retainedFrames: report.frames?.length || 0,
      exportedCallbacks: samples.length, droppedCallbacks: report.sampleEventsDropped || 0,
      note: "One timeline point per profiler callback; batched samples are not individually timed. Source lines can be approximate due to JIT inlining." },
  };
}

module.exports = { toCpuProfile };
