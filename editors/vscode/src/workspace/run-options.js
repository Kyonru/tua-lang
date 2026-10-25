"use strict";
const DEFAULT_CONFIGURATIONS = [
  { name: "Development", mode: "run" },
  { name: "Debug", mode: "debug" },
  { name: "Hot reload", mode: "hotReload" },
  { name: "Package", mode: "package", packageTarget: "love" },
];

function normalizeConfigurations(value) {
  if (!Array.isArray(value) || value.length === 0) throw new Error("Add at least one Tua run configuration.");
  const names = new Set();
  return value.map(item => {
    const name = typeof item?.name === "string" ? item.name.trim() : "";
    if (!name || names.has(name)) throw new Error("Run configurations need unique, non-empty names.");
    if (!["run", "debug", "hotReload", "package"].includes(item.mode)) throw new Error(`Unknown run mode for ${name}.`);
    if (item.packageTarget && !["love", "windows", "macos"].includes(item.packageTarget)) throw new Error(`Unknown package target for ${name}.`);
    if (item.instances !== undefined && item.mode === "package") throw new Error(`Instances are not supported in Package mode (${name}).`);
    const instances = item.instances === undefined ? undefined : normalizeInstances(item.instances);
    names.add(name);
    return { name, mode: item.mode, ...(instances ? { instances } : {}), ...(item.packageTarget ? { packageTarget: item.packageTarget } : {}) };
  });
}

function normalizeInstances(value) {
  if (!Array.isArray(value) || value.length < 1 || value.length > 16) throw new Error("Configure between 1 and 16 LOVE instances.");
  const names = new Set();
  return value.map(item => {
    const name = typeof item?.name === "string" ? item.name.trim() : "";
    if (!name || names.has(name)) throw new Error("LOVE instances need unique, non-empty names.");
    const args = item.args === undefined ? [] : item.args;
    if (!Array.isArray(args) || args.some(arg => typeof arg !== "string" || arg.includes("\0"))) throw new Error(`Arguments for ${name} must be strings without null characters.`);
    names.add(name);
    return { name, args: [...args] };
  });
}

module.exports = { DEFAULT_CONFIGURATIONS, normalizeConfigurations, normalizeInstances };
