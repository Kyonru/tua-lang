"use strict";

// Test-only timing boundary. The normal extension never installs this
// middleware or records source paths and completion requests.
function withCompletionTrace(middleware) {
  if (process.env.TUA_COMPLETION_TRACE !== "1") return middleware;
  const { performance } = require("node:perf_hooks");
  const fs = require("node:fs");
  const output = process.env.TUA_COMPLETION_TRACE_OUTPUT;
  if (output) fs.writeFileSync(output, "");
  return {
    ...middleware,
    async provideCompletionItem(document, position, context, token, next) {
      const started = performance.timeOrigin + performance.now();
      try {
        return await next(document, position, context, token);
      } finally {
        const sample = {
          started,
          finished: performance.timeOrigin + performance.now(),
          uri: document.uri.toString(),
          line: position.line,
          character: position.character,
          triggerKind: context.triggerKind,
        };
        if (output) fs.appendFileSync(output, JSON.stringify(sample) + "\n");
      }
    },
  };
}

module.exports = { withCompletionTrace };
