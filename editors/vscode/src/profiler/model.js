"use strict";
const path = require("node:path");
const { lookupSourceLocation, tracebackLocations } = require("../editor/source-maps");

const MAX_REPORT_BYTES = 2 * 1024 * 1024;

function parseReport(text) {
  if (typeof text !== "string" || Buffer.byteLength(text) > MAX_REPORT_BYTES) throw new Error("profile report exceeds 2 MiB");
  const value = JSON.parse(text);
  if (value?.version !== 1 || !Array.isArray(value.frames) || value.frames.length > 2048 ||
      !Array.isArray(value.stacks) || value.stacks.length > 512 ||
      !Number.isSafeInteger(value.frameCount) || value.frameCount < 0 ||
      !Number.isSafeInteger(value.sampleCount) || value.sampleCount < 0) {
    throw new Error("invalid Tua profile report");
  }
  if (!value.frames.every(frame => Number.isSafeInteger(frame?.index) && frame.index > 0 &&
      Number.isFinite(frame.wallMs) && frame.wallMs >= 0) ||
      !value.stacks.every(stack => typeof stack?.stack === "string" && stack.stack.length <= 1024 &&
      Number.isSafeInteger(stack.count) && stack.count >= 0)) {
    throw new Error("invalid Tua profile data");
  }
  return value;
}

function framePercentile(frames, fraction) {
  const values = frames.map(frame => frame.wallMs).sort((a, b) => a - b);
  return values.length ? values[Math.min(values.length - 1, Math.ceil(fraction * values.length) - 1)] : 0;
}

function stackLocation(stack, maps, outputDirectory) {
  for (const frame of stack.split("\n")) {
    const match = /^(.*\.lua):(\d+)$/u.exec(frame.trim());
    if (!match) continue;
    const generatedLine = Number(match[2]);
    const luaPath = match[1].replace(/^@/u, "");
    const generatedPath = path.isAbsolute(luaPath) ? luaPath : path.join(outputDirectory, luaPath);
    const source = lookupSourceLocation(maps, generatedPath, generatedLine);
    if (source) return { sourcePath: source.sourcePath, sourceLine: source.sourceLine,
      generatedPath: source.generatedPath, generatedLine };
  }
  return undefined;
}

function buildProfileModel(report, maps, outputDirectory, budgetMs = 16.67) {
  const frames = report.frames;
  const slowFrames = [...frames].sort((a, b) => b.wallMs - a.wallMs).slice(0, 20);
  const stacks = [...report.stacks].sort((a, b) => b.count - a.count).slice(0, 50)
    .map(item => ({ ...item, location: stackLocation(item.stack, maps, outputDirectory),
      share: report.sampleCount ? item.count / report.sampleCount : 0 }));
  const failureLocations = report.errorTraceback ? tracebackLocations(report.errorTraceback, maps) : [];
  return {
    reason: report.reason, durationMs: report.durationMs,
    frameCount: report.frameCount, retainedFrames: frames.length,
    sampleCount: report.sampleCount, droppedSamples: report.droppedSamples,
    profileError: report.profileError, errorMessage: report.errorMessage,
    failureLocation: failureLocations[0],
    p50: framePercentile(frames, 0.5), p95: framePercentile(frames, 0.95),
    worst: slowFrames[0]?.wallMs || 0, slowFrames, stacks,
    budgetMs, missedBudget: frames.filter(frame => frame.wallMs > budgetMs).length,
  };
}

module.exports = { MAX_REPORT_BYTES, parseReport, framePercentile, stackLocation, buildProfileModel };
