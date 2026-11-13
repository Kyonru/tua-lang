"use strict";

// Keep a hidden preview current when it is revealed without doing analysis or
// rebuilding its HTML for every edit while another editor tab is active.
function createVisiblePreviewRefresh(getPanel, render, delayMs) {
  let timer;
  let dirty = false;

  function cancel() {
    clearTimeout(timer);
    timer = undefined;
    dirty = false;
  }

  function schedule() {
    clearTimeout(timer);
    timer = undefined;
    if (!getPanel()) return;
    if (getPanel().visible === false) {
      dirty = true;
      return;
    }
    dirty = false;
    timer = setTimeout(() => {
      timer = undefined;
      if (getPanel()?.visible === false) {
        dirty = true;
      } else if (getPanel()) {
        void render();
      }
    }, delayMs);
  }

  function onVisible() {
    if (!dirty || getPanel()?.visible === false) return;
    cancel();
    return render();
  }

  return { cancel, onVisible, schedule };
}

module.exports = { createVisiblePreviewRefresh };
