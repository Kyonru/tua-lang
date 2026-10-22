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

const STATS_KEY = "tua.ollamaCompletionStats.v2";
const LEGACY_STATS_KEY = "tua.ollamaCompletionStats.v1";
const LOCAL_STATS_KEY = "tua.localInlineCompletionStats.v2";
const LEGACY_LOCAL_STATS_KEY = "tua.localInlineCompletionStats.v1";
const LOCAL_INLINE_METHOD = "tua/localInlineCompletion";
const DEBOUNCE_MS = 65;
const LOCAL_REQUEST_TIMEOUT_MS = 250;
const LOCAL_STATS_FLUSH_MS = 30_000;
const EMPTY_STATS = Object.freeze({
  requests: 0,
  cancellations: 0,
  timeouts: 0,
  emptyResponses: 0,
  validationRejections: 0,
  returnedSuggestions: 0,
  acceptedSuggestions: 0,
  acceptedCharacters: 0,
});
const EMPTY_LOCAL_STATS = Object.freeze({
  semanticReturned: 0,
  semanticAccepted: 0,
  currentFileReturned: 0,
  currentFileAccepted: 0,
  projectReturned: 0,
  projectAccepted: 0,
  projectStatisticalReturned: 0,
  projectStatisticalAccepted: 0,
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
    this.scheduleLocalStatsFlush = options.scheduleLocalStatsFlush || setTimeout;
    this.clearLocalStatsFlush = options.clearLocalStatsFlush || clearTimeout;
    this.replayCapture = createInlineReplayCapture({
      appendOutputLine: this.appendOutputLine,
      env: options.env,
    });
    this.inFlight = undefined;
    this.sequence = 0;
    this.cache = new Map();
    this.disposables = [];
    const storedLocalStats = this.context.workspaceState.get(LOCAL_STATS_KEY);
    const legacyLocalStats = storedLocalStats
      ? undefined
      : this.context.workspaceState.get(LEGACY_LOCAL_STATS_KEY);
    this.localCounters = normalizedLocalStats(storedLocalStats || legacyLocalStats || {});
    this.ollamaCounters = loadOllamaStats(this.context.workspaceState);
    this.localStatsDirty = false;
    this.localStatsTimer = undefined;
    this.localCounterWrite = legacyLocalStats
      ? this.context.workspaceState
        .update(LOCAL_STATS_KEY, { ...this.localCounters })
        .then(() => this.context.workspaceState.update(LEGACY_LOCAL_STATS_KEY, undefined))
        .catch((error) => {
          this.appendOutputLine?.(
            `Could not migrate Tua inline completion counters: ${formatError(error)}`,
          );
        })
      : Promise.resolve();
    this.status = vscode.languages.createLanguageStatusItem(
      "tua.inlineCompletions",
      STATUS_SELECTOR,
    );
    this.status.name = "Tua Inline Completions";
    this.updateStatus("ready");
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
      vscode.commands.registerCommand(COMMANDS.toggleLocalInlineCompletions, () =>
        this.toggleLocal(),
      ),
      vscode.commands.registerCommand(COMMANDS.toggleOllamaCompletions, () => this.toggle()),
      vscode.commands.registerCommand(COMMANDS.warmOllamaModel, () => this.warm()),
      vscode.commands.registerCommand(COMMANDS.localInlineCompletionAccepted, (source) =>
        this.recordLocalAcceptance(source),
      ),
      vscode.commands.registerCommand(COMMANDS.ollamaCompletionAccepted, (characters) =>
        this.recordAcceptance(characters),
      ),
    );
  }

  dispose() {
    this.cancel("dispose");
    void this.flushLocalStats();
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

  deterministicSourceLabels() {
    const configuration = vscode.workspace.getConfiguration("tua");
    const sources = [
      ["completion.inline.sources.semantic", true, "Semantic"],
      ["completion.inline.sources.currentFile", true, "Current file"],
      ["completion.inline.sources.project", false, "Project"],
      ["completion.inline.sources.statistical", false, "Project statistical"],
    ];
    return sources
      .filter(([key, fallback]) => configuration.get(key, fallback))
      .map(([, , label]) => label);
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
      item: this.localInlineItem(
        candidate.insertText,
        candidate.range,
        candidate.isSnippet,
        candidate.source,
      ),
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
        await this.incrementStat("returnedSuggestions");
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
      await this.incrementStat("returnedSuggestions");
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

  localInlineItem(insertText, range, isSnippet, source) {
    const item = this.inlineItem(insertText, range, isSnippet, false);
    const returnedCounter = localCounterName(source, "Returned");
    if (!returnedCounter) {
      return item;
    }

    this.incrementLocalCounter(returnedCounter);
    item.command = {
      command: COMMANDS.localInlineCompletionAccepted,
      title: "Record Tua deterministic completion acceptance",
      arguments: [source],
    };
    return item;
  }

  async toggleLocal() {
    const configuration = vscode.workspace.getConfiguration("tua");
    const enabled = this.localEnabled();
    await configuration.update(
      "completion.inline.enabled",
      !enabled,
      vscode.ConfigurationTarget.Workspace,
    );
    this.updateStatus("ready");
    vscode.window.showInformationMessage(
      `Tua deterministic inline completions ${enabled ? "disabled" : "enabled"} for this workspace.`,
    );
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
    return { ...this.ollamaCounters };
  }

  async incrementStat(name, amount = 1) {
    this.ollamaCounters[name] += amount;
    await this.context.workspaceState.update(STATS_KEY, { ...this.ollamaCounters });
  }

  async recordAcceptance(characters) {
    await this.incrementStat("acceptedSuggestions");
    await this.incrementStat(
      "acceptedCharacters",
      Number.isFinite(characters) ? Math.max(0, Number(characters)) : 0,
    );
  }

  incrementLocalCounter(name) {
    this.localCounters[name] += 1;
    this.localStatsDirty = true;
    if (this.localStatsTimer === undefined) {
      this.localStatsTimer = this.scheduleLocalStatsFlush(
        () => {
          this.localStatsTimer = undefined;
          void this.flushLocalStats();
        },
        LOCAL_STATS_FLUSH_MS,
      );
    }
  }

  recordLocalAcceptance(source) {
    const counter = localCounterName(source, "Accepted");
    if (counter) {
      this.incrementLocalCounter(counter);
    }
    return Promise.resolve();
  }

  flushLocalStats() {
    this.cancelLocalStatsFlush();
    if (!this.localStatsDirty) {
      return this.localCounterWrite;
    }

    this.localStatsDirty = false;
    const snapshot = { ...this.localCounters };
    this.localCounterWrite = this.localCounterWrite
      .then(() => this.context.workspaceState.update(LOCAL_STATS_KEY, snapshot))
      .catch((error) => {
        this.appendOutputLine?.(
          `Could not persist Tua inline completion counters: ${formatError(error)}`,
        );
      });
    return this.localCounterWrite;
  }

  cancelLocalStatsFlush() {
    if (this.localStatsTimer !== undefined) {
      this.clearLocalStatsFlush(this.localStatsTimer);
      this.localStatsTimer = undefined;
    }
  }

  async resetStats() {
    this.cancelLocalStatsFlush();
    this.localStatsDirty = false;
    await this.localCounterWrite;
    this.cancelLocalStatsFlush();
    this.localCounters = { ...EMPTY_LOCAL_STATS };
    this.ollamaCounters = { ...EMPTY_STATS };
    await this.context.workspaceState.update(STATS_KEY, { ...this.ollamaCounters });
    await this.context.workspaceState.update(LEGACY_STATS_KEY, undefined);
    await this.context.workspaceState.update(LOCAL_STATS_KEY, { ...EMPTY_LOCAL_STATS });
    await this.context.workspaceState.update(LEGACY_LOCAL_STATS_KEY, undefined);
  }

  debugInfo() {
    return JSON.stringify({
      deterministic: { ...this.localCounters },
      ollama: this.stats(),
    });
  }

  updateStatus(state, detail) {
    const config = this.configuration();
    const mode = this.inlineMode();
    const deterministicSources = this.deterministicSourceLabels();
    this.status.busy = state === "loading";
    this.status.severity =
      state === "missing_server" ||
      state === "language_server" ||
      state === "missing_model" ||
      state === "timeout" ||
      state === "invalid_endpoint"
        ? vscode.LanguageStatusSeverity.Warning
        : vscode.LanguageStatusSeverity.Information;
    this.status.command = statusCommand(mode, state);

    const labels = {
      disabled: configuredStatus(mode, config, deterministicSources),
      loading: [
        `$(sync~spin) Tua inline: ${mode === "both" ? "Local + Ollama" : "Ollama"} loading`,
        `Loading ${config.model}.`,
      ],
      ready: configuredStatus(mode, config, deterministicSources),
      language_server: [
        "$(warning) Tua completion unavailable",
        "The Tua language server is unavailable.",
      ],
      missing_server: [
        `$(warning) Tua inline: ${mode === "both" ? "Local (Ollama unavailable)" : "Ollama unavailable"}`,
        "Ollama server is unavailable.",
      ],
      missing_model: [
        `$(warning) Tua inline: ${mode === "both" ? "Local (Ollama model missing)" : "Ollama model missing"}`,
        `Run: ollama pull ${config.model}`,
      ],
      timeout: [
        `$(watch) Tua inline: ${mode === "both" ? "Local (Ollama timeout)" : "Ollama timeout"}`,
        `No response within ${config.timeoutMs} ms.`,
      ],
      rejected: [
        `$(info) Tua inline: ${mode === "both" ? "Local (Ollama rejected)" : "Ollama rejected"}`,
        "The semantic validator rejected an Ollama candidate.",
      ],
      invalid_endpoint: [
        `$(warning) Tua inline: ${mode === "both" ? "Local (Ollama endpoint blocked)" : "Ollama endpoint blocked"}`,
        "Only loopback HTTP endpoints are allowed.",
      ],
    };
    const [text, baseDetail] = labels[state] || labels.ready;
    this.status.text = text;
    this.status.detail = detail ? `${baseDetail}\n${detail}` : baseDetail;
    if (detail) {
      this.appendOutputLine?.(`Tua inline completion: ${state}: ${detail}`);
    }
  }

  inlineMode() {
    const local = this.localEnabled();
    const model = this.modelEnabled();
    if (local && model) {
      return "both";
    }
    if (local) {
      return "local";
    }
    if (model) {
      return "ollama";
    }
    return "off";
  }
}

function configuredStatus(mode, config, deterministicSources) {
  const sources = deterministicSources.length > 0
    ? deterministicSources.join(", ")
    : "none";
  switch (mode) {
    case "local":
      return [
        "$(check) Tua inline: Local",
        `Enabled deterministic sources: ${sources}. Ollama is off.`,
      ];
    case "ollama":
      return [
        "$(check) Tua inline: Ollama",
        `Ollama inline completion is enabled with ${config.model}; deterministic local completion is off.`,
      ];
    case "both":
      return [
        "$(check) Tua inline: Local + Ollama",
        `Enabled deterministic sources: ${sources}. ${config.model} is the fallback.`,
      ];
    default:
      return [
        "$(circle-slash) Tua inline off",
        "Deterministic local and Ollama inline completions are off.",
      ];
  }
}

function loadOllamaStats(workspaceState) {
  const current = workspaceState.get(STATS_KEY);
  if (current) {
    return normalizedOllamaStats(current, current.returnedSuggestions);
  }

  const legacy = workspaceState.get(LEGACY_STATS_KEY) || {};
  return normalizedOllamaStats(
    legacy,
    legacy.returnedSuggestions ?? legacy.shownSuggestions,
  );
}

function normalizedOllamaStats(stats, returnedSuggestions) {
  return {
    requests: numericCounter(stats.requests),
    cancellations: numericCounter(stats.cancellations),
    timeouts: numericCounter(stats.timeouts),
    emptyResponses: numericCounter(stats.emptyResponses),
    validationRejections: numericCounter(stats.validationRejections),
    returnedSuggestions: numericCounter(returnedSuggestions),
    acceptedSuggestions: numericCounter(stats.acceptedSuggestions),
    acceptedCharacters: numericCounter(stats.acceptedCharacters),
  };
}

function normalizedLocalStats(stats) {
  return {
    semanticReturned: numericCounter(stats.semanticReturned),
    semanticAccepted: numericCounter(stats.semanticAccepted),
    currentFileReturned: numericCounter(stats.currentFileReturned),
    currentFileAccepted: numericCounter(stats.currentFileAccepted),
    projectReturned: numericCounter(stats.projectReturned),
    projectAccepted: numericCounter(stats.projectAccepted),
    projectStatisticalReturned: numericCounter(stats.projectStatisticalReturned),
    projectStatisticalAccepted: numericCounter(stats.projectStatisticalAccepted),
  };
}

function numericCounter(value) {
  return Number.isFinite(value) ? Math.max(0, Number(value)) : 0;
}

function statusCommand(mode, state) {
  if (state === "language_server") {
    return undefined;
  }
  if (mode === "ollama" || mode === "both") {
    return {
      command: COMMANDS.warmOllamaModel,
      title: "Test and Warm Ollama Model",
    };
  }
  return {
    command: COMMANDS.toggleLocalInlineCompletions,
    title: mode === "local"
      ? "Disable Tua Deterministic Inline Completions"
      : "Enable Tua Deterministic Inline Completions",
  };
}

function localCounterName(source, suffix) {
  if (source === "semantic") {
    return `semantic${suffix}`;
  }
  if (source === "currentFile") {
    return `currentFile${suffix}`;
  }
  if (source === "project") {
    return `project${suffix}`;
  }
  if (source === "projectStatistical") {
    return `projectStatistical${suffix}`;
  }
  return undefined;
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
  LOCAL_STATS_FLUSH_MS,
  isSilentLocalError,
  protocolRange,
  registerInlineCompletion,
};
