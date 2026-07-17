"use strict";

const { displayUriPath } = require("../shared/locations");

function ecsExplorerViewModel(response, focusUri) {
  const graph = response?.graph || {};
  const components = array(graph.components)
    .filter(hasIdentityAndLocation)
    .map(componentViewModel);
  const systems = array(graph.systems)
    .filter(hasIdentityAndLocation)
    .map(systemViewModel);
  const queries = array(graph.queries)
    .filter(hasIdentityAndLocation)
    .map(queryViewModel);
  const insights = array(graph.insights)
    .filter(hasIdentityAndLocation)
    .map(insightViewModel);

  return {
    analysisRevision: Number(response?.analysisRevision) || 0,
    focusUri: typeof focusUri === "string" ? focusUri : undefined,
    components,
    systems,
    queries,
    insights,
  };
}

function componentViewModel(component) {
  return {
    id: component.id,
    uri: component.uri,
    file: displayUriPath(component.uri),
    line: lineNumber(component.nameRange || component.range),
    localName: stringOr(component.localName, component.runtimeName, "Component"),
    runtimeName: stringOr(component.runtimeName, component.localName, "Component"),
    range: component.range,
    nameRange: component.nameRange || component.range,
    fields: array(component.fields)
      .filter((field) => typeof field?.name === "string" && field?.range)
      .map((field) => ({
        name: field.name,
        type: typeof field.type === "string" ? field.type : "any",
        range: field.range,
      })),
  };
}

function systemViewModel(system) {
  return {
    id: system.id,
    uri: system.uri,
    file: displayUriPath(system.uri),
    line: lineNumber(system.nameRange || system.range),
    name: stringOr(system.name, `System at line ${lineNumber(system.range)}`),
    range: system.range,
    nameRange: system.nameRange || system.range,
    callbackRange: system.callbackRange || system.range,
    components: componentUses(system.components, system.uri),
  };
}

function queryViewModel(query) {
  return {
    id: query.id,
    uri: query.uri,
    file: displayUriPath(query.uri),
    line: lineNumber(query.nameRange || query.range),
    name: stringOr(query.name, `Query at line ${lineNumber(query.range)}`),
    kind: query.kind === "system" ? "system" : "standalone",
    ownerSystemId:
      typeof query.ownerSystemId === "string" ? query.ownerSystemId : undefined,
    range: query.range,
    nameRange: query.nameRange || query.range,
    components: componentUses(query.components, query.uri),
  };
}

function insightViewModel(insight) {
  const knownKinds = [
    "wideQueryAllocation",
    "callbackAllocation",
    "resourceConstruction",
  ];

  return {
    id: insight.id,
    uri: insight.uri,
    file: displayUriPath(insight.uri),
    line: lineNumber(insight.range),
    kind: knownKinds.includes(insight.kind) ? insight.kind : "callbackAllocation",
    title: stringOr(insight.title, "ECS performance insight"),
    message: stringOr(insight.message, "Review this ECS update pattern."),
    subjectId: typeof insight.subjectId === "string" ? insight.subjectId : "",
    range: insight.range,
  };
}

function componentUses(uses, uri) {
  const knownAccess = ["filter", "read", "write", "readWrite", "unknown"];

  return array(uses)
    .filter((component) => typeof component?.reference === "string" && component?.range)
    .map((component) => ({
      uri,
      reference: component.reference,
      componentId:
        typeof component.componentId === "string" ? component.componentId : undefined,
      access: knownAccess.includes(component.access) ? component.access : "unknown",
      range: component.range,
      line: lineNumber(component.range),
    }));
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
  ecsExplorerViewModel,
};
