"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../../constants");
const { formatError } = require("../../shared/errors");
const { lspRangeToVscode } = require("../../shared/locations");
const { AssetTreeDataProvider } = require("./tree");

const ASSET_GLOB = "**/*.{bmp,exr,hdr,jpeg,jpg,pic,png,tga,flac,mp3,ogg,wav,otf,ttf,frag,fs,glsl,vert,vs,ogm,ogv}";
const REFRESH_DELAY_MS = 150;

function registerAssetExplorerFeature(options) {
  const controller = new AssetExplorerController(options);
  const disposables = [controller];

  disposables.push(
    vscode.commands.registerCommand(COMMANDS.refreshAssets, () => controller.refresh()),
    vscode.commands.registerCommand(COMMANDS.previewAsset, (target) => previewAsset(target)),
    vscode.commands.registerCommand(COMMANDS.openAsset, (node) => openAsset(node)),
    vscode.commands.registerCommand(COMMANDS.openAssetReference, (node) => openReference(node)),
    vscode.commands.registerCommand(COMMANDS.findAssetReferences, (node) =>
      findReferences(node),
    ),
    vscode.commands.registerCommand(COMMANDS.renameAsset, (node) =>
      renameAsset(node, controller),
    ),
    vscode.commands.registerCommand(COMMANDS.revealAsset, (node) => revealAsset(node)),
  );

  const watcher = vscode.workspace.createFileSystemWatcher(ASSET_GLOB);
  disposables.push(
    watcher,
    watcher.onDidCreate(() => controller.scheduleRefresh()),
    watcher.onDidChange(() => controller.scheduleRefresh()),
    watcher.onDidDelete(() => controller.scheduleRefresh()),
    vscode.workspace.onDidSaveTextDocument((document) => {
      if (document.languageId === "tua" || document.languageId === "lua") {
        controller.scheduleRefresh();
      }
    }),
  );

  return {
    refreshAssets: () => controller.refresh(),
    dispose() {
      vscode.Disposable.from(...disposables).dispose();
    },
  };
}

class AssetExplorerController {
  constructor(options) {
    this.getClient = options.getClient;
    this.provider = new AssetTreeDataProvider();
    this.diagnostics = vscode.languages.createDiagnosticCollection("tua-assets");
    this.view = vscode.window.createTreeView("tua.assets", {
      treeDataProvider: this.provider,
      showCollapseAll: true,
    });
    this.requestSerial = 0;
    this.refreshTimer = undefined;
    this.visibilityDisposable = this.view.onDidChangeVisibility((event) => {
      if (event.visible) {
        this.refresh();
      }
    });
  }

  dispose() {
    if (this.refreshTimer) {
      clearTimeout(this.refreshTimer);
    }

    this.visibilityDisposable.dispose();
    this.view.dispose();
    this.diagnostics.dispose();
    this.provider.dispose();
  }

  scheduleRefresh() {
    if (this.refreshTimer) {
      clearTimeout(this.refreshTimer);
    }

    this.refreshTimer = setTimeout(() => {
      this.refreshTimer = undefined;
      this.refresh();
    }, REFRESH_DELAY_MS);
  }

  async refresh() {
    const client = this.getClient();
    if (!client) {
      return undefined;
    }

    const serial = ++this.requestSerial;

    try {
      const response = await client.sendRequest("tua/assetInventory", null);

      if (serial !== this.requestSerial || client !== this.getClient()) {
        return undefined;
      }

      const inventory = response?.inventory || {};
      this.provider.update(inventory.assets);
      updateDiagnostics(this.diagnostics, inventory.diagnostics);
      this.view.message = undefined;
      return inventory;
    } catch (error) {
      if (serial === this.requestSerial && client === this.getClient()) {
        this.view.message = `Asset inventory unavailable: ${formatError(error)}`;
      }

      return undefined;
    }
  }
}

async function openAsset(node) {
  const uri = assetUri(node);
  if (uri) {
    await vscode.commands.executeCommand("vscode.open", uri, { preview: true });
  }
}

async function previewAsset(target) {
  const uri = assetPreviewUri(target);
  if (uri) {
    await vscode.commands.executeCommand("vscode.open", uri, { preview: true });
    return uri.toString();
  }

  return undefined;
}

async function revealAsset(node) {
  const uri = assetUri(node);
  if (uri) {
    await vscode.commands.executeCommand("revealInExplorer", uri);
  }
}

async function renameAsset(node, controller) {
  const uri = assetUri(node);
  const currentName = node?.asset?.path?.split(/[\\/]/).at(-1);
  if (!uri || !currentName) {
    return;
  }

  const extensionStart = currentName.lastIndexOf(".");
  const newName = await vscode.window.showInputBox({
    prompt: "New asset file name",
    value: currentName,
    valueSelection: [0, extensionStart > 0 ? extensionStart : currentName.length],
    validateInput(value) {
      if (!value.trim()) {
        return "Asset file name cannot be empty.";
      }
      if (value !== value.trim() || /[\\/]/.test(value)) {
        return "Enter a file name without a path.";
      }
      if (value === "." || value === "..") {
        return "Enter a valid asset file name.";
      }
      return undefined;
    },
  });

  if (!newName || newName === currentName) {
    return;
  }

  const newUri = vscode.Uri.joinPath(uri, "..", newName);
  const edit = new vscode.WorkspaceEdit();
  edit.renameFile(uri, newUri, { overwrite: false });

  if (!(await vscode.workspace.applyEdit(edit))) {
    await vscode.window.showErrorMessage(`Could not rename ${currentName}.`);
    return;
  }

  controller.scheduleRefresh();
}

async function openReference(node) {
  const reference = node?.reference;
  if (!reference?.uri || !reference.range) {
    return;
  }

  await vscode.commands.executeCommand("vscode.open", vscode.Uri.parse(reference.uri), {
    preview: true,
    selection: lspRangeToVscode(reference.range),
  });
}

async function findReferences(node) {
  const asset = node?.asset;
  if (!asset?.uri) {
    return;
  }

  const locations = (asset.references || []).map(
    (reference) =>
      new vscode.Location(vscode.Uri.parse(reference.uri), lspRangeToVscode(reference.range)),
  );

  if (locations.length === 0) {
    await vscode.window.showInformationMessage(`No static references found for ${asset.path}.`);
    return;
  }

  await vscode.commands.executeCommand(
    "editor.action.showReferences",
    locations[0].uri,
    locations[0].range.start,
    locations,
  );
}

function updateDiagnostics(collection, entries) {
  collection.clear();
  const byUri = new Map();

  for (const entry of entries || []) {
    if (!entry?.uri || !entry.diagnostic?.range) {
      continue;
    }

    const uri = vscode.Uri.parse(entry.uri);
    const key = uri.toString();
    const diagnostic = new vscode.Diagnostic(
      lspRangeToVscode(entry.diagnostic.range),
      entry.diagnostic.message,
      diagnosticSeverity(entry.diagnostic.severity),
    );

    diagnostic.code = entry.diagnostic.code;
    diagnostic.source = entry.diagnostic.source || "tua";

    const group = byUri.get(key) || { uri, diagnostics: [] };
    group.diagnostics.push(diagnostic);
    byUri.set(key, group);
  }

  for (const group of byUri.values()) {
    collection.set(group.uri, group.diagnostics);
  }
}

function diagnosticSeverity(severity) {
  switch (severity) {
    case 1:
      return vscode.DiagnosticSeverity.Error;
    case 2:
      return vscode.DiagnosticSeverity.Warning;
    case 3:
      return vscode.DiagnosticSeverity.Information;
    default:
      return vscode.DiagnosticSeverity.Hint;
  }
}

function assetUri(node) {
  return node?.asset?.uri ? vscode.Uri.parse(node.asset.uri) : undefined;
}

function assetPreviewUri(target) {
  if (target?.asset?.uri) {
    return vscode.Uri.parse(target.asset.uri);
  }
  if (typeof target?.uri === "string") {
    return vscode.Uri.parse(target.uri);
  }
  if (typeof target === "string") {
    return vscode.Uri.parse(target);
  }
  return undefined;
}

module.exports = {
  registerAssetExplorerFeature,
};
