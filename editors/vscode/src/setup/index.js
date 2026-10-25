"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { formatError } = require("../shared/errors");
const { createPresetManager, MATERIAL_ICONS_EXTENSION } = require("./presets");

const { shaderEnvironment, probeShaderTools } = require("./shader-tools");
const { createShaderSetup } = require("./shaders");

function registerSetup(context, { getServerCommand } = {}) {
  const shaders = createShaderSetup({ workspace: vscode.workspace, state: context.workspaceState, target: vscode.ConfigurationTarget.Workspace, globalTarget: vscode.ConfigurationTarget.Global });
  const presets = createPresetManager({
    workspace: vscode.workspace,
    state: context.globalState,
    target: vscode.ConfigurationTarget.Global,
    resource: () => vscode.window.activeTextEditor?.document.uri,
  });
  let pending = Promise.resolve();
  const operations = new Map();

  const run = (id, action) => {
    // Deduplicate double-clicks; queue different actions in click order.
    // No settings are touched during activation.
    if (operations.has(id)) return operations.get(id);
    const operation = pending.then(action).catch((error) => {
      void vscode.window.showErrorMessage(
        `Tua setup: ${formatError(error)}. Any settings already changed can be undone with the matching Restore command.`,
      );
      return false;
    }).finally(() => { operations.delete(id); });
    pending = operation;
    operations.set(id, operation);
    return operation;
  };

  async function apply(id, label) {
    const result = await presets.apply(id);
    if (result.overridden.length) {
      void vscode.window.showWarningMessage(
        `${label} saved to user settings. Workspace or folder settings still override: ${result.overridden.join(", ")}. Your workspace settings were kept.`,
      );
    } else {
      vscode.window.setStatusBarMessage(
        result.changed ? `${label} applied. Restore is available in Tua setup.` : `${label} is already enabled.`,
        5000,
      );
    }
    return result;
  }

  async function restore(id, label) {
    const result = await presets.restore(id);
    vscode.window.setStatusBarMessage(
      result.changed
        ? `${label} restored.${result.preserved ? " Later manual changes were kept." : ""}`
        : `No ${label.toLowerCase()} settings to restore.${result.preserved ? " Later manual changes were kept." : ""}`,
      5000,
    );
    return result;
  }

  async function materialIcons() {
    if (!vscode.extensions.getExtension(MATERIAL_ICONS_EXTENSION)) {
      await vscode.commands.executeCommand("workbench.extensions.installExtension", MATERIAL_ICONS_EXTENSION);
    }
    const extension = vscode.extensions.getExtension(MATERIAL_ICONS_EXTENSION);
    if (!extension?.packageJSON?.contributes?.iconThemes?.some((theme) => theme.id === "material-icon-theme")) {
      throw new Error("Material Icon Theme is not available yet. Enable it in Extensions, reload if prompted, then try Use Material Icons again");
    }
    return apply("icons", "Material file icons");
  }

  const register = (id, action) => vscode.commands.registerCommand(id, () => run(id, action));
  return vscode.Disposable.from(
    vscode.commands.registerCommand(COMMANDS.openSetup, () =>
      vscode.commands.executeCommand("workbench.action.openWalkthrough", `${context.extension.id}#tua.setup`, false),
    ),
    register(COMMANDS.setupCheckShaderTools, async () => {
      if (!vscode.workspace.isTrusted) throw new Error("Trust this workspace before running shader validators");
      const command = getServerCommand?.();
      if (!command) throw new Error("Select a Tua server first");
      const report = await vscode.window.withProgress({ location: vscode.ProgressLocation.Notification, title: "Checking shader validators (LÖVE may open a small window)" }, () => probeShaderTools(command, shaderEnvironment(vscode.workspace)));
      const love = report.love;
      const glslang = report.glslang;
      const message = `LÖVE${love.version ? ` ${love.version}` : ""}: ${love.canValidate ? "validation works (preferred)" : love.message}. glslang: ${glslang.canValidate ? "validation works (fallback)" : glslang.message}.`;
      void vscode.window.showInformationMessage(message);
    }),
    register(COMMANDS.setupSelectGlslang, async () => {
      if (!vscode.workspace.isTrusted) throw new Error("Trust this workspace before configuring shader validators");
      const selected = await vscode.window.showOpenDialog({ canSelectMany: false, canSelectFiles: true, canSelectFolders: false, title: "Select glslangValidator executable" });
      if (selected?.[0]) await shaders.selectPath(selected[0].fsPath);
    }),
    register(COMMANDS.setupApplyShaders, async () => {
      await shaders.apply();
      await vscode.commands.executeCommand("setContext", "tua.setup.shadersApplied", true);
      void vscode.window.showInformationMessage("LÖVE shader associations applied to this workspace. Tua validates them; GLSL Lint only checks the glsl language. Restore is available in Tua setup.");
    }),
    register(COMMANDS.setupRestoreShaders, async () => {
      await shaders.restore();
      await vscode.commands.executeCommand("setContext", "tua.setup.shadersApplied", false);
    }),
    register(COMMANDS.setupSkipShaders, () => undefined),
    register(COMMANDS.setupInstallShaderHighlighting, () => vscode.commands.executeCommand("workbench.extensions.installExtension", "slevesque.shader")),
    register(COMMANDS.setupApplyTheme, () => apply("theme", "Indigo Carbon")),
    register(COMMANDS.setupRestoreTheme, () => restore("theme", "Theme")),
    register(COMMANDS.setupApplyLayout, () => apply("layout", "Tua layout")),
    register(COMMANDS.setupRestoreLayout, () => restore("layout", "Layout")),
    register(COMMANDS.setupUseMaterialIcons, materialIcons),
    register(COMMANDS.setupRestoreIcons, () => restore("icons", "File icons")),
  );
}

module.exports = { registerSetup };
