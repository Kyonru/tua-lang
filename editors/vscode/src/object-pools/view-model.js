"use strict";

const { displayUriPath } = require("../shared/locations");

function objectPoolExplorerViewModel(response, focusUri) {
  const graph = response?.graph || {};
  const pools = array(graph.pools).filter(hasIdentityAndLocation).map(poolViewModel);
  const operations = array(graph.operations)
    .filter(hasIdentityAndLocation)
    .map(operationViewModel);
  const insights = array(graph.insights)
    .filter(hasIdentityAndLocation)
    .map(insightViewModel);
  const operationsByPool = new Map();
  const insightsByPool = new Map();

  for (const operation of operations) {
    if (!operation.poolId) continue;
    const list = operationsByPool.get(operation.poolId) || [];
    list.push(operation);
    operationsByPool.set(operation.poolId, list);
  }

  for (const insight of insights) {
    if (!insight.poolId) continue;
    const list = insightsByPool.get(insight.poolId) || [];
    list.push(insight);
    insightsByPool.set(insight.poolId, list);
  }

  return {
    analysisRevision: Number(response?.analysisRevision) || 0,
    focusUri: typeof focusUri === "string" ? focusUri : undefined,
    pools: pools.map((pool) => ({
      ...pool,
      operationIds: array(operationsByPool.get(pool.id)).map((operation) => operation.id),
      insightIds: array(insightsByPool.get(pool.id)).map((insight) => insight.id),
    })),
    operations,
    insights,
  };
}

function poolViewModel(pool) {
  return {
    id: pool.id,
    uri: pool.uri,
    file: displayUriPath(pool.uri),
    line: lineNumber(pool.nameRange || pool.range),
    name: stringOr(pool.name, "objectPool"),
    itemType: stringOr(pool.itemType, "any"),
    range: pool.range,
    nameRange: pool.nameRange || pool.range,
  };
}

function operationViewModel(operation) {
  return {
    id: operation.id,
    uri: operation.uri,
    file: displayUriPath(operation.uri),
    line: lineNumber(operation.itemRange || operation.range),
    poolId: typeof operation.poolId === "string" ? operation.poolId : undefined,
    poolName: stringOr(operation.poolName, "pool"),
    kind: operationKind(operation.kind),
    itemName: typeof operation.itemName === "string" ? operation.itemName : "",
    range: operation.range,
    itemRange: operation.itemRange || operation.range,
  };
}

function insightViewModel(insight) {
  return {
    id: insight.id,
    uri: insight.uri,
    file: displayUriPath(insight.uri),
    line: lineNumber(insight.range),
    kind: "unreleasedAcquire",
    title: stringOr(insight.title, "Object pool lifecycle insight"),
    message: stringOr(insight.message, "Review this object-pool lifecycle."),
    poolId: typeof insight.poolId === "string" ? insight.poolId : undefined,
    acquireOperationId:
      typeof insight.acquireOperationId === "string" ? insight.acquireOperationId : undefined,
    itemName: typeof insight.itemName === "string" ? insight.itemName : "",
    range: insight.range,
  };
}

function operationKind(kind) {
  return ["acquire", "release", "clear", "prewarm"].includes(kind) ? kind : "acquire";
}

function hasIdentityAndLocation(value) {
  return typeof value?.id === "string" && typeof value?.uri === "string" && value?.range;
}

function lineNumber(range) {
  return (Number(range?.start?.line) || 0) + 1;
}

function stringOr(...values) {
  return values.find((value) => typeof value === "string" && value.length > 0) || "";
}

function array(value) {
  return Array.isArray(value) ? value : [];
}

module.exports = {
  objectPoolExplorerViewModel,
};
