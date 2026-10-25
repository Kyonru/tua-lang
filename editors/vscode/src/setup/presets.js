"use strict";

const THEME_NAME = "Indigo Carbon HC";
const MATERIAL_ICONS_EXTENSION = "PKief.material-icon-theme";
const PRESETS = {
  theme: { "workbench.colorTheme": THEME_NAME },
  layout: {
    "window.commandCenter": true,
    "workbench.editor.editorActionsLocation": "titleBar",
    "workbench.layoutControl.enabled": true,
    "workbench.layoutControl.type": "both",
    "tua.editor.toolbarMode": "compact",
  },
  icons: { "workbench.iconTheme": "material-icon-theme" },
};
const BACKUP_PREFIX = "tua.setup.previous.v1.";

// Only own the user-setting values written by this setup. Workspace overrides
// and subsequent manual changes belong to the user, including on Restore.
function createPresetManager({ workspace, state, target, resource }) {
  let pending = Promise.resolve();
  const serialize = (operation) => {
    const result = pending.then(operation);
    pending = result.catch(() => {});
    return result;
  };

  function settingsFor(id) {
    if (!Object.hasOwn(PRESETS, id)) {
      throw new Error(`Unknown Tua setup preset: ${id}`);
    }
    return PRESETS[id];
  }

  function configuration() {
    return workspace.getConfiguration(undefined, null);
  }

  function read(key) {
    const value = configuration().inspect(key)?.globalValue;
    return value === undefined ? { present: false } : { present: true, value };
  }

  async function apply(id) {
    const settings = settingsFor(id);
    const backupKey = BACKUP_PREFIX + id;
    const backup = { ...state.get(backupKey, {}) };
    let changed = 0;

    for (const [key, value] of Object.entries(settings)) {
      const current = read(key);
      const saved = backup[key];
      // A repeat click keeps the original backup. If the user changed a value
      // since applying, a new explicit Apply starts from their latest choice.
      if (saved && (!current.present || current.value !== saved.applied)) {
        delete backup[key];
        await state.update(backupKey, { ...backup });
      }
      if (current.present && current.value === value) continue;
      if (!current.present && configuration().get(key) === value) continue;

      backup[key] = { previous: current, applied: value };
      // Save before writing so a failed/partial apply remains recoverable.
      await state.update(backupKey, { ...backup });
      await configuration().update(key, value, target);
      changed += 1;
    }

    const effective = workspace.getConfiguration(undefined, resource?.());
    const overridden = Object.entries(settings)
      .filter(([key, value]) => effective.get(key) !== value)
      .map(([key]) => key);
    return { changed, overridden };
  }

  async function restore(id) {
    const settings = settingsFor(id);
    const backupKey = BACKUP_PREFIX + id;
    const backup = { ...state.get(backupKey, {}) };
    let changed = 0;
    let preserved = 0;
    for (const key of Object.keys(backup)) {
      const entry = backup[key];
      // Ignore unknown keys even if older state contains them.
      if (Object.hasOwn(settings, key) && entry.applied === settings[key]) {
        const current = read(key);
        if (current.present && current.value === entry.applied) {
          await configuration().update(
            key, entry.previous.present ? entry.previous.value : undefined, target,
          );
          changed += 1;
        } else {
          preserved += 1;
        }
      }
      delete backup[key];
      // Keep remaining entries if a later write fails; Restore can be retried.
      await state.update(backupKey, Object.keys(backup).length ? { ...backup } : undefined);
    }
    return { changed, preserved };
  }

  return {
    apply: (id) => serialize(() => apply(id)),
    restore: (id) => serialize(() => restore(id)),
  };
}

module.exports = { BACKUP_PREFIX, createPresetManager, MATERIAL_ICONS_EXTENSION, PRESETS, THEME_NAME };
