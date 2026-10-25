"use strict";

function contextualHints(hints, { debugging = false, selections = [], inspectedLine } = {}) {
  if (debugging) return hints;
  const ranges = selections.filter(selection => !selection.isEmpty).map(selection => ({
    first: selection.start.line,
    last: selection.end.line > selection.start.line && selection.end.character === 0 ? selection.end.line - 1 : selection.end.line,
  }));
  return hints.filter(hint => hint.position.line === inspectedLine || ranges.some(range => hint.position.line >= range.first && hint.position.line <= range.last));
}

module.exports = { contextualHints };
