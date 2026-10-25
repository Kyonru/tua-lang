"use strict";
const CATEGORIES = ["ecs", "states", "signals", "pools", "stores"];
function normalizeOptions(value = {}) {
  const categories = Array.isArray(value.categories) ? CATEGORIES.filter(kind => value.categories.includes(kind)) : [...CATEGORIES];
  const interval = Number(value.updateInterval);
  return { categories, updateInterval: Number.isFinite(interval) ? Math.max(100, Math.min(10000, Math.round(interval))) : 500 };
}
module.exports = { CATEGORIES, normalizeOptions };
