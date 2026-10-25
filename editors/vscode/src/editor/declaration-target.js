"use strict";
const path = require("node:path");
const { maskSource } = require("../inspector/origins");
const KINDS = { stateMachine: "states", objectPool: "pools", store: "stores", input: "input" };

// Find direct local helper declarations without interpreting strings or comments.
// The caller must confirm the helper's definition with the language server.
function declarationAt(source, offset) {
  const masked = maskSource(source);
  const pattern = /\blocal\s+([A-Za-z_]\w*)\s*(?::[^=\n]+)?=\s*(stateMachine|objectPool|store|input)\s*\(/g;
  let match, result;
  while ((match = pattern.exec(masked)) && match.index <= offset) {
    const opening = pattern.lastIndex - 1;
    let depth = 1, end = opening + 1;
    for (; end < masked.length && depth; end++) {
      if (masked[end] === "(") depth++;
      else if (masked[end] === ")") depth--;
    }
    if (depth || offset >= end) continue;
    result = { name: match[1], helper: match[2], kind: KINDS[match[2]],
      start: match.index, end, nameOffset: match.index + match[0].indexOf(match[1], 5),
      helperOffset: opening - masked.slice(match.index, opening).match(/\s*$/)[0].length - match[2].length };
  }
  return result;
}

function matchingInstances(entries, target) {
  if (!target.path) return [];
  const matches = [];
  for (const entry of entries.values()) {
    if (!entry.session.active || entry.status !== "connected") continue;
    for (const object of entry.snapshot?.objects || []) {
      const origin = object.origin;
      if (object.kind === target.kind && origin && !origin.generated &&
          path.resolve(origin.path) === path.resolve(target.path) && origin.line === target.line &&
          origin.name === target.name) matches.push({ entry, object });
    }
  }
  return matches;
}
module.exports = { declarationAt, matchingInstances };
