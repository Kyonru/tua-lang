"use strict";

// A preview can be refreshed while its previous server request is still pending.
// Only the most recent request may replace the webview document.
function createRenderGate() {
  let revision = 0;
  return {
    begin() { return ++revision; },
    isCurrent(request) { return request === revision; },
    invalidate() { revision += 1; },
  };
}

module.exports = { createRenderGate };
