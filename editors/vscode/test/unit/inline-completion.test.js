"use strict";

const assert = require("assert/strict");
const Module = require("module");
const test = require("node:test");

const settings = new Map();

class CancellationTokenSource {
  constructor() {
    const listeners = new Set();
    this.token = {
      isCancellationRequested: false,
      onCancellationRequested(listener) {
        listeners.add(listener);
        return { dispose: () => listeners.delete(listener) };
      },
    };
    this.listeners = listeners;
    this.disposed = false;
    CancellationTokenSource.instances.push(this);
  }

  cancel() {
    if (this.token.isCancellationRequested) {
      return;
    }
    this.token.isCancellationRequested = true;
    for (const listener of this.listeners) {
      listener();
    }
  }

  dispose() {
    this.disposed = true;
    this.listeners.clear();
  }
}

CancellationTokenSource.instances = [];

class Range {
  constructor(startLine, startCharacter, endLine, endCharacter) {
    this.start = { line: startLine, character: startCharacter };
    this.end = { line: endLine, character: endCharacter };
  }
}

class SnippetString {
  constructor(value) {
    this.value = value;
  }
}

class InlineCompletionItem {
  constructor(insertText, range) {
    this.insertText = insertText;
    this.range = range;
  }
}

const vscodeMock = {
  CancellationTokenSource,
  ConfigurationTarget: { Global: 1 },
  InlineCompletionItem,
  LanguageStatusSeverity: { Information: 1, Warning: 2 },
  Range,
  SnippetString,
  commands: {
    registerCommand: () => disposable(),
  },
  languages: {
    createLanguageStatusItem: () => ({ dispose() {} }),
    registerInlineCompletionItemProvider: () => disposable(),
  },
  window: {
    activeTextEditor: { selections: [{}] },
    onDidChangeTextEditorSelection: () => disposable(),
    showInformationMessage: async () => undefined,
    showWarningMessage: async () => undefined,
  },
  workspace: {
    getConfiguration: () => ({
      get: (key, fallback) => (settings.has(key) ? settings.get(key) : fallback),
      update: async (key, value) => settings.set(key, value),
    }),
    onDidChangeConfiguration: () => disposable(),
    onDidChangeTextDocument: () => disposable(),
  },
};

const originalLoad = Module._load;
Module._load = function load(request, parent, isMain) {
  if (request === "vscode") {
    return vscodeMock;
  }
  return originalLoad.call(this, request, parent, isMain);
};
const {
  InlineCompletionController,
  isSilentLocalError,
} = require("../../src/editor/inline-completion");
Module._load = originalLoad;

test.beforeEach(() => {
  settings.clear();
  CancellationTokenSource.instances = [];
  vscodeMock.window.activeTextEditor = { selections: [{}] };
});

test("disabled inline completion does not contact the language server", async () => {
  const calls = [];
  const controller = makeController(async (method) => calls.push(method));

  assert.equal(await controller.provide(document(), position(), {}, token()), undefined);
  assert.deepEqual(calls, []);
});

test("local completion runs before Ollama and creates snippet items", async () => {
  settings.set("completion.inline.enabled", true);
  settings.set("completion.multiToken.allowModelIntegration", true);
  const calls = [];
  const controller = makeController(async (method, params) => {
    calls.push(method);
    assert.equal(params.documentVersion, 7);
    return localResponse("for ${1:item} in ${2:items} do", true);
  });

  const result = await controller.provide(document(), position(), {}, token());

  assert.deepEqual(calls, ["tua/localInlineCompletion"]);
  assert.equal(result.length, 1);
  assert(result[0].insertText instanceof SnippetString);
  assert.equal(result[0].insertText.value, "for ${1:item} in ${2:items} do");
  assert.equal(result[0].command, undefined);
});

test("developer replay capture observes the real menu state before the local request", async () => {
  settings.set("completion.inline.enabled", true);
  const calls = [];
  const controller = makeController(async (method) => {
    calls.push(method);
    return { documentVersion: 7, items: [] };
  });
  const completionContext = {
    selectedCompletionInfo: {
      text: "player",
      range: new Range(2, 1, 2, 7),
    },
  };
  controller.replayCapture = {
    record: (capturedDocument, capturedPosition, capturedContext) => {
      calls.push("capture");
      assert.equal(capturedDocument.version, 7);
      assert.deepEqual(capturedPosition, position());
      assert.equal(capturedContext, completionContext);
    },
  };

  await controller.provide(document(), position(), completionContext, token());

  assert.deepEqual(calls, ["capture", "tua/localInlineCompletion"]);
});

test("Ollama context is requested only after the local backend returns empty", async () => {
  settings.set("completion.inline.enabled", true);
  settings.set("completion.multiToken.allowModelIntegration", true);
  const calls = [];
  const controller = makeController(async (method) => {
    calls.push(method);
    return method === "tua/localInlineCompletion"
      ? { documentVersion: 7, items: [] }
      : undefined;
  });

  assert.equal(await controller.provide(document(), position(), {}, token()), undefined);
  assert.deepEqual(calls, ["tua/localInlineCompletion", "tua/inlineCompletionContext"]);
});

test("an older server without the local method can still fall back to Ollama", async () => {
  settings.set("completion.inline.enabled", true);
  settings.set("completion.multiToken.allowModelIntegration", true);
  const calls = [];
  const controller = makeController(async (method) => {
    calls.push(method);
    if (method === "tua/localInlineCompletion") {
      throw responseError(-32601, "method not found");
    }
    return undefined;
  });

  assert.equal(await controller.provide(document(), position(), {}, token()), undefined);
  assert.deepEqual(calls, ["tua/localInlineCompletion", "tua/inlineCompletionContext"]);
});

test("selected menu completion is sent to the server and equivalent text is hidden", async () => {
  settings.set("completion.inline.enabled", true);
  const calls = [];
  const selectedCompletionInfo = {
    text: "update",
    range: new Range(2, 4, 2, 7),
  };
  const controller = makeController(async (method, params) => {
    calls.push({ method, params });
    return localResponse("update", false);
  });

  const result = await controller.provide(
    document(),
    position(),
    { selectedCompletionInfo },
    token(),
  );

  assert.equal(result, undefined);
  assert.deepEqual(calls[0].params.selectedCompletion, {
    text: "update",
    range: {
      start: { line: 2, character: 4 },
      end: { line: 2, character: 7 },
    },
  });
});

test("a local completion can extend the selected menu item over its replacement range", async () => {
  settings.set("completion.inline.enabled", true);
  const selectedCompletionInfo = {
    text: "update",
    range: new Range(2, 1, 2, 7),
  };
  const controller = makeController(async () => ({
    documentVersion: 7,
    items: [
      {
        insertText: "update(dt)",
        isSnippet: false,
        range: {
          start: { line: 2, character: 1 },
          end: { line: 2, character: 7 },
        },
      },
    ],
  }));

  const [item] = await controller.provide(
    document(),
    position(),
    { selectedCompletionInfo },
    token(),
  );

  assert.equal(item.insertText, "update(dt)");
  assert.deepEqual(item.range, selectedCompletionInfo.range);
});

test("a response for a stale document version is ignored", async () => {
  settings.set("completion.inline.enabled", true);
  const currentDocument = document();
  const controller = makeController(async () => {
    currentDocument.version = 8;
    return localResponse("velocity", false);
  });

  assert.equal(
    await controller.provide(currentDocument, position(), {}, token()),
    undefined,
  );
});

test("capacity, content-modified, and cancellation errors stay silent", async () => {
  settings.set("completion.inline.enabled", true);
  for (const error of [
    responseError(-32000, "language server request capacity exceeded"),
    responseError(-32601, "method not found"),
    responseError(-32801, "content modified"),
    responseError(-32800, "request cancelled"),
  ]) {
    const output = [];
    const controller = makeController(async () => {
      throw error;
    }, output);
    assert.equal(await controller.provide(document(), position(), {}, token()), undefined);
    assert.deepEqual(output, []);
    assert.equal(isSilentLocalError(error), true);
  }
});

test("transient local failures abstain instead of falling through to Ollama", async () => {
  settings.set("completion.inline.enabled", true);
  settings.set("completion.multiToken.allowModelIntegration", true);
  for (const error of [
    responseError(-32000, "language server request capacity exceeded"),
    responseError(-32801, "content modified"),
    responseError(-32800, "request cancelled"),
  ]) {
    const calls = [];
    const controller = makeController(async (method) => {
      calls.push(method);
      throw error;
    });

    assert.equal(await controller.provide(document(), position(), {}, token()), undefined);
    assert.deepEqual(calls, ["tua/localInlineCompletion"]);
  }
});

test("cancelling during debounce prevents a request", async () => {
  settings.set("completion.inline.enabled", true);
  const calls = [];
  const controller = makeController(async (method) => calls.push(method));
  const pending = controller.provide(document(), position(), {}, token());
  controller.cancel("edit");

  assert.equal(await pending, undefined);
  assert.deepEqual(calls, []);
});

test("late cancellation of a superseded provider cannot cancel the newer request", async () => {
  settings.set("completion.inline.enabled", true);
  const calls = [];
  const controller = makeController(async (method) => {
    calls.push(method);
    return localResponse("velocity", false);
  });
  const oldToken = new CancellationTokenSource();
  const newToken = new CancellationTokenSource();

  const oldRequest = controller.provide(document(), position(), {}, oldToken.token);
  const newRequest = controller.provide(document(), position(), {}, newToken.token);
  oldToken.cancel();

  assert.equal(await oldRequest, undefined);
  assert.equal((await newRequest).length, 1);
  assert.deepEqual(calls, ["tua/localInlineCompletion"]);
});

test("local request timeout cancels linked work and returns a transient abstention", async () => {
  settings.set("completion.inline.enabled", true);
  const timers = fakeLocalTimers();
  const parent = new CancellationTokenSource();
  const controller = makeController(
    async (_method, _params, requestToken) =>
      new Promise(() => {
        assert.equal(requestToken.isCancellationRequested, false);
      }),
    [],
    timers.options,
  );
  controller.sequence = 1;

  const pending = controller.provideLocalWithinTimeout(
    document(),
    position(),
    {},
    7,
    1,
    parent.token,
  );
  const linked = CancellationTokenSource.instances.at(-1);
  assert.equal(timers.pending.size, 1);
  timers.fire();

  assert.deepEqual(await pending, { status: "transient" });
  assert.equal(linked.token.isCancellationRequested, true);
  assert.equal(linked.disposed, true);
  assert.equal(parent.listeners.size, 0);
  assert.equal(timers.pending.size, 0);
  assert.equal(timers.clearCalls, 1);
});

test("successful local request clears timeout and disposes its linked token", async () => {
  settings.set("completion.inline.enabled", true);
  const timers = fakeLocalTimers();
  const parent = new CancellationTokenSource();
  const controller = makeController(
    async () => ({ documentVersion: 7, items: [] }),
    [],
    timers.options,
  );
  controller.sequence = 1;

  const result = await controller.provideLocalWithinTimeout(
    document(),
    position(),
    {},
    7,
    1,
    parent.token,
  );
  const linked = CancellationTokenSource.instances.at(-1);

  assert.deepEqual(result, { status: "empty" });
  assert.equal(linked.token.isCancellationRequested, false);
  assert.equal(linked.disposed, true);
  assert.equal(parent.listeners.size, 0);
  assert.equal(timers.pending.size, 0);
  assert.equal(timers.clearCalls, 1);
});

test("provider cancellation propagates to the linked local request", async () => {
  settings.set("completion.inline.enabled", true);
  const timers = fakeLocalTimers();
  const parent = new CancellationTokenSource();
  const controller = makeController(
    async () => new Promise(() => {}),
    [],
    timers.options,
  );
  controller.sequence = 1;

  const pending = controller.provideLocalWithinTimeout(
    document(),
    position(),
    {},
    7,
    1,
    parent.token,
  );
  const linked = CancellationTokenSource.instances.at(-1);
  parent.cancel();

  assert.deepEqual(await pending, { status: "transient" });
  assert.equal(linked.token.isCancellationRequested, true);
  assert.equal(linked.disposed, true);
  assert.equal(parent.listeners.size, 0);
  assert.equal(timers.pending.size, 0);
  assert.equal(timers.clearCalls, 1);
});

test("timed-out local request never falls through to Ollama", async () => {
  settings.set("completion.inline.enabled", true);
  settings.set("completion.multiToken.allowModelIntegration", true);
  const timers = fakeLocalTimers();
  const calls = [];
  const controller = makeController(
    async (method) => {
      calls.push(method);
      return new Promise(() => {});
    },
    [],
    timers.options,
  );
  controller.debounce = async () => true;

  const pending = controller.provide(document(), position(), {}, token());
  await Promise.resolve();
  timers.fire();

  assert.equal(await pending, undefined);
  assert.deepEqual(calls, ["tua/localInlineCompletion"]);
  assert.equal(timers.pending.size, 0);
});

function makeController(sendRequest, output = [], options = {}) {
  return new InlineCompletionController({
    appendOutputLine: (line) => output.push(line),
    context: {
      workspaceState: {
        get: () => undefined,
        update: async () => undefined,
      },
    },
    env: {},
    getClient: () => ({ sendRequest }),
    ...options,
  });
}

function fakeLocalTimers() {
  const pending = new Map();
  let nextId = 0;
  const timers = {
    clearCalls: 0,
    pending,
    options: {
      clearLocalTimeout(id) {
        timers.clearCalls += 1;
        pending.delete(id);
      },
      scheduleLocalTimeout(callback) {
        nextId += 1;
        pending.set(nextId, callback);
        return nextId;
      },
    },
    fire() {
      const [id, callback] = pending.entries().next().value || [];
      assert(id, "a local request timeout should be scheduled");
      pending.delete(id);
      callback();
    },
  };
  return timers;
}

function document() {
  return {
    uri: { toString: () => "file:///workspace/main.tua" },
    version: 7,
  };
}

function position() {
  return { line: 2, character: 7 };
}

function token() {
  return new CancellationTokenSource().token;
}

function localResponse(insertText, isSnippet) {
  return {
    documentVersion: 7,
    items: [
      {
        insertText,
        isSnippet,
        range: {
          start: { line: 2, character: 0 },
          end: { line: 2, character: 7 },
        },
      },
    ],
  };
}

function responseError(code, message) {
  return Object.assign(new Error(message), { code });
}

function disposable() {
  return { dispose() {} };
}
