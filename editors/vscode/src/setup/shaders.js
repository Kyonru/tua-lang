"use strict";

const BACKUP_KEY = "tua.setup.previous.v1.shaders";
const PATTERNS = ["*.glsl", "*.vert", "*.frag", "*.vs", "*.fs"];

// Back up individual association keys, not the whole object: unrelated edits
// made after Apply must survive Restore and partial failures must be retryable.
function createShaderSetup({ workspace, state, target }) {
  const config = () => workspace.getConfiguration("files", null);
  const read = () => ({ ...config().inspect("associations")?.workspaceValue });
  async function apply() {
    if (!workspace.workspaceFolders?.length) throw new Error("Open a workspace before applying shader associations");
    const saved = { ...state.get(BACKUP_KEY, {}) };
    for (const pattern of PATTERNS) {
      const current = read();
      if (current[pattern] === "love-glsl") continue;
      saved[pattern] = { present: Object.hasOwn(current, pattern), value: current[pattern] };
      await state.update(BACKUP_KEY, { ...saved });
      await config().update("associations", { ...current, [pattern]: "love-glsl" }, target);
    }
  }
  async function restore() {
    const saved = { ...state.get(BACKUP_KEY, {}) };
    for (const pattern of Object.keys(saved)) {
      const current = read();
      if (PATTERNS.includes(pattern) && current[pattern] === "love-glsl") {
        if (saved[pattern].present) current[pattern] = saved[pattern].value;
        else delete current[pattern];
        await config().update("associations", Object.keys(current).length ? current : undefined, target);
      }
      delete saved[pattern];
      await state.update(BACKUP_KEY, Object.keys(saved).length ? { ...saved } : undefined);
    }
  }
  return { apply, restore };
}

module.exports = { BACKUP_KEY, PATTERNS, createShaderSetup };
