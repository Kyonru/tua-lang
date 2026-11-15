"use strict";

const assert = require("assert/strict");
const fs = require("fs");
const path = require("path");
const test = require("node:test");
const vm = require("vm");

function fixture({ installFails = false, disabled = false, installedExtensions = [] } = {}) {
  const commands = new Map();
  const settings = {};
  const saved = new Map();
  const executed = [];
  const errors = [];
  const installed = new Set(installedExtensions);
  const vscode = {
    ConfigurationTarget: { Global: 1 },
    Disposable: { from: (...items) => ({ dispose: () => items.forEach((item) => item.dispose()) }) },
    commands: {
      registerCommand: (id, action) => {
        commands.set(id, action);
        return { dispose: () => commands.delete(id) };
      },
      executeCommand: async (id, ...args) => {
        executed.push([id, ...args]);
        if (id === "workbench.extensions.installExtension") {
          if (installFails) throw new Error("registry unavailable");
          installed.add(args[0]);
        }
      },
    },
    workspace: {
      getConfiguration: () => ({
        get: (key) => settings[key],
        inspect: (key) => ({ globalValue: settings[key] }),
        update: async (key, value) => {
          if (value === undefined) delete settings[key];
          else settings[key] = value;
        },
      }),
    },
    extensions: {
      getExtension: (id) => installed.has(id) && !disabled
        ? { packageJSON: { contributes: { iconThemes: id === "PKief.material-icon-theme" ? [{ id: "material-icon-theme" }] : [] } } }
        : undefined,
    },
    window: {
      setStatusBarMessage: () => {},
      showWarningMessage: async () => {},
      showErrorMessage: async (message) => { errors.push(message); },
    },
  };
  const context = {
    extension: { id: "SolenodonteLabs.tua" },
    globalState: {
      get: (key, fallback) => saved.get(key) ?? fallback,
      update: async (key, value) => { saved.set(key, value); },
    },
  };
  const directory = path.resolve(__dirname, "../../src/setup");
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(path.join(directory, "index.js"), "utf8"), {
    module,
    require: (id) => id === "vscode" ? vscode : require(path.resolve(directory, id)),
  });
  const disposable = module.exports.registerSetup(context);
  return { commands, settings, executed, errors, disposable };
}

test("setup activation is inert and its opener selects the native walkthrough", async () => {
  const f = fixture();
  assert.deepEqual(f.executed, []);
  assert.deepEqual(f.settings, {});
  await f.commands.get("tua.openSetup")();
  assert.deepEqual(f.executed, [["workbench.action.openWalkthrough", "SolenodonteLabs.tua#tua.setup", false]]);
  assert.deepEqual(f.settings, {});
  f.disposable.dispose();
  assert.equal(f.commands.size, 0);
});

test("icon installation happens only on request, and a double-click installs once", async () => {
  const f = fixture();
  const action = f.commands.get("tua.setup.useMaterialIcons");
  await Promise.all([action(), action()]);
  assert.equal(f.executed.length, 1);
  assert.deepEqual(f.executed[0], ["workbench.extensions.installExtension", "PKief.material-icon-theme"]);
  assert.equal(f.settings["workbench.iconTheme"], "material-icon-theme");
  await f.commands.get("tua.setup.restoreIcons")();
  assert.equal(f.settings["workbench.iconTheme"], undefined);
  await action();
  assert.equal(f.executed.length, 1, "installed icons are reused");
});

test("debugger setup installs only on request and accepts the supported legacy debugger", async () => {
  const f = fixture();
  assert.deepEqual(f.executed, []);
  const install = f.commands.get("tua.setup.installDebugger");
  await Promise.all([install(), install()]);
  assert.deepEqual(f.executed, [["workbench.extensions.installExtension", "ismoh-games.second-local-lua-debugger-vscode"]]);
  await install();
  assert.equal(f.executed.length, 1, "an installed debugger must not be installed again");
  assert.deepEqual(f.settings, {}, "installing the debugger must not change editor settings");

  const legacy = fixture({ installedExtensions: ["tomblind.local-lua-debugger-vscode"] });
  await legacy.commands.get("tua.setup.installDebugger")();
  assert.deepEqual(legacy.executed, [], "the existing supported debugger is sufficient");
});

test("shader highlighting setup is optional and does not reinstall an existing extension", async () => {
  const f = fixture();
  assert.deepEqual(f.executed, []);
  const install = f.commands.get("tua.setup.installShaderHighlighting");
  await install();
  await install();
  assert.deepEqual(f.executed, [["workbench.extensions.installExtension", "slevesque.shader"]]);
  assert.deepEqual(f.settings, {});
});

test("failed debugger installation reports the registry error without changing settings", async () => {
  const f = fixture({ installFails: true });
  assert.equal(await f.commands.get("tua.setup.installDebugger")(), false);
  assert.deepEqual(f.settings, {});
  assert.match(f.errors[0], /registry unavailable/);
});

test("failed or disabled icon installs preserve the current icon selection", async () => {
  for (const options of [{ installFails: true }, { disabled: true }]) {
    const f = fixture(options);
    f.settings["workbench.iconTheme"] = "current-icons";
    assert.equal(await f.commands.get("tua.setup.useMaterialIcons")(), false);
    assert.equal(f.settings["workbench.iconTheme"], "current-icons");
    assert.equal(f.errors.length, 1);
  }
});

test("restore clicked while apply is pending still runs after it", async () => {
  const f = fixture();
  f.settings["workbench.colorTheme"] = "Before";
  await Promise.all([
    f.commands.get("tua.setup.applyTheme")(),
    f.commands.get("tua.setup.restoreTheme")(),
  ]);
  assert.equal(f.settings["workbench.colorTheme"], "Before");
});
