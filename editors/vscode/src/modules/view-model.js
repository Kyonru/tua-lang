"use strict";

const path = require("path");
const vscode = require("vscode");
const { displayUriPath } = require("../shared/locations");

function moduleGraphViewModel(response, focus = {}) {
  const graph = response?.graph || {};
  const modules = (Array.isArray(graph.modules) ? graph.modules : [])
    .filter((module) => typeof module?.uri === "string")
    .map((module) => moduleViewModel(module));
  const moduleUris = new Set(modules.map((module) => module.uri));
  const edges = (Array.isArray(graph.edges) ? graph.edges : [])
    .filter((edge) => typeof edge?.sourceUri === "string" && edge?.range)
    .map((edge, index) => edgeViewModel(edge, index));
  const cycles = (Array.isArray(graph.cycles) ? graph.cycles : [])
    .map((cycle, index) => ({
      id: typeof cycle?.id === "string" ? cycle.id : `cycle:${index}`,
      moduleUris: (Array.isArray(cycle?.moduleUris) ? cycle.moduleUris : []).filter((uri) =>
        moduleUris.has(uri),
      ),
    }))
    .filter((cycle) => cycle.moduleUris.length > 0);
  const focusUri = moduleUriForFocus(modules, focus?.uri);

  return {
    analysisRevision: Number(response?.analysisRevision) || 0,
    focusUri,
    initialTab: ["overview", "dependencies", "dependents", "exports", "why"].includes(
      focus?.initialTab,
    )
      ? focus.initialTab
      : "overview",
    modules,
    edges,
    cycles,
  };
}

function moduleUriForFocus(modules, focusUri) {
  if (typeof focusUri !== "string") {
    return undefined;
  }

  const exact = modules.find((module) => module.uri === focusUri);
  if (exact) {
    return exact.uri;
  }

  const focusPath = comparableFilePath(focusUri);
  return modules.find((module) => comparableFilePath(module.uri) === focusPath)?.uri;
}

function comparableFilePath(uriValue) {
  const uri = vscode.Uri.parse(uriValue);
  const normalized = uri.scheme === "file" && uri.fsPath ? path.resolve(uri.fsPath) : uriValue;
  return process.platform === "win32" ? normalized.toLowerCase() : normalized;
}

function moduleViewModel(module) {
  const inclusionPath = module.inclusionPath;

  return {
    uri: module.uri,
    file: displayUriPath(module.uri),
    sourceKind: module.sourceKind === "lua" ? "lua" : "tua",
    root: Boolean(module.root),
    inCycle: Boolean(module.inCycle),
    dependencyCount: nonNegativeInteger(module.dependencyCount),
    unresolvedDependencyCount: nonNegativeInteger(module.unresolvedDependencyCount),
    reverseDependencyCount: nonNegativeInteger(module.reverseDependencyCount),
    exports: (Array.isArray(module.exports) ? module.exports : [])
      .filter((item) => typeof item?.name === "string" && item?.range)
      .map((item) => ({
        name: item.name,
        type: typeof item.type === "string" ? item.type : "any",
        documentation: typeof item.documentation === "string" ? item.documentation : "",
        origin: knownExportOrigin(item.origin),
        range: item.range,
      })),
    inclusionPath:
      typeof inclusionPath?.rootUri === "string"
        ? {
            rootUri: inclusionPath.rootUri,
            rootFile: displayUriPath(inclusionPath.rootUri),
            steps: (Array.isArray(inclusionPath.steps) ? inclusionPath.steps : [])
              .filter(
                (step) =>
                  typeof step?.sourceUri === "string" &&
                  typeof step?.targetUri === "string" &&
                  step?.range,
              )
              .map((step) => ({
                sourceUri: step.sourceUri,
                sourceFile: displayUriPath(step.sourceUri),
                targetUri: step.targetUri,
                targetFile: displayUriPath(step.targetUri),
                specifier: typeof step.specifier === "string" ? step.specifier : "",
                kind: step.kind === "import" ? "import" : "require",
                range: step.range,
              })),
          }
        : null,
  };
}

function edgeViewModel(edge, index) {
  const resolution = edge.resolution || {};
  const status = ["resolved", "missing", "invalid", "duplicate"].includes(resolution.status)
    ? resolution.status
    : "invalid";

  return {
    id: `edge:${index}`,
    sourceUri: edge.sourceUri,
    sourceFile: displayUriPath(edge.sourceUri),
    specifier: typeof edge.specifier === "string" ? edge.specifier : "",
    kind: edge.kind === "import" ? "import" : "require",
    range: edge.range,
    inCycle: Boolean(edge.inCycle),
    resolution: {
      status,
      targetUri: typeof resolution.targetUri === "string" ? resolution.targetUri : undefined,
      targetFile:
        typeof resolution.targetUri === "string" ? displayUriPath(resolution.targetUri) : undefined,
      reason: typeof resolution.reason === "string" ? resolution.reason : "",
      candidates: (Array.isArray(resolution.candidateUris)
        ? resolution.candidateUris
        : []
      ).map((uri) => ({ uri, file: displayUriPath(uri) })),
      searchRoots: (Array.isArray(resolution.searchRootUris)
        ? resolution.searchRootUris
        : []
      ).map((uri) => displayUriPath(uri)),
    },
  };
}

function knownExportOrigin(origin) {
  return ["tua", "lua", "luaDoc", "generatedBuiltin"].includes(origin) ? origin : "tua";
}

function nonNegativeInteger(value) {
  const number = Number(value);
  return Number.isInteger(number) && number >= 0 ? number : 0;
}

module.exports = {
  moduleGraphViewModel,
};
