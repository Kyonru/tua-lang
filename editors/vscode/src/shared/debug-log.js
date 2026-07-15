"use strict";

const DEBUG_LSP_ENV = "TUA_DEBUG_LSP";
const DEBUG_LOG_REPEAT_FLUSH_MS = 250;

function createDebugLogger(outputChannel) {
  let pendingRepeat;

  function isEnabled() {
    return ["1", "true", "yes", "on"].includes(String(process.env[DEBUG_LSP_ENV] || "").toLowerCase());
  }

  function log(message, options = {}) {
    if (!isEnabled()) {
      return;
    }

    const repeatKey = options.repeatKey || message;
    const repeatLabel = options.repeatLabel || message;

    if (pendingRepeat?.key === repeatKey) {
      pendingRepeat.count += 1;
      pendingRepeat.latestMessage = message;
      scheduleRepeatFlush();
      return;
    }

    flush();
    appendLine(message);
    pendingRepeat = {
      key: repeatKey,
      label: repeatLabel,
      firstMessage: message,
      latestMessage: message,
      count: 1,
      timer: undefined,
    };
  }

  function appendLine(message) {
    outputChannel?.appendLine(`[debug ${new Date().toISOString()}] ${message}`);
  }

  function scheduleRepeatFlush() {
    if (pendingRepeat?.timer) {
      clearTimeout(pendingRepeat.timer);
    }

    pendingRepeat.timer = setTimeout(flush, DEBUG_LOG_REPEAT_FLUSH_MS);
  }

  function flush() {
    const repeat = pendingRepeat;

    if (!repeat) {
      return;
    }

    if (repeat.timer) {
      clearTimeout(repeat.timer);
    }

    pendingRepeat = undefined;

    if (repeat.count <= 1) {
      return;
    }

    const latest =
      repeat.latestMessage === repeat.firstMessage ? "" : ` latest=${repeat.latestMessage}`;

    appendLine(`debug repeat: ${repeat.label} count=${repeat.count}${latest}`);
  }

  return {
    flush,
    isEnabled,
    log,
  };
}

module.exports = {
  DEBUG_LSP_ENV,
  createDebugLogger,
};
