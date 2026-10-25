"use strict";

// Only used against the isolated test instance's loopback debugging port.
class BrowserSession {
  constructor(socket) {
    this.socket = socket;
    this.serial = 0;
    this.pending = new Map();
    this.contexts = new Map();
    this.errors = [];
    socket.addEventListener("message", ({ data }) => {
      const message = JSON.parse(String(data));
      if (message.id) {
        const request = this.pending.get(message.id);
        if (!request) return;
        clearTimeout(request.timeout);
        this.pending.delete(message.id);
        if (message.error) request.reject(new Error(message.error.message));
        else request.resolve(message.result);
      } else if (message.method === "Runtime.executionContextCreated") {
        this.contexts.set(message.params.context.id, message.params.context);
      } else if (message.method === "Runtime.executionContextDestroyed") {
        this.contexts.delete(message.params.executionContextId);
      } else if (message.method === "Runtime.executionContextsCleared") {
        this.contexts.clear();
      } else if (message.method === "Runtime.exceptionThrown") {
        this.errors.push(message.params.exceptionDetails);
      }
    });
  }

  static async connect(url) {
    const socket = new WebSocket(url);
    await new Promise((resolve, reject) => {
      socket.addEventListener("open", resolve, { once: true });
      socket.addEventListener("error", reject, { once: true });
    });
    const session = new BrowserSession(socket);
    await session.send("Runtime.enable");
    return session;
  }

  send(method, params = {}) {
    return new Promise((resolve, reject) => {
      const id = ++this.serial;
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`Browser command timed out: ${method}`));
      }, 8000);
      this.pending.set(id, { resolve, reject, timeout });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  async evaluate(expression, contextId) {
    const result = await this.send("Runtime.evaluate", { expression, contextId, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description || result.exceptionDetails.text);
    return result.result.value;
  }

  async documentContext(title) {
    return waitFor(async () => {
      for (const context of this.contexts.values()) {
        try {
          if (await this.evaluate("document.title", context.id) === title) return context.id;
        } catch { /* A replaced webview's execution context may already be gone. */ }
      }
    }, `webview document ${title}`);
  }

  dispose() {
    for (const request of this.pending.values()) {
      clearTimeout(request.timeout);
      request.reject(new Error("Browser session closed"));
    }
    this.pending.clear();
    this.socket.close();
  }
}

async function waitFor(predicate, label, timeout = 8000) {
  const until = Date.now() + timeout;
  while (Date.now() < until) {
    const result = await predicate();
    if (result) return result;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for ${label}`);
}

module.exports = { BrowserSession, waitFor };
