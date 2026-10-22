"use strict";

const assert = require("assert/strict");
const Module = require("module");
const test = require("node:test");

const settings = new Map();
const configurationUpdates = [];

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
  ConfigurationTarget: { Global: 1, Workspace: 2 },
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
      update: async (key, value, target) => {
        settings.set(key, value);
        configurationUpdates.push({ key, target, value });
      },
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
  configurationUpdates.length = 0;
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

test("deterministic completion status names the configured backend mode", () => {
  const off = makeController(async () => undefined);
  assert.match(off.status.text, /inline off/);

  settings.set("completion.inline.enabled", true);
  const local = makeController(async () => undefined);
  assert.match(local.status.text, /inline: Local$/);
  assert.match(local.status.detail, /Semantic, Current file/);

  settings.set("completion.inline.sources.project", true);
  settings.set("completion.inline.sources.statistical", true);
  local.updateStatus("ready");
  assert.match(
    local.status.detail,
    /Semantic, Current file, Project, Project statistical/,
  );

  settings.set("completion.inline.enabled", false);
  settings.set("completion.multiToken.allowModelIntegration", true);
  const ollama = makeController(async () => undefined);
  assert.match(ollama.status.text, /inline: Ollama$/);
  assert.match(ollama.status.detail, /Ollama inline completion is enabled/);
  assert.doesNotMatch(ollama.status.detail, /fallback/);

  settings.set("completion.inline.enabled", true);
  const both = makeController(async () => undefined);
  assert.match(both.status.text, /inline: Local \+ Ollama$/);
});

test("local toggle changes only deterministic completion for the workspace", async () => {
  const controller = makeController(async () => undefined);

  await controller.toggleLocal();

  assert.equal(settings.get("completion.inline.enabled"), true);
  assert.deepEqual(configurationUpdates, [
    {
      key: "completion.inline.enabled",
      target: vscodeMock.ConfigurationTarget.Workspace,
      value: true,
    },
  ]);
  assert.match(controller.status.text, /inline: Local$/);
  assert.equal(settings.has("completion.multiToken.allowModelIntegration"), false);
});

test("local counters record every deterministic source without storing completion text", async () => {
  settings.set("completion.inline.enabled", true);
  const state = memento();
  const sources = ["semantic", "currentFile", "project", "projectStatistical"];
  const controller = makeController(
    async () => localResponse("update(dt)", false, sources.shift()),
    [],
    { context: { workspaceState: state } },
  );

  for (const source of ["semantic", "currentFile", "project", "projectStatistical"]) {
    const [item] = await controller.provide(document(), position(), {}, token());
    assert.equal(item.command.command, "tua.completion.localInlineAccepted");
    assert.deepEqual(item.command.arguments, [source]);
    await controller.recordLocalAcceptance(source);
  }
  await controller.flushLocalStats();

  assert.deepEqual(JSON.parse(controller.debugInfo()).deterministic, {
    semanticReturned: 1,
    semanticAccepted: 1,
    currentFileReturned: 1,
    currentFileAccepted: 1,
    projectReturned: 1,
    projectAccepted: 1,
    projectStatisticalReturned: 1,
    projectStatisticalAccepted: 1,
  });
  assert.equal(
    JSON.stringify([...state.values.entries()]).includes("update(dt)"),
    false,
    "counter persistence must not retain completion text",
  );
});

test("local counter persistence batches many returned items into one update", async () => {
  const state = memento();
  const timers = fakeStatsTimers();
  const controller = makeController(async () => undefined, [], {
    context: { workspaceState: state },
    ...timers.options,
  });
  const range = completionRange();

  controller.localInlineItem("first", range, false, "semantic");
  controller.localInlineItem("second", range, false, "semantic");
  controller.localInlineItem("third", range, false, "currentFile");

  assert.equal(state.updateCalls.length, 0);
  assert.equal(timers.pending.size, 1);
  timers.fire();
  await controller.localCounterWrite;

  assert.equal(state.updateCalls.length, 1);
  assert.deepEqual(state.updateCalls[0].value, {
    semanticReturned: 2,
    semanticAccepted: 0,
    currentFileReturned: 1,
    currentFileAccepted: 0,
    projectReturned: 0,
    projectAccepted: 0,
    projectStatisticalReturned: 0,
    projectStatisticalAccepted: 0,
  });
});

test("acceptance followed by reset cannot replay a stale delayed snapshot", async () => {
  const state = memento({
    "tua.ollamaCompletionStats.v1": {
      shownSuggestions: 9,
      acceptedSuggestions: 3,
    },
  });
  const timers = fakeStatsTimers();
  const controller = makeController(async () => undefined, [], {
    context: { workspaceState: state },
    ...timers.options,
  });

  controller.localInlineItem("update(dt)", completionRange(), false, "semantic");
  await controller.recordLocalAcceptance("semantic");
  assert.equal(timers.pending.size, 1);

  await controller.resetStats();
  assert.equal(timers.pending.size, 0);
  await controller.flushLocalStats();

  const localUpdates = state.updateCalls.filter(
    (entry) => entry.key === "tua.localInlineCompletionStats.v2",
  );
  assert.equal(localUpdates.length, 1);
  assert.deepEqual(localUpdates[0].value, {
    semanticReturned: 0,
    semanticAccepted: 0,
    currentFileReturned: 0,
    currentFileAccepted: 0,
    projectReturned: 0,
    projectAccepted: 0,
    projectStatisticalReturned: 0,
    projectStatisticalAccepted: 0,
  });
  assert.equal(state.values.has("tua.ollamaCompletionStats.v1"), false);
  assert.deepEqual(state.values.get("tua.ollamaCompletionStats.v2"), {
    requests: 0,
    cancellations: 0,
    timeouts: 0,
    emptyResponses: 0,
    validationRejections: 0,
    returnedSuggestions: 0,
    acceptedSuggestions: 0,
    acceptedCharacters: 0,
  });
  assert.deepEqual(
    state.values.get("tua.localInlineCompletionStats.v2"),
    localUpdates[0].value,
  );
  assert.equal(state.values.has("tua.localInlineCompletionStats.v1"), false);
});

test("dispose flushes a pending local counter batch", async () => {
  const state = memento();
  const timers = fakeStatsTimers();
  const controller = makeController(async () => undefined, [], {
    context: { workspaceState: state },
    ...timers.options,
  });

  controller.localInlineItem("update(dt)", completionRange(), false, "semantic");
  controller.dispose();
  await controller.localCounterWrite;

  assert.equal(timers.pending.size, 0);
  assert.equal(state.updateCalls.length, 1);
  assert.equal(state.updateCalls[0].value.semanticReturned, 1);
});

test("legacy Ollama shown counts migrate to returned counts without leaking the old name", async () => {
  const state = memento({
    "tua.ollamaCompletionStats.v1": {
      requests: 4,
      shownSuggestions: 7,
      acceptedSuggestions: 2,
      acceptedCharacters: 19,
    },
  });
  const controller = makeController(async () => undefined, [], {
    context: { workspaceState: state },
  });

  const ollama = JSON.parse(controller.debugInfo()).ollama;
  assert.equal(ollama.requests, 4);
  assert.equal(ollama.returnedSuggestions, 7);
  assert.equal(ollama.acceptedSuggestions, 2);
  assert.equal(Object.hasOwn(ollama, "shownSuggestions"), false);

  await controller.incrementStat("acceptedSuggestions");
  assert.equal(
    state.values.get("tua.ollamaCompletionStats.v2").returnedSuggestions,
    7,
  );
  assert.equal(
    Object.hasOwn(state.values.get("tua.ollamaCompletionStats.v2"), "shownSuggestions"),
    false,
  );
});

test("stored local counters migrate from v1 to v2 and allowlist numeric fields", async () => {
  const timers = fakeStatsTimers();
  const state = memento({
    "tua.localInlineCompletionStats.v1": {
      semanticReturned: 4,
      semanticAccepted: Number.POSITIVE_INFINITY,
      currentFileReturned: "7",
      currentFileAccepted: -2,
      sourceText: "must not escape",
      projectReturned: 3,
      projectAccepted: Number.POSITIVE_INFINITY,
      projectStatisticalReturned: 2,
      projectStatisticalAccepted: 1,
    },
  });
  const controller = makeController(async () => undefined, [], {
    context: { workspaceState: state },
    ...timers.options,
  });
  await controller.localCounterWrite;

  const expected = {
    semanticReturned: 4,
    semanticAccepted: 0,
    currentFileReturned: 0,
    currentFileAccepted: 0,
    projectReturned: 3,
    projectAccepted: 0,
    projectStatisticalReturned: 2,
    projectStatisticalAccepted: 1,
  };
  assert.deepEqual(JSON.parse(controller.debugInfo()).deterministic, expected);
  assert.deepEqual(state.values.get("tua.localInlineCompletionStats.v2"), expected);
  assert.equal(state.values.has("tua.localInlineCompletionStats.v1"), false);
  controller.incrementLocalCounter("semanticReturned");
  assert.equal(controller.localCounters.semanticReturned, 5);
  assert.equal(timers.pending.size, 1);
});

test("a filtered local candidate is not counted as returned", async () => {
  settings.set("completion.inline.enabled", true);
  const state = memento();
  const controller = makeController(
    async () => localResponse("update", false, "semantic"),
    [],
    { context: { workspaceState: state } },
  );
  const selectedCompletionInfo = {
    text: "update",
    range: new Range(2, 1, 2, 7),
  };

  assert.equal(
    await controller.provide(document(), position(), { selectedCompletionInfo }, token()),
    undefined,
  );
  assert.equal(JSON.parse(controller.debugInfo()).deterministic.semanticReturned, 0);
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

function memento(initial = {}) {
  const values = new Map(Object.entries(initial));
  const updateCalls = [];
  return {
    updateCalls,
    values,
    get(key) {
      return values.get(key);
    },
    async update(key, value) {
      if (value === undefined) {
        values.delete(key);
      } else {
        values.set(key, value);
      }
      updateCalls.push({ key, value });
    },
  };
}

function fakeStatsTimers() {
  const pending = new Map();
  let nextId = 0;
  return {
    pending,
    options: {
      clearLocalStatsFlush(id) {
        pending.delete(id);
      },
      scheduleLocalStatsFlush(callback) {
        nextId += 1;
        pending.set(nextId, callback);
        return nextId;
      },
    },
    fire() {
      const [id, callback] = pending.entries().next().value || [];
      assert(id, "a local counter flush should be scheduled");
      pending.delete(id);
      callback();
    },
  };
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

function completionRange() {
  return {
    start: { line: 2, character: 0 },
    end: { line: 2, character: 7 },
  };
}

function localResponse(insertText, isSnippet, source) {
  return {
    documentVersion: 7,
    items: [
      {
        insertText,
        isSnippet,
        source,
        range: completionRange(),
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
