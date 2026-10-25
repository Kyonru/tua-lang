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
    names.add(name);
    return { name, mode: item.mode, ...(item.packageTarget ? { packageTarget: item.packageTarget } : {}) };
  });
}

module.exports = { DEFAULT_CONFIGURATIONS, normalizeConfigurations };
