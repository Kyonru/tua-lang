"use strict";

const vscode = require("vscode");
const { COMMANDS, STATUS_SELECTOR } = require("../constants");
const { formatError } = require("../shared/errors");
const {
  STOP_TOKENS,
  generate,
  generationTokenBudget,
  isCancellationError,
  loopbackGenerateUrl,
  touchCache,
} = require("./ollama-client");
const { createInlineReplayCapture } = require("./inline-replay-capture");

const STATS_KEY = "tua.ollamaCompletionStats.v1";
const LOCAL_INLINE_METHOD = "tua/localInlineCompletion";
const DEBOUNCE_MS = 65;
const LOCAL_REQUEST_TIMEOUT_MS = 250;
const EMPTY_STATS = Object.freeze({
  requests: 0,
  cancellations: 0,
  timeouts: 0,
  emptyResponses: 0,
  validationRejections: 0,
  shownSuggestions: 0,
  acceptedSuggestions: 0,
  acceptedCharacters: 0,
});

function registerInlineCompletion(options) {
  const controller = new InlineCompletionController(options);
  controller.register();
  return controller;
}

class InlineCompletionController {
  constructor(options) {
    this.context = options.context;
    this.getClient = options.getClient;
    this.appendOutputLine = options.appendOutputLine;
    this.localRequestTimeoutMs =
      options.localRequestTimeoutMs ?? LOCAL_REQUEST_TIMEOUT_MS;
    this.scheduleLocalTimeout = options.scheduleLocalTimeout || setTimeout;
    this.clearLocalTimeout = options.clearLocalTimeout || clearTimeout;
    this.replayCapture = createInlineReplayCapture({
      appendOutputLine: this.appendOutputLine,
      env: options.env,
    });
    this.inFlight = undefined;
    this.sequence = 0;
    this.cache = new Map();
    this.disposables = [];
    this.status = vscode.languages.createLanguageStatusItem(
      "tua.ollamaCompletions",
      STATUS_SELECTOR,
    );
    this.status.name = "Tua Local Ollama Completions";
    this.updateStatus("disabled");
  }

  register() {
    this.disposables.push(
      this.status,
      vscode.languages.registerInlineCompletionItemProvider(
        STATUS_SELECTOR,
        {
          provideInlineCompletionItems: (document, position, completionContext, token) =>
            this.provide(document, position, completionContext, token),
        },
      ),
      vscode.workspace.onDidChangeTextDocument((event) => {
        this.replayCapture?.noteEdit(event.document);
        this.cancel("edit");
      }),
      vscode.window.onDidChangeTextEditorSelection(() => this.cancel("cursor")),
      vscode.workspace.onDidChangeConfiguration((event) => {
        if (
          event.affectsConfiguration("tua.completion.inline") ||
          event.affectsConfiguration("tua.completion.multiToken.allowModelIntegration") ||
          event.affectsConfiguration("tua.completion.ollama")
        ) {
          this.cache.clear();
          this.cancel("configuration");
          this.updateStatus(this.modelEnabled() ? "ready" : "disabled");
        }
      }),
      vscode.commands.registerCommand(COMMANDS.toggleOllamaCompletions, () => this.toggle()),
      vscode.commands.registerCommand(COMMANDS.warmOllamaModel, () => this.warm()),
      vscode.commands.registerCommand(COMMANDS.ollamaCompletionAccepted, (characters) =>
        this.recordAcceptance(characters),
      ),
    );
  }

  dispose() {
    this.cancel("dispose");
    this.replayCapture?.dispose();
    for (const disposable of this.disposables.splice(0)) {
      disposable.dispose();
    }
  }

  localEnabled() {
    return vscode.workspace.getConfiguration("tua").get("completion.inline.enabled", false);
  }

  modelEnabled() {
    return vscode.workspace
      .getConfiguration("tua")
      .get("completion.multiToken.allowModelIntegration", false);
  }

  configuration() {
    const configuration = vscode.workspace.getConfiguration("tua");
    return {
      endpoint: configuration.get(
        "completion.ollama.endpoint",
        "http://127.0.0.1:11434",
      ),
      model: configuration.get(
        "completion.ollama.model",
        "qwen2.5-coder:1.5b-base",
      ),
      timeoutMs: configuration.get("completion.ollama.timeoutMs", 5000),
      keepAlive: configuration.get("completion.ollama.keepAlive", "10m"),
      contextTokens: configuration.get("completion.ollama.contextTokens", 4096),
      maxTokens: configuration.get("completion.ollama.maxTokens", 256),
    };
  }

  async provide(document, position, completionContext, token) {
    if (
      (!this.localEnabled() && !this.modelEnabled()) ||
      !this.getClient() ||
      vscode.window.activeTextEditor?.selections?.length !== 1
    ) {
      this.updateStatus(this.modelEnabled() ? "ready" : "disabled");
      return undefined;
    }

    this.cancel("newer_request");
    const requestId = ++this.sequence;
    const tokenSource = new vscode.CancellationTokenSource();
    const abortController = new AbortController();
    this.inFlight = { abortController, requestId, tokenSource };
    const cancellation = token.onCancellationRequested(() => {
      if (this.inFlight?.requestId === requestId) {
        this.cancel("provider");
      }
    });
    const requestedVersion = document.version;

    try {
      if (!(await this.debounce(requestId, tokenSource.token))) {
        return undefined;
      }

      if (this.localEnabled()) {
        this.replayCapture?.record(document, position, completionContext);
        const local = await this.provideLocalWithinTimeout(
          document,
          position,
          completionContext,
          requestedVersion,
          requestId,
          tokenSource.token,
        );
        if (local.status === "candidate") {
          return [local.item];
        }
        if (local.status === "transient") {
          return undefined;
        }
      }

      if (
        !this.modelEnabled() ||
        completionContext.selectedCompletionInfo ||
        !this.requestIsCurrent(document, requestedVersion, requestId, tokenSource.token)
      ) {
        return undefined;
      }

      const ollama = await this.provideOllama(
        document,
        position,
        requestedVersion,
        requestId,
        tokenSource.token,
        abortController,
      );
      return ollama ? [ollama] : undefined;
    } catch (error) {
      if (isSilentLocalError(error)) {
        return undefined;
      }
      this.appendOutputLine?.(`Tua inline completion unavailable: ${formatError(error)}`);
      return undefined;
    } finally {
      cancellation.dispose();
      tokenSource.dispose();
      if (this.inFlight?.requestId === requestId) {
        this.inFlight = undefined;
      }
    }
  }

  async provideLocalWithinTimeout(
    document,
    position,
    completionContext,
    requestedVersion,
    requestId,
    parentToken,
  ) {
    const tokenSource = new vscode.CancellationTokenSource();
    let abstain;
    const abstention = new Promise((resolve) => {
      abstain = () => resolve({ status: "transient" });
    });
    const cancel = () => {
      tokenSource.cancel();
      abstain();
    };
    const parentCancellation = parentToken.onCancellationRequested(cancel);
    const timeout = this.scheduleLocalTimeout(cancel, this.localRequestTimeoutMs);

    try {
      if (parentToken.isCancellationRequested) {
        cancel();
      }
      return await Promise.race([
        this.provideLocal(
          document,
          position,
          completionContext,
          requestedVersion,
          requestId,
          tokenSource.token,
        ),
        abstention,
      ]);
    } finally {
      this.clearLocalTimeout(timeout);
      parentCancellation.dispose();
      tokenSource.dispose();
    }
  }

  async provideLocal(
    document,
    position,
    completionContext,
    requestedVersion,
    requestId,
    token,
  ) {
    const params = {
      textDocument: { uri: document.uri.toString() },
      position: { line: position.line, character: position.character },
      documentVersion: requestedVersion,
    };
    const selected = completionContext.selectedCompletionInfo;
    if (selected) {
      params.selectedCompletion = {
        text: selected.text,
        range: protocolRange(selected.range),
      };
    }

    let response;
    try {
      response = await this.getClient().sendRequest(LOCAL_INLINE_METHOD, params, token);
    } catch (error) {
      const status = localErrorStatus(error);
      if (status) {
        return { status };
      }
      throw error;
    }

    if (!response) {
      return { status: "empty" };
    }
    if (
      response.documentVersion !== requestedVersion ||
      !this.requestIsCurrent(document, requestedVersion, requestId, token)
    ) {
      return { status: "transient" };
    }

    const candidate = Array.isArray(response.items) ? response.items[0] : undefined;
    if (!candidate?.insertText || !candidate.range) {
      return { status: "empty" };
    }
    if (
      selected &&
      (candidate.insertText === selected.text || !candidate.insertText.startsWith(selected.text))
    ) {
      return { status: "empty" };
    }

    return {
      status: "candidate",
      item: this.inlineItem(candidate.insertText, candidate.range, candidate.isSnippet, false),
    };
  }

  async provideOllama(
    document,
    position,
    requestedVersion,
    requestId,
    token,
    abortController,
  ) {
    const config = this.configuration();
    let endpoint;
    try {
      endpoint = loopbackGenerateUrl(config.endpoint);
    } catch (error) {
      this.updateStatus("invalid_endpoint", formatError(error));
      return undefined;
    }

    let phase = "language_server";
    try {
      const params = {
        textDocument: { uri: document.uri.toString() },
        position: { line: position.line, character: position.character },
        documentVersion: requestedVersion,
        contextBytes: Math.min(config.contextTokens * 3, 32 * 1024),
      };
      const semanticContext = await this.getClient().sendRequest(
        "tua/inlineCompletionContext",
        params,
        token,
      );
      if (
        !semanticContext ||
        semanticContext.documentVersion !== requestedVersion ||
        !this.requestIsCurrent(document, requestedVersion, requestId, token)
      ) {
        return undefined;
      }

      const cacheKey = JSON.stringify([
        document.uri.toString(),
        requestedVersion,
        position.line,
        position.character,
        semanticContext.contextId,
        config,
      ]);
      const cached = this.cache.get(cacheKey);
      if (cached) {
        touchCache(this.cache, cacheKey, cached);
        await this.incrementStat("shownSuggestions");
        return this.inlineItem(cached.insertText, cached.range, false, true);
      }

      await this.incrementStat("requests");
      this.updateStatus("loading");
      phase = "ollama";
      const response = await generate(
        endpoint,
        {
          model: config.model,
          prompt: semanticContext.prompt,
          suffix: semanticContext.suffix,
          stream: false,
          keep_alive: config.keepAlive,
          options: {
            temperature: 0.1,
            top_p: 0.9,
            num_ctx: config.contextTokens,
            num_predict: generationTokenBudget(
              config.maxTokens,
              semanticContext.completionScope,
            ),
            stop: STOP_TOKENS,
          },
        },
        config.timeoutMs,
        abortController.signal,
      );

      const candidate = typeof response.response === "string" ? response.response : "";
      if (!candidate) {
        await this.incrementStat("emptyResponses");
        this.updateStatus("rejected", "Ollama returned an empty completion.");
        return undefined;
      }

      phase = "language_server";
      const validation = await this.getClient().sendRequest(
        "tua/validateInlineCompletion",
        {
          ...params,
          contextId: semanticContext.contextId,
          analysisRevision: semanticContext.analysisRevision,
          contextBytes: semanticContext.contextBytes,
          sourceFingerprint: semanticContext.sourceFingerprint,
          candidate,
        },
        token,
      );
      if (
        validation?.status !== "accepted" ||
        !validation.insertText ||
        validation.documentVersion !== requestedVersion ||
        !this.requestIsCurrent(document, requestedVersion, requestId, token)
      ) {
        await this.incrementStat("validationRejections");
        this.updateStatus("rejected", validation?.reason || "validation rejected");
        return undefined;
      }

      const value = { insertText: validation.insertText, range: validation.range };
      touchCache(this.cache, cacheKey, value);
      await this.incrementStat("shownSuggestions");
      this.updateStatus("ready");
      return this.inlineItem(value.insertText, value.range, false, true);
    } catch (error) {
      if (isCancellationError(error)) {
        await this.incrementStat("cancellations");
        this.updateStatus(this.modelEnabled() ? "ready" : "disabled");
        return undefined;
      }
      if (phase === "language_server") {
        this.updateStatus("language_server", formatError(error));
        return undefined;
      }
      if (error?.code === "TUA_OLLAMA_TIMEOUT") {
        await this.incrementStat("timeouts");
        this.updateStatus("timeout");
        return undefined;
      }
      if (error?.statusCode === 404) {
        this.updateStatus("missing_model", config.model);
        return undefined;
      }
      this.updateStatus("missing_server", formatError(error));
      return undefined;
    }
  }

  requestIsCurrent(document, requestedVersion, requestId, token) {
    return (
      requestId === this.sequence &&
      document.version === requestedVersion &&
      !token.isCancellationRequested
    );
  }

  debounce(requestId, token) {
    return new Promise((resolve) => {
      const finish = (ready) => {
        if (this.inFlight?.requestId === requestId) {
          this.inFlight.finishDebounce = undefined;
          this.inFlight.debounceTimer = undefined;
        }
        resolve(ready);
      };
      const timer = setTimeout(
        () => finish(requestId === this.sequence && !token.isCancellationRequested),
        DEBOUNCE_MS,
      );
      if (this.inFlight?.requestId === requestId) {
        this.inFlight.debounceTimer = timer;
        this.inFlight.finishDebounce = finish;
      }
    });
  }

  inlineItem(insertText, range, isSnippet, recordOllamaAcceptance) {
    const item = new vscode.InlineCompletionItem(
      isSnippet ? new vscode.SnippetString(insertText) : insertText,
      new vscode.Range(
        range.start.line,
        range.start.character,
        range.end.line,
        range.end.character,
      ),
    );
    if (recordOllamaAcceptance) {
      item.command = {
        command: COMMANDS.ollamaCompletionAccepted,
        title: "Record Tua local completion acceptance",
        arguments: [insertText.length],
      };
    }
    return item;
  }

  async toggle() {
    const configuration = vscode.workspace.getConfiguration("tua");
    const enabled = this.modelEnabled();
    if (!enabled) {
      const consent = await vscode.window.showWarningMessage(
        "Enable local Ollama completions? Tua will send bounded source excerpts and semantic type context to the loopback Ollama server configured on this machine. No source is recorded in Tua completion statistics.",
        { modal: true },
        "Enable",
      );
      if (consent !== "Enable") {
        return;
      }
    }

    await configuration.update(
      "completion.multiToken.allowModelIntegration",
      !enabled,
      vscode.ConfigurationTarget.Global,
    );
    this.updateStatus(enabled ? "disabled" : "ready");
    vscode.window.showInformationMessage(
      `Tua local Ollama completions ${enabled ? "disabled" : "enabled"}.`,
    );
  }

  async warm() {
    const config = this.configuration();
    let endpoint;
    try {
      endpoint = loopbackGenerateUrl(config.endpoint);
    } catch (error) {
      this.updateStatus("invalid_endpoint", formatError(error));
      vscode.window.showErrorMessage(formatError(error));
      return;
    }

    this.cancel("warm");
    const abortController = new AbortController();
    const requestId = ++this.sequence;
    this.inFlight = { abortController, requestId };
    this.updateStatus("loading");
    try {
      await generate(
        endpoint,
        {
          model: config.model,
          prompt: "",
          stream: false,
          keep_alive: config.keepAlive,
        },
        Math.max(config.timeoutMs, 30_000),
        abortController.signal,
      );
      this.updateStatus("ready");
      vscode.window.showInformationMessage(`Ollama model ${config.model} is warm and ready.`);
    } catch (error) {
      if (isCancellationError(error)) {
        this.updateStatus(this.modelEnabled() ? "ready" : "disabled");
        return;
      }
      if (error?.statusCode === 404) {
        this.updateStatus("missing_model", config.model);
        vscode.window.showWarningMessage(
          `Ollama model ${config.model} is missing. Run: ollama pull ${config.model}`,
        );
      } else {
        this.updateStatus("missing_server", formatError(error));
        vscode.window.showWarningMessage(`Could not warm Ollama: ${formatError(error)}`);
      }
    } finally {
      if (this.inFlight?.requestId === requestId) {
        this.inFlight = undefined;
      }
    }
  }

  cancel() {
    if (!this.inFlight) {
      return;
    }
    this.inFlight.tokenSource?.cancel();
    this.inFlight.abortController?.abort();
    if (this.inFlight.debounceTimer) {
      clearTimeout(this.inFlight.debounceTimer);
      this.inFlight.finishDebounce?.(false);
    }
    this.inFlight = undefined;
  }

  stats() {
    return { ...EMPTY_STATS, ...(this.context.workspaceState.get(STATS_KEY) || {}) };
  }

  async incrementStat(name, amount = 1) {
    const stats = this.stats();
    stats[name] += amount;
    await this.context.workspaceState.update(STATS_KEY, stats);
  }

  async recordAcceptance(characters) {
    await this.incrementStat("acceptedSuggestions");
    await this.incrementStat(
      "acceptedCharacters",
      Number.isFinite(characters) ? Math.max(0, Number(characters)) : 0,
    );
  }

  async resetStats() {
    await this.context.workspaceState.update(STATS_KEY, { ...EMPTY_STATS });
  }

  debugInfo() {
    return JSON.stringify(this.stats());
  }

  updateStatus(state, detail) {
    const config = this.configuration();
    this.status.busy = state === "loading";
    this.status.severity =
      state === "missing_server" ||
      state === "language_server" ||
      state === "missing_model" ||
      state === "timeout" ||
      state === "invalid_endpoint"
        ? vscode.LanguageStatusSeverity.Warning
        : vscode.LanguageStatusSeverity.Information;
    this.status.command =
      state === "language_server"
        ? undefined
        : {
            command:
              state === "disabled"
                ? COMMANDS.toggleOllamaCompletions
                : COMMANDS.warmOllamaModel,
            title:
              state === "disabled"
                ? "Enable Tua Local Ollama Completions"
                : "Test and Warm Ollama Model",
          };

    const labels = {
      disabled: ["$(circle-slash) Tua Ollama off", "Local model completions are disabled."],
      loading: ["$(sync~spin) Tua Ollama loading", `Loading ${config.model}.`],
      ready: ["$(check) Tua Ollama", `${config.model} is ready.`],
      language_server: [
        "$(warning) Tua completion unavailable",
        "The Tua language server is unavailable.",
      ],
      missing_server: ["$(warning) Tua Ollama unavailable", "Ollama server is unavailable."],
      missing_model: [
        "$(warning) Tua Ollama model missing",
        `Run: ollama pull ${config.model}`,
      ],
      timeout: ["$(watch) Tua Ollama timeout", `No response within ${config.timeoutMs} ms.`],
      rejected: ["$(info) Tua Ollama rejected", "The semantic validator rejected a candidate."],
      invalid_endpoint: [
        "$(warning) Tua Ollama endpoint blocked",
        "Only loopback HTTP endpoints are allowed.",
      ],
    };
    const [text, baseDetail] = labels[state] || labels.ready;
    this.status.text = text;
    this.status.detail = detail ? `${baseDetail}\n${detail}` : baseDetail;
    if (detail) {
      this.appendOutputLine?.(`Tua Ollama: ${state}: ${detail}`);
    }
  }
}

function protocolRange(range) {
  return {
    start: { line: range.start.line, character: range.start.character },
    end: { line: range.end.line, character: range.end.character },
  };
}

function isSilentLocalError(error) {
  return localErrorStatus(error) !== undefined;
}

function localErrorStatus(error) {
  const code = Number(error?.code);
  const message = String(error?.message || "");
  if (code === -32601) {
    return "unsupported";
  }
  if (
    isCancellationError(error) ||
    code === -32802 ||
    (code === -32000 && /capacity exceeded/i.test(message))
  ) {
    return "transient";
  }
  return undefined;
}

module.exports = {
  DEBOUNCE_MS,
  EMPTY_STATS,
  InlineCompletionController,
  LOCAL_REQUEST_TIMEOUT_MS,
  isSilentLocalError,
  protocolRange,
  registerInlineCompletion,
};
